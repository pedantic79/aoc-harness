# aoc-harness

Typed Advent of Code generators, solvers, executable entry points, and automatic
Criterion benchmarks. Inputs can be borrowed, mutated, or consumed without Clone.

Install [`aoc-harness-cli`](https://crates.io/crates/aoc-harness-cli) to get the
`cargo aoc-harness` command for running, benchmarking, profiling, downloading
inputs, and generating day modules:

```sh
cargo install aoc-harness-cli --locked
```

Then add `aoc-harness` to your solution project's `Cargo.toml`. It provides the
runtime and re-exports `aoc`, `aoc_generator`, `aoc_lib`, and `aoc_main` from
[`aoc-harness-macros`](https://crates.io/crates/aoc-harness-macros):

```toml
[dependencies]
aoc-harness = "0.5.0"
```

```rust,ignore
use aoc_harness::{aoc, aoc_generator, aoc_lib};

#[aoc_generator(day1)]
fn parse(raw: &str) -> Vec<&str> { raw.lines().collect() }

#[aoc(day1, part1)]
fn part1(lines: &[&str]) -> usize { lines.len() }

aoc_lib! { year = 2025 }
```

The CLI and this runtime are released separately. When a CLI feature needs a
newer runtime, the CLI names the `aoc-harness` release and the command or
declaration to update. When the runtime has features the CLI doesn't recognize,
it suggests `cargo install aoc-harness-cli --locked`. Upgrade the CLI whenever
you move `aoc-harness` across a 0.x release.

Use `aoc_harness::aoc_main! { lib = your_library }` in a binary, or finish a
standalone binary with `aoc_harness::aoc_main! { year = 2025 }`. `year` takes a
constant `u16` expression, such as `2025` or a `const EVENT_YEAR`.

In a solution module's regression test, `let raw = aoc_harness::input!(2);`
reads day 2's local input for the declared event year. It returns a `String`
ready to pass to `parse(&raw)`, with the same filename lookup and trailing
newline removal as the runner. `input!(2025, 2)` names the year explicitly; it
must be a year the crate declares. A missing or unreadable input fails the test.

Inputs live in `input/{year}/day{day}.txt` beside your package's `Cargo.toml`.
To keep them elsewhere, set a directory in that manifest. Relative paths start
in the package's directory:

```toml
[package.metadata.aoc-harness]
input-dir = "../advent-of-code-input"
```

The runner, benchmarks, `input!`, and CLI downloads all use this directory.
The setting requires `aoc-harness` 0.5.0 or later; older runtimes read `input/`.

Select multiple days with `--day 1,3` or `--day 1 --day 3`; `-d` is the short form
of `--day`. Select a part with `--part 1` or `-p 1`. Use `--all-days` to select
every registered day. `run --all-solutions` compares each part's displayed
answers exactly, including whitespace, and fails if any variants disagree.
`run --format json` provides ordered results, string answers, integer nanosecond
timings, and partial failure or disagreement details; solution output goes to
stderr. Plain `run` keeps the human-readable report.

`cargo aoc-harness input -d 1` and `input --all` use the event year declared in
`aoc_lib!`, including constant expressions, without compiling unfinished solvers.

One crate can hold several events. Put each year's day modules under its own
module, such as `src/y2019/mod.rs`, and map them at the crate root:

```rust,ignore
pub mod common;

mod y2019;
mod y2025;

aoc_harness::aoc_lib! {
    years = {
        2019 => y2019,
        2025 => y2025,
    },
    default_year = 2025,
}
```

Annotations inside each year module are unchanged, and registrations stay
within their year, so two years can reuse the same day, part, and function
names. The keys and `default_year` are integer literals. Select an event with
`--year` on `run`, `list`, `bench`, `profile`, `input`, and `generate`. Without
it, commands use the only registered year or `default_year`, and plain `list`
shows every year. In tests, use `input!(2019, 1)`: `input!(day)` is a compile
error in a crate that registers several years. Each year keeps its own Criterion
reports, baselines, and profiles in a `<year>/` subdirectory; single-year crates
keep their existing paths. The
[migration guide](https://github.com/pedantic79/aoc-harness/blob/main/docs/multi-year-migration.md)
covers combining per-year crates into one.

`cargo aoc-harness bench` benchmarks solvers by default. Select any combination
of `generator`, `solver`, `end-to-end`, and `full-run` with `--family`, for example
`cargo aoc-harness bench --family generator,solver`. Comma-separated values and
repeated `--family` flags work with measured benchmarks and `--smoke`. Smoke
executes each workload once without measurements or reports. Save selected
measurements with `--save-baseline NAME`, then compare edits with
`--baseline NAME`, preserving the reference. Missing workload references fail.
Choose one baseline operation per measured run; neither works with `--smoke`.
Profiling uses the same `--family` and `--filter` selection, defaults to
`end-to-end`, and records each selected workload separately.

See the [repository documentation](https://github.com/pedantic79/aoc-harness)
for selection, Option/Result handling, custom Borrow projections, benchmarks,
and external profiling through the same typed workload adapters.

Licensed under MIT OR Apache-2.0.
