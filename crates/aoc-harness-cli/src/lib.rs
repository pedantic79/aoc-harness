use clap::Parser;
use serde_json::Value;
use std::{
    collections::hash_map::DefaultHasher,
    ffi::OsString,
    hash::{Hash, Hasher},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug, Default, Parser)]
#[command(
    name = "aoc-harness",
    about = "Run a library-only Advent of Code consumer",
    after_help = "Commands: run (default), list, bench\nSelection: --day N --part N --solution NAME --all-solutions --input PATH\nBenchmarks: --filter REGEX --sample-size N --warm-up-time SECONDS --measurement-time SECONDS --smoke\nAll options may appear before or after the command."
)]
struct BuildOptions {
    #[arg(long)]
    manifest_path: Option<PathBuf>,
    #[arg(short, long)]
    package: Option<String>,
    #[arg(short = 'F', long, action = clap::ArgAction::Append)]
    features: Vec<String>,
    #[arg(long)]
    all_features: bool,
    #[arg(long)]
    no_default_features: bool,
    #[arg(long, default_value = "release")]
    profile: String,
    #[arg(long)]
    target: Option<String>,
    #[arg(long)]
    target_dir: Option<PathBuf>,
    #[arg(long)]
    offline: bool,
    #[arg(long)]
    locked: bool,
    #[arg(long, action = clap::ArgAction::Append)]
    config: Vec<String>,
}

fn split_arguments(args: Vec<OsString>) -> (Vec<OsString>, Vec<OsString>) {
    let mut build = vec![OsString::from("aoc-harness")];
    let mut runtime = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let text = arg.to_string_lossy();
        let key = text.split('=').next().unwrap();
        let takes_value = matches!(
            key,
            "--manifest-path"
                | "--package"
                | "-p"
                | "--features"
                | "-F"
                | "--profile"
                | "--target"
                | "--target-dir"
                | "--config"
        );
        let flag = matches!(
            key,
            "--all-features" | "--no-default-features" | "--offline" | "--locked"
        );
        if takes_value || flag {
            let inline = text.contains('=');
            build.push(arg);
            if takes_value && !inline {
                if let Some(value) = args.next() {
                    build.push(value);
                }
            }
        } else {
            runtime.push(arg);
        }
    }
    (build, runtime)
}

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

fn run(options: BuildOptions, runtime: Vec<OsString>) -> Result<i32, String> {
    let mut metadata = cargo();
    metadata.args(["metadata", "--format-version", "1", "--no-deps"]);
    if let Some(manifest) = &options.manifest_path {
        let manifest = absolute(manifest)?;
        metadata.current_dir(manifest.parent().ok_or("manifest path has no directory")?);
        metadata.arg("--manifest-path").arg(manifest);
    }
    if let Some(target_dir) = &options.target_dir {
        metadata.env("CARGO_TARGET_DIR", absolute(target_dir)?);
    }
    common(&mut metadata, &options);
    let output = metadata
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !output.status.success() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        return Ok(output.status.code().unwrap_or(1));
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
    )
        .hash(&mut hash);
    let name = format!("aoc-harness-bridge-{:016x}", hash.finish());
    let bridge = target_dir.join("aoc-harness").join(&name);
    std::fs::create_dir_all(bridge.join("src")).map_err(|e| format!("create bridge: {e}"))?;

    let mut manifest: toml::Value = toml::from_str("[package]\nname = 'bridge'\nversion = '0.0.0'\nedition = '2021'\n[workspace]\n[dependencies]\n").unwrap();
    manifest["package"]["name"] = name.clone().into();
    let mut dependency = toml::map::Map::new();
    dependency.insert("package".into(), package_name.into());
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
                normalize_patch_paths(&mut value, &workspace_root);
            }
            manifest.as_table_mut().unwrap().insert(key.into(), value);
        }
    }
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
        .arg(&target_dir)
        .arg("--profile")
        .arg(&options.profile);
    if options.locked {
        build.arg("--locked");
    }
    if let Some(target) = &options.target {
        build.arg("--target").arg(target);
    }
    common(&mut build, &options);
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
        return Ok(status.code().unwrap_or(1));
    }
    let executable = executable.ok_or("Cargo succeeded without reporting the bridge executable")?;

    let status = Command::new(executable)
        .args(runtime)
        .arg("--report-dir")
        .arg(target_dir.join("criterion").join(package_name))
        .status()
        .map_err(|e| format!("run generated bridge: {e}"))?;
    Ok(status.code().unwrap_or(1))
}

pub fn main_entry(cargo_subcommand: bool) -> i32 {
    let mut args: Vec<_> = std::env::args_os().skip(1).collect();
    if cargo_subcommand && args.first().is_some_and(|s| s == "aoc-harness") {
        args.remove(0);
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        use clap::CommandFactory;
        BuildOptions::command().print_help().expect("print help");
        println!();
        return 0;
    }
    let (build, runtime) = split_arguments(args);
    let options = match BuildOptions::try_parse_from(build) {
        Ok(options) => options,
        Err(error) => {
            let _ = error.print();
            return error.exit_code();
        }
    };
    match run(options, runtime) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error}");
            1
        }
    }
}
