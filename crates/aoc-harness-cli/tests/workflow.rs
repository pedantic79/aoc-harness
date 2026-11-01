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
    let cli = installed.join("aoc-harness");
    let run = |args: &[&str]| invoke(&cli, &consumer, args, &target, &installed);

    let output = success(run(&["--profile", "dev"]));

    assert!(output.contains("Day 1 part 1 [default]: 6"));
    assert!(output.contains("Day 1 part 2 [owned]: 12"));
    assert!(output.contains("Day 3 part 2 [default]: 2"));
    assert!(output.contains("Full run (2025, 6 selected)"));
    assert!(output.contains("generator 0ns"));

    let standalone = success(run(&["list", "--profile", "dev"]));
    let cargo = success(invoke(
        "cargo",
        &consumer,
        &["aoc-harness", "list", "--profile", "dev"],
        &target,
        &installed,
    ));

    assert_eq!(standalone, cargo);
    assert!(standalone.contains("[owned] (selected default)"));
    assert!(standalone.contains("[mutable]"));

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

    assert!(output.contains("[mutable]: 306"));
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

    assert!(output.contains("[default]: 4"));
    let output = success(run(&[
        "list",
        "--all-features",
        "--no-default-features",
        "--profile",
        "dev",
    ]));

    assert!(output.contains("day 5 part 1 [only]"));

    let output = success(run(&["bench", "--smoke", "--profile", "dev"]));
    for family in ["generator/", "solver/", "end-to-end/", "full-run/"] {
        assert!(output.contains(family));
    }
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
    let output = success(run(&[
        "bench",
        "--day",
        "1",
        "--sample-size",
        "10",
        "--warm-up-time",
        "0.01",
        "--measurement-time",
        "0.02",
        "--filter",
        "^(generator/day1/part0/default|solver/day1/part1/(default|cached)|solver/day1/part2/(owned|mutable)|end-to-end/day1/part2/owned|full-run/)",
        "--profile",
        "release",
    ]));

    assert!(output.contains("time:"));
    assert!(output.contains("Benchmark reports:"));
    let reports = target.join("criterion/winter-library");
    assert!(reports.join("report/index.html").exists());

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

    assert!(output.contains("[default]: 7"), "{output}");
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

    assert!(output.contains("[default]: 13"), "stale bridge: {output}");
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
