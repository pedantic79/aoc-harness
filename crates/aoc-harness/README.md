# aoc-harness

Typed Advent of Code generators, solvers, executable entry points, and automatic
Criterion benchmarks. Inputs can be borrowed, mutated, or consumed without Clone.

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
for selection, Option/Result handling, custom Borrow projections, and benchmarks.

Licensed under MIT OR Apache-2.0. Publication is separate from development;
checkout installation is documented in the repository until a release exists.
