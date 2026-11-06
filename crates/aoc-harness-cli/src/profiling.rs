use clap::ValueEnum;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum Profiler {
    Flamegraph,
    Samply,
    Perf,
    Xctrace,
}

pub(crate) struct ProfileOptions {
    pub profiler: Profiler,
    pub output: Option<PathBuf>,
    pub template: Option<String>,
}

pub(crate) struct Backend {
    profiler: Profiler,
    program: PathBuf,
    environment: Vec<(&'static str, PathBuf)>,
}

impl Profiler {
    fn name(self) -> &'static str {
        match self {
            Self::Flamegraph => "flamegraph",
            Self::Samply => "samply",
            Self::Perf => "perf",
            Self::Xctrace => "xctrace",
        }
    }

    fn filename(self) -> &'static str {
        match self {
            Self::Flamegraph => "flamegraph.svg",
            Self::Samply => "profile.json.gz",
            Self::Perf => "perf.data",
            Self::Xctrace => "profile.trace",
        }
    }

    fn validate_platform(self, os: &str) -> Result<(), String> {
        match self {
            Self::Perf if os != "linux" => Err("--profiler perf is only supported on Linux".into()),
            Self::Xctrace if os != "macos" => {
                Err("--profiler xctrace is only supported on macOS".into())
            }
            // Let the portable tools determine support on additional platforms.
            _ => Ok(()),
        }
    }
}

fn executable(name: &str) -> Result<PathBuf, String> {
    let names = vec![OsString::from(name)];
    #[cfg(windows)]
    let names: Vec<_> = names
        .into_iter()
        .chain(
            ["exe", "com"]
                .into_iter()
                .map(|extension| OsString::from(format!("{name}.{extension}"))),
        )
        .collect();

    for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        for name in &names {
            let path = directory.join(name);
            if !path.is_file() {
                continue;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if std::fs::metadata(&path).is_ok_and(|m| m.permissions().mode() & 0o111 == 0) {
                    continue;
                }
            }
            return path
                .canonicalize()
                .map_err(|e| format!("resolve {}: {e}", path.display()));
        }
    }

    Err(format!(
        "{name} is not installed or not on PATH; install it before profiling"
    ))
}

fn xctrace() -> Result<PathBuf, String> {
    let output = Command::new(executable("xcrun")?)
        .args(["--find", "xctrace"])
        .output()
        .map_err(|e| format!("locate xctrace: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "xctrace is unavailable; install/select full Xcode: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let path = PathBuf::from(
        String::from_utf8(output.stdout)
            .map_err(|e| format!("xctrace path: {e}"))?
            .trim(),
    );
    if !path.is_absolute() || !path.is_file() {
        return Err("xcrun did not return an existing absolute xctrace path".into());
    }
    Ok(path)
}

fn available_output(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Err(format!(
            "profiling output already exists: {}; choose a new --output path",
            path.display()
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("inspect profiling output {}: {e}", path.display())),
    }
}

impl ProfileOptions {
    pub fn preflight(&mut self) -> Result<Backend, String> {
        self.profiler.validate_platform(std::env::consts::OS)?;
        if let Some(path) = &mut self.output {
            if !path.is_absolute() {
                *path = std::env::current_dir()
                    .map_err(|e| e.to_string())?
                    .join(&*path);
            }
            available_output(path)?;
        }

        let program = if self.profiler == Profiler::Xctrace {
            xctrace()?
        } else {
            let program = executable(self.profiler.name())?;
            let status = Command::new(&program)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map_err(|e| format!("check {}: {e}", self.profiler.name()))?;
            if !status.success() {
                return Err(format!(
                    "{} --version failed: {status}",
                    self.profiler.name()
                ));
            }
            program
        };
        let mut environment = Vec::new();
        if self.profiler == Profiler::Flamegraph {
            match std::env::consts::OS {
                "linux" => environment.push(("PERF", executable("perf")?)),
                "macos" => environment.push(("XCTRACE", xctrace()?)),
                _ => {}
            }
        }

        Ok(Backend {
            profiler: self.profiler,
            program,
            environment,
        })
    }
}

impl Backend {
    fn command(
        &self,
        output: &Path,
        template: Option<&str>,
        bridge: &Path,
        runtime: &[OsString],
    ) -> Command {
        let mut command = Command::new(&self.program);
        match self.profiler {
            Profiler::Flamegraph => {
                command.arg("--output").arg(output).arg("--");
            }
            Profiler::Samply => {
                command
                    .args(["record", "--save-only", "--output"])
                    .arg(output)
                    .arg("--");
            }
            Profiler::Perf => {
                command
                    .args([
                        "record",
                        "-F",
                        "997",
                        "--call-graph",
                        "dwarf,64000",
                        "--output",
                    ])
                    .arg(output)
                    .arg("--");
            }
            Profiler::Xctrace => {
                command
                    .args(["record", "--template"])
                    .arg(template.unwrap_or("Time Profiler"))
                    .arg("--output")
                    .arg(output)
                    .args(["--target-stdout", "-", "--launch", "--"]);
            }
        }
        command.arg(bridge).args(runtime);
        command.envs(self.environment.iter().map(|(key, value)| (key, value)));
        command
    }

    pub fn record(
        &self,
        options: &ProfileOptions,
        root: &Path,
        bridge: &Path,
        runtime: &[OsString],
    ) -> Result<i32, String> {
        std::fs::create_dir_all(root).map_err(|e| format!("create profiling directory: {e}"))?;
        let directory = tempfile::Builder::new()
            .prefix(self.profiler.name())
            .tempdir_in(root)
            .map_err(|e| format!("create recording directory: {e}"))?;
        // Keep recordings and diagnostic intermediates even when a tool fails.
        let directory = directory.keep();
        let output = options
            .output
            .clone()
            .unwrap_or_else(|| directory.join(self.profiler.filename()));
        available_output(&output)?;
        std::fs::create_dir_all(output.parent().ok_or("profiling output has no parent")?)
            .map_err(|e| format!("create output parent: {e}"))?;
        let completion = directory.join("workload-completed");
        let mut runtime = runtime.to_vec();
        runtime.extend([
            OsString::from("--profile-status"),
            completion.clone().into_os_string(),
        ]);
        runtime.extend([
            OsString::from("--report-dir"),
            directory.join("criterion").into_os_string(),
        ]);
        runtime.extend([
            OsString::from("--profile-working-dir"),
            std::env::current_dir()
                .map_err(|e| e.to_string())?
                .into_os_string(),
        ]);

        let mut command = self.command(&output, options.template.as_deref(), bridge, &runtime);
        // flamegraph uses fixed intermediate filenames. Isolate recordings from
        // each other; the bridge restores the invocation directory for user code.
        command.current_dir(&directory);
        eprintln!(
            "Recording with {}. Output: {}",
            self.profiler.name(),
            output.display()
        );
        let status = command
            .status()
            .map_err(|e| format!("launch {}: {e}", self.profiler.name()))?;
        if !status.success() {
            eprintln!(
                "{} recording failed; diagnostic files: {}",
                self.profiler.name(),
                directory.display()
            );
            return Ok(status.code().filter(|code| *code != 0).unwrap_or(1));
        }

        // Some collectors return success even when the target process fails.
        if std::fs::read(&completion).ok().as_deref() != Some(b"ok\n") {
            return Err(format!("{} finished, but the profiling workload did not complete successfully; recording: {}",
                self.profiler.name(), output.display()));
        }
        let metadata = std::fs::metadata(&output).map_err(|e| {
            format!(
                "{} finished without a recording at {}: {e}",
                self.profiler.name(),
                output.display()
            )
        })?;
        if self.profiler == Profiler::Xctrace {
            if !metadata.is_dir()
                || std::fs::read_dir(&output)
                    .map_err(|e| e.to_string())?
                    .next()
                    .is_none()
            {
                return Err(format!(
                    "xctrace produced an empty or invalid trace: {}",
                    output.display()
                ));
            }
        } else if !metadata.is_file() || metadata.len() == 0 {
            return Err(format!(
                "{} produced an empty or invalid recording: {}",
                self.profiler.name(),
                output.display()
            ));
        }

        println!("Profile recording: {}", output.display());
        match self.profiler {
            Profiler::Samply => println!("View with: samply load {:?}", output),
            Profiler::Perf => println!("View with: perf report -i {:?}", output),
            Profiler::Xctrace => println!("Open the .trace recording in Instruments."),
            Profiler::Flamegraph => println!("Open the SVG recording in a browser."),
        }
        Ok(0)
    }
}

pub(crate) fn workload(description: &[u8]) -> Result<String, String> {
    let invalid = || {
        "profiling requires an aoc-harness runtime with profiling-v1 support; upgrade the consumer's runtime".to_owned()
    };
    let value: serde_json::Value = serde_json::from_slice(description).map_err(|_| invalid())?;
    if !value["capabilities"]
        .as_array()
        .is_some_and(|items| items.iter().any(|v| v == "profiling-v1"))
    {
        return Err(invalid());
    }
    let labels = value["profile_workloads"].as_array().ok_or_else(invalid)?;
    if labels.len() != 1 {
        return Err("profiling requires exactly one compiled workload".into());
    }
    labels[0].as_str().map(str::to_owned).ok_or_else(invalid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_checks_only_restrict_native_collectors() {
        for os in ["linux", "macos", "windows"] {
            assert!(Profiler::Flamegraph.validate_platform(os).is_ok());
            assert!(Profiler::Samply.validate_platform(os).is_ok());
            assert_eq!(Profiler::Perf.validate_platform(os).is_ok(), os == "linux");
            assert_eq!(
                Profiler::Xctrace.validate_platform(os).is_ok(),
                os == "macos"
            );
        }
    }

    #[test]
    fn collectors_receive_the_exact_bridge_and_arguments_without_a_shell() {
        let output = Path::new("recordings with spaces/profile");
        let bridge = Path::new("build with spaces/bridge");
        let runtime = [
            "profile",
            "--solution",
            "variant with spaces",
            "--duration",
            "10",
        ]
        .map(OsString::from);

        for (profiler, prefix) in [
            (
                Profiler::Flamegraph,
                vec!["--output", output.to_str().unwrap(), "--"],
            ),
            (
                Profiler::Samply,
                vec![
                    "record",
                    "--save-only",
                    "--output",
                    output.to_str().unwrap(),
                    "--",
                ],
            ),
            (
                Profiler::Perf,
                vec![
                    "record",
                    "-F",
                    "997",
                    "--call-graph",
                    "dwarf,64000",
                    "--output",
                    output.to_str().unwrap(),
                    "--",
                ],
            ),
            (
                Profiler::Xctrace,
                vec![
                    "record",
                    "--template",
                    "CPU Counters",
                    "--output",
                    output.to_str().unwrap(),
                    "--target-stdout",
                    "-",
                    "--launch",
                    "--",
                ],
            ),
        ] {
            let backend = Backend {
                profiler,
                program: "collector".into(),
                environment: Vec::new(),
            };

            let command = backend.command(output, Some("CPU Counters"), bridge, &runtime);
            let expected: Vec<OsString> = prefix
                .into_iter()
                .map(OsString::from)
                .chain(std::iter::once(bridge.as_os_str().to_owned()))
                .chain(runtime.clone())
                .collect();

            assert_eq!(command.get_program(), "collector");
            assert_eq!(command.get_args().collect::<Vec<_>>(), expected);
        }
    }

    #[test]
    fn output_and_protocol_validation_reject_stale_or_unsupported_results() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("existing.svg");
        std::fs::write(&output, "preserved").unwrap();

        assert!(available_output(&output)
            .unwrap_err()
            .contains("already exists"));
        assert!(workload(br#"{"capabilities":["input-plan-v1"]}"#)
            .unwrap_err()
            .contains("upgrade"));
        assert!(workload(br#"{"capabilities":["profiling-v1"],"profile_workloads":[]}"#).is_err());
        assert_eq!(std::fs::read_to_string(&output).unwrap(), "preserved");
    }
}
