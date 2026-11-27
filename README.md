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
aoc-harness = "0.6.0"
```

Replace `src/lib.rs` with your day modules and event year:

```rust
mod day01;

aoc_harness::aoc_lib! { year = 2025 }
```

`year` takes a constant `u16` expression, such as `2025` or a `const EVENT_YEAR`.
An expression that can only be evaluated at run time, such as a call to a
non-`const` function, is a compile error.

Create `src/day01.rs` with a generator and solver functions. The example below sums
numbers for part 1 and finds the largest number for part 2; replace these with
your puzzle solutions.

### Generator functions

A generator parses the puzzle input into the type your solvers need. Start
`src/day01.rs` with:

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

Tag each solver with its day and part. Add these functions to `src/day01.rs`:

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
cargo aoc-harness input --day all
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
and library source before credentials or downloads are attempted. `--day all`
uses that event's released days; omitting `--day` selects today's day only during
that event's December release window. Keep personal inputs out of Git.

`--year YEAR` selects one of the years that `aoc_lib!` registers; it never
relabels another year's inputs. In a single-year crate it must match the declared
year. See [Multiple event years](#multiple-event-years).

Zero-padded input names such as `day01.txt` also work; `day1.txt` takes precedence
when both exist. The harness removes trailing `\n` characters before parsing and
preserves other whitespace. Relative `--input` paths use the invocation directory.

To keep inputs somewhere else, such as a private repository checked out beside
your solutions, set `input-dir` in the solution package's `Cargo.toml`:

```toml
[package.metadata.aoc-harness]
input-dir = "../advent-of-code-input"
```

The layout under that directory stays `{year}/day{day}.txt`. A relative path
starts in the directory containing that `Cargo.toml`, not where you run the
command, so running from a workspace root with `--package` or from elsewhere with
`--manifest-path` finds the same files. `input`, `--download`, `run`, `bench`,
`profile`, and `aoc_harness::input!` all read and write this directory.
`input-dir` requires `aoc-harness` 0.5.0 or later; older runtimes ignore it and
read `input/`. `cargo update` doesn't cross a 0.x release, so upgrade with
`cargo add aoc-harness@0.5.0`.

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
cargo aoc-harness run --day 1 --solution all
cargo aoc-harness run --day all
cargo aoc-harness run --day all --format json
cargo aoc-harness list
```

`cargo aoc-harness run` runs the latest registered day by default.
`cargo aoc-harness` is a shortcut for `run`, with the same flags and selection
rules. Human-readable and JSON reports name the event year.

`run` and `bench` build in release mode by default; `list` shows every registered
day and marks default implementations. As you add days, use `--day 1,3` or repeat
the flag with `--day 1 --day 3` to select several at once, or `--day all` to
select every registered day. `-d` is the short form of `--day`. Use
`--input PATH` with a single `--day` to try an alternate input file.

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

`--solution all` compares the displayed answers of every completed variant for
each day and part, including the default. Whitespace and line breaks must match
exactly. A disagreement prints each variant's answer and causes a nonzero exit
status after all selected implementations finish. Generator and solver failures
still stop execution and preserve the completed results. Agreement alone does
not establish correctness; keep tests for known answers.

`run --format json` writes one JSON object to stdout. Build messages, download
progress, and anything printed by solution code go to stderr. The default
`--format human` keeps the terminal report. Answers are formatted once after the
full-run timer stops, and JSON preserves them as strings, including numeric,
negative, and multiline answers. The object has an overall `outcome` and a
`years` array with one report per event year: one entry, or one per year with
`--year all`. Each report has its `year`, `outcome`, `results`, `totals`,
`failure`, and `disagreements`. Results are ordered by day, part, and variant.
Runtimes before `aoc-harness` 0.6.0 write a single report without the `years`
wrapper.

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

In a standard project this creates `src/day02.rs` and adds `mod day02;` before
`aoc_lib!` in `src/lib.rs`. New module names use two-digit day numbers, except
in projects whose library root already declares only unpadded single-digit
modules such as `mod day4;`, which keep that style. Generation works offline.
It skips a day whose module is declared with a matching file, such as
`mod day04;` with `src/day04.rs`. An undeclared day file, or a declaration
whose file name doesn't match, stops generation before anything is written.

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

`@DAY@` expands to the unpadded day number. `@YEAR@` expands to the selected
event year from the package's `aoc_lib!`. The CLI replaces both wherever
they appear, including comments and strings. Write ordinary Rust braces directly.

With `aoc_lib! { year = 2025 }`, running `cargo aoc-harness generate --day 3,4`
creates modules with puzzle links ending in `/2025/day/3` and `/2025/day/4`.

In a regression test, pass the day to `aoc_harness::input!`. For day 2,
`generate` writes `aoc_harness::input!(2)`. When the test runs, the macro
reads the downloaded input for the calling module's event year and returns a
`String` with trailing `\n` characters removed, just as the runner does. It
looks for `day2.txt` first and then `day02.txt`, in the package's `input-dir`
when one is set. In a crate that registers [several years](#multiple-event-years),
the year comes from the year module containing the call, so a test in
`src/y2019/day02.rs` reads 2019's input. To name the event year explicitly, use
`aoc_harness::input!(2025, 2)`; the year must be one the crate declares.

```rust
let raw = aoc_harness::input!(@DAY@);
let input = generator(&raw);
```

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
cargo aoc-harness bench --day all
cargo aoc-harness bench --day 1 --family generator,solver
cargo aoc-harness bench --day all --family full-run
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
Measured reports go to `<target-dir>/criterion/<your-package>/`, or to
`<target-dir>/criterion/<your-package>/<year>/` in a crate that registers
several years. The summary names the event year, and `--open` opens the HTML
report after a successful run.

Save a reference measurement, then compare subsequent edits against it:

```sh
cargo aoc-harness bench -d 4 -p 2 --save-baseline before
cargo aoc-harness bench -d 4 -p 2 --baseline before
```

Both operations honor day, part, solution, family, and workload filters and use
the package's Criterion report directory, which is per year in a multi-year crate,
so a baseline saved for one year is never compared with another. Saving again replaces only the
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
cargo aoc-harness profile --day all --family full-run
```

Profiling defaults to Samply and approximately 10 seconds of end-to-end work.
Select workloads with `--day`, `--part`, and optionally `--solution`, or
choose `--family full-run` for a complete pass. Profiling uses default
implementations unless `--solution` names one, or `--solution all` selects every
variant. Debug symbols are enabled
automatically while preserving the selected Cargo profile's optimization.

Use `--profiler flamegraph`, `--profiler perf` on Linux, or `--profiler xctrace`
on macOS for other recorders, installed separately. Recordings go under
`<target-dir>/aoc-harness/profiles/<your-package>/`, with a `<year>/` subdirectory
in a multi-year crate; `--output PATH` selects a new output path, used as given. With multiple workloads, `PATH` is a new directory containing a
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

### Multiple event years

One crate can hold several events, each in its own module, with shared code
beside them:

```text
my-solutions/
├── Cargo.toml
├── input/2019/day1.txt
├── input/2025/day1.txt
└── src/
    ├── lib.rs
    ├── common/mod.rs
    ├── y2019/{mod.rs, day01.rs}
    └── y2025/{mod.rs, day01.rs}
```

Declare each year's day modules in its `mod.rs`, such as `mod day01;` in
`src/y2019/mod.rs`, and map the years in `src/lib.rs`:

```rust
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

`#[aoc]` and `#[aoc_generator]` annotations are written exactly as in a
single-year crate. Each year's registrations stay within that year, so two years
can reuse the same day, part, and function names, and a generator only serves
its own year's solvers. A year's module can be `src/y2019/mod.rs` or
`src/y2019.rs`, and may contain nested modules.

The `years` keys and `default_year` are integer literals, 2015 or later, so the
CLI can read them without compiling your solutions. Each year maps to a
different module, and mapped modules can't be nested inside each other. Use
either `year` or `years`, not both. With one entry in `years`, `default_year` is
optional. Duplicate years, a `default_year` that isn't in `years`, and a
registration outside every mapped module are compile errors.

Select an event with `--year` on `run`, `list`, `bench`, `profile`, `input`, and
`generate`, or every event with `--year all` on all of them except `generate`:

```sh
cargo aoc-harness run --year 2019 --day 1
cargo aoc-harness input --year 2019 --day all
cargo aoc-harness generate --year 2019 --day 2
cargo aoc-harness run --year all --day all
```

Without `--year`, commands use the only registered year or `default_year`; with
several years and no default, they list the registered years and ask for one.
Plain `list` shows every year, ordered by year, and `list --year` narrows it.
`--year all` covers each registered year in order, keeping the usual day
default: `run --year all` runs each year's latest day, and `--day all` adds the
rest. Years that don't register a requested day are skipped; a day that no year
registers is an error. `input --year all` also skips years where no selected day
has been released.
An unregistered year is rejected before credentials, downloads, or source
writes. `generate --year 2019` writes `src/y2019/day02.rs`, declares it in the
`y2019` module (`src/y2019/mod.rs` or `src/y2019.rs`), and follows that
module's padded or unpadded naming. `@YEAR@` in a day template expands to the
selected year.

In tests inside a year module, `aoc_harness::input!(1)` reads that module's
year, so `input!(1)` in `src/y2019/day01.rs` reads `input/2019/day1.txt`. This
requires `aoc-harness` 0.5.1 or later. Outside every mapped module, such as a
test at the crate root, `input!(day)` doesn't compile, even with `default_year`;
name the year with `aoc_harness::input!(2019, 1)` instead. Naming a year that
`years` doesn't register fails the test.

Each year keeps its own benchmark reports, baselines, and profiles under a
`<year>/` subdirectory, so `bench --year 2019 --save-baseline before` and the
same command for 2025 never overwrite each other, and `--baseline before` only
finds the selected year's measurements. Benchmark names are unchanged.
Single-year crates keep their existing paths and saved baselines. An explicit
profile `--output` path is used as given, so choose a distinct path per year;
with `--year all`, it names a directory that gets one subdirectory per year.

`--year` needs an `aoc-harness` runtime of 0.5.0 or later, and `--year all` needs
0.6.0 or later; with an older runtime, the CLI stops before passing the flag through and
[names the upgrade](#keeping-the-cli-and-runtime-in-sync). Standalone
binaries using `aoc_main! { year = ... }` support one event year; map several
years in a library with `aoc_lib!` and run it with `aoc_main! { lib = ... }`.

To combine existing `advent-of-code-YYYY` crates and a shared utilities crate
into one, follow the [multi-year migration guide](docs/multi-year-migration.md).

### Generator overrides

Generators can apply to a whole day, one part, a named implementation of one
part, or a named implementation of either part. The most specific registration
wins. Each line below is a separate registration. For a solver
registered as `#[aoc(day1, part2, sorted)]`, the harness tries them in this
order:

```rust,ignore
#[aoc_generator(day1, part2, sorted)] // part 2's sorted implementation
#[aoc_generator(day1, _, sorted)]     // the sorted implementation of either part
#[aoc_generator(day1, part2)]         // every part 2 implementation
#[aoc_generator(day1)]                // every implementation of the day
```

A name match outranks a part match. A named generator applies only to
implementations with that name, so unnamed implementations never use one. `_`
is for generators only and must be followed by a name, and it needs
`aoc-harness-macros` 0.3.3 or later; in an existing project, run
`cargo update -p aoc-harness-macros` to get it. Without a matching
generator, solvers that take `&str` or `&[u8]` receive the raw input.

To share one generator between implementations with different names, name its
registration with `generator = ...` on each solver:

```rust,ignore
#[aoc_generator(day1, part1, alpha)]
fn alpha(input: &str) -> Alpha { /* ... */ }

#[aoc(day1, part1, alpha_1, generator = alpha)]
fn part1_alpha_1(input: &Alpha) -> u64 { /* ... */ }

#[aoc(day1, part1, alpha_2, generator = alpha)]
fn part1_alpha_2(input: &Alpha) -> u64 { /* ... */ }
```

`generator = alpha` refers to the registration name, not the Rust function name.
It replaces the solver's own name when choosing a generator and tries
`(day1, part1, alpha)`, then `(day1, _, alpha)`. It never falls back to an
unnamed generator or the raw input: without a match, compilation fails.
Naming the solver's own registration, as in
`#[aoc(day1, part1, alpha, generator = alpha)]`, requires that generator to
exist. A solver can't name a different generator while one is registered under
its own name; compilation fails so the solver's own generator isn't silently
skipped. `generator = ...` needs `aoc-harness-macros` 0.3.3 or later.

Stacking registrations on one function also shares it:

```rust,ignore
#[aoc_generator(day1, part1, sorted)]
#[aoc_generator(day1, part2, heap)]
fn sorted_numbers(input: &str) -> Vec<u32> { /* ... */ }
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

### Keeping the CLI and runtime in sync

`aoc-harness-cli` and your project's `aoc-harness` dependency are released
separately. Before a command uses a runtime feature, the CLI asks the compiled
runtime what it supports and says which side to upgrade:

- **Runtime too old.** The error names the feature, the first `aoc-harness`
  release with it, your manifest, and the version it uses, followed by a fix.
  For a crates.io dependency, the fix is
  `cargo add aoc-harness@<version> --manifest-path <path>`, which crosses a
  breaking 0.x release that `cargo update` won't. For a renamed,
  workspace-inherited, path, or git dependency, it names the declaration or
  source to update.
- **CLI too old.** If the runtime no longer speaks the CLI's protocol, the error
  names the `aoc-harness-cli` release it needs. If it supports features the CLI
  doesn't recognize, commands still run and print a note. Either way, run
  `cargo install aoc-harness-cli --locked` to get the latest CLI.

Runtimes before `aoc-harness` 0.6.0 spell `--day all` and `--solution all` as
`--all-days` and `--all-solutions`; the CLI translates for them, except for
`--day all` with `--part`, which needs 0.6.0. When you upgrade `aoc-harness`
across a 0.x release, upgrade the CLI too.

## Further documentation

Use `cargo aoc-harness --help` or a command's `--help` for its options.
Cargo integration supports `--manifest-path`, `--package`, `--features`,
`--profile`, `--target-dir`, `--offline`, and `--locked`. `--package` is required
when workspace package selection is ambiguous. `--profile` selects the Cargo
build profile; `profile` is the profiling subcommand.

- [Complete example](fixtures/library): a runnable library with synthetic inputs,
  ownership variants, and feature-dependent registrations.
- [Multi-year migration guide](docs/multi-year-migration.md): consolidate
  per-year crates and shared utilities into one crate.
- [Release guide](RELEASE.md): validation and publication instructions.

Licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
