use clap::{ColorChoice, Parser};
use serde_json::Value;
use std::{
    collections::hash_map::DefaultHasher,
    ffi::OsString,
    hash::{Hash, Hasher},
    io::{self, BufRead, BufReader, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, SyncSender},
    thread,
};

mod help;

#[derive(Debug, Default, Parser)]
#[command(
    name = "aoc-harness",
    about = "Run a library-only Advent of Code consumer",
    next_help_heading = "Build options"
)]
struct BuildOptions {
    /// Path to the consumer's Cargo.toml.
    #[arg(long)]
    manifest_path: Option<PathBuf>,
    /// Consumer package to select from a workspace.
    #[arg(short, long)]
    package: Option<String>,
    /// Enable consumer features (comma-separated or repeated).
    #[arg(short = 'F', long, action = clap::ArgAction::Append)]
    features: Vec<String>,
    /// Enable all consumer features.
    #[arg(long)]
    all_features: bool,
    /// Disable the consumer's default features.
    #[arg(long)]
    no_default_features: bool,
    /// Cargo build profile.
    #[arg(long, default_value = "release")]
    profile: String,
    /// Compile for a target triple.
    #[arg(long)]
    target: Option<String>,
    /// Directory for build artifacts and generated bridges.
    #[arg(long)]
    target_dir: Option<PathBuf>,
    /// Build without accessing the network.
    #[arg(long)]
    offline: bool,
    /// Require an up-to-date consumer lockfile.
    #[arg(long)]
    locked: bool,
    /// Override Cargo configuration (repeatable).
    #[arg(long, action = clap::ArgAction::Append)]
    config: Vec<String>,
    /// Benchmark result colors; auto uses the terminal receiving the final results.
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto, help_heading = "Benchmark options")]
    color: ColorChoice,
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
                | "--color"
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

#[derive(Default)]
struct RuntimeMode<'a> {
    action: Option<&'a str>,
    smoke: bool,
    help: bool,
}

fn runtime_mode(args: &[OsString]) -> RuntimeMode<'_> {
    let mut args = args.iter();
    let mut mode = RuntimeMode::default();

    while let Some(arg) = args.next() {
        if matches!(arg.to_str(), Some("run" | "list" | "bench")) {
            mode.action = arg.to_str();
        } else if matches!(arg.to_str(), Some("--help" | "-h" | "help")) {
            mode.help = true;
        } else if arg == "--smoke" {
            mode.smoke = true;
        } else if matches!(
            arg.to_str(),
            Some(
                "--day"
                    | "--days"
                    | "--part"
                    | "--solution"
                    | "--input"
                    | "--filter"
                    | "--sample-size"
                    | "--warm-up-time"
                    | "--measurement-time"
                    | "--report-dir"
            )
        ) {
            args.next();
        }
    }

    mode
}

fn measured_bench(args: &[OsString]) -> bool {
    let mode = runtime_mode(args);

    mode.action == Some("bench") && !mode.smoke
}

fn benchmark_status(line: &[u8]) -> Option<(&str, &str)> {
    let text = std::str::from_utf8(line)
        .ok()?
        .trim_end_matches(['\r', '\n']);
    let text = text.strip_prefix("Benchmarking ")?;
    if !["generator/", "solver/", "end-to-end/", "full-run/"]
        .iter()
        .any(|family| text.starts_with(family))
    {
        return None;
    }

    match text.split_once(": ") {
        None => Some((text, "starting")),
        Some((name, phase)) if phase.starts_with("Warming up for ") => Some((name, "warming up")),
        Some((name, phase))
            if phase.starts_with("Collecting ") && phase.contains(" samples in estimated ") =>
        {
            Some((name, "measuring"))
        }
        Some((name, "Analyzing")) => Some((name, "analyzing")),
        _ => None,
    }
}

fn color_enabled(choice: ColorChoice, terminal: bool, no_color: bool, dumb_terminal: bool) -> bool {
    match choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => terminal && !no_color && !dumb_terminal,
    }
}

enum BenchmarkEvent {
    Output(Vec<u8>),
    Diagnostic(Vec<u8>),
    ReadError(io::Error),
}

fn read_benchmark_stream(
    stream: impl Read,
    events: SyncSender<BenchmarkEvent>,
    event: fn(Vec<u8>) -> BenchmarkEvent,
) {
    let mut stream = BufReader::new(stream);

    loop {
        let mut line = Vec::new();
        match stream.read_until(b'\n', &mut line) {
            Ok(0) => break,
            Ok(_) => {
                if events.send(event(line)).is_err() {
                    break;
                }
            }
            Err(error) => {
                let _ = events.send(BenchmarkEvent::ReadError(error));
                break;
            }
        }
    }
}

fn clear_benchmark_progress(progress: &mut impl Write) -> io::Result<()> {
    write!(progress, "\r\x1b[2K")?;
    progress.flush()
}

fn forward_benchmark_output(
    events: impl IntoIterator<Item = BenchmarkEvent>,
    stdout: &mut impl Write,
    progress: &mut impl Write,
    terminal: bool,
) -> io::Result<bool> {
    let mut started = false;
    let mut progress_visible = false;
    let mut line_open = false;

    let forward = (|| -> io::Result<()> {
        if terminal {
            write!(progress, "Running benchmarks...")?;
            progress_visible = true;
        } else {
            writeln!(progress, "Running benchmarks...")?;
        }
        progress.flush()?;

        for event in events {
            match event {
                BenchmarkEvent::Output(line) => {
                    if progress_visible {
                        clear_benchmark_progress(progress)?;
                        progress_visible = false;
                    }
                    if !started {
                        writeln!(stdout, "Benchmark results\n")?;
                        started = true;
                    }

                    stdout.write_all(&line)?;
                    stdout.flush()?;
                    line_open = !line.ends_with(b"\n");
                }
                BenchmarkEvent::Diagnostic(line) => {
                    if let Some((name, phase)) = benchmark_status(&line) {
                        if terminal && !line_open {
                            write!(progress, "\r\x1b[2KBenchmark {name}: {phase}...")?;
                            progress.flush()?;
                            progress_visible = true;
                        }
                    } else {
                        if progress_visible {
                            clear_benchmark_progress(progress)?;
                            progress_visible = false;
                        }
                        progress.write_all(&line)?;
                        progress.flush()?;
                    }
                }
                BenchmarkEvent::ReadError(error) => return Err(error),
            }
        }
        Ok(())
    })();

    if progress_visible {
        let _ = clear_benchmark_progress(progress);
    }

    forward.map(|()| started)
}

fn run_benchmarks(command: &mut Command) -> Result<i32, String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("run generated benchmark bridge: {e}"))?;
    let output = child.stdout.take().unwrap();
    let diagnostics = child.stderr.take().unwrap();
    let (events, receiver) = mpsc::sync_channel(64);
    let output_events = events.clone();
    let mut stdout = io::stdout().lock();
    let mut progress = io::stderr();
    let terminal =
        progress.is_terminal() && std::env::var_os("TERM").is_none_or(|term| term != "dumb");

    let forward = thread::scope(|scope| {
        scope.spawn(move || read_benchmark_stream(output, output_events, BenchmarkEvent::Output));
        scope.spawn(move || read_benchmark_stream(diagnostics, events, BenchmarkEvent::Diagnostic));

        let forward =
            forward_benchmark_output(receiver.iter(), &mut stdout, &mut progress, terminal);
        if forward.is_err() {
            let _ = child.kill();
        }
        drop(receiver);

        forward
    });

    // Always reap the process; results already printed remain visible on failure.
    let status = child
        .wait()
        .map_err(|e| format!("wait for benchmark bridge: {e}"))?;
    let started = forward.map_err(|e| format!("forward benchmark output: {e}"))?;
    if !status.success() && started {
        writeln!(stdout, "\nBenchmark output (incomplete)")
            .map_err(|e| format!("write benchmark failure: {e}"))?;
        stdout
            .flush()
            .map_err(|e| format!("flush benchmark failure: {e}"))?;
    }

    Ok(status.code().unwrap_or(1))
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

    let benchmark = measured_bench(&runtime);
    let color = color_enabled(
        options.color,
        io::stdout().is_terminal(),
        std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()),
        std::env::var_os("TERM").is_some_and(|term| term == "dumb"),
    );
    let mut command = Command::new(executable);
    command
        .args(runtime)
        .arg("--report-dir")
        .arg(target_dir.join("criterion").join(package_name))
        .arg("--color")
        .arg(if color { "always" } else { "never" });
    if benchmark {
        return run_benchmarks(&mut command);
    }

    let status = command
        .status()
        .map_err(|e| format!("run generated bridge: {e}"))?;
    Ok(status.code().unwrap_or(1))
}

pub fn main_entry() -> i32 {
    let mut args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|s| s == "aoc-harness") {
        args.remove(0);
    }

    let (build, runtime) = split_arguments(args);
    let mode = runtime_mode(&runtime);
    if mode.help {
        help::command(mode.action).print_help().expect("print help");
        println!();
        return 0;
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_scores_are_flushed_before_the_next_result() {
        struct LiveOutput {
            pending: Vec<u8>,
            flushed: mpsc::Sender<Vec<u8>>,
        }

        impl Write for LiveOutput {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.pending.extend_from_slice(bytes);
                Ok(bytes.len())
            }

            fn flush(&mut self) -> io::Result<()> {
                self.flushed
                    .send(std::mem::take(&mut self.pending))
                    .map_err(|error| io::Error::new(io::ErrorKind::BrokenPipe, error))
            }
        }

        let (events, receiver) = mpsc::channel();
        let (flushed, scores) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut output = LiveOutput {
                pending: Vec::new(),
                flushed,
            };
            let mut diagnostics = Vec::new();

            let result = forward_benchmark_output(receiver, &mut output, &mut diagnostics, false);

            (result, diagnostics)
        });
        let first_score = b"generator/day12/default time: [1 ns 2 ns 3 ns]\n";
        let second_score = b"solver/day12/part1/default time: [4 ns 5 ns 6 ns]\n";

        events
            .send(BenchmarkEvent::Diagnostic(
                b"Benchmarking generator/day12/default: Analyzing\n".to_vec(),
            ))
            .unwrap();
        events
            .send(BenchmarkEvent::Output(first_score.to_vec()))
            .unwrap();
        let first = scores.recv_timeout(std::time::Duration::from_secs(5));

        events
            .send(BenchmarkEvent::Diagnostic(
                b"Benchmarking solver/day12/part1/default: Warming up for 10.000 ms\n".to_vec(),
            ))
            .unwrap();
        events
            .send(BenchmarkEvent::Output(second_score.to_vec()))
            .unwrap();
        let second = scores.recv_timeout(std::time::Duration::from_secs(5));
        drop(events);
        let (result, diagnostics) = worker.join().unwrap();

        assert!(result.unwrap());
        assert_eq!(
            first.unwrap(),
            [b"Benchmark results\n\n", first_score.as_slice()].concat()
        );
        assert_eq!(second.unwrap(), second_score);
        assert_eq!(diagnostics, b"Running benchmarks...\n");
    }

    #[test]
    fn benchmark_color_respects_terminal_and_explicit_choices() {
        let cases = [
            (ColorChoice::Auto, true, false, false, true),
            (ColorChoice::Auto, false, false, false, false),
            (ColorChoice::Auto, true, true, false, false),
            (ColorChoice::Auto, true, false, true, false),
            (ColorChoice::Always, false, true, true, true),
            (ColorChoice::Never, true, false, false, false),
        ];

        for (choice, terminal, no_color, dumb, expected) in cases {
            let actual = color_enabled(choice, terminal, no_color, dumb);

            assert_eq!(actual, expected, "{choice:?} {terminal} {no_color} {dumb}");
        }
    }

    #[test]
    fn benchmark_output_mode_respects_option_values_and_smoke() {
        let cases: &[(&[&str], bool)] = &[
            (&["bench"], true),
            (&["--day", "1", "bench", "--filter=solver/"], true),
            (&["bench", "--smoke"], false),
            (&["--smoke", "bench"], false),
            (&["run", "--day", "1", "--input", "bench"], false),
            (
                &["run", "--day", "1", "--part", "1", "--solution", "bench"],
                false,
            ),
            (&["bench", "--filter", "bench"], true),
            (&["list"], false),
            (&[], false),
        ];

        for (args, expected) in cases {
            let args: Vec<_> = args.iter().map(OsString::from).collect();

            let actual = measured_bench(&args);

            assert_eq!(actual, *expected, "{args:?}");
        }
    }

    #[test]
    fn criterion_status_filter_preserves_diagnostics() {
        type StatusCase = (&'static [u8], Option<(&'static str, &'static str)>);
        let cases: &[StatusCase] = &[
            (b"Benchmarking solver/day1/part1/default\n", Some(("solver/day1/part1/default", "starting"))),
            (b"Benchmarking generator/day1/default: Warming up for 10.000 ms\n", Some(("generator/day1/default", "warming up"))),
            (b"Benchmarking generator/day12/default: Collecting 10 samples in estimated 20.000 ms (500k iterations)\r\n", Some(("generator/day12/default", "measuring"))),
            (b"Benchmarking end-to-end/day1/part2/owned: Collecting 10 samples in estimated 20.000 ms (500k iterations)\n", Some(("end-to-end/day1/part2/owned", "measuring"))),
            (b"Benchmarking full-run/2025: Analyzing\n", Some(("full-run/2025", "analyzing"))),
            (b"Warning: unable to complete 10 samples\n", None),
            (b"Benchmarking solver/day1/part1/default: unexpected failure\n", None),
            (b"Benchmarking user operation failed\n", None),
            (b"thread 'main' panicked: solver failed\n", None),
            (b"\xff\n", None),
        ];

        for (line, expected) in cases {
            let actual = benchmark_status(line);

            assert_eq!(actual, *expected, "{line:?}");
        }
    }
}
