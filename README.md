# aoc-harness

A Rust helper for [Advent of Code](https://adventofcode.com), inspired by
[cargo-aoc](https://github.com/gobanos/cargo-aoc). Annotate your generators and
solvers, then run, benchmark, and profile them from Cargo.

Your solutions live in a library. The harness generates the runner and benchmark
entry points, so you can focus on the puzzles.

## Features

- Download puzzle inputs and generate day modules.
- Run solutions with separate generator, solver, and full-run timings, or JSON reports.
- Check agreement between every variant of a puzzle part.
- Compare named implementations with automatic Criterion benchmarks and saved baselines.
- Borrow, mutate, or consume parsed input without requiring `Clone` or `'static`.
- Profile a workload with Samply, Flamegraph, Linux perf, or macOS Instruments.

## Getting started

### Install the CLI

Install the CLI from [crates.io](https://crates.io/crates/aoc-harness-cli):

```sh
cargo install aoc-harness-cli --locked
```

This installs `cargo-aoc-harness`, which Cargo exposes as `cargo aoc-harness`.

### Set up your project

Create a library project for your solutions:

```sh
cargo new --lib my-solutions
cd my-solutions
```

Add [`aoc-harness`](https://crates.io/crates/aoc-harness) to your solution
project's `Cargo.toml`. It provides the runtime and re-exports `aoc`,
`aoc_generator`, `aoc_lib`, and `aoc_main` from
[`aoc-harness-macros`](https://crates.io/crates/aoc-harness-macros). Import these
macros through `aoc_harness` in your solution code.

```toml
[dependencies]
aoc-harness = "0.4.0"
```

Replace `src/lib.rs` with your day modules and event year:

```rust
mod day1;

aoc_harness::aoc_lib! { year = 2025 }
```

Create `src/day1.rs` with a generator and solver functions. The example below sums
numbers for part 1 and finds the largest number for part 2; replace these with
your puzzle solutions.

### Generator functions

A generator parses the puzzle input into the type your solvers need. Start
`src/day1.rs` with:

```rust
use aoc_harness::{aoc, aoc_generator};

pub struct Numbers(Vec<u64>);

#[aoc_generator(day1)]
fn parse(input: &str) -> Result<Numbers, std::num::ParseIntError> {
    let numbers = input
        .lines()
        .map(str::parse)
        .collect::<Result<_, _>>()?;

    Ok(Numbers(numbers))
}
```

Generators accept `&str` or `&[u8]`. Their output can borrow from the raw input,
and each solver invocation gets freshly generated input. `Numbers` above does
not need to implement `Clone`.

### Solver functions

Tag each solver with its day and part. Add these functions to `src/day1.rs`:

```rust
#[aoc(day1, part1)]
fn part1(numbers: &Numbers) -> u64 {
    numbers.0.iter().sum()
}

#[aoc(day1, part2)]
fn part2(numbers: Numbers) -> u64 {
    numbers.0.into_iter().max().unwrap_or(0)
}
```

Part 1 borrows its input; part 2 takes ownership. Solvers can also take `&mut T`,
and generated vectors can be passed as slices. Without a generator, a solver
can take raw `&str` or `&[u8]` directly.

Answers must implement `Display + 'static`. Generators and solvers may return
`Result<T, E>` where `E` implements `std::error::Error`, or `Option<T>`. An `Err`
or `None` stops the command with an error identifying the function's day, part,
variant, and phase. Annotated functions remain ordinary Rust functions that you
can call from tests.

### Multiple implementations

Give an alternative implementation a name:

```rust
#[aoc(day1, part2, sorted)]
fn part2_sorted(numbers: &mut Numbers) -> u64 {
    numbers.0.sort_unstable();

    numbers.0.last().copied().unwrap_or(0)
}
```

Normal runs use an explicit `default` implementation, otherwise the unnamed
implementation, otherwise the only registered implementation. For example,
`#[aoc(day1, part2, sorted, default)]` makes the sorted variant the default.
Benchmarks include all variants in scope.

## Downloading inputs

For automatic downloads, sign in to [Advent of Code](https://adventofcode.com)
and copy the value of its `session` cookie from your browser's developer tools.
Save only the raw token in `$HOME/.adventofcode.session`, the same file used by
[aoc-cli](https://github.com/scarvalhojr/aoc-cli). On Unix, restrict its permissions
with `chmod 600 "$HOME/.adventofcode.session"`.

```sh
cargo aoc-harness credentials status
cargo aoc-harness input --day 1
cargo aoc-harness input --all
```

`credentials status` validates the local token format without printing the token
or contacting Advent of Code. You can also use `ADVENT_OF_CODE_SESSION` or
`--session-file PATH`; an explicit file takes precedence over the environment,
then the home file is used.

Inputs are stored in `input/{year}/day{day}.txt` relative to your solution
project's `Cargo.toml`. Existing inputs are preserved. `input` downloads without
compiling your solutions, so it also works while a solver is unfinished. The event
year always comes from the selected package's `aoc_lib!` declaration, including
expressions such as `year = EVENT_YEAR`. An unresolved year reports the package
and library source before credentials or downloads are attempted. `--all` uses
that event's released days; omitting `--day` selects today's day only during that
event's December release window. Keep personal inputs out of Git.

The former `input --year YEAR` flag has been removed. Migrate scripts by dropping
that flag and its value, for example `cargo aoc-harness input --all`.

Zero-padded input names such as `day01.txt` also work; `day1.txt` takes precedence
when both exist. The harness removes trailing `\n` characters before parsing and
preserves other whitespace. Relative `--input` paths use the invocation directory.

You can supply input files yourself and use the harness offline. For the example
above, create `input/2025/day1.txt` containing:

```text
1
2
4
```

## Running solutions

Run these commands from your solution project:

```sh
cargo aoc-harness run
cargo aoc-harness run --day 1
cargo aoc-harness run --day 1 --part 2 --solution sorted
cargo aoc-harness run --day 1 --all-solutions
cargo aoc-harness run --all-days
cargo aoc-harness run --all-days --format json
cargo aoc-harness list
```

`cargo aoc-harness run` runs the latest registered day by default.
`cargo aoc-harness` is a shortcut for `run`, with the same flags and selection
rules.

`run` and `bench` build in release mode by default; `list` shows every registered
day and marks default implementations. As you add days, use `--day 1,3` or repeat
the flag with `--day 1 --day 3` to select several at once. `-d` is the short form
of `--day`. Use `--input PATH` with a single `--day` to try an alternate input
file.

Use `--part 1` or `-p 1` to select a part, for example
`cargo aoc-harness run -d 1 -p 2`. Workspace package selection uses `--package`.

For `run --day 1`, the example produces output like this (timings vary):

```text
AOC 2025

Day 1 - Part 1: 7
        generator: 8µs,
        runner: 150ns

Day 1 - Part 2: 4
        generator: 7µs,
        runner: 150ns

Full run (2025 day 1, 2 selected):
        generator: 15µs,
        runner: 300ns,
        total: 16µs
```

`runner` measures the solver call. The full-run total measures the complete
selected pass, including parsing and cleanup. Compilation, input file I/O, and
answer printing happen outside that interval.

`--all-solutions` compares the displayed answers of every completed variant for
each day and part, including the default. Whitespace and line breaks must match
exactly. A disagreement prints each variant's answer and causes a nonzero exit
status after all selected implementations finish. Generator and solver failures
still stop execution and preserve the completed results. Agreement alone does
not establish correctness; keep tests for known answers.

`run --format json` writes one JSON object to stdout. Build messages, download
progress, and anything printed by solution code go to stderr. The default
`--format human` keeps the terminal report. Answers are formatted once after the
full-run timer stops, and JSON preserves them as strings, including numeric,
negative, and multiline answers. Results are ordered by day, part, and variant.

The report has `year`, `outcome` (`success`, `disagreement`, or `failed`),
`results`, `totals`, `failure`, and `disagreements`. Each result contains `day`,
`part`, `variant` (`default` for unnamed implementations), `answer`,
`generator_ns`, and `solver_ns`. Totals contain `generator_ns`, `solver_ns`, and
`total_ns`, all integer nanoseconds. On a generator or solver failure, `failure`
identifies the day, part, variant, phase, and message; otherwise it is `null`.
Each disagreement identifies its day, part, and variants with their answers.
JSON and human reports use the same exit status.

Downloads happen only when requested. Add `--download` to `run`, `bench`, or
`profile` to fetch missing inputs before execution:

```sh
cargo aoc-harness run --day 1 --download
```

## Starting another day

The bundled template includes a parser, both solver stubs, and ignored example
tests. Preview the changes, then create the day.

```sh
cargo aoc-harness generate --day 2 --dry-run
cargo aoc-harness generate --day 2
```

In a standard project this creates `src/day2.rs` and adds `mod day2;` before
`aoc_lib!` in `src/lib.rs`. Generation works offline and skips existing day modules.

### Custom day templates

Save your starter module as a UTF-8 file at `templates/day.rs` beside your
solution package's `Cargo.toml`. The CLI picks it up automatically, including
when you invoke it from the workspace root. For example, this template links
to the puzzle and starts part 1 with a line count.

```rust
//! Puzzle: https://adventofcode.com/@YEAR@/day/@DAY@

use aoc_harness::aoc;

#[aoc(day@DAY@, part1)]
fn part1(input: &str) -> usize {
    input.lines().count()
}
```

`@DAY@` expands to the unpadded day number. `@YEAR@` expands to the event year
declared by the selected package's `aoc_lib!`. The CLI replaces both wherever
they appear, including comments and strings. Write ordinary Rust braces directly.

With `aoc_lib! { year = 2025 }`, running `cargo aoc-harness generate --day 3,4`
creates modules with puzzle links ending in `/2025/day/3` and `/2025/day/4`.

The CLI selects the first applicable template in this order.

1. `--template PATH` on the command line. Relative paths start in the directory
   where you run the command.
2. `day-template` under `[package.metadata.aoc-harness]` in the selected
   package's `Cargo.toml`. Relative paths start in that package's directory.
3. `templates/day.rs` in the selected package.
4. The [bundled template](crates/aoc-harness-cli/templates/day.rs).

For example, to use a different project-wide template, set its path in
`Cargo.toml`.

```toml
[package.metadata.aoc-harness]
day-template = "starters/puzzle.rs"
```

To preview a one-off override, run this command with the path to your file.

```sh
cargo aoc-harness generate --day 5 --template starters/alternate.rs --dry-run
```

For scripts that used `input --generate` or `run --generate`, call
`cargo aoc-harness generate --day 2` as a separate step. To download input
and execute the new day after implementing it, use
`cargo aoc-harness run --day 2 --download`.

## Benchmarking solutions

Benchmarks use [Criterion](https://bheisler.github.io/criterion.rs/book/) and
require no extra dependencies or benchmark targets in your solution project.

```sh
cargo aoc-harness bench
cargo aoc-harness bench --day 1 --open
cargo aoc-harness bench --day 1 --part 2 --solution sorted
cargo aoc-harness bench --all-days
cargo aoc-harness bench --day 1 --family generator,solver
cargo aoc-harness bench --all-days --family full-run
```

Benchmarks default to the `solver` family. Use `--family` to select any combination
of `generator`, `solver`, `end-to-end`, and `full-run`, with comma-separated values
or repeated flags, such as `--family generator --family solver`. `--filter`
further narrows the selected families by benchmark name.

| Family | Measured work |
| --- | --- |
| `generator` | Parse input once. |
| `solver` | Run one solver, excluding input preparation and external cleanup. |
| `end-to-end` | Generate input, solve, and clean up for one implementation. |
| `full-run` | Execute the selected default implementations, including parsing and cleanup. |

Mutable and consuming solver benchmarks receive fresh input for each iteration.
Immutable solvers reuse prepared input; add `bench_input = "fresh"` to the
`#[aoc(...)]` annotation if a solver mutates through a shared reference.

Filter by benchmark name or adjust measurement settings:

```sh
cargo aoc-harness bench --day 1 --filter '^solver/' --sample-size 20 \
  --warm-up-time 1 --measurement-time 3
cargo aoc-harness bench --day 1 --smoke
```

`--smoke` executes each selected workload exactly once, using that execution to
validate it, without statistical measurement or Criterion reports. A solver
selected in both `solver` and `end-to-end` runs once per workload; `full-run`
executes one complete pass through the selected default implementations.
Measured reports go to `<target-dir>/criterion/<your-package>/`; `--open` opens
the HTML report after a successful run.

Save a reference measurement, then compare subsequent edits against it:

```sh
cargo aoc-harness bench -d 4 -p 2 --save-baseline before
cargo aoc-harness bench -d 4 -p 2 --baseline before
```

Both operations honor day, part, solution, family, and workload filters and use
the package's Criterion report directory. Saving again replaces only the
selected workloads' references. Comparisons report Criterion's timing change
and preserve the named reference. Every selected workload must have a saved
measurement; otherwise the command fails with the missing name and workload and
suggests `--save-baseline NAME`. Choose one operation per measured run; neither
can be combined with `--smoke`. Names must be single directory components other
than Criterion's reserved `base`, `new`, `change`, `report`, and `profile` directories.

## Profiling solutions

Install an external profiler, then record selected workloads:

```sh
cargo install --locked samply
cargo aoc-harness profile --day 1 --part 2
cargo aoc-harness profile --day 1 --part 2 --family solver --duration 30
cargo aoc-harness profile --day 1 --family generator,solver
cargo aoc-harness profile --all-days --family full-run
```

Profiling defaults to Samply and approximately 10 seconds of end-to-end work.
Select workloads with `--day`, `--part`, and optionally `--solution`, or
choose `--family full-run` for a complete pass. Debug symbols are enabled
automatically while preserving the selected Cargo profile's optimization.

Use `--profiler flamegraph`, `--profiler perf` on Linux, or `--profiler xctrace`
on macOS for other recorders, installed separately. Recordings go under
`<target-dir>/aoc-harness/profiles/<your-package>/`; `--output PATH` selects a new
output path. With multiple workloads, `PATH` is a new directory containing a
recording for each workload, organized by workload name. The CLI prints how to
open each recording.

| Profiler | Recording | Open with |
| --- | --- | --- |
| Samply | `profile.json.gz` | `samply load PATH` |
| Flamegraph | `flamegraph.svg` | A browser |
| Linux perf | `perf.data` | `perf report -i PATH` |
| macOS xctrace | `profile.trace` | Instruments |

Flamegraph also needs its platform's collector: `perf` on Linux or full Xcode's
`xctrace` on macOS. For xctrace, `--template` selects the recording template;
the default is `Time Profiler`.

Profiling shares the benchmark command's `--family` and `--filter` flags:
select any combination of `generator`, `solver`, `end-to-end`, and `full-run`
with comma-separated values or repeated `--family` flags, then optionally narrow
the selection with `--filter`. Each selected workload gets its own recording,
and `--duration` applies to each workload, so total recording time grows with
the selection. Builds, downloads, and preflight validation of all selected
workloads finish before recording starts. Recordings include process
startup, input loading, and preparation and cleanup around solver calls.
Profiling does not save benchmark baselines or statistical reports. Existing
recording paths are preserved; choose a new path for each `--output`.

## Advanced usage

### Generator overrides

Generators can apply to a whole day, one part, or a named implementation.
The most specific registration wins. These attributes belong on separate
generator functions:

```rust,ignore
#[aoc_generator(day1)]
#[aoc_generator(day1, part2)]
#[aoc_generator(day1, part2, sorted)]
```

Duplicate registrations and conflicting defaults fail compilation. Several named
implementations without an unnamed implementation require an explicit default.
Use direct `#[aoc]` and `#[aoc_generator]` attributes in your module tree, with
`#[cfg]` for conditional registration. Attributes introduced through `cfg_attr`,
another macro, `include!`, or renamed attribute imports are not discovered.

### Borrowed input and optional values

Parsed values may borrow from the raw input, for example `Vec<&str>`, without a
`'static` bound. Ordinary Rust borrowing, slice, and deref coercions apply.
For custom `Borrow<T>` or `BorrowMut<T>` projections, add `input = "borrow"` to
the solver's `#[aoc(...)]` annotation.

The harness unwraps one outer `Result` or `Option` on generators and solvers.
Add `output = "value"` to treat that wrapper as an ordinary value instead,
including when a generator deliberately returns optional parsed input. Return
types must spell their final path segment `Result` or `Option`; arbitrary aliases
are not inferred.

### Your own executable

To run a solution library through `cargo run`, add `src/main.rs` with your
library's Rust crate name:

```rust,ignore
aoc_harness::aoc_main! { lib = my_solutions }
```

Or put annotated functions in a standalone binary and finish its crate root
with `aoc_harness::aoc_main! { year = 2025 }`. Both forms support the runtime's
run, list, and benchmark commands, for example `cargo run --release -- run`.
Downloads and external-profiler launching use the Cargo subcommand.

## Further documentation

Use `cargo aoc-harness --help` or a command's `--help` for its options.
Cargo integration supports `--manifest-path`, `--package`, `--features`,
`--profile`, `--target-dir`, `--offline`, and `--locked`. `--package` is required
when workspace package selection is ambiguous. `--profile` selects the Cargo
build profile; `profile` is the profiling subcommand.

- [Complete example](fixtures/library): a runnable library with synthetic inputs,
  ownership variants, and feature-dependent registrations.
- [Release guide](RELEASE.md): validation and publication instructions.

Licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
