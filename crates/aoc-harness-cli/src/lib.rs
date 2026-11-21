use clap::{ColorChoice, Parser};
use std::{
    io::{self, BufRead, BufReader, IsTerminal, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc::{self, SyncSender},
    thread,
};

mod project;
use project::{build_bridge, discover};

mod calendar;
mod command;
mod compat;
use compat::{ProbeError, Runtime, RuntimeYears};
mod credentials;
mod input;
mod profiling;
mod reports;
mod scaffold;
mod source_year;

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
    #[arg(long)]
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
    #[arg(skip)]
    debug_info: bool,
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

/// `base`, or `base/<year>` when artifacts are scoped by year.
fn year_scoped(base: std::path::PathBuf, year: Option<u16>) -> std::path::PathBuf {
    match year {
        Some(year) => base.join(year.to_string()),
        None => base,
    }
}

fn execute(request: command::Request) -> Result<i32, String> {
    use command::Action;
    let command::Request {
        build: options,
        action,
        runtime,
        year,
        days,
        json,
        all,
        day_template,
        dry_run,
        download,
        session_file,
        open,
        smoke,
        mut profiling,
    } = request;
    if action == Action::Credentials {
        let session = credentials::resolve(session_file.as_deref())?;
        println!(
            "Session source: {} (loaded and validated locally)",
            session.source
        );
        return Ok(0);
    }

    let backend = profiling
        .as_mut()
        .map(profiling::ProfileOptions::preflight)
        .transpose()?;

    let project = discover(&options)?;

    if action == Action::Generate {
        let template_path = match day_template {
            Some(path) => Some(path),
            None => project.day_template()?,
        };
        let selection = source_year::select(&project, &options, year)?;
        let plan = scaffold::ScaffoldPlan::prepare(
            &project.manifest_dir,
            &project.library_source,
            &selection.module,
            &days,
            template_path.as_deref(),
            || match selection.year {
                Some(year) => Ok(year),
                None => source_year::resolve(&project, &options, None),
            },
        )?;

        if dry_run {
            plan.print();
        } else {
            plan.apply()?;
        }
        return Ok(0);
    }

    if action == Action::Input {
        let year = source_year::resolve(&project, &options, year)?;
        let (year, days) = calendar::select(year, &days, all, chrono::Utc::now())?;
        let inputs = project.input_dir()?;

        input::acquire(
            &inputs,
            year,
            &days,
            options.offline,
            session_file.as_deref(),
        )?;
        return Ok(0);
    }

    let bridge = build_bridge(&options, &project)?;
    let scoped = matches!(action, Action::Bench | Action::Profile);
    let needs_runtime = year.is_some() || scoped || download || profiling.is_some();
    let compiled = match Runtime::probe(&bridge.executable) {
        Ok(compiled) => Some(compiled),
        Err(error) if needs_runtime => return Err(error.to_string()),
        Err(ProbeError::Protocol(message)) => {
            eprintln!("warning: {message}");
            None
        }
        Err(ProbeError::Failed(_)) => None,
    };

    if let Some(compiled) = &compiled {
        if let Some(notice) = compiled.notice() {
            eprintln!("note: {notice}");
        }
        if let Some(year) = year {
            compiled.require_year(year, &project, &bridge)?;
        }
        if download {
            compiled.require("input-plan-v1", "--download", &project, &bridge)?;
        }
        if profiling.is_some() {
            compiled.require("profiling-v1", "profiling", &project, &bridge)?;
            compiled.require("input-plan-v1", "profiling", &project, &bridge)?;
        }
    }
    let artifact_year = RuntimeYears::artifact_year(
        compiled
            .as_ref()
            .and_then(|compiled| compiled.years.as_ref()),
        year,
    );
    let executable = bridge.executable;
    let mut profile_workloads = Vec::new();
    if download || profiling.is_some() {
        let description = Command::new(&executable)
            .args(["__describe", "--protocol", &compat::PROTOCOL.to_string()])
            .args(&runtime)
            .output()
            .map_err(|e| format!("describe compiled input plan: {e}"))?;
        if !description.status.success() {
            return Err(compat::describe_failure(
                "cannot describe compiled plan",
                &description,
            ));
        }
        let (year, required) =
            input::validate_description(&description.stdout, &project.manifest_dir, action.name())?;
        if profiling.is_some() {
            profile_workloads = profiling::workloads(&description.stdout)?;
        }
        if download {
            let mut progress: Box<dyn Write> = if json {
                Box::new(io::stderr())
            } else {
                Box::new(io::stdout())
            };
            calendar::validate(year, &required, chrono::Utc::now())?;
            let inputs = project.input_dir()?;
            input::acquire_to(
                &inputs,
                year,
                &required,
                options.offline,
                session_file.as_deref(),
                &mut progress,
            )?;
        }
    }

    if let (Some(profiling), Some(backend)) = (&profiling, &backend) {
        let validation = Command::new(&executable)
            .args(&runtime)
            .arg("--smoke")
            .status()
            .map_err(|e| format!("validate profiling workload: {e}"))?;
        if !validation.success() {
            return Ok(validation.code().unwrap_or(1));
        }

        let root = year_scoped(
            project
                .target_dir
                .join("aoc-harness/profiles")
                .join(&project.package_name),
            artifact_year,
        );
        return backend.record(profiling, &root, &executable, &runtime, &profile_workloads);
    }

    let report_dir = year_scoped(
        project
            .target_dir
            .join("criterion")
            .join(&project.package_name),
        artifact_year,
    );
    let color = color_enabled(
        options.color,
        io::stdout().is_terminal(),
        std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()),
        std::env::var_os("TERM").is_some_and(|v| v == "dumb"),
    );
    let mut command = Command::new(executable);
    command
        .args(runtime)
        .arg("--report-dir")
        .arg(&report_dir)
        .arg("--color")
        .arg(if color { "always" } else { "never" });
    let code = if action == Action::Bench && !smoke {
        run_benchmarks(&mut command)?
    } else {
        command
            .status()
            .map_err(|e| format!("run generated bridge: {e}"))?
            .code()
            .unwrap_or(1)
    };
    if code == 0 && open {
        reports::open(&report_dir, |url| {
            webbrowser::open(url).map_err(|e| e.to_string())
        })?;
    }
    Ok(code)
}

pub fn main_entry() -> i32 {
    let mut args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|s| s == "aoc-harness") {
        args.remove(0);
    }
    let request = match command::parse(args) {
        Ok(request) => request,
        Err(error) => {
            let _ = error.print();
            return error.exit_code();
        }
    };
    match execute(request) {
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
    use std::ffi::OsString;

    #[test]
    fn artifacts_are_scoped_by_the_selected_year_only_in_multi_year_runtimes() {
        let multi = RuntimeYears {
            years: vec![2019, 2025],
            default_year: Some(2025),
        };
        let ambiguous = RuntimeYears {
            years: vec![2019, 2025],
            default_year: None,
        };
        let single = RuntimeYears {
            years: vec![2025],
            default_year: Some(2025),
        };
        let base = std::path::PathBuf::from("target/criterion/package");

        let requested = RuntimeYears::artifact_year(Some(&multi), Some(2019));
        let default = RuntimeYears::artifact_year(Some(&multi), None);

        assert_eq!(requested, Some(2019));
        assert_eq!(default, Some(2025));
        assert_eq!(RuntimeYears::artifact_year(Some(&ambiguous), None), None);
        assert_eq!(RuntimeYears::artifact_year(Some(&single), Some(2025)), None);
        assert_eq!(RuntimeYears::artifact_year(None, None), None);
        assert_eq!(year_scoped(base.clone(), requested), base.join("2019"));
        assert_eq!(year_scoped(base.clone(), None), base);
    }

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

            let request = command::parse(args.clone()).unwrap();
            let actual = request.action == command::Action::Bench && !request.smoke;

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
