# aoc-harness

A Rust helper for [Advent of Code](https://adventofcode.com), inspired by
[cargo-aoc](https://github.com/gobanos/cargo-aoc). Annotate your generators and
solvers, then run, benchmark, and profile them from Cargo.

Your solutions live in a library. The harness generates the runner and benchmark
entry points, so you can focus on the puzzles.

## Features

- Download puzzle inputs and generate day modules.
- Run solutions with separate generator, solver, and full-run timings.
- Compare named implementations with automatic Criterion benchmarks.
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

Add the [runtime](https://crates.io/crates/aoc-harness) to `Cargo.toml`. It
re-exports all the macros you need.

```toml
[dependencies]
aoc-harness = "0.2"
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
cargo aoc-harness input --year 2025 --day 1
cargo aoc-harness input --year 2025 --all
```

`credentials status` validates the local token format without printing the token
or contacting Advent of Code. You can also use `ADVENT_OF_CODE_SESSION` or
`--session-file PATH`; an explicit file takes precedence over the environment,
then the home file is used.

Inputs are stored in `input/{year}/day{day}.txt` relative to your solution
project's `Cargo.toml`. Existing inputs are preserved. `input` downloads without
compiling your solutions, so it also works while a solver is unfinished.
Keep personal inputs out of Git.

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
cargo aoc-harness list
```

`cargo aoc-harness run` runs the latest registered day by default.
`cargo aoc-harness` is a shortcut for `run`, with the same flags and selection
rules.

`run` and `bench` build in release mode by default; `list` shows every registered
day and marks default implementations. As you add days, use `--days 1,3` to
select several at once. Use `--input PATH` with a single `--day` to try an
alternate input file.

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

Downloads happen only when requested. Add `--download` to `run`, `bench`, or
`profile` to fetch missing inputs before execution:

```sh
cargo aoc-harness run --day 1 --download
```

## Starting another day

Generate a module with a parser, both solver stubs, and ignored example tests:

```sh
cargo aoc-harness generate --day 2 --dry-run
cargo aoc-harness generate --day 2
```

Generation adds `src/day2.rs` and its module declaration in `src/lib.rs`.
It works offline, preserves existing day modules, and can be combined with input
acquisition using `cargo aoc-harness input --year 2025 --day 2 --generate`.
Implement the generated stubs before running the new day.

## Benchmarking solutions

Benchmarks use [Criterion](https://bheisler.github.io/criterion.rs/book/) and
require no extra dependencies or benchmark targets in your solution project.

```sh
cargo aoc-harness bench
cargo aoc-harness bench --day 1 --open
cargo aoc-harness bench --day 1 --part 2 --solution sorted
cargo aoc-harness bench --all-days
```

| Family | Measured work |
| --- | --- |
| Generator | Parse input once. |
| Solver | Run one solver, excluding input preparation and external cleanup. |
| End to end | Generate input, solve, and clean up for one implementation. |
| Full run | Execute the selected default implementations, including parsing and cleanup. |

Mutable and consuming solver benchmarks receive fresh input for each iteration.
Immutable solvers reuse prepared input; add `bench_input = "fresh"` to the
`#[aoc(...)]` annotation if a solver mutates through a shared reference.

Filter by benchmark name or adjust measurement settings:

```sh
cargo aoc-harness bench --day 1 --filter '^solver/' --sample-size 20 \
  --warm-up-time 1 --measurement-time 3
cargo aoc-harness bench --day 1 --smoke
```

`--smoke` checks the selected benchmark families without statistical measurement.
Measured reports go to `<target-dir>/criterion/<your-package>/`; `--open` opens
the HTML report after a successful run.

## Profiling solutions

Install an external profiler, then record one workload:

```sh
cargo install --locked samply
cargo aoc-harness profile --day 1 --part 2
cargo aoc-harness profile --day 1 --part 2 --family solver --duration 30
cargo aoc-harness profile --all-days --family full-run
```

Profiling defaults to Samply and approximately 10 seconds of end-to-end work.
Select a single workload with `--day`, `--part`, and optionally `--solution`, or
choose `--family full-run` for a complete pass. Debug symbols are enabled
automatically while preserving the selected Cargo profile's optimization.

Use `--profiler flamegraph`, `--profiler perf` on Linux, or `--profiler xctrace`
on macOS for other recorders, installed separately. Recordings go under
`<target-dir>/aoc-harness/profiles/<your-package>/`; `--output PATH` selects a new
output path. The CLI prints how to open the recording.

| Profiler | Recording | Open with |
| --- | --- | --- |
| Samply | `profile.json.gz` | `samply load PATH` |
| Flamegraph | `flamegraph.svg` | A browser |
| Linux perf | `perf.data` | `perf report -i PATH` |
| macOS xctrace | `profile.trace` | Instruments |

Flamegraph also needs its platform's collector: `perf` on Linux or full Xcode's
`xctrace` on macOS. For xctrace, `--template` selects the recording template;
the default is `Time Profiler`.

Profiling supports the generator, solver, end-to-end, and full-run families.
Use `--filter` to narrow a family to exactly one workload. Builds, downloads, and
preflight validation finish before recording starts. Recordings include process
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
