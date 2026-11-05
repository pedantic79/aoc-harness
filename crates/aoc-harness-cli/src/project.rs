use crate::{reports, BuildOptions};
use serde_json::Value;
use std::{
    collections::hash_map::DefaultHasher,
    ffi::OsString,
    hash::{Hash, Hasher},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}

fn common(command: &mut Command, options: &BuildOptions) {
    if options.offline {
        command.arg("--offline");
    }
    for config in &options.config {
        command.arg("--config").arg(config);
    }
}

fn write_changed(path: &Path, contents: &str) -> Result<(), String> {
    if std::fs::read_to_string(path).ok().as_deref() == Some(contents) {
        return Ok(());
    }
    std::fs::write(path, contents).map_err(|e| format!("write {}: {e}", path.display()))
}

fn absolute(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path))
    }
}

fn normalize_patch_paths(value: &mut toml::Value, root: &Path) {
    if let Some(table) = value.as_table_mut() {
        if let Some(path) = table.get_mut("path") {
            if let Some(text) = path.as_str() {
                *path = toml::Value::String(root.join(text).to_string_lossy().into_owned());
            }
        }
        for (_, value) in table.iter_mut() {
            normalize_patch_paths(value, root);
        }
    }
}

pub(crate) struct ConsumerProject {
    pub package: Value,
    pub manifest_path: PathBuf,
    pub manifest_dir: PathBuf,
    pub library_source: PathBuf,
    pub library_name: String,
    pub package_name: String,
    pub target_dir: PathBuf,
    pub workspace_root: PathBuf,
}

pub(crate) fn discover(options: &BuildOptions) -> Result<ConsumerProject, String> {
    let mut metadata = cargo();
    metadata.args(["metadata", "--format-version", "1", "--no-deps", "--locked"]);
    if let Some(manifest) = &options.manifest_path {
        let manifest = absolute(manifest)?;
        metadata.current_dir(manifest.parent().ok_or("manifest path has no directory")?);
        metadata.arg("--manifest-path").arg(manifest);
    }
    if let Some(target_dir) = &options.target_dir {
        metadata.env("CARGO_TARGET_DIR", absolute(target_dir)?);
    }
    common(&mut metadata, options);
    let output = metadata
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !output.status.success() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        return Err("Cargo metadata failed without changing the consumer lockfile; resolve the package/lockfile with Cargo first".into());
    }
    let metadata: Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("cargo metadata JSON: {e}"))?;
    let members = metadata["workspace_members"]
        .as_array()
        .ok_or("metadata has no workspace members")?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or("metadata has no packages")?;
    let workspace_packages: Vec<_> = packages
        .iter()
        .filter(|p| members.contains(&p["id"]))
        .collect();
    let package = if let Some(name) = &options.package {
        workspace_packages
            .iter()
            .copied()
            .find(|p| p["name"].as_str() == Some(name) || p["id"].as_str() == Some(name))
            .ok_or_else(|| format!("package {name:?} is not a workspace member"))?
    } else {
        let defaults = metadata["workspace_default_members"]
            .as_array()
            .ok_or("metadata has no default members")?;
        if defaults.len() != 1 {
            return Err(format!(
                "workspace package selection is ambiguous; use --package ({})",
                workspace_packages
                    .iter()
                    .filter_map(|p| p["name"].as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        workspace_packages
            .iter()
            .copied()
            .find(|p| p["id"] == defaults[0])
            .ok_or("default package not found")?
    };
    let library = package["targets"].as_array().and_then(|targets| targets.iter().find(|target| {
        target["kind"].as_array().is_some_and(|kinds| kinds.iter().any(|k| k == "lib" || k == "rlib"))
    })).ok_or("selected package has no Rust library target; add src/lib.rs and aoc_lib! { year = ... }")?;
    let library_name = library["name"].as_str().ok_or("library has no name")?;
    let package_name = package["name"].as_str().ok_or("package has no name")?;
    let manifest_path = PathBuf::from(
        package["manifest_path"]
            .as_str()
            .ok_or("package has no manifest path")?,
    );
    let consumer_dir = manifest_path.parent().ok_or("manifest has no directory")?;
    let target_dir = PathBuf::from(
        metadata["target_directory"]
            .as_str()
            .ok_or("metadata has no target directory")?,
    );
    let workspace_root = PathBuf::from(
        metadata["workspace_root"]
            .as_str()
            .ok_or("metadata has no workspace root")?,
    );

    Ok(ConsumerProject {
        package: package.clone(),
        manifest_path: manifest_path.clone(),
        manifest_dir: consumer_dir.to_owned(),
        library_source: PathBuf::from(
            library["src_path"]
                .as_str()
                .ok_or("library has no source path")?,
        ),
        library_name: library_name.to_owned(),
        package_name: package_name.to_owned(),
        target_dir,
        workspace_root,
    })
}

pub(crate) fn build_bridge(
    options: &BuildOptions,
    project: &ConsumerProject,
) -> Result<PathBuf, String> {
    let ConsumerProject {
        package,
        manifest_path,
        manifest_dir: consumer_dir,
        library_name,
        package_name,
        target_dir,
        workspace_root,
        ..
    } = project;

    let mut features = Vec::new();
    for list in &options.features {
        for feature in list.split([',', ' ']).filter(|f| !f.is_empty()) {
            let prefix = format!("{package_name}/");
            features.push(feature.strip_prefix(&prefix).unwrap_or(feature).to_owned());
        }
    }
    if options.all_features {
        features.extend(
            package["features"]
                .as_object()
                .ok_or("metadata has no feature map")?
                .keys()
                .cloned(),
        );
    }
    features.sort();
    features.dedup();

    let mut hash = DefaultHasher::new();
    (
        manifest_path.as_os_str(),
        library_name,
        &features,
        options.no_default_features,
        &options.profile,
        &options.target,
        options.debug_info,
        &options.config,
    )
        .hash(&mut hash);
    let name = format!("aoc-harness-bridge-{:016x}", hash.finish());
    let bridge = target_dir.join("aoc-harness").join(&name);
    std::fs::create_dir_all(bridge.join("src")).map_err(|e| format!("create bridge: {e}"))?;

    let mut manifest: toml::Value = toml::from_str("[package]\nname = 'bridge'\nversion = '0.0.0'\nedition = '2021'\n[workspace]\n[dependencies]\n").unwrap();
    manifest["package"]["name"] = name.clone().into();
    let mut dependency = toml::map::Map::new();
    dependency.insert("package".into(), package_name.as_str().into());
    dependency.insert(
        "path".into(),
        consumer_dir.to_string_lossy().into_owned().into(),
    );
    dependency.insert(
        "default-features".into(),
        (!options.no_default_features).into(),
    );
    dependency.insert(
        "features".into(),
        toml::Value::Array(features.into_iter().map(Into::into).collect()),
    );
    manifest["dependencies"]
        .as_table_mut()
        .unwrap()
        .insert("consumer".into(), toml::Value::Table(dependency));

    let root_manifest: toml::Value = std::fs::read_to_string(workspace_root.join("Cargo.toml"))
        .map_err(|e| e.to_string())?
        .parse()
        .map_err(|e| format!("workspace manifest: {e}"))?;
    for key in ["profile", "patch", "replace"] {
        if let Some(value) = root_manifest.get(key) {
            let mut value = value.clone();
            if key != "profile" {
                normalize_patch_paths(&mut value, workspace_root);
            }
            manifest.as_table_mut().unwrap().insert(key.into(), value);
        }
    }
    let debug_config = reports::debug_config(options, &manifest, consumer_dir)?;
    reports::configure_debug(&mut manifest, options)?;
    write_changed(
        &bridge.join("Cargo.toml"),
        &toml::to_string(&manifest).map_err(|e| e.to_string())?,
    )?;
    write_changed(
        &bridge.join("src/main.rs"),
        "fn main() { std::process::exit(consumer::__aoc_harness_main()); }\n",
    )?;

    // Seed dependency resolution from the consumer; never modify its lockfile.
    // Include the generated root package so --locked works on the first build.
    let bridge_lock = bridge.join("Cargo.lock");
    let consumer_lock = workspace_root.join("Cargo.lock");
    if consumer_lock.exists() {
        let mut lock: toml::Value = std::fs::read_to_string(&consumer_lock)
            .map_err(|e| e.to_string())?
            .parse()
            .map_err(|e| format!("consumer lockfile: {e}"))?;
        let mut entry = toml::map::Map::new();
        entry.insert("name".into(), name.clone().into());
        entry.insert("version".into(), "0.0.0".into());
        entry.insert(
            "dependencies".into(),
            toml::Value::Array(vec![format!(
                "{} {}",
                package_name,
                package["version"]
                    .as_str()
                    .ok_or("package has no version")?
            )
            .into()]),
        );
        lock["package"]
            .as_array_mut()
            .ok_or("consumer lockfile has no packages")?
            .push(toml::Value::Table(entry));
        write_changed(
            &bridge_lock,
            &toml::to_string(&lock).map_err(|e| e.to_string())?,
        )?;
    }

    let mut build = cargo();
    build
        .current_dir(consumer_dir)
        .args(["build", "--message-format=json", "--manifest-path"])
        .arg(bridge.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(target_dir)
        .arg("--profile")
        .arg(&options.profile);
    if options.locked {
        build.arg("--locked");
    }
    if let Some(target) = &options.target {
        build.arg("--target").arg(target);
    }
    common(&mut build, options);
    for config in debug_config {
        build.arg("--config").arg(config);
    }
    let mut child = build
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("cargo build: {e}"))?;
    let mut executable = None;
    for line in BufReader::new(child.stdout.take().unwrap()).lines() {
        let line = line.map_err(|e| format!("read Cargo build messages: {e}"))?;
        if let Ok(message) = serde_json::from_str::<Value>(&line) {
            if message["reason"] == "compiler-artifact"
                && message["target"]["name"].as_str() == Some(&name)
            {
                if let Some(path) = message["executable"].as_str() {
                    executable = Some(PathBuf::from(path));
                }
            }
            if let Some(rendered) = message["message"]["rendered"].as_str() {
                eprint!("{rendered}");
            }
        } else {
            eprintln!("{line}");
        }
    }
    let status = child.wait().map_err(|e| format!("wait for Cargo: {e}"))?;
    if !status.success() {
        return Err(format!(
            "Cargo build failed (status {})",
            status.code().unwrap_or(1)
        ));
    }
    let executable = executable.ok_or("Cargo succeeded without reporting the bridge executable")?;

    Ok(executable)
}
