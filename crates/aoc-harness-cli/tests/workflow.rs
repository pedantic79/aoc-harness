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
                assert!(output.contains("--all-days"));
                assert!(output.contains("--solution"));
                assert!(output.contains("Examples:"));
                assert!(!output.contains("Commands:"));
                assert_eq!(output.contains("--all-solutions"), action == "run");
                assert_eq!(output.contains("--input"), action != "list");
                assert_eq!(output.contains("--filter"), action == "bench");
                assert_eq!(output.contains("--sample-size"), action == "bench");
                assert_eq!(output.contains("--smoke"), action == "bench");
                assert_eq!(output.contains("--color"), action == "bench");
                assert_eq!(output.contains("latest registered day"), action != "list");
            }
        }

        let output = help(&["profile", "--help"]);

        assert!(output.contains("Usage: cargo aoc-harness profile [OPTIONS]"));
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
        vec!["run", "--days=1,3"],
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

    let output = success(run(&["list", "--days", "1,3", "--profile", "dev"]));

    assert!(output.contains("day 1 part 2 [owned] (selected default)"));
    assert!(output.contains("day 3 part 2 [default] (selected default)"));
    assert!(!output.contains("day 2 part"));

    let output = success(run(&[
        "run",
        "--day",
        "1",
        "--part",
        "2",
        "--solution",
        "mutable",
        "--profile",
        "quick",
    ]));

    assert!(output.contains("Day 1 - Part 2 - mutable: 306"));
    let output = success(run(&[
        "run",
        "--day",
        "1",
        "--all-solutions",
        "--profile",
        "dev",
    ]));

    assert!(output.contains("Comparison run (2025 day 1, 5 selected)"));

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

    assert!(output.contains("Smoke: generator/day3/default\n"));
    assert!(output.contains("Smoke: solver/day3/part1/default\n"));
    assert!(output.contains("Smoke: solver/day3/part2/default\n"));
    assert!(output.contains("Smoke: full-run/2025-day-3\n"));
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
        "--features",
        "bonus",
        "--smoke",
        "--profile",
        "dev",
    ]));

    assert!(output.contains("Smoke: solver/day5/part1/only\n"));
    assert!(output.contains("Smoke: end-to-end/day5/part1/only\n"));
    assert!(output.contains("Smoke: full-run/2025-day-5\n"));
    assert!(!output.contains("day3/"));

    let output = success(run(&["bench", "--all-days", "--smoke", "--profile", "dev"]));

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
        "--days",
        "1,3",
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
            "--filter",
            "^solver/",
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
            "--days",
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
        "--days",
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
            vec!["run", "--day", day],
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
    fs::write(&source, "uncompilable Rust source").unwrap();
    let output = success(run(&["input", "--year", "2025", "--day", "1", "--offline"]));

    assert!(output.contains("1 cached"));
    fs::write(&source, &original).unwrap();
    let output = success(run(&["generate", "--days", "6,7", "--dry-run"]));

    assert!(output.contains("+++ ") && output.contains("+mod day6;"));
    assert_eq!(fs::read_to_string(&source).unwrap(), original);
    assert!(!consumer.join("src/day6.rs").exists());
    failure(
        run(&[
            "input",
            "--year",
            "2025",
            "--day",
            "6",
            "--generate",
            "--offline",
        ]),
        "--offline prevents downloading",
    );
    assert_eq!(fs::read_to_string(&source).unwrap(), original);
    assert!(!consumer.join("src/day6.rs").exists());
    success(run(&["generate", "--days", "6,7"]));
    let generated = snapshot(&consumer);
    success(run(&["generate", "--days", "7,6"]));

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
        vec!["run", "--generate"],
        vec!["run", "--day", "1", "--session-file", "session"],
        vec!["run", "--day", "1", "--input", "alternate", "--download"],
        vec!["bench", "--open", "--smoke"],
        vec!["input", "--all"],
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
    // Smoke prevalidation catches None before announcing a successful family.
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
                "--part",
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
    failure(run(&["--day", "3"], ""), "exactly one workload");
    assert!(
        !log.exists(),
        "launched a collector for an ambiguous workload"
    );
    fs::write(consumer.join("input/2025/day3.txt"), "").unwrap();
    failure(
        run(&["--day", "3", "--part", "2"], ""),
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
