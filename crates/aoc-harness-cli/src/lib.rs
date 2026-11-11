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

fn execute(request: command::Request) -> Result<i32, String> {
    use command::Action;
    let command::Request {
        build: options,
        action,
        runtime,
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
        let plan = scaffold::ScaffoldPlan::prepare(
            &project.manifest_dir,
            &project.library_source,
            &days,
            template_path.as_deref(),
            || source_year::resolve(&project, &options),
        )?;

        if dry_run {
            plan.print();
        } else {
            plan.apply()?;
        }
        return Ok(0);
    }

    if action == Action::Input {
        let year = source_year::resolve(&project, &options)?;
        let (year, days) = calendar::select(year, &days, all, chrono::Utc::now())?;

        input::acquire(
            &project.manifest_dir,
            year,
            &days,
            options.offline,
            session_file.as_deref(),
        )?;
        return Ok(0);
    }

    let executable = build_bridge(&options, &project)?;
    let mut profile_workloads = Vec::new();
    if download || profiling.is_some() {
        let description = Command::new(&executable)
            .args(["__describe", "--protocol", "1"])
            .args(&runtime)
            .output()
            .map_err(|e| format!("describe compiled input plan: {e}"))?;
        if !description.status.success() {
            let diagnostic = String::from_utf8_lossy(&description.stderr);
            let capability = if profiling.is_some() {
                "profiling-v1"
            } else {
                "input-plan-v1"
            };
            return Err(format!("cannot describe compiled plan; this operation requires an aoc-harness runtime with {capability} support (upgrade older consumers). {diagnostic}"));
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
            input::acquire_to(
                &project.manifest_dir,
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

        let root = project
            .target_dir
            .join("aoc-harness/profiles")
            .join(&project.package_name);
        return backend.record(profiling, &root, &executable, &runtime, &profile_workloads);
    }

    let report_dir = project
        .target_dir
        .join("criterion")
        .join(&project.package_name);
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
