//! Compatibility between this CLI and the compiled `aoc-harness` runtime.
//!
//! The two crates are versioned independently, so the CLI never compares
//! versions. It asks the runtime which capabilities it has and turns a missing
//! one into an upgrade of the runtime, naming the first release with it. A
//! newer runtime explains a protocol it no longer speaks, and a capability the
//! CLI doesn't recognize shows that a newer CLI exists.

use crate::project::{Bridge, ConsumerProject};
use serde_json::Value;
use std::{
    fmt,
    path::Path,
    process::{Command, Output},
};

/// The describe protocol this CLI speaks.
pub(crate) const PROTOCOL: u64 = 1;

const CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Installs the latest CLI, across breaking 0.x releases.
const UPGRADE_CLI: &str = "cargo install aoc-harness-cli --locked";

/// Capabilities this CLI uses, each with the first `aoc-harness` release that
/// advertises it.
const CAPABILITIES: [(&str, &str); 3] = [
    ("input-plan-v1", "0.1.0"),
    ("profiling-v1", "0.2.0"),
    ("years-v1", "0.5.0"),
];

/// Why the compiled runtime couldn't be described.
#[derive(Debug)]
pub(crate) enum ProbeError {
    /// The runtime doesn't speak this CLI's protocol; carries its explanation.
    Protocol(String),
    /// The runtime crashed or answered with something other than a description.
    Failed(String),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(message) | Self::Failed(message) => f.write_str(message),
        }
    }
}

/// What a compiled runtime reports about itself.
pub(crate) struct Runtime {
    capabilities: Vec<String>,
    version: Option<String>,
    pub years: Option<RuntimeYears>,
}

impl Runtime {
    /// Ask the compiled runtime for its capabilities.
    ///
    /// Runtimes before `years-v1` (`aoc-harness` 0.4 and older) reject
    /// `--capabilities` but still describe plans, and a plan lists the
    /// runtime's capabilities. Only a protocol 1 `list` plan lacking
    /// `years-v1` counts as an older runtime; any other failure is an error.
    pub fn probe(executable: &Path) -> Result<Self, ProbeError> {
        let capabilities = describe(executable, "--capabilities")?;
        if capabilities.status.success() {
            return parse(&capabilities.stdout).ok_or_else(|| {
                ProbeError::Failed(
                    "the compiled aoc-harness runtime returned malformed capabilities".into(),
                )
            });
        }
        if let Some(message) = rejection(&capabilities) {
            return Err(ProbeError::Protocol(message));
        }

        let plan = describe(executable, "list")?;
        if let Some(message) = rejection(&plan) {
            return Err(ProbeError::Protocol(message));
        }

        let listed = serde_json::from_slice::<Value>(&plan.stdout)
            .is_ok_and(|plan| plan["protocol"] == PROTOCOL && plan["action"] == "list");

        plan.status
            .success()
            .then(|| parse(&plan.stdout))
            .flatten()
            .filter(|runtime| listed && !runtime.has("years-v1"))
            .ok_or_else(|| {
                ProbeError::Failed(failure(
                    "cannot describe the compiled aoc-harness runtime",
                    &capabilities,
                ))
            })
    }

    fn has(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }

    /// A notice when the runtime has capabilities this CLI predates.
    pub fn notice(&self) -> Option<String> {
        let unknown = self
            .capabilities
            .iter()
            .any(|c| CAPABILITIES.iter().all(|(known, _)| known != c));
        if !unknown {
            return None;
        }

        let runtime = match &self.version {
            Some(version) => format!("aoc-harness {version} runtime"),
            None => "aoc-harness runtime".into(),
        };
        Some(format!(
            "this solution's {runtime} supports features aoc-harness-cli {CLI_VERSION} \
             doesn't recognize; run `{UPGRADE_CLI}` to upgrade"
        ))
    }

    /// Require a capability for `feature`, or explain how to upgrade the runtime.
    pub fn require(
        &self,
        capability: &str,
        feature: &str,
        project: &ConsumerProject,
        bridge: &Bridge,
    ) -> Result<(), String> {
        if self.has(capability) {
            return Ok(());
        }

        let needed = CAPABILITIES
            .iter()
            .find(|(known, _)| *known == capability)
            .map_or("a newer release", |(_, release)| release);
        let found = locked_runtime_version(&bridge.lockfile, project).or(self.version.clone());
        let found = match found {
            Some(version) => format!("uses aoc-harness {version}"),
            None => "uses an older aoc-harness".into(),
        };

        Err(format!(
            "{feature} needs aoc-harness {needed} or later, but {} {found}.\nFix: {}",
            project.manifest_path.display(),
            upgrade_runtime(project, needed)
        ))
    }

    /// Require `years-v1` and a registered `year`.
    pub fn require_year(
        &self,
        year: u16,
        project: &ConsumerProject,
        bridge: &Bridge,
    ) -> Result<(), String> {
        self.require("years-v1", "--year", project, bridge)?;

        let registered = self.years.as_ref().map_or(&[][..], |r| &r.years);
        if registered.contains(&year) {
            return Ok(());
        }
        let years: Vec<_> = registered.iter().map(u16::to_string).collect();
        Err(format!(
            "event year {year} is not registered; registered years: {}",
            years.join(", ")
        ))
    }
}

/// The event years a compiled runtime registers, from its capabilities.
pub(crate) struct RuntimeYears {
    pub years: Vec<u16>,
    pub default_year: Option<u16>,
}

impl RuntimeYears {
    /// The year that names benchmark and profiling directories. Only a crate
    /// that registers several years scopes them, so single-year paths and
    /// their saved baselines stay where they were. `None` also covers an
    /// ambiguous selection, which the runtime rejects before writing anything.
    pub fn artifact_year(this: Option<&Self>, requested: Option<u16>) -> Option<u16> {
        let runtime = this.filter(|runtime| runtime.years.len() > 1)?;

        requested.or(runtime.default_year)
    }
}

fn describe(executable: &Path, request: &str) -> Result<Output, ProbeError> {
    Command::new(executable)
        .args(["__describe", "--protocol", &PROTOCOL.to_string(), request])
        .output()
        .map_err(|e| ProbeError::Failed(format!("describe compiled runtime: {e}")))
}

/// A runtime description, or `None` when it is malformed.
fn parse(stdout: &[u8]) -> Option<Runtime> {
    let description: Value = serde_json::from_slice(stdout).ok()?;
    let capabilities = description["capabilities"]
        .as_array()?
        .iter()
        .map(|c| c.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()?;

    let year = |value: &Value| value.as_u64().and_then(|v| u16::try_from(v).ok());
    let years = if capabilities.iter().any(|c| c == "years-v1") {
        let years = description["years"]
            .as_array()?
            .iter()
            .map(year)
            .collect::<Option<Vec<_>>>()?;
        Some(RuntimeYears {
            years,
            default_year: year(&description["default_year"]),
        })
    } else {
        None
    };

    Some(Runtime {
        capabilities,
        version: description["runtime_version"].as_str().map(str::to_owned),
        years,
    })
}

/// Explain a failed describe request: the runtime's protocol rejection, or its
/// diagnostic.
pub(crate) fn describe_failure(context: &str, output: &Output) -> String {
    rejection(output).unwrap_or_else(|| failure(context, output))
}

fn failure(context: &str, output: &Output) -> String {
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    let diagnostic = diagnostic.trim();
    let diagnostic = diagnostic.strip_prefix("error: ").unwrap_or(diagnostic);

    if diagnostic.is_empty() {
        format!("{context} ({})", output.status)
    } else {
        format!("{context} ({}): {diagnostic}", output.status)
    }
}

/// The runtime's explanation when it rejects this CLI's protocol.
fn rejection(output: &Output) -> Option<String> {
    let structured = serde_json::from_slice::<Value>(&output.stdout)
        .ok()
        .filter(|value| value["error"] == "unsupported-protocol");
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    let diagnostic = diagnostic.trim();
    if structured.is_none() && !diagnostic.contains("unsupported describe protocol") {
        return None;
    }

    // The runtime's own message comes first; it knows what changed.
    let message = diagnostic.strip_prefix("error: ").unwrap_or(diagnostic);
    if !message.is_empty() {
        return Some(format!(
            "{message}\nYou have aoc-harness-cli {CLI_VERSION}, which speaks describe protocol {PROTOCOL}."
        ));
    }

    // Without one, a runtime that only speaks newer protocols needs a newer CLI.
    let error = structured?;
    let oldest = error["protocols"]
        .as_array()
        .and_then(|p| p.iter().filter_map(Value::as_u64).min());
    let runtime = error["runtime_version"].as_str();
    let cli = error["min_cli_version"].as_str();
    if let (Some(oldest), Some(runtime), Some(cli)) = (oldest, runtime, cli) {
        if oldest > PROTOCOL {
            return Some(format!(
                "this solution uses aoc-harness {runtime}, which needs aoc-harness-cli \
                 {cli} or later (you have {CLI_VERSION}).\nFix: {UPGRADE_CLI}"
            ));
        }
    }
    Some(format!(
        "the compiled aoc-harness runtime doesn't support describe protocol {PROTOCOL}, \
         which aoc-harness-cli {CLI_VERSION} speaks"
    ))
}

/// The `aoc-harness` version the bridge resolved for the selected package.
fn locked_runtime_version(lockfile: &Path, project: &ConsumerProject) -> Option<String> {
    let lock: toml::Value = std::fs::read_to_string(lockfile).ok()?.parse().ok()?;
    let packages = lock.get("package")?.as_array()?;
    let field = |package: &toml::Value, key: &str| package.get(key)?.as_str().map(str::to_owned);

    // The consumer is a path package; its entry names the runtime it uses,
    // with a version only when several are locked.
    let consumer = packages.iter().find(|package| {
        field(package, "name").as_deref() == Some(project.package_name.as_str())
            && package.get("source").is_none()
            && field(package, "version").as_deref() == project.package["version"].as_str()
    });
    let reference = consumer
        .and_then(|package| package.get("dependencies")?.as_array())
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .find(|dependency| dependency.split(' ').next() == Some("aoc-harness"));
    if let Some(version) = reference.and_then(|dependency| dependency.split(' ').nth(1)) {
        return Some(version.to_owned());
    }

    let mut runtimes = packages
        .iter()
        .filter(|package| field(package, "name").as_deref() == Some("aoc-harness"));
    let only = runtimes.next()?;
    runtimes
        .next()
        .is_none()
        .then(|| field(only, "version"))
        .flatten()
}

/// How to move the selected package to `needed`, from its declaration.
fn upgrade_runtime(project: &ConsumerProject, needed: &str) -> String {
    let manifest = project.manifest_path.display();
    let dependencies = project.package["dependencies"].as_array();
    let dependency = dependencies
        .into_iter()
        .flatten()
        .find(|dependency| dependency["name"] == "aoc-harness" && dependency["kind"].is_null());
    let Some(dependency) = dependency else {
        return format!("depend on aoc-harness {needed} or later in {manifest}");
    };
    let key = dependency["rename"].as_str().unwrap_or("aoc-harness");

    if inherits_from_workspace(&project.manifest_path, key) {
        return format!(
            "update `{key}` in [workspace.dependencies] of {} to aoc-harness {needed} or later",
            project.workspace_root.join("Cargo.toml").display()
        );
    }
    if let Some(path) = dependency["path"].as_str() {
        return format!("update the aoc-harness source at {path} to {needed} or later");
    }
    if dependency["source"]
        .as_str()
        .is_some_and(|source| source.starts_with("git+"))
    {
        return format!(
            "point the `{key}` git dependency in {manifest} at a revision with aoc-harness {needed} or later"
        );
    }
    if !dependency["rename"].is_null()
        || !dependency["registry"].is_null()
        || !dependency["target"].is_null()
    {
        return format!("set the `{key}` dependency in {manifest} to version \"{needed}\"");
    }

    format!("cargo add aoc-harness@{needed} --manifest-path {manifest}")
}

/// Whether the manifest declares `key` with `workspace = true`.
fn inherits_from_workspace(manifest: &Path, key: &str) -> bool {
    let Some(manifest) = std::fs::read_to_string(manifest)
        .ok()
        .and_then(|text| text.parse::<toml::Value>().ok())
    else {
        return false;
    };

    let targets = manifest
        .get("target")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(|targets| targets.values());
    std::iter::once(&manifest)
        .chain(targets)
        .filter_map(|table| {
            table
                .get("dependencies")?
                .get(key)?
                .get("workspace")?
                .as_bool()
        })
        .any(|workspace| workspace)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn project(directory: &Path, manifest: &str, dependency: Value) -> ConsumerProject {
        let manifest_path = directory.join("Cargo.toml");
        std::fs::write(&manifest_path, manifest).unwrap();

        ConsumerProject {
            package: serde_json::json!({ "version": "0.1.0", "dependencies": [dependency] }),
            manifest_path,
            manifest_dir: directory.to_owned(),
            library_source: directory.join("src/lib.rs"),
            library_name: "solutions".into(),
            package_name: "solutions".into(),
            target_dir: directory.join("target"),
            workspace_root: directory.to_owned(),
        }
    }

    fn registry(extra: Value) -> Value {
        let mut dependency = serde_json::json!({
            "name": "aoc-harness", "kind": null, "rename": null, "target": null, "registry": null,
            "source": "registry+https://github.com/rust-lang/crates.io-index",
        });
        for (key, value) in extra.as_object().unwrap() {
            dependency[key] = value.clone();
        }
        dependency
    }

    #[cfg(unix)]
    fn output(code: i32, stdout: &str, stderr: &str) -> Output {
        use std::os::unix::process::ExitStatusExt;

        Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    #[test]
    fn upgrades_follow_how_the_runtime_dependency_is_declared() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        let manifest = dir.join("Cargo.toml").display().to_string();
        let plain = "[dependencies]\naoc-harness = \"0.4\"\n";
        let cases = [
            (plain, registry(serde_json::json!({})), format!("cargo add aoc-harness@0.5.0 --manifest-path {manifest}")),
            ("[dependencies]\nharness = { package = \"aoc-harness\", version = \"0.4\" }\n", registry(serde_json::json!({ "rename": "harness" })), format!("set the `harness` dependency in {manifest} to version \"0.5.0\"")),
            ("[dependencies]\naoc-harness.workspace = true\n", registry(serde_json::json!({})), format!("update `aoc-harness` in [workspace.dependencies] of {manifest} to aoc-harness 0.5.0 or later")),
            ("[target.'cfg(unix)'.dependencies]\nharness = { workspace = true }\n", registry(serde_json::json!({ "rename": "harness" })), format!("update `harness` in [workspace.dependencies] of {manifest} to aoc-harness 0.5.0 or later")),
            (plain, registry(serde_json::json!({ "source": null, "path": "/src/aoc-harness" })), "update the aoc-harness source at /src/aoc-harness to 0.5.0 or later".into()),
            (plain, registry(serde_json::json!({ "source": "git+https://example.com/aoc-harness" })), format!("point the `aoc-harness` git dependency in {manifest} at a revision with aoc-harness 0.5.0 or later")),
            (plain, registry(serde_json::json!({ "kind": "dev" })), format!("depend on aoc-harness 0.5.0 or later in {manifest}")),
        ];

        for (contents, dependency, expected) in cases {
            let project = project(dir, contents, dependency.clone());

            let fix = upgrade_runtime(&project, "0.5.0");

            assert_eq!(fix, expected, "{dependency}");
        }
    }

    #[test]
    fn missing_capabilities_name_the_release_found_version_and_fix() {
        let temp = tempfile::tempdir().unwrap();
        let project = project(
            temp.path(),
            "[dependencies]\naoc-harness = \"0.4\"\n",
            registry(serde_json::json!({})),
        );
        let lockfile = temp.path().join("Cargo.lock");
        std::fs::write(
            &lockfile,
            "[[package]]\nname = 'solutions'\nversion = '0.1.0'\ndependencies = ['aoc-harness 0.4.1']\n\
             [[package]]\nname = 'aoc-harness'\nversion = '0.4.1'\nsource = 'registry+x'\n\
             [[package]]\nname = 'aoc-harness'\nversion = '0.3.0'\nsource = 'registry+x'\n",
        )
        .unwrap();
        let bridge = Bridge {
            executable: PathBuf::new(),
            lockfile,
        };
        let legacy =
            parse(br#"{"protocol":1,"capabilities":["input-plan-v1","profiling-v1"]}"#).unwrap();

        let year = legacy.require_year(2025, &project, &bridge).unwrap_err();
        let profiling = legacy.require("profiling-v1", "profiling", &project, &bridge);

        assert_eq!(
            year,
            format!(
                "--year needs aoc-harness 0.5.0 or later, but {0} uses aoc-harness 0.4.1.\n\
                 Fix: cargo add aoc-harness@0.5.0 --manifest-path {0}",
                project.manifest_path.display()
            )
        );
        assert!(profiling.is_ok());
        assert!(legacy.notice().is_none());
    }

    #[test]
    fn single_locked_runtimes_are_found_without_a_versioned_reference() {
        let temp = tempfile::tempdir().unwrap();
        let project = project(temp.path(), "", registry(serde_json::json!({})));
        let lockfile = temp.path().join("Cargo.lock");
        std::fs::write(
            &lockfile,
            "[[package]]\nname = 'solutions'\nversion = '0.1.0'\ndependencies = ['aoc-harness']\n\
             [[package]]\nname = 'aoc-harness'\nversion = '0.4.1'\n",
        )
        .unwrap();

        let version = locked_runtime_version(&lockfile, &project);
        let missing = locked_runtime_version(&temp.path().join("missing.lock"), &project);

        assert_eq!(version.as_deref(), Some("0.4.1"));
        assert_eq!(missing, None);
    }

    #[test]
    fn years_are_registered_only_by_runtimes_with_years_support() {
        let temp = tempfile::tempdir().unwrap();
        let project = project(temp.path(), "", registry(serde_json::json!({})));
        let bridge = Bridge {
            executable: PathBuf::new(),
            lockfile: temp.path().join("Cargo.lock"),
        };
        let current = parse(br#"{"capabilities":["years-v1"],"years":[2019,2025],"default_year":null,"runtime_version":"0.5.0"}"#).unwrap();
        let older = parse(br#"{"capabilities":[],"runtime_version":"0.4.9"}"#).unwrap();

        let registered = current.require_year(2019, &project, &bridge);
        let unknown = current.require_year(2018, &project, &bridge).unwrap_err();
        let reported = older.require_year(2019, &project, &bridge).unwrap_err();

        assert!(registered.is_ok());
        assert_eq!(
            unknown,
            "event year 2018 is not registered; registered years: 2019, 2025"
        );
        assert!(reported.contains("uses aoc-harness 0.4.9."), "{reported}");
        assert!(parse(br#"{"capabilities":["years-v1"]}"#).is_none());
        assert!(parse(b"not json").is_none());
    }

    #[test]
    fn unrecognized_capabilities_suggest_a_newer_cli() {
        let newer =
            parse(br#"{"capabilities":["input-plan-v1","future-v1"],"runtime_version":"9.0.0"}"#)
                .unwrap();
        let current = parse(
            br#"{"capabilities":["input-plan-v1","profiling-v1","years-v1"],"years":[2025]}"#,
        )
        .unwrap();

        let notice = newer.notice().unwrap();

        assert!(
            notice.contains("aoc-harness 9.0.0 runtime supports features"),
            "{notice}"
        );
        assert!(
            notice.contains("cargo install aoc-harness-cli --locked"),
            "{notice}"
        );
        assert!(current.notice().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn protocol_rejections_explain_which_side_to_upgrade() {
        let structured = r#"{"error":"unsupported-protocol","requested":1,"protocols":[2],"runtime_version":"0.9.0","min_cli_version":"0.8.0"}"#;
        let newer_runtime = output(
            1,
            structured,
            "error: this solution uses aoc-harness 0.9.0, which needs aoc-harness-cli 0.8.0 or later.\nFix: see the release notes\n",
        );
        let silent_runtime = output(1, structured, "");
        let unstructured = output(
            1,
            "",
            "error: unsupported describe protocol; expected --protocol 2\n",
        );
        let crashed = output(101, "", "thread 'main' panicked\n");

        let newer = describe_failure("cannot describe plan", &newer_runtime);
        let silent = describe_failure("cannot describe plan", &silent_runtime);
        let message = describe_failure("cannot describe plan", &unstructured);
        let crash = describe_failure("cannot describe plan", &crashed);

        assert_eq!(
            newer,
            format!(
                "this solution uses aoc-harness 0.9.0, which needs aoc-harness-cli 0.8.0 or later.\n\
                 Fix: see the release notes\n\
                 You have aoc-harness-cli {CLI_VERSION}, which speaks describe protocol 1."
            )
        );
        assert_eq!(
            silent,
            format!(
                "this solution uses aoc-harness 0.9.0, which needs aoc-harness-cli 0.8.0 or later \
                 (you have {CLI_VERSION}).\nFix: cargo install aoc-harness-cli --locked"
            )
        );
        assert!(
            message.starts_with("unsupported describe protocol; expected --protocol 2\n"),
            "{message}"
        );
        assert!(
            crash.starts_with("cannot describe plan (exit status: 101): thread 'main' panicked"),
            "{crash}"
        );
    }
}
