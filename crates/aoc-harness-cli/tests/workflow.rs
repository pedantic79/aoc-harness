use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn invoke(
    program: impl AsRef<Path>,
    cwd: &Path,
    args: &[&str],
    target: &Path,
    installed: &Path,
) -> Output {
    let path = std::env::join_paths(
        std::iter::once(installed.to_owned())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    Command::new(program.as_ref())
        .current_dir(cwd)
        .args(args)
        .env("PATH", path)
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .unwrap()
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn failure(output: Output, expected: &str) {
    assert!(
        !output.status.success(),
        "unexpected success: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains(expected),
        "expected {expected:?} in:\n{combined}"
    );
}

fn snapshot(path: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut entries: Vec<_> = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    let mut files = Vec::new();
    for entry in entries {
        if entry.file_name().unwrap() == "target" {
            continue;
        }
        if entry.is_dir() {
            files.extend(snapshot(&entry));
        } else {
            files.push((entry.clone(), fs::read(entry).unwrap()));
        }
    }
    files
}

fn copy_fixture(source: &Path, destination: &Path, runtime: &Path) {
    for (path, bytes) in snapshot(source) {
        if path.file_name().unwrap() == "Cargo.lock" {
            continue;
        }
        let out = destination.join(path.strip_prefix(source).unwrap());
        fs::create_dir_all(out.parent().unwrap()).unwrap();
        fs::write(out, bytes).unwrap();
    }
    let manifest = destination.join("Cargo.toml");
    let mut value: toml::Value = fs::read_to_string(&manifest).unwrap().parse().unwrap();
    value["dependencies"]["aoc-harness"]["path"] = runtime.to_str().unwrap().into();
    fs::write(manifest, toml::to_string(&value).unwrap()).unwrap();
}

#[test]
fn command_help_works_without_a_consumer_or_cargo() {
    let directory = tempfile::tempdir().unwrap();
    let programs: &[(&str, &[&str])] =
        &[(env!("CARGO_BIN_EXE_cargo-aoc-harness"), &["aoc-harness"])];

    for (program, prefix) in programs {
        let help = |args: &[&str]| {
            let output = Command::new(program)
                .args(*prefix)
                .args(args)
                .current_dir(directory.path())
                .env("CARGO", directory.path().join("missing-cargo"))
                .output()
                .unwrap();

            assert!(output.stderr.is_empty(), "{output:?}");
            success(output)
        };

        let output = help(&["--help"]);

        assert!(output.contains("Usage: cargo aoc-harness [OPTIONS] [COMMAND]"));
        assert!(output.contains("Commands:"));
        for action in ["run", "list", "bench", "profile"] {
            assert!(output.contains(action));
        }

        for action in ["run", "list", "bench"] {
            for args in [
                vec![action, "--help"],
                vec![action, "-h"],
                vec!["help", action],
            ] {
                let output = help(&args);

                assert!(output.contains(&format!("Usage: cargo aoc-harness {action} [OPTIONS]")));
                assert!(output.contains("--manifest-path"));
                assert!(output.contains("--day"));
                assert!(!output.contains("--days"));
                assert!(output.contains("--all-days"));
                assert!(output.contains("--solution"));
                assert!(output.contains("-p, --part"));
                assert!(!output.contains("-p, --package"));
                assert!(output.contains("Examples:"));
                assert!(!output.contains("Commands:"));
                assert_eq!(output.contains("--all-solutions"), action == "run");
                assert_eq!(output.contains("--format"), action == "run");
                assert_eq!(output.contains("--save-baseline"), action == "bench");
                assert_eq!(output.contains("--baseline"), action == "bench");
                assert_eq!(output.contains("--input"), action != "list");
                assert_eq!(output.contains("--filter"), action == "bench");
                assert_eq!(output.contains("--family"), action == "bench");
                assert_eq!(output.contains("--sample-size"), action == "bench");
                assert_eq!(output.contains("--smoke"), action == "bench");
                assert_eq!(output.contains("--color"), action == "bench");
                assert_eq!(output.contains("latest registered day"), action != "list");
            }
        }

        let output = help(&["profile", "--help"]);

        assert!(output.contains("Usage: cargo aoc-harness profile [OPTIONS]"));
        assert!(!output.contains("--days"));
        assert!(output.contains("-p, --part"));
        assert!(!output.contains("-p, --package"));
        for option in [
            "--profiler",
            "--duration",
            "--family",
            "--output",
            "--template",
            "--download",
        ] {
            assert!(output.contains(option));
        }
        for backend in ["samply", "flamegraph", "perf", "xctrace"] {
            assert!(output.contains(backend));
        }
        assert!(output.contains("[default: 10]"));
        assert!(output.contains("[default: end-to-end]"));
        assert!(output.contains("Each workload gets its own recording"));

        let output = help(&["bench", "--help"]);

        assert!(output.contains("[default: solver]"));
        assert!(output.contains("Execute each selected workload once"));
        assert!(output.contains("--save-baseline <NAME>"));
        assert!(output.contains("--baseline <NAME>"));

        let input_help = help(&["input", "--help"]);
        assert!(!input_help.contains("--year"));
        assert!(input_help.contains("project's declared event"));
        let run_help = help(&["run", "--help"]);
        assert!(run_help.contains("fail if displayed answers disagree"));
        assert!(run_help.contains("--format"));
        assert!(run_help.contains("json"));
        assert!(output.contains("comma-separated or repeated"));
        for family in ["generator", "solver", "end-to-end", "full-run"] {
            assert!(output.contains(family));
        }

        for (args, action) in [
            (vec!["--day", "1", "bench", "--help"], "bench"),
            (vec!["--manifest-path", "bench", "list", "--help"], "list"),
            (
                vec!["run", "--day", "1", "--input", "list", "--help"],
                "run",
            ),
            (
                vec![
                    "run",
                    "--day",
                    "1",
                    "--part",
                    "1",
                    "--solution",
                    "bench",
                    "--help",
                ],
                "run",
            ),
        ] {
            let output = help(&args);

            assert!(output.contains(&format!("Usage: cargo aoc-harness {action} [OPTIONS]")));
        }
    }
}

#[test]
fn installed_library_workflow() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let target = root.join("target");
    fs::create_dir_all(&target).unwrap();
    let temp = tempfile::Builder::new()
        .prefix("workflow with spaces ")
        .tempdir_in(&target)
        .unwrap();
    let installed = temp.path().join("installed/bin");
    let consumer = temp.path().join("consumer with spaces");
    copy_fixture(
        &root.join("fixtures/library"),
        &consumer,
        &root.join("crates/aoc-harness"),
    );
    let before = snapshot(&consumer);

    let install = invoke(
        "cargo",
        root,
        &[
            "install",
            "--path",
            "crates/aoc-harness-cli",
            "--root",
            temp.path().join("installed").to_str().unwrap(),
            "--debug",
            "--offline",
        ],
        &target,
        &installed,
    );
    success(install);
    let cli = installed.join("cargo-aoc-harness");

    assert!(!installed.join("aoc-harness").exists());

    let run = |args: &[&str]| {
        let args: Vec<_> = std::iter::once("aoc-harness")
            .chain(args.iter().copied())
            .collect();

        invoke("cargo", &consumer, &args, &target, &installed)
    };

    let output = success(run(&["--profile", "dev"]));

    assert!(output.starts_with("AOC 2025\n\nDay 3 - Part 1: 2\n\tgenerator: "));
    assert!(output.contains("Day 3 - Part 2: 2"));
    assert!(!output.contains("Day 1 -"));
    assert!(!output.contains("Day 2 -"));
    assert!(output.contains("Full run (2025 day 3, 2 selected)"));

    let output = success(invoke(
        "cargo",
        &consumer,
        &["aoc-harness", "run", "--profile", "dev"],
        &target,
        &installed,
    ));

    assert!(output.contains("Full run (2025 day 3, 2 selected)"));
    assert!(!output.contains("Day 1 -"));

    let output = success(run(&["run", "--all-days", "--profile", "dev"]));

    assert!(output.starts_with("AOC 2025\n\nDay 1 - Part 1: 6\n\tgenerator: "));
    assert!(output.contains("\n\trunner: "));
    assert!(output.contains("\n\nDay 1 - Part 2 - owned: 12\n\tgenerator: "));
    assert!(output.contains("Day 3 - Part 2: 2"));
    assert!(output.contains("Full run (2025, 6 selected):\n\tgenerator: "));
    assert!(output.contains("\n\ttotal: "));
    assert!(output.contains("generator: 0ns"));

    for selection in [
        vec!["run", "--day", "3,1,3"],
        vec!["--day", "3", "run", "--day", "1"],
        vec!["run", "--day=1,3"],
    ] {
        let mut args = selection;
        args.extend(["--profile", "dev"]);

        let output = success(run(&args));

        assert!(output.starts_with("AOC 2025\n\nDay 1 - Part 1: 6"));
        assert!(output.contains("Day 1 - Part 2 - owned: 12"));
        assert!(output.contains("Day 3 - Part 2: 2"));
        assert!(!output.contains("Day 2 -"));
        assert!(output.contains("Full run (2025 days 1,3, 4 selected)"));
    }

    let output = success(run(&["list", "--profile", "dev"]));

    assert!(output.contains("day 1 part 1 [default] (selected default)"));
    assert!(output.contains("[owned] (selected default)"));
    assert!(output.contains("[mutable]"));
    assert!(output.contains("day 3 part 2 [default] (selected default)"));

    let output = success(run(&["list", "--day", "1,3", "--profile", "dev"]));

    assert!(output.contains("day 1 part 2 [owned] (selected default)"));
    assert!(output.contains("day 3 part 2 [default] (selected default)"));
    assert!(!output.contains("day 2 part"));

    let output = success(run(&[
        "run",
        "--day",
        "1",
        "-p",
        "2",
        "--solution",
        "mutable",
        "--profile",
        "quick",
    ]));

    assert!(output.contains("Day 1 - Part 2 - mutable: 306"));
    let output = run(&["run", "--day", "1", "--all-solutions", "--profile", "dev"]);

    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Comparison run (2025 day 1, 5 selected)")
    );
    failure(output, "Variant disagreement: day 1 part 2");

    let output = success(run(&["list", "--features", "bonus", "--profile", "dev"]));

    assert!(output.contains("day 5 part 1 [only]"));
    let output = success(run(&["run", "--features", "bonus", "--profile", "dev"]));

    assert!(output.contains("Day 5 - Part 1 - only:"));
    assert!(output.contains("Full run (2025 day 5, 1 selected)"));
    assert!(!output.contains("Day 3 -"));

    let output = success(run(&[
        "run",
        "--day",
        "3",
        "--part",
        "1",
        "--features",
        "bonus",
        "--profile",
        "dev",
    ]));

    assert!(output.contains("Day 3 - Part 1: 4"));
    let output = success(run(&[
        "list",
        "--all-features",
        "--no-default-features",
        "--profile",
        "dev",
    ]));

    assert!(output.contains("day 5 part 1 [only]"));

    let output = success(run(&["bench", "--smoke", "--profile", "dev"]));

    assert!(!output.contains("Smoke: generator/"));
    assert!(output.contains("Smoke: solver/day3/part1/default\n"));
    assert!(output.contains("Smoke: solver/day3/part2/default\n"));
    assert!(!output.contains("Smoke: end-to-end/"));
    assert!(!output.contains("Smoke: full-run/"));
    assert!(!output.contains("day1/"));
    assert!(!output.contains("day2/"));

    let cargo = success(invoke(
        "cargo",
        &consumer,
        &["aoc-harness", "bench", "--smoke", "--profile", "dev"],
        &target,
        &installed,
    ));

    assert_eq!(output, cargo);

    let output = success(run(&[
        "bench",
        "--family",
        "end-to-end",
        "--family",
        "full-run",
        "--features",
        "bonus",
        "--smoke",
        "--profile",
        "dev",
    ]));

    assert!(!output.contains("Smoke: solver/"));
    assert!(output.contains("Smoke: end-to-end/day5/part1/only\n"));
    assert!(output.contains("Smoke: full-run/2025-day-5\n"));
    assert!(!output.contains("day3/"));

    let output = success(run(&[
        "bench",
        "--all-days",
        "--family",
        "generator,solver,end-to-end,full-run",
        "--smoke",
        "--profile",
        "dev",
    ]));

    for family in ["generator/", "solver/", "end-to-end/", "full-run/"] {
        assert!(output.contains(family));
    }
    assert!(output.contains("Smoke: generator/day1/default\n"));
    assert!(output.contains("Smoke: generator/day1/part2/default\n"));
    assert!(output.contains("Smoke: generator/day1/part2/mutable\n"));
    assert!(output.contains("Smoke: solver/day2/part1/default\n"));
    assert!(output.contains("Smoke: generator/day3/default\n"));
    assert!(output.contains("Smoke: full-run/2025\n"));
    assert!(!output.contains("part0"));
    let output = success(run(&[
        "bench",
        "--day",
        "1,3",
        "--family",
        "generator,full-run",
        "--smoke",
        "--profile",
        "dev",
    ]));

    assert!(output.contains("Smoke: generator/day1/default\n"));
    assert!(output.contains("Smoke: generator/day3/default\n"));
    assert!(output.contains("Smoke: full-run/2025-days-1,3\n"));
    assert!(!output.contains("day2/"));

    success(invoke(
        "cargo",
        &consumer,
        &[
            "aoc-harness",
            "bench",
            "--day",
            "1",
            "--part",
            "2",
            "--solution",
            "owned",
            "--smoke",
            "--profile",
            "dev",
        ],
        &target,
        &installed,
    ));

    // Actual Criterion measurement covers all ownership modes and every family.
    let benchmark = run(&[
        "--day",
        "1",
        "bench",
        "--family",
        "generator,solver,end-to-end,full-run",
        "--sample-size",
        "10",
        "--warm-up-time",
        "0.01",
        "--measurement-time",
        "0.02",
        "--filter",
        "^(generator/day1/default|solver/day1/part1/(default|cached)|solver/day1/part2/(owned|mutable)|end-to-end/day1/part2/owned|full-run/)",
        "--profile",
        "release",
    ]);
    let diagnostics = String::from_utf8_lossy(&benchmark.stderr).into_owned();
    let output = success(benchmark);

    assert!(output.starts_with("Benchmark results\n\n"));
    assert!(output.contains("time:"));
    assert!(output.contains("Benchmark reports:"));
    assert!(output.contains("generator/day1/default"));
    assert!(output.contains("end-to-end/day1/part2/owned"));
    assert!(output.contains("full-run/2025-day-1"));
    assert!(!output.contains("part0"));
    assert!(!output.contains('\x1b'));
    assert!(diagnostics.contains("Running benchmarks..."));
    assert!(!diagnostics
        .lines()
        .any(|line| line.starts_with("Benchmarking ")));
    assert!(!diagnostics.contains('\r'));
    assert!(!diagnostics.contains('\x1b'));
    let reports = target.join("criterion/winter-library");
    assert!(reports.join("report/index.html").exists());

    let output = success(invoke(
        "cargo",
        &consumer,
        &[
            "aoc-harness",
            "bench",
            "--day",
            "1",
            "--part",
            "2",
            "--solution",
            "owned",
            "--color=always",
            "--sample-size",
            "10",
            "--warm-up-time",
            "0.01",
            "--measurement-time",
            "0.02",
        ],
        &target,
        &installed,
    ));

    assert!(output.starts_with("Benchmark results\n\n"));
    assert!(output.contains("\x1b["));
    assert!(output.contains("time:"));
    assert!(output.contains("Benchmark reports:"));
    for family in ["generator/", "end-to-end/", "full-run/"] {
        assert!(!output.contains(family));
    }

    failure(
        run(&[
            "run",
            "--day",
            "1",
            "--part",
            "2",
            "--solution",
            "absent",
            "--profile",
            "dev",
        ]),
        "no solutions match",
    );
    failure(
        run(&[
            "run",
            "--day",
            "1",
            "--input",
            "missing input",
            "--profile",
            "dev",
        ]),
        "day 1 input",
    );
    failure(run(&["run", "--part", "1", "--profile", "dev"]), "--day");
    failure(
        run(&["run", "--day", "1", "--all-days", "--profile", "dev"]),
        "cannot be used with",
    );
    failure(
        run(&["run", "--day", "1,25", "--profile", "dev"]),
        "no solutions match",
    );
    failure(
        run(&[
            "run",
            "--day",
            "1,3",
            "--input",
            "missing input",
            "--profile",
            "dev",
        ]),
        "--input requires exactly one selected day",
    );
    failure(
        run(&["bench", "--filter", "[", "--smoke", "--profile", "dev"]),
        "benchmark filter",
    );
    failure(
        run(&[
            "bench",
            "--filter",
            "does-not-exist",
            "--smoke",
            "--profile",
            "dev",
        ]),
        "no benchmarks match",
    );

    let unpadded_input = consumer.join("input/2025/day1.txt");
    let padded_input = consumer.join("input/2025/day01.txt");
    let original_input = fs::read(&unpadded_input).unwrap();
    fs::write(&padded_input, "10\n20\n30\n").unwrap();

    let output = success(run(&[
        "run",
        "--day",
        "1",
        "--part",
        "1",
        "--profile",
        "dev",
    ]));

    assert!(output.contains("Day 1 - Part 1: 6\n"), "{output}");

    fs::remove_file(&unpadded_input).unwrap();

    let output = success(run(&[
        "run",
        "--day",
        "1",
        "--part",
        "1",
        "--profile",
        "dev",
    ]));

    assert!(output.contains("Day 1 - Part 1: 60\n"), "{output}");

    failure(
        run(&[
            "run",
            "--day",
            "1",
            "--input",
            "input/2025/day1.txt",
            "--profile",
            "dev",
        ]),
        "day 1 input input/2025/day1.txt",
    );

    fs::write(&unpadded_input, original_input).unwrap();
    fs::remove_file(&padded_input).unwrap();

    assert_eq!(before, snapshot(&consumer), "CLI changed the consumer");

    success(invoke(
        "cargo",
        &consumer,
        &["generate-lockfile", "--offline"],
        &target,
        &installed,
    ));
    let locked = snapshot(&consumer);
    success(run(&["list", "--profile", "dev", "--locked"]));
    assert_eq!(locked, snapshot(&consumer));

    let input = temp.path().join("override input.txt");
    fs::write(&input, "hello \r\n\n").unwrap();
    let output = success(run(&[
        "run",
        "--day",
        "2",
        "--part",
        "1",
        "--input",
        input.to_str().unwrap(),
        "--profile",
        "dev",
    ]));

    assert!(output.contains("Day 2 - Part 1: 7"), "{output}");
    fs::write(&input, "bad-number").unwrap();
    failure(
        run(&[
            "run",
            "--day",
            "1",
            "--input",
            input.to_str().unwrap(),
            "--profile",
            "dev",
        ]),
        "[default]: day 1 part * [default] generator",
    );
    fs::write(&input, "-1").unwrap();
    let output = run(&[
        "run",
        "--day",
        "1",
        "--input",
        input.to_str().unwrap(),
        "--profile",
        "dev",
    ]);

    assert!(String::from_utf8_lossy(&output.stdout).contains("Partial run"));
    failure(output, "[owned] solver: negative sentinel");
    failure(
        run(&[
            "bench",
            "--day",
            "1",
            "--input",
            input.to_str().unwrap(),
            "--smoke",
            "--profile",
            "dev",
        ]),
        "negative sentinel",
    );

    let source = consumer.join("src/lib.rs");
    let original = fs::read_to_string(&source).unwrap();
    fs::write(
        &source,
        original.replacen(
            "numbers.values.iter().sum()",
            "numbers.values.iter().sum::<i64>() + 7",
            1,
        ),
    )
    .unwrap();
    let output = success(run(&[
        "run",
        "--day",
        "1",
        "--part",
        "1",
        "--profile",
        "dev",
    ]));

    assert!(
        output.contains("Day 1 - Part 1: 13"),
        "stale bridge: {output}"
    );
    fs::write(&source, format!("{original}\ninvalid Rust syntax\n")).unwrap();
    failure(run(&["list", "--profile", "dev"]), "could not compile");

    // Compile-time registry diagnostics, including cross-function ambiguity.
    for (functions, expected) in [
        ("#[aoc(day1, part1, a)] fn a(s: &str) -> usize { s.len() }\n#[aoc(day1, part1, b)] fn b(s: &str) -> usize { s.len() }", "several named solutions"),
        ("#[aoc(day1, part1, a, default)] fn a(s: &str) -> usize { s.len() }\n#[aoc(day1, part1, b, default)] fn b(s: &str) -> usize { s.len() }", "multiple explicit defaults"),
        ("#[aoc(day1, part1)] fn a(s: &str) -> usize { s.len() }\n#[aoc(day1, part1)] fn b(s: &str) -> usize { s.len() }", "duplicate solution identity"),
        ("#[aoc_harness::aoc_generator(day1)] fn a(s: &str) -> &str { s }\n#[aoc_harness::aoc_generator(day1)] fn b(s: &str) -> &str { s }\n#[aoc(day1, part1)] fn solve(s: &str) -> usize { s.len() }", "duplicate generator identity"),
    ] {
        fs::write(&source, format!("use aoc_harness::aoc;\n{functions}\naoc_harness::aoc_lib! {{ year = 2025 }}\n")).unwrap();
        failure(run(&["list", "--profile", "dev"]), expected);
    }

    // A failure inside Criterion must preserve streamed output and live diagnostics.
    fs::write(
        &source,
        r#"
use aoc_harness::aoc;
use std::sync::atomic::{AtomicUsize, Ordering};

static CALLS: AtomicUsize = AtomicUsize::new(0);

#[aoc(day1, part1)]
fn solver(raw: &str) -> usize {
    println!("consumer output before measurement failure");
    if CALLS.fetch_add(1, Ordering::SeqCst) > 0 {
        eprintln!("consumer diagnostic before measurement failure");
        panic!("measurement failure");
    }

    raw.len()
}

aoc_harness::aoc_lib! { year = 2025 }
"#,
    )
    .unwrap();

    let benchmark = run(&[
        "bench",
        "--day",
        "1",
        "--filter",
        "^solver/",
        "--warm-up-time",
        "0.01",
        "--measurement-time",
        "0.02",
        "--sample-size",
        "10",
    ]);
    let output = String::from_utf8_lossy(&benchmark.stdout);
    let diagnostics = String::from_utf8_lossy(&benchmark.stderr);

    assert!(!benchmark.status.success());
    assert!(output.starts_with("Benchmark results\n\n"));
    assert!(output.contains("Benchmark output (incomplete)"));
    assert!(output.contains("consumer output before measurement failure"));
    assert!(!output.contains("Benchmark reports:"));
    assert!(diagnostics.contains("consumer diagnostic before measurement failure"));
    assert!(diagnostics.contains("panicked"));
    assert!(diagnostics.contains("measurement failure"));
    assert!(!diagnostics
        .lines()
        .any(|line| line.starts_with("Benchmarking ")));

    let workspace = temp.path().join("workspace with spaces");
    fs::create_dir_all(&workspace).unwrap();
    let runtime =
        toml::Value::String(root.join("crates/aoc-harness").to_str().unwrap().into()).to_string();
    fs::write(workspace.join("Cargo.toml"), format!("[workspace]\nmembers=['one','two']\nresolver='2'\n[workspace.dependencies]\nharness = {{ package='aoc-harness', path={runtime} }}\n")).unwrap();
    for member in ["one", "two"] {
        let dir = workspace.join(member);
        fs::create_dir_all(dir.join("code")).unwrap();
        fs::write(dir.join("Cargo.toml"), format!("[package]\nname='{member}'\nversion='0.0.0'\nedition='2021'\n[lib]\nname='custom_{member}'\npath='code/solutions.rs'\n[dependencies]\nharness.workspace = true\n[features]\ndefault=['bonus']\nbonus=[]\n")).unwrap();
        fs::write(dir.join("code/solutions.rs"), "mod nested;\n#[cfg(feature=\"bonus\")]\n#[harness::aoc(day2, part1)] fn bonus(raw: &str) -> usize { raw.len() }\nharness::aoc_lib! { year = 2025 }\n").unwrap();
        fs::write(
            dir.join("code/nested.rs"),
            "#[harness::aoc(day1, part1)] fn solve(raw: &str) -> usize { raw.len() }\n",
        )
        .unwrap();
    }
    let before = snapshot(&workspace);
    let from_workspace = |args: &[&str]| invoke(&cli, &workspace, args, &target, &installed);
    failure(
        from_workspace(&["list", "--profile", "dev"]),
        "ambiguous; use --package",
    );
    let output = success(from_workspace(&[
        "list",
        "--package",
        "one",
        "-d",
        "2",
        "-p",
        "1",
        "--profile",
        "dev",
    ]));

    assert!(output.contains("day 2 part 1"));
    let output = success(from_workspace(&[
        "list",
        "--package",
        "two",
        "--no-default-features",
        "--profile",
        "dev",
    ]));

    assert!(!output.contains("day 2 part 1"));
    let output = success(from_workspace(&[
        "list",
        "--manifest-path",
        "one/Cargo.toml",
        "--profile",
        "dev",
    ]));

    assert!(output.contains("day 1 part 1"));
    assert_eq!(before, snapshot(&workspace));

    let one = workspace.join("one");
    fs::create_dir_all(one.join(".cargo")).unwrap();
    let config_target = temp.path().join("configured target");
    let target_value = toml::Value::String(config_target.to_str().unwrap().into()).to_string();
    fs::write(one.join(".cargo/config.toml"), format!("[build]\ntarget-dir={target_value}\nrustflags=['--cfg=configured_consumer','--check-cfg=cfg(configured_consumer)']\n")).unwrap();
    let nested = one.join("code/nested.rs");
    fs::write(&nested, "#[cfg(configured_consumer)]\n#[harness::aoc(day3, part1)] fn configured(raw: &str) -> usize { raw.len() }\n#[harness::aoc(day1, part1)] fn solve(raw: &str) -> usize { raw.len() }\n").unwrap();
    let before = snapshot(&workspace);
    let output = Command::new(&cli)
        .current_dir(root)
        .args([
            "list",
            "--manifest-path",
            one.join("Cargo.toml").to_str().unwrap(),
            "--profile",
            "dev",
            "--offline",
        ])
        .env_remove("CARGO_TARGET_DIR")
        .output()
        .unwrap();

    assert!(success(output).contains("day 3 part 1"));
    assert!(config_target.join("aoc-harness").exists());
    assert_eq!(before, snapshot(&workspace));

    // Explicit manifest discovery also works outside the consumer's directory.
    let output = success(invoke(
        &cli,
        root,
        &[
            "list",
            "--manifest-path",
            workspace.join("one/Cargo.toml").to_str().unwrap(),
            "--profile",
            "dev",
        ],
        &target,
        &installed,
    ));

    assert!(output.contains("day 1 part 1"));

    // Preserve a compile-time dependency on both shipped executable targets.
    assert!(!OsString::from(env!("CARGO_BIN_EXE_cargo-aoc-harness")).is_empty());
}

#[test]
fn parity_commands_and_consumer_entry_points() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let target = root.join("target");
    let temp = tempfile::Builder::new()
        .prefix("parity workflow ")
        .tempdir_in(&target)
        .unwrap();
    let consumer = temp.path().join("consumer with spaces");
    copy_fixture(
        &root.join("fixtures/parity"),
        &consumer,
        &root.join("crates/aoc-harness"),
    );
    let cli = env!("CARGO_BIN_EXE_cargo-aoc-harness");
    let run = |args: &[&str]| invoke(cli, &consumer, args, &target, temp.path());
    let cargo = |args: &[&str]| invoke("cargo", &consumer, args, &target, temp.path());
    let before = snapshot(&consumer);

    let output = success(run(&[
        "run",
        "-d",
        "day1",
        "--part",
        "part1",
        "--download",
        "--offline",
        "--profile",
        "dev",
    ]));

    assert!(output.contains("1 cached, 0 downloaded, 0 failed"));
    assert!(output.contains("Day 1 - Part 1: 2"));
    let output = success(run(&[
        "run",
        "--features",
        "bonus",
        "--download",
        "--offline",
        "--profile",
        "dev",
    ]));

    assert!(output.contains("Day 5 - Part 1"));
    assert!(!output.contains("Day 4 - Part 1"));
    let output = success(run(&[
        "bench",
        "--day",
        "1,2,3,4",
        "--download",
        "--offline",
        "--smoke",
        "--profile",
        "dev",
    ]));

    assert!(output.contains("4 cached, 0 downloaded, 0 failed"));
    assert!(output.contains("Smoke: solver/day1/part2/default"));
    assert!(output.contains("Smoke: solver/day1/part1/interior"));
    assert_eq!(snapshot(&consumer), before);

    // Exercise custom projections in actual Criterion batched measurement loops.
    let output = success(run(&[
        "bench",
        "--day",
        "1",
        "--family",
        "generator,solver,end-to-end,full-run",
        "--filter",
        "^(generator/|solver/|end-to-end/|full-run/)",
        "--sample-size",
        "10",
        "--warm-up-time",
        "0.01",
        "--measurement-time",
        "0.02",
    ]));

    assert!(output.contains("time:"));
    assert!(output.contains("solver/day1/part1/interior"));
    assert!(output.contains("solver/day1/part2/owned"));
    for (binary, day) in [
        ("parity-fixture", "1"),
        ("standalone", "1"),
        ("nested", "5"),
        ("custom", "3"),
    ] {
        for args in [
            vec!["run", "--day", day, "-p", "1"],
            vec!["list"],
            vec!["bench", "--day", day, "--smoke"],
        ] {
            let mut command = vec!["run", "--quiet", "--bin", binary, "--"];
            command.extend(args);

            success(cargo(&command));
        }
    }
    // Description must not load a missing input or call any generator/solver.
    fs::remove_file(consumer.join("input/2025/day1.txt")).unwrap();
    let output = success(cargo(&[
        "run",
        "--quiet",
        "--bin",
        "standalone",
        "--",
        "__describe",
        "--protocol",
        "1",
        "bench",
        "--day",
        "day1",
    ]));
    let value: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(value["required_input_days"], serde_json::json!([1]));
    assert_eq!(value["action"], "bench");
    let failure_output = run(&[
        "run",
        "--day",
        "1",
        "--download",
        "--offline",
        "--profile",
        "dev",
    ]);

    assert!(!String::from_utf8_lossy(&failure_output.stdout).contains("AOC 2025"));
    failure(failure_output, "--offline prevents downloading");
    fs::write(consumer.join("input/2025/day1.txt"), "alpha\nbeta\n").unwrap();

    let source = consumer.join("src/lib.rs");
    let original = fs::read_to_string(&source).unwrap();
    fs::write(&source, "#[aoc_harness::aoc(day1, part1)] fn unfinished(_: &str) -> usize { missing_value }\naoc_harness::aoc_lib! { year = 2025 }\n").unwrap();
    let output = success(run(&["input", "--day", "1", "--offline"]));

    assert!(output.contains("1 cached"));
    fs::write(&source, &original).unwrap();
    let output = success(run(&["generate", "--day", "6,7", "--dry-run"]));

    assert!(output.contains("+++ ") && output.contains("+mod day6;"));
    assert_eq!(fs::read_to_string(&source).unwrap(), original);
    assert!(!consumer.join("src/day6.rs").exists());
    failure(
        run(&["input", "--day", "6", "--generate", "--offline"]),
        "--offline prevents downloading",
    );
    assert_eq!(fs::read_to_string(&source).unwrap(), original);
    assert!(!consumer.join("src/day6.rs").exists());
    success(run(&["generate", "--day", "6,7"]));
    let generated = snapshot(&consumer);
    success(run(&["generate", "--day", "7,6"]));

    assert_eq!(snapshot(&consumer), generated);
    success(cargo(&["check", "--all-targets", "--offline"]));
    success(cargo(&["test", "--lib", "--offline"]));
    // Run composition applies the scaffold before compiling the updated registry.
    fs::write(consumer.join("input/2025/day8.txt"), "synthetic\n").unwrap();
    failure(
        run(&[
            "run",
            "--day",
            "8",
            "--generate",
            "--download",
            "--offline",
            "--profile",
            "dev",
        ]),
        "day 8 part 1 is not implemented",
    );
    assert!(consumer.join("src/day8.rs").exists());

    // Runtime-dependent operations diagnose an old consumer; ordinary commands still work.
    fs::write(
        &source,
        r#"
pub fn __aoc_harness_main() -> i32 {
    if std::env::args().any(|a| a == "__describe") {
        eprintln!("unrecognized request");
        return 2;
    }
    println!("legacy consumer executed");
    0
}
"#,
    )
    .unwrap();
    assert!(success(run(&["list", "--profile", "dev"])).contains("legacy consumer executed"));
    failure(
        run(&[
            "run",
            "--day",
            "1",
            "--download",
            "--offline",
            "--profile",
            "dev",
        ]),
        "upgrade older consumers",
    );
}

#[test]
fn cli_preflight_and_session_status_work_outside_cargo() {
    let temp = tempfile::tempdir().unwrap();
    let cli = env!("CARGO_BIN_EXE_cargo-aoc-harness");
    let token_path = temp.path().join(".adventofcode.session");
    fs::write(&token_path, "abcdef012345\r\n").unwrap();
    let original = fs::read(&token_path).unwrap();
    let permissions = fs::metadata(&token_path).unwrap().permissions();
    let call = |args: &[&str]| {
        Command::new(cli)
            .args(args)
            .current_dir(temp.path())
            .env("HOME", temp.path())
            .env_remove("ADVENT_OF_CODE_SESSION")
            .env("CARGO", temp.path().join("missing-cargo"))
            .output()
            .unwrap()
    };

    assert!(success(call(&["--version"])).contains(env!("CARGO_PKG_VERSION")));
    assert!(success(call(&["-V"])).contains(env!("CARGO_PKG_VERSION")));
    for args in [
        vec!["input", "--help"],
        vec!["profile", "--help"],
        vec!["generate", "--help"],
        vec!["credentials", "--help"],
        vec!["credentials", "status", "--help"],
    ] {
        success(call(&args));
    }
    let output = success(call(&["credentials", "status"]));

    assert!(output.contains("loaded and validated locally"));
    assert!(!output.contains("abcdef012345"));
    assert_eq!(fs::read(&token_path).unwrap(), original);
    assert_eq!(
        fs::metadata(&token_path).unwrap().permissions(),
        permissions
    );
    failure(
        call(&["credentials", "set", "secret"]),
        "unrecognized subcommand",
    );
    for args in [
        vec!["run", "--days", "1,3"],
        vec!["run", "--generate"],
        vec!["run", "--day", "1", "--session-file", "session"],
        vec!["run", "--day", "1", "--input", "alternate", "--download"],
        vec!["bench", "--open", "--smoke"],
        vec!["input", "--year", "2025"],
        vec!["input", "--year", "2025", "--all", "--day", "1"],
        vec!["generate", "--day", "26"],
        vec!["profile", "--duration", "0"],
        vec!["profile", "--template", "CPU Counters"],
    ] {
        let output = call(&args);
        let diagnostic = String::from_utf8_lossy(&output.stderr);

        assert!(!output.status.success(), "{args:?}");
        assert!(
            !diagnostic.contains("cargo metadata"),
            "preflight invoked Cargo: {diagnostic}"
        );
    }

    let output = Command::new(cli)
        .args(["profile", "--day", "1", "--part", "1"])
        .current_dir(temp.path())
        .env("PATH", "")
        .env("CARGO", temp.path().join("missing-cargo"))
        .output()
        .unwrap();

    failure(output, "samply is not installed or not on PATH");
    for (profiler, supported) in [
        ("perf", cfg!(target_os = "linux")),
        ("xctrace", cfg!(target_os = "macos")),
    ] {
        if !supported {
            failure(call(&["profile", "--profiler", profiler]), "only supported");
        }
    }
}

#[test]
fn adapter_diagnostics_and_measurement_only_none() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let target = root.join("target");
    let temp = tempfile::Builder::new()
        .prefix("adapter diagnostics ")
        .tempdir_in(&target)
        .unwrap();
    let consumer = temp.path().join("consumer");
    copy_fixture(
        &root.join("fixtures/parity"),
        &consumer,
        &root.join("crates/aoc-harness"),
    );
    let cli = env!("CARGO_BIN_EXE_cargo-aoc-harness");
    let source = consumer.join("src/lib.rs");
    let run = |args: &[&str]| invoke(cli, &consumer, args, &target, temp.path());
    for (functions, diagnostic) in [
        ("#[aoc(day1, part1, input=\"borrow\")] fn solve(raw: &str) -> usize { raw.len() }", "requires a registered generator"),
        ("#[aoc_generator(day1)] fn parse(raw: &str) -> String { raw.into() }\n#[aoc(day1, part1, input=\"borrow\")] fn solve(raw: String) -> usize { raw.len() }", "requires a reference argument"),
        ("struct Owner; struct View;\n#[aoc_generator(day1)] fn parse(_: &str) -> Owner { Owner }\n#[aoc(day1, part1, input=\"borrow\")] fn solve(_: &View) -> usize { 0 }", "Borrow<View>"),
        ("#[aoc(day1, part1)] fn solve(_: &str) -> Result<Option<usize>, std::io::Error> { Ok(Some(1)) }", "Display"),
    ] {
        fs::write(&source, format!("use aoc_harness::{{aoc,aoc_generator}};\n{functions}\naoc_harness::aoc_lib! {{ year=2025 }}\n")).unwrap();

        failure(run(&["list", "--profile", "dev"]), diagnostic);
    }
    fs::write(
        &source,
        r#"
use std::sync::atomic::{AtomicUsize, Ordering};
static CALLS: AtomicUsize = AtomicUsize::new(0);

#[aoc_harness::aoc(day1, part1)]
fn solve(_: &str) -> Option<usize> {
    println!("optional measurement output");
    if CALLS.fetch_add(1, Ordering::SeqCst) == 0 { Some(1) } else { None }
}

aoc_harness::aoc_lib! { year=2025 }
"#,
    )
    .unwrap();

    let output = run(&[
        "bench",
        "--day",
        "1",
        "--filter",
        "^solver/",
        "--open",
        "--sample-size",
        "10",
        "--warm-up-time",
        "0.01",
        "--measurement-time",
        "0.02",
    ]);

    assert!(String::from_utf8_lossy(&output.stdout).contains("Benchmark output (incomplete)"));
    failure(output, "solver: returned None");
    // Smoke execution catches None before announcing a successful workload.
    fs::write(&source, "#[aoc_harness::aoc(day1,part1)] fn solve(_: &str) -> Option<usize> { None }\naoc_harness::aoc_lib! { year=2025 }\n").unwrap();
    let output = run(&["bench", "--day", "1", "--smoke", "--profile", "dev"]);

    assert!(!String::from_utf8_lossy(&output.stdout).contains("Smoke:"));
    failure(output, "solver: returned None");
}

#[test]
fn debug_symbols_override_profiles_without_changing_optimization() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let target = root.join("target");
    let temp = tempfile::Builder::new()
        .prefix("debug configuration ")
        .tempdir_in(&target)
        .unwrap();
    let consumer = temp.path().join("consumer");
    copy_fixture(
        &root.join("fixtures/parity"),
        &consumer,
        &root.join("crates/aoc-harness"),
    );
    let manifest = consumer.join("Cargo.toml");
    let original = fs::read_to_string(&manifest).unwrap();
    fs::write(&manifest, format!("{original}\n[profile.release]\ndebug=0\nstrip='symbols'\n[profile.release.package.parity-fixture]\ndebug=0\nstrip='symbols'\n[profile.quick]\ninherits='release'\nopt-level=1\n")).unwrap();
    fs::create_dir_all(consumer.join(".cargo")).unwrap();
    fs::write(
        consumer.join(".cargo/config.toml"),
        "[profile.release.package.parity-fixture]\ndebug=0\nstrip='symbols'\n",
    )
    .unwrap();
    let before = snapshot(&consumer);
    let cli = env!("CARGO_BIN_EXE_cargo-aoc-harness");
    let build = |profile| {
        invoke(
            cli,
            &consumer,
            &[
                "run",
                "--day",
                "1",
                "--part",
                "1",
                "--profile",
                profile,
                "--debug-info",
                "--config",
                "profile.release.debug=false",
                "--config",
                "term.verbose=true",
            ],
            &target,
            temp.path(),
        )
    };

    for (profile, optimization) in [("release", "opt-level=3"), ("quick", "opt-level=1")] {
        let output = build(profile);
        let diagnostics = String::from_utf8_lossy(&output.stderr);
        let rustc = diagnostics
            .lines()
            .find(|line| line.contains("--crate-name parity_fixture"))
            .unwrap_or_else(|| panic!("missing consumer compiler invocation: {diagnostics}"));

        assert!(rustc.contains("debuginfo=2"), "{rustc}");
        assert!(rustc.contains(optimization), "{rustc}");
        assert!(!rustc.contains("strip=symbols"), "{rustc}");
        success(output);
    }

    assert_eq!(snapshot(&consumer), before);
    let mut checked = 0;
    for path in fs::read_dir(target.join("aoc-harness"))
        .unwrap()
        .map(|p| p.unwrap().path().join("Cargo.toml"))
    {
        // The shared target can contain incomplete bridges from earlier builds.
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let value: toml::Value = text.parse().unwrap();
        if value["dependencies"]["consumer"]["path"].as_str() != consumer.to_str() {
            continue;
        }
        let profiles = &value["profile"];
        for (profile, opt) in [("release", None), ("quick", Some(1))] {
            if profiles
                .get(profile)
                .and_then(|p| p.get("debug"))
                .and_then(toml::Value::as_integer)
                != Some(2)
            {
                continue;
            }

            assert_eq!(profiles[profile]["strip"].as_bool(), Some(false));
            if let Some(opt) = opt {
                assert_eq!(profiles[profile]["opt-level"].as_integer(), Some(opt));
            }
            checked += 1;
        }
    }
    assert!(checked >= 2);
    failure(
        build("unknown-profile"),
        "requires a declared custom profile",
    );
}

#[test]
fn profiling_collectors_run_compiled_workloads_and_preserve_consumer_files() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let target = root.join("target");
    fs::create_dir_all(&target).unwrap();
    let temp = tempfile::Builder::new()
        .prefix("profiling with spaces ")
        .tempdir_in(&target)
        .unwrap();
    let consumer = temp.path().join("consumer with spaces");
    let tools = temp.path().join("tools with spaces");
    fs::create_dir_all(&tools).unwrap();
    copy_fixture(
        &root.join("fixtures/library"),
        &consumer,
        &root.join("crates/aoc-harness"),
    );
    fs::write(consumer.join("alternate input.txt"), "alpha\nbeta\n").unwrap();
    let library = consumer.join("src/lib.rs");
    let original = fs::read_to_string(&library).unwrap();
    let with_relative_file = original.replace(
        "fn lines(raw: &str) -> Vec<&str> {",
        "fn lines(raw: &str) -> Vec<&str> {\n        assert!(std::path::Path::new(\"alternate input.txt\").is_file(), \"workload directory changed\");",
    );
    assert_ne!(original, with_relative_file);
    fs::write(&library, with_relative_file).unwrap();
    let before = snapshot(&consumer);
    let source = temp.path().join("collector.rs");
    fs::write(&source, r#"
use std::{env, fs, path::PathBuf, process::{Command, exit}};

fn main() {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--version") {
        return;
    }
    let program = env::current_exe().unwrap();
    let name = program.file_stem().unwrap().to_str().unwrap();
    if name == "xcrun" {
        if args == ["--find", "xctrace"] {
            println!("{}", program.with_file_name(format!("xctrace{}", env::consts::EXE_SUFFIX)).display());
            return;
        }
        exit(Command::new("/usr/bin/xcrun").args(&args).status().unwrap().code().unwrap_or(1));
    }
    fs::write(env::var_os("AOC_TEST_PROFILE_LOG").unwrap(), args.iter()
        .map(|arg| arg.to_string_lossy()).collect::<Vec<_>>().join("\n")).unwrap();
    let mode = env::var("AOC_TEST_PROFILE_MODE").unwrap_or_default();
    if mode == "fail" {
        exit(17);
    }
    let start = args.iter().position(|arg| arg == "--").unwrap() + 1;
    if mode != "skip-child" {
        let status = Command::new(&args[start]).args(&args[start + 1..]).status().unwrap();
        if !status.success() && mode != "ignore-child-status" {
            exit(status.code().unwrap_or(1));
        }
    }
    if mode != "no-output" {
        let output = PathBuf::from(&args[args.iter().position(|arg| arg == "--output").unwrap() + 1]);
        if name == "xctrace" {
            fs::create_dir(&output).unwrap();
            fs::write(output.join("trace.data"), b"synthetic trace").unwrap();
        } else {
            fs::write(output, b"synthetic recording").unwrap();
        }
    }
}
"#).unwrap();
    let collector = tools.join(format!("samply{}", std::env::consts::EXE_SUFFIX));
    success(
        Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .arg(&source)
            .arg("--edition=2021")
            .arg("-o")
            .arg(&collector)
            .output()
            .unwrap(),
    );
    for name in ["flamegraph", "perf", "xctrace", "xcrun"] {
        fs::copy(
            &collector,
            tools.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
        )
        .unwrap();
    }
    let cli = env!("CARGO_BIN_EXE_cargo-aoc-harness");
    let log = temp.path().join("arguments.txt");
    let path = std::env::join_paths(
        std::iter::once(tools.clone())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let run = |args: &[&str], mode: &str| {
        Command::new(cli)
            .arg("profile")
            .args(args)
            .args(["--duration", "1", "--profile", "dev"])
            .current_dir(&consumer)
            .env("PATH", &path)
            .env("CARGO_TARGET_DIR", &target)
            .env("CARGO_NET_OFFLINE", "true")
            .env("AOC_TEST_PROFILE_LOG", &log)
            .env("AOC_TEST_PROFILE_MODE", mode)
            .output()
            .unwrap()
    };

    let mut backends = vec![("samply", "json.gz"), ("flamegraph", "svg")];
    if cfg!(target_os = "linux") {
        backends.push(("perf", "data"));
    }
    if cfg!(target_os = "macos") {
        backends.push(("xctrace", "trace"));
    }
    for (backend, extension) in backends {
        let output = temp
            .path()
            .join(format!("recording with spaces.{extension}"));

        let result = success(run(
            &[
                "--day",
                "3",
                "-p",
                "1",
                "--input",
                "alternate input.txt",
                "--profiler",
                backend,
                "--output",
                output.to_str().unwrap(),
            ],
            "",
        ));
        let arguments = fs::read_to_string(&log).unwrap();

        assert!(result.contains("Profile recording:"));
        assert!(!result.contains("Benchmark reports:"));
        assert!(output.exists());
        assert!(arguments.contains(consumer.join("alternate input.txt").to_str().unwrap()));
        assert!(arguments.contains("--duration\n1"));
        assert!(arguments.contains("--family\nend-to-end"));
        assert!(arguments.contains("--profile-status\n"));
        assert!(arguments.contains("--\n"));

        fs::remove_file(&log).unwrap();
        failure(
            run(
                &[
                    "--day",
                    "3",
                    "--part",
                    "1",
                    "--profiler",
                    backend,
                    "--output",
                    output.to_str().unwrap(),
                ],
                "",
            ),
            "already exists",
        );
        assert!(!log.exists(), "launched a collector for an existing output");
    }

    for args in [
        vec![
            "--day",
            "1",
            "--part",
            "1",
            "--family",
            "generator",
            "--filter",
            "/default$",
        ],
        vec!["--day", "1", "--part", "1", "--family", "solver"],
        vec![
            "--day",
            "1",
            "--part",
            "1",
            "--solution",
            "cached",
            "--family",
            "solver",
        ],
        vec![
            "--day",
            "1",
            "--part",
            "2",
            "--solution",
            "mutable",
            "--family",
            "solver",
        ],
        vec![
            "--day",
            "1",
            "--part",
            "2",
            "--solution",
            "owned",
            "--family",
            "solver",
        ],
        vec!["--all-days", "--family", "full-run"],
    ] {
        let output = success(run(&args, ""));

        assert!(output.contains("Profile recording:"));
        assert!(!output.contains("Benchmark reports:"));
    }
    assert_eq!(snapshot(&consumer), before);

    let recordings = temp.path().join("multiple recordings");
    let output = success(run(
        &[
            "--day",
            "3",
            "--part",
            "1",
            "--family",
            "generator,solver",
            "--family",
            "end-to-end,full-run",
            "--output",
            recordings.to_str().unwrap(),
        ],
        "",
    ));
    let files: Vec<_> = snapshot(&recordings)
        .into_iter()
        .map(|(path, _)| path.strip_prefix(&recordings).unwrap().to_owned())
        .collect();
    let arguments = fs::read_to_string(&log).unwrap();

    assert_eq!(output.matches("Profile recording:").count(), 4);
    assert_eq!(
        files,
        [
            "end-to-end/day3/part1/default/profile.json.gz",
            "full-run/2025-day-3-part-1/profile.json.gz",
            "generator/day3/default/profile.json.gz",
            "solver/day3/part1/default/profile.json.gz",
        ]
        .map(PathBuf::from)
    );
    assert!(arguments.contains("--profile-workload\nfull-run/2025-day-3-part-1"));
    assert!(!output.contains("Benchmark reports:"));

    fs::remove_file(&log).unwrap();
    failure(
        run(
            &[
                "--day",
                "3",
                "--family",
                "generator,solver",
                "--output",
                recordings.to_str().unwrap(),
            ],
            "",
        ),
        "already exists",
    );
    assert!(!log.exists());

    let output = success(run(
        &[
            "--day",
            "3",
            "--part",
            "1",
            "--family",
            "generator,solver,end-to-end,full-run",
            "--family",
            "solver",
            "--filter",
            "^(generator|solver)/",
        ],
        "",
    ));

    assert_eq!(output.matches("Profile recording:").count(), 2);
    assert!(!output.contains("Benchmark reports:"));

    let output = run(&["--day", "3", "--part", "1"], "fail");

    assert_eq!(output.status.code(), Some(17));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Profile recording:"));
    failure(
        run(&["--day", "3", "--part", "1"], "skip-child"),
        "did not complete successfully",
    );
    failure(
        run(&["--day", "3", "--part", "1"], "no-output"),
        "without a recording",
    );

    fs::remove_file(&log).unwrap();
    let output = success(run(&["--day", "3"], ""));

    assert_eq!(output.matches("Profile recording:").count(), 2);

    fs::remove_file(&log).unwrap();
    fs::write(consumer.join("input/2025/day3.txt"), "").unwrap();
    failure(
        run(&["--day", "3", "--family", "solver,end-to-end"], ""),
        "index out of bounds",
    );
    assert!(
        !log.exists(),
        "launched a collector after prevalidation failed"
    );

    fs::write(
        consumer.join("src/lib.rs"),
        r#"
use std::sync::atomic::{AtomicUsize, Ordering};
static CALLS: AtomicUsize = AtomicUsize::new(0);

#[aoc_harness::aoc(day1, part1)]
fn solve(_: &str) -> Option<usize> {
    if CALLS.fetch_add(1, Ordering::SeqCst) == 0 { Some(1) } else { None }
}

aoc_harness::aoc_lib! { year = 2025 }
"#,
    )
    .unwrap();

    let output = run(
        &["--day", "1", "--part", "1", "--family", "solver"],
        "ignore-child-status",
    );
    let diagnostic = String::from_utf8_lossy(&output.stderr);

    assert!(diagnostic.contains("solver: returned None"));
    failure(output, "did not complete successfully");
}

#[test]
fn run_reports_preserve_answers_and_check_every_completed_variant() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let target = root.join("target");
    fs::create_dir_all(&target).unwrap();
    let temp = tempfile::tempdir_in(&target).unwrap();
    let consumer = temp.path().join("consumer");
    copy_fixture(
        &root.join("fixtures/library"),
        &consumer,
        &root.join("crates/aoc-harness"),
    );
    let cli = env!("CARGO_BIN_EXE_cargo-aoc-harness");
    let run = |args: &[&str]| {
        let mut args = args.to_vec();
        args.extend(["--profile", "dev", "--offline"]);
        invoke(cli, &consumer, &args, &target, temp.path())
    };
    let report = |output: &Output| -> serde_json::Value {
        serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("{e}: {output:?}"))
    };

    let output = run(&[
        "run",
        "-d",
        "1",
        "-p",
        "1",
        "--all-solutions",
        "--format",
        "json",
        "--download",
    ]);
    let value = report(&output);

    assert!(output.status.success());
    assert_eq!(value["year"], 2025);
    assert_eq!(value["outcome"], "success");
    assert_eq!(value["results"].as_array().unwrap().len(), 2);
    assert_eq!(value["results"][0]["variant"], "cached");
    assert_eq!(value["results"][1]["variant"], "default");
    assert!(value["results"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["answer"] == "6"));
    assert!(value["disagreements"].as_array().unwrap().is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("1 cached"));
    for result in value["results"].as_array().unwrap() {
        assert!(result["generator_ns"].is_u64());
        assert!(result["solver_ns"].is_u64());
    }
    assert!(value["totals"]["total_ns"].is_u64());

    let output = run(&[
        "run",
        "-d",
        "1",
        "-p",
        "2",
        "--all-solutions",
        "--format",
        "json",
    ]);
    let value = report(&output);

    assert!(!output.status.success());
    assert_eq!(value["outcome"], "disagreement");
    assert_eq!(value["disagreements"][0]["day"], 1);
    assert_eq!(value["disagreements"][0]["part"], 2);
    assert_eq!(
        value["disagreements"][0]["variants"],
        serde_json::json!([
            {"variant": "default", "answer": "12"},
            {"variant": "mutable", "answer": "306"},
            {"variant": "owned", "answer": "12"},
        ])
    );

    let alternate = consumer.join("negative.txt");
    fs::write(&alternate, "-1\n").unwrap();
    let output = run(&[
        "run",
        "-d",
        "1",
        "-p",
        "2",
        "--all-solutions",
        "--input",
        alternate.to_str().unwrap(),
        "--format",
        "json",
    ]);
    let value = report(&output);

    assert!(!output.status.success());
    assert_eq!(value["outcome"], "failed");
    assert_eq!(value["results"].as_array().unwrap().len(), 2);
    assert_eq!(value["results"][0]["answer"], "-2");
    assert_eq!(value["failure"]["variant"], "owned");
    assert_eq!(value["failure"]["phase"], "solver");
    assert_eq!(value["disagreements"].as_array().unwrap().len(), 1);

    fs::write(&alternate, "invalid\n").unwrap();
    let output = run(&[
        "run",
        "-d",
        "1",
        "-p",
        "1",
        "--input",
        alternate.to_str().unwrap(),
        "--format",
        "json",
    ]);
    let value = report(&output);

    assert!(!output.status.success());
    assert!(value["results"].as_array().unwrap().is_empty());
    assert_eq!(value["failure"]["phase"], "generator");
    assert_eq!(value["failure"]["day"], 1);
    assert_eq!(value["failure"]["part"], 1);

    fs::write(
        consumer.join("src/lib.rs"),
        r#"
use std::fmt;
struct Answer;
impl fmt::Display for Answer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        println!("format diagnostic");
        f.write_str("  -42\nline two\n ")
    }
}
#[aoc_harness::aoc(day1, part1, named, default)]
fn solve(_: &str) -> Answer { println!("solver diagnostic"); Answer }
#[aoc_harness::aoc(day1, part1, other)]
fn other(_: &str) -> Answer { Answer }
#[aoc_harness::aoc(day1, part2)]
fn negative(_: &str) -> i32 { -42 }
aoc_harness::aoc_lib! { year = 2025 }
"#,
    )
    .unwrap();
    let output = run(&["run", "-d", "1", "--all-solutions", "--format", "json"]);
    let value = report(&output);
    let diagnostics = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "{output:?}");
    assert_eq!(value["results"][0]["variant"], "named");
    assert_eq!(value["results"][0]["answer"], "  -42\nline two\n ");
    assert_eq!(value["results"][1]["answer"], value["results"][0]["answer"]);
    assert_eq!(value["results"][2]["answer"], "-42");
    assert!(diagnostics.contains("solver diagnostic"));
    assert_eq!(diagnostics.matches("format diagnostic").count(), 2);

    let output = run(&["run", "-d", "1", "--all-solutions"]);
    let output = success(output);

    assert_eq!(output.matches("format diagnostic").count(), 2);
    assert!(output.contains("Comparison run"));

    fs::write(
        consumer.join("src/lib.rs"),
        r#"
#[aoc_harness::aoc(day1, part1, default)] fn first(_: &str) -> &'static str { "42" }
#[aoc_harness::aoc(day1, part1, spaced)] fn second(_: &str) -> &'static str { "42 " }
#[aoc_harness::aoc(day1, part2, default)] fn third(_: &str) -> &'static str { "one\ntwo" }
#[aoc_harness::aoc(day1, part2, newline)] fn fourth(_: &str) -> &'static str { "one\ntwo\n" }
aoc_harness::aoc_lib! { year = 2025 }
"#,
    )
    .unwrap();
    let output = run(&["run", "-d", "1", "--all-solutions", "--format", "json"]);
    let value = report(&output);

    assert!(!output.status.success());
    assert_eq!(value["results"].as_array().unwrap().len(), 4);
    assert_eq!(value["disagreements"].as_array().unwrap().len(), 2);

    fs::write(
        consumer.join("src/day1.rs"),
        "#[aoc_harness::aoc(day1, part1)] fn solve(_: &str) -> i32 { -42 }",
    )
    .unwrap();
    fs::write(
        consumer.join("src/lib.rs"),
        "mod day1; aoc_harness::aoc_lib! { year = 2025 }",
    )
    .unwrap();
    let output = run(&["run", "-d", "1", "--format", "json", "--generate"]);
    let value = report(&output);

    assert!(output.status.success());
    assert_eq!(value["results"][0]["answer"], "-42");
    assert!(String::from_utf8_lossy(&output.stderr).contains("No source changes required"));

    let output = run(&["run", "-d", "2", "--format", "json", "--generate"]);
    let value = report(&output);

    assert!(!output.status.success());
    assert_eq!(value["failure"]["phase"], "solver");
    assert!(String::from_utf8_lossy(&output.stderr).contains("Generated:"));
}

#[test]
fn benchmark_smoke_executes_once_and_named_baselines_are_preserved() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let target = root.join("target");
    fs::create_dir_all(&target).unwrap();
    let temp = tempfile::tempdir_in(&target).unwrap();
    let consumer = temp.path().join("consumer");
    copy_fixture(
        &root.join("fixtures/library"),
        &consumer,
        &root.join("crates/aoc-harness"),
    );
    let cli = env!("CARGO_BIN_EXE_cargo-aoc-harness");
    let source = consumer.join("src/lib.rs");
    fs::write(&source, r#"
use std::io::Write;
fn record(phase: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("calls.txt");
    writeln!(std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap(), "{phase}").unwrap();
}
#[aoc_harness::aoc_generator(day1)]
fn parse(raw: &str) -> String { record("generator"); raw.into() }
#[aoc_harness::aoc(day1, part1)]
fn solve(raw: &str) -> usize { record("solver"); raw.len() }
aoc_harness::aoc_lib! { year = 2025 }
"#).unwrap();
    let run = |args: &[&str]| {
        let mut args = args.to_vec();
        let artifacts = temp.path().join("artifacts");
        args.extend([
            "--profile",
            "dev",
            "--offline",
            "--target-dir",
            artifacts.to_str().unwrap(),
        ]);
        invoke(cli, &consumer, &args, &target, temp.path())
    };
    let calls = consumer.join("calls.txt");
    for (families, generators, solvers) in [
        ("solver", 1, 1),
        ("solver,end-to-end", 2, 2),
        ("generator", 1, 0),
        ("full-run", 1, 1),
        ("solver,full-run", 2, 2),
        ("generator,solver,end-to-end,full-run", 4, 3),
    ] {
        fs::write(&calls, "").unwrap();

        let output = success(run(&["bench", "-d", "1", "--smoke", "--family", families]));
        let recorded = fs::read_to_string(&calls).unwrap();

        assert_eq!(
            recorded.lines().filter(|s| *s == "generator").count(),
            generators,
            "{families}: {recorded}"
        );
        assert_eq!(
            recorded.lines().filter(|s| *s == "solver").count(),
            solvers,
            "{families}: {recorded}"
        );
        assert_eq!(
            output.matches("Smoke:").count(),
            families.split(',').count()
        );
        assert!(!temp.path().join("artifacts/criterion").exists());
    }

    fs::write(
        &source,
        r#"
#[aoc_harness::aoc(day1, part1)] fn solve(raw: &str) -> usize { raw.len() }
#[aoc_harness::aoc(day1, part2)] fn other(raw: &str) -> usize { raw.len() + 1 }
aoc_harness::aoc_lib! { year = 2025 }
"#,
    )
    .unwrap();
    let measured = |part: &str, operation: &str| {
        run(&[
            "bench",
            "-d",
            "1",
            "-p",
            part,
            operation,
            "before",
            "--sample-size",
            "10",
            "--warm-up-time",
            "0.01",
            "--measurement-time",
            "0.02",
        ])
    };

    success(measured("1", "--save-baseline"));
    let reports = temp.path().join("artifacts/criterion/winter-library");
    let baseline = fs::read_dir(&reports)
        .unwrap()
        .map(|e| e.unwrap().path().join("before"))
        .find(|p| p.join("sample.json").is_file())
        .unwrap();
    let before = snapshot(&baseline);

    for _ in 0..2 {
        let output = success(measured("1", "--baseline"));

        assert!(output.contains("change:"), "{output}");
        assert_eq!(snapshot(&baseline), before);
    }
    let output = measured("2", "--baseline");

    assert_eq!(snapshot(&baseline), before);
    failure(output, "solver/day1/part2/default");
    failure(measured("2", "--baseline"), "--save-baseline before");

    success(measured("1", "--save-baseline"));
    let replaced = snapshot(&baseline);

    assert_ne!(replaced, before);
    success(measured("2", "--save-baseline"));
    assert_eq!(snapshot(&baseline), replaced);
    success(measured("2", "--baseline"));
}

#[test]
fn input_resolves_the_declared_event_without_compiling_unfinished_solvers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let target = root.join("target");
    fs::create_dir_all(&target).unwrap();
    let temp = tempfile::tempdir_in(&target).unwrap();
    let consumer = temp.path().join("consumer");
    copy_fixture(
        &root.join("fixtures/library"),
        &consumer,
        &root.join("crates/aoc-harness"),
    );
    for day in 1..=12 {
        fs::write(
            consumer.join(format!("input/2025/day{day}.txt")),
            "cached\n",
        )
        .unwrap();
    }
    let cli = env!("CARGO_BIN_EXE_cargo-aoc-harness");
    let source = consumer.join("src/lib.rs");
    let run = |args: &[&str]| invoke(cli, &consumer, args, &target, temp.path());
    for declaration in [
        "aoc_harness::aoc_lib! { year = 2025 }",
        "const EVENT_YEAR: u16 = 2025; aoc_harness::aoc_lib! { year = EVENT_YEAR }",
        "const BASE: u16 = 2000; const fn event() -> u16 { BASE + 25 } aoc_harness::aoc_lib! { year = event() }",
        "mod event { pub const YEAR: u16 = super::BASE + 25; } const BASE: u16 = 2000; aoc_harness::aoc_lib! { year = event::YEAR }",
        "struct Event; impl Event { const YEAR: u16 = 2025; } aoc_harness::aoc_lib! { year = Event::YEAR }",
        "use aoc_harness::*; const EVENT_YEAR: u16 = 2025; aoc_lib! { year = { EVENT_YEAR } }",
    ] {
        fs::write(&source, format!("mod unfinished;\n#[aoc_harness::aoc(day1, part1)] fn broken(_: &str) -> usize {{ unknown_value }}\n{declaration}\n")).unwrap();
        let before = snapshot(&consumer);

        let output = success(run(&["input", "-d", "1", "--offline"]));
        let all = success(run(&["input", "--all", "--offline"]));

        assert!(output.contains("input/2025/day1.txt"), "{declaration}: {output}");
        assert!(all.contains("12 cached"), "{declaration}: {all}");
        assert_eq!(snapshot(&consumer), before);
    }

    fs::write(consumer.join("src/event.rs"), "pub const YEAR: u16 = 2025;").unwrap();
    fs::write(&source, "mod unfinished; mod event; use event::YEAR as EVENT_YEAR; aoc_harness::aoc_lib! { year = EVENT_YEAR }").unwrap();

    success(run(&["input", "-d", "1", "--offline"]));

    fs::write(&source, "#[cfg(feature=\"bonus\")] const YEAR: u16 = 2025; #[cfg(not(feature=\"bonus\"))] const YEAR: u16 = 2024; mod unfinished; aoc_harness::aoc_lib! { year = YEAR }").unwrap();

    let output = success(run(&[
        "input",
        "-d",
        "1",
        "--offline",
        "--features",
        "bonus",
    ]));

    assert!(output.contains("input/2025/day1.txt"));

    for declaration in [
        "pub fn unrelated() {}",
        "aoc_harness::aoc_lib! { year = UNKNOWN }",
    ] {
        fs::write(&source, declaration).unwrap();
        let output = run(&["input", "-d", "1", "--session-file", "missing-session"]);
        let diagnostic = String::from_utf8_lossy(&output.stderr);

        assert!(!output.status.success());
        assert!(
            diagnostic.contains("cannot determine the aoc_lib! event year"),
            "{diagnostic}"
        );
        assert!(diagnostic.contains("winter-library"));
        assert!(diagnostic.contains(source.to_str().unwrap()));
        assert!(!diagnostic.contains("read session"));
        assert!(output.stdout.is_empty());
    }
    failure(
        run(&["input", "--year", "2025", "-d", "1"]),
        "unexpected argument '--year'",
    );
}
