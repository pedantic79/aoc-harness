# aoc-harness

Typed Advent of Code generators, solvers, executable entry points, and automatic
Criterion benchmarks. Inputs can be borrowed, mutated, or consumed without Clone.

Add the runtime to your `Cargo.toml`; it re-exports the macros:

```toml
[dependencies]
aoc-harness = "0.1"
```

```rust,ignore
use aoc_harness::{aoc, aoc_generator, aoc_lib};

#[aoc_generator(day1)]
fn parse(raw: &str) -> Vec<&str> { raw.lines().collect() }

#[aoc(day1, part1)]
fn part1(lines: &[&str]) -> usize { lines.len() }

aoc_lib! { year = 2025 }
```

Use `aoc_main! { lib = your_library }` in a binary, or finish a standalone binary
with `aoc_main! { year = 2025 }`. The Cargo subcommand is `aoc-harness-cli`.
See the [repository documentation](https://github.com/pedantic79/aoc-harness)
for selection, Option/Result handling, custom Borrow projections, benchmarks,
and external profiling through the same typed workload adapters.

Licensed under MIT OR Apache-2.0.
