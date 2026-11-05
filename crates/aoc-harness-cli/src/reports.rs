use super::BuildOptions;
use std::path::Path;

pub(crate) fn open(
    report_dir: &Path,
    opener: impl FnOnce(&str) -> Result<(), String>,
) -> Result<(), String> {
    let path = report_dir.join("report/index.html");
    let path = path.canonicalize().map_err(|e| {
        format!(
            "measurement succeeded, but report {} is unavailable: {e}",
            path.display()
        )
    })?;
    let url = reqwest::Url::from_file_path(&path).map_err(|_| {
        format!(
            "measurement succeeded, but report path cannot be opened: {}",
            path.display()
        )
    })?;
    opener(url.as_str()).map_err(|e| {
        format!(
            "measurement succeeded, but opening report {} failed: {e}",
            path.display()
        )
    })
}

pub(crate) fn configure_debug(
    manifest: &mut toml::Value,
    options: &BuildOptions,
) -> Result<(), String> {
    if !options.debug_info {
        return Ok(());
    }
    let profile = options.profile.as_str();
    let root = manifest
        .as_table_mut()
        .unwrap()
        .entry("profile")
        .or_insert_with(|| toml::Value::Table(Default::default()));
    let table = root
        .as_table_mut()
        .ok_or("invalid profile configuration")?
        .entry(profile)
        .or_insert_with(|| toml::Value::Table(Default::default()));
    override_debug(table);
    Ok(())
}

fn override_debug(value: &mut toml::Value) {
    if let Some(table) = value.as_table_mut() {
        table.insert("debug".into(), toml::Value::Integer(2));
        table.insert("strip".into(), toml::Value::Boolean(false));
        for name in ["package", "build-override"] {
            if let Some(child) = table.get_mut(name) {
                if name == "package" {
                    if let Some(packages) = child.as_table_mut() {
                        for (_, package) in packages.iter_mut() {
                            override_debug(package);
                        }
                    }
                } else {
                    override_debug(child);
                }
            }
        }
    }
}

// Command-line configuration outranks copied manifest settings and environment.
// Include package overrides from Cargo configuration as well as the workspace.
pub(crate) fn debug_config(
    options: &BuildOptions,
    manifest: &toml::Value,
    consumer: &Path,
) -> Result<Vec<String>, String> {
    if !options.debug_info {
        return Ok(Vec::new());
    }
    let profile = options.profile.as_str();
    let mut settings = vec![manifest.clone()];
    let mut directories: Vec<_> = consumer.ancestors().map(|p| p.join(".cargo")).collect();
    if let Some(home) = std::env::var_os("CARGO_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| dirs::home_dir().map(|p| p.join(".cargo")))
    {
        directories.push(home);
    }
    for directory in directories {
        let legacy = directory.join("config");
        let path = if legacy.exists() {
            legacy
        } else {
            directory.join("config.toml")
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => settings.push(
                text.parse::<toml::Value>()
                    .map_err(|e| format!("Cargo debug configuration {}: {e}", path.display()))?,
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("read Cargo configuration {}: {e}", path.display())),
        }
    }
    for config in &options.config {
        let text = if config.contains('=') {
            config.clone()
        } else {
            std::fs::read_to_string(config).map_err(|e| format!("read --config {config}: {e}"))?
        };
        settings.push(
            text.parse::<toml::Value>()
                .map_err(|e| format!("parse --config for debug info: {e}"))?,
        );
    }
    if !["dev", "release", "test", "bench"].contains(&profile)
        && !settings
            .iter()
            .any(|s| s.get("profile").and_then(|p| p.get(profile)).is_some())
    {
        return Err(format!(
            "--debug-info requires a declared custom profile: {profile}"
        ));
    }
    let mut prefixes = std::collections::BTreeSet::from([
        format!("profile.{profile}"),
        format!("profile.{profile}.build-override"),
    ]);
    // Package overrides can be inherited from another profile or loaded from
    // Cargo configuration. Override their debug/strip fields individually.
    for source in settings {
        if let Some(profiles) = source.get("profile").and_then(|p| p.as_table()) {
            for (_, value) in profiles {
                if let Some(packages) = value.get("package").and_then(|p| p.as_table()) {
                    for name in packages.keys() {
                        prefixes.insert(format!(
                            "profile.{profile}.package.{}",
                            toml::Value::String(name.clone())
                        ));
                    }
                }
            }
        }
    }
    Ok(prefixes
        .into_iter()
        .flat_map(|p| [format!("{p}.debug=2"), format!("{p}.strip=false")])
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opener_uses_file_url_and_reports_measurement_success_on_failure() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("report")).unwrap();
        std::fs::write(temp.path().join("report/index.html"), "report").unwrap();

        open(temp.path(), |url| {
            assert!(url.starts_with("file:///"));
            Ok(())
        })
        .unwrap();
        let error = open(temp.path(), |_| Err("synthetic opener failure".into())).unwrap_err();

        assert!(error.contains("measurement succeeded"));
        assert!(error.contains("report/index.html"));
        assert!(open(&temp.path().join("missing"), |_| panic!("missing report")).is_err());
    }
}
