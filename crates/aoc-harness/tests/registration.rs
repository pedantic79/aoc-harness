//! Year-scoped registration, compiled in a copy of the multi-year fixture.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

struct Consumer {
    _temp: tempfile::TempDir,
    dir: PathBuf,
    target: PathBuf,
}

impl Consumer {
    fn new() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let target = root.join("target");
        fs::create_dir_all(&target).unwrap();
        let temp = tempfile::Builder::new()
            .prefix("registration ")
            .tempdir_in(&target)
            .unwrap();
        let dir = temp.path().join("consumer");

        copy(&root.join("fixtures/years"), &dir);
        let manifest = dir.join("Cargo.toml");
        let mut value: toml::Value = fs::read_to_string(&manifest).unwrap().parse().unwrap();
        value["dependencies"]["aoc-harness"]["path"] =
            root.join("crates/aoc-harness").to_str().unwrap().into();
        fs::write(manifest, toml::to_string(&value).unwrap()).unwrap();

        Self {
            _temp: temp,
            dir,
            target,
        }
    }

    fn write(&self, path: &str, contents: &str) {
        let path = self.dir.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn cargo(&self, args: &[&str]) -> Output {
        Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .args(args)
            .current_dir(&self.dir)
            .env("CARGO_TARGET_DIR", &self.target)
            .env("CARGO_NET_OFFLINE", "true")
            .output()
            .unwrap()
    }
}

fn copy(source: &Path, destination: &Path) {
    for entry in fs::read_dir(source).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap();
        if name == "target" || name == "Cargo.lock" {
            continue;
        }

        let out = destination.join(name);
        if path.is_dir() {
            copy(&path, &out);
        } else {
            fs::create_dir_all(destination).unwrap();
            fs::copy(&path, out).unwrap();
        }
    }
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn success(output: Output) -> String {
    assert!(output.status.success(), "{}", text(&output));
    String::from_utf8(output.stdout).unwrap()
}

fn failure(output: Output, expected: &str) {
    let combined = text(&output);

    assert!(!output.status.success(), "unexpected success:\n{combined}");
    assert!(
        combined.contains(expected),
        "expected {expected:?} in:\n{combined}"
    );
}

const MODULES: &str = "pub mod common;\nmod y2019;\nmod y2025;\n";

fn library(declaration: &str) -> String {
    format!("{MODULES}aoc_harness::aoc_lib! {{ {declaration} }}\n")
}

#[test]
fn years_share_identities_and_select_a_default() {
    let consumer = Consumer::new();

    let tests = consumer.cargo(&["test", "--lib", "--quiet"]);
    let bonus = consumer.cargo(&["test", "--lib", "--quiet", "--features", "bonus"]);
    let listed = success(consumer.cargo(&["run", "--quiet", "--", "list", "--all-days"]));
    let narrowed = success(consumer.cargo(&["run", "--quiet", "--", "list", "--year", "2025"]));

    success(tests);
    success(bonus);
    let newer = listed.find("2025 day 2 part 1 [default]").unwrap();
    let older = listed.find("2019 day 1 part 1 [default]").unwrap();
    assert!(older < newer, "{listed}");
    assert!(
        narrowed.contains("2025 day 2 part 1 [default]"),
        "{narrowed}"
    );
    assert!(!narrowed.contains("2019"), "{narrowed}");

    consumer.write(
        "src/lib.rs",
        &library("years = { 2019 => y2019, 2025 => y2025 }"),
    );

    failure(
        consumer.cargo(&["run", "--quiet", "--", "run"]),
        "several event years are registered (2019, 2025) and aoc_lib! sets no default_year; select one with --year",
    );
    success(consumer.cargo(&["run", "--quiet", "--", "list"]));
    success(consumer.cargo(&["run", "--quiet", "--", "list", "--year", "2019"]));

    // One mapped year is the default, and input!(day) needs no year.
    consumer.write(
        "src/lib.rs",
        "pub mod common;\nmod y2025;\naoc_harness::aoc_lib! { years = { 2025 => crate::y2025 } }\n\
         #[test] fn reads() { assert_eq!(aoc_harness::input!(1), \"alpha\\nbeta\"); }\n",
    );

    success(consumer.cargo(&["test", "--lib", "--quiet"]));
}

#[test]
fn invalid_mappings_and_registrations_are_compile_errors() {
    let consumer = Consumer::new();
    let cases = [
        (
            library("year = 2025, years = { 2025 => y2025 }"),
            "use either year = ... or years = { ... }, not both",
        ),
        (
            library("year = 2025, default_year = 2025"),
            "default_year requires years = { ... }",
        ),
        (
            library("years = { 2019 => y2019, 2019 => y2025 }"),
            "year 2019 is mapped more than once",
        ),
        (
            format!("const OLD: u16 = 2019;\n{}", library("years = { OLD => y2019, 2025 => y2025 }")),
            "each years key must be an integer literal such as 2025",
        ),
        (
            library("years = { 2014 => y2019, 2025 => y2025 }"),
            "each years key must be an Advent of Code event year, 2015 or later",
        ),
        (
            library("years = { 2019 => y2019, 2025 => y2019 }"),
            "module `y2019` is mapped more than once",
        ),
        (
            library("years = { 2019 => y2019, 2025 => y2019::day01 }"),
            "mapped modules must not nest: `y2019::day01` and `y2019`",
        ),
        (
            library("years = { 2019 => y2019, 2025 => y2026 }"),
            "year 2025 maps to module `y2026`, which the crate does not declare",
        ),
        (
            library("years = { 2019 => y2019, 2025 => y2025 }, default_year = 2024"),
            "default_year 2024 is not registered in years",
        ),
        (
            format!("{}#[aoc_harness::aoc(day4, part1)]\nfn stray(raw: &str) -> usize {{ raw.len() }}\n", library("years = { 2019 => y2019, 2025 => y2025 }")),
            "#[aoc] registration `stray` (day4/part1/default) in ",
        ),
        (
            library("years = { 2019 => y2019, 2025 => y2025 }, default_year = 2025")
                + "pub fn read() -> String { aoc_harness::input!(1) }\n",
            "use input!(year, day)",
        ),
        (
            "#[aoc_harness::aoc(day1, part1)]\nfn part1(raw: &str) -> usize { raw.len() }\n\
             fn current() -> u16 { 2025 }\naoc_harness::aoc_lib! { year = current() }\n"
                .into(),
            "cannot call non-const function `current`",
        ),
    ];

    for (source, diagnostic) in cases {
        consumer.write("src/lib.rs", &source);

        failure(consumer.cargo(&["check", "--lib", "--quiet"]), diagnostic);
    }

    // A registration in an unmapped module names its file and suggests a mapping.
    let common = fs::read_to_string(consumer.dir.join("src/common/mod.rs")).unwrap();
    consumer.write(
        "src/lib.rs",
        &library("years = { 2019 => y2019, 2025 => y2025 }, default_year = 2025"),
    );
    consumer.write(
        "src/common/mod.rs",
        "#[aoc_harness::aoc(day1, part1)]\nfn part1(raw: &str) -> usize { raw.len() }\n",
    );
    let file = Path::new("src").join("common").join("mod.rs");

    let output = text(&consumer.cargo(&["check", "--lib", "--quiet"]));

    assert!(output.contains(&file.display().to_string()), "{output}");
    assert!(output.contains("add `YEAR => common` to years"), "{output}");

    // Const evaluation rejects registrations the source scan cannot see.
    consumer.write("src/common/mod.rs", "include!(\"hidden.rs\");\n");
    consumer.write(
        "src/common/hidden.rs",
        "#[aoc_harness::aoc(day1, part1)]\nfn part1(raw: &str) -> usize { raw.len() }\n",
    );

    failure(
        consumer.cargo(&["check", "--lib", "--quiet"]),
        "registration is outside every module mapped by aoc_lib! years",
    );

    consumer.write("src/common/mod.rs", &common);
    consumer.write(
        "src/bin/solo.rs",
        "mod y2025 {}\naoc_harness::aoc_main! { years = { 2025 => y2025 } }\n",
    );

    failure(
        consumer.cargo(&["check", "--bin", "solo", "--quiet"]),
        "standalone aoc_main! supports one event year",
    );
}
