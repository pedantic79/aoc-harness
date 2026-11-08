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
aoc-harness = "0.3.0"
```

```rust,ignore
use aoc_harness::{aoc, aoc_generator, aoc_lib};

#[aoc_generator(day1)]
fn parse(raw: &str) -> Vec<&str> { raw.lines().collect() }

#[aoc(day1, part1)]
fn part1(lines: &[&str]) -> usize { lines.len() }

aoc_lib! { year = 2025 }
```

Use `aoc_harness::aoc_main! { lib = your_library }` in a binary, or finish a
standalone binary with `aoc_harness::aoc_main! { year = 2025 }`.

Select multiple days with `--day 1,3` or `--day 1 --day 3`; `-d` is the short form
of `--day`. Select a part with `--part 1` or `-p 1`. Use `--all-days` to select
every registered day.

`cargo aoc-harness bench` benchmarks solvers by default. Select any combination
of `generator`, `solver`, `end-to-end`, and `full-run` with `--family`, for example
`cargo aoc-harness bench --family generator,solver`. Comma-separated values and
repeated `--family` flags work with measured benchmarks and `--smoke`.
Profiling uses the same `--family` and `--filter` selection, defaults to
`end-to-end`, and records each selected workload separately.

See the [repository documentation](https://github.com/pedantic79/aoc-harness)
for selection, Option/Result handling, custom Borrow projections, benchmarks,
and external profiling through the same typed workload adapters.

Licensed under MIT OR Apache-2.0.
