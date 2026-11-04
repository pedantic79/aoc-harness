# Proposed harness API

Status: design for review. The existing harness and solutions are unchanged.

Keep the attribute-based workflow from `cargo-aoc`, add ownership-aware solver
arguments, and generate both a runner and benchmarks from the same registrations.

## Solution API

The proposed public crate is `aoc_harness`. Existing day and part annotations keep
their spelling:

```rust
use aoc_harness::{aoc, aoc_generator};

pub struct Grid {
    pub cells: Vec<u8>,
    pub height: usize,
    pub width: usize,
}

#[aoc_generator(day4)]
pub fn generator(input: &str) -> Grid {
    // Parse the input.
    todo!()
}

#[aoc(day4, part1)]
pub fn part1(grid: &Grid) -> usize {
    // Inspect the grid.
    todo!()
}

#[aoc(day4, part2)]
pub fn part2(mut grid: Grid) -> usize {
    // Consume and modify the grid without cloning it.
    todo!()
}

#[aoc(day4, part2, flood_fill)]
pub fn part2_flood_fill(grid: &mut Grid) -> usize {
    // An alternative implementation with mutable access.
    todo!()
}
```

The function signature determines how the generated value is passed:

| Argument | Harness behavior |
| --- | --- |
| `&T` | Generate a value, then borrow it for the solver. |
| `&mut T` | Generate a fresh value, then borrow it mutably. |
| `T` | Generate a fresh value, then move it into the solver. |

Borrowed slices remain supported: a generator returning `Vec<Item>` can feed a
solver taking `&[Item]` or `&mut [Item]`. Ordinary Rust borrowing and coercion rules
apply. Ownership is inferred from the parameter; there is no `owned` annotation
and no implicit `Clone` requirement.

Functions remain ordinary Rust functions that tests can call directly. Answers
must implement `Display + 'static` (for example, numbers or owned strings), so
the runner can retain them for reporting after execution. This restriction is on
answers, not parsed input. Generators and solvers may also return
`Result<T, E>` with an error implementing `std::error::Error`. An error identifies
its day, part, variant, and phase and makes the command exit unsuccessfully.

For this repository, day 4 part 2 could take its current tuple by value and remove
the `inputs.to_vec()` inside the solver. Other parts can keep their borrowed
signatures.

## Input preparation

**Start with fresh generation for every solver invocation.** A day-level generator
is the default parser for that day's parts, rather than a cache shared between
parts. Two parts using the same parser invoke it twice. This preserves independent
inputs and gives borrowed, mutable, and consuming solvers the same preparation
policy.

Generation is measured separately from solving. Repeated generation is included
in the full-run time. The runner never clones generated data automatically.

Generator selection uses the most specific registration available:

```rust
#[aoc_generator(day4)]                    // Default for the day.
#[aoc_generator(day4, part2)]             // Override for part 2.
#[aoc_generator(day4, part2, flood_fill)] // Override for this variant.
```

These are registration forms, not three annotations on one function. A solver
taking raw `&str` or `&[u8]` needs no generator. Raw input remains alive throughout
execution, so generators can also return values borrowing from it, such as
`Vec<&str>`; the runtime must not impose a `'static` bound on parsed data.

Input defaults to `input/<year>/day<day>.txt`, relative to the solution crate's
manifest directory. Match the current harness's removal of trailing `\n`
characters; preserve other whitespace. An explicit input path can override the
default for a single-day run.

## Multiple solutions and defaults

Each solution has an identity `(day, part, name)`. The unnamed implementation is
displayed as `default`; a third attribute argument registers a named alternative.

Normal execution selects one implementation per part, using these rules:

1. Use the implementation explicitly marked `default`, if present:
   `#[aoc(day4, part2, flood_fill, default)]`.
2. Otherwise, use the unnamed implementation.
3. Otherwise, use the sole registered implementation.
4. If several named implementations remain, report a compile-time error asking
   for a default.

Duplicate identities and multiple explicit defaults are compile-time errors.
`--solution <name>` selects a variant for a specified day and part.
`--all-solutions` runs every registered implementation in the selected scope.
Its total is labeled as a comparison run, so adding alternatives does not silently
change the default full-year total. The runner does not pick a default based on
which variant happened to benchmark fastest.

## Cargo subcommand and timing

Keep the existing library declaration as the consumer's entry point:

```rust
// src/lib.rs, after the day modules
aoc_harness::aoc_lib! { year = 2025 }
```

The CLI package supplies the `cargo-aoc-harness` executable, invoked through Cargo's subcommand convention:

```sh
cargo aoc-harness run
cargo aoc-harness run --day 4 --part 2
cargo aoc-harness run --day 4 --part 2 --solution flood_fill
cargo aoc-harness run --day 4 --all-solutions
```

Run selected solutions sequentially in day/part/name order. Each result reports
the answer, generator duration, and solver duration. The footer reports:

- **Generator total:** sum of every generator invocation actually executed.
- **Solver total:** sum of every solver invocation actually executed.
- **Full run:** one elapsed measurement around the entire selected execution,
  including generation, solving, cleanup, and runtime dispatch.

Load input files before the full-run timer and format/print results after it.
Consequently, full-run time measures execution on in-memory raw input, and can
differ from the sum of phase times because it includes cleanup and dispatch.
Compilation, file I/O, and terminal output are outside that measurement. For
filtered runs, label the footer with the selected scope. Errors produce a partial
run report, never a successful full-year total.

## Automatic benchmarks

A single benchmark entry point registers everything; adding a new annotated
solution requires no individual benchmark code:

```rust
// benches/aoc.rs
aoc_harness::aoc_bench! { lib = advent_of_code_2025 }
```

```toml
[[bench]]
name = "aoc"
harness = false
```

Use Criterion behind an optional benchmark feature, enabled as a development
dependency. Generate these benchmark families:

| Benchmark | Timed work |
| --- | --- |
| Generator | One parser call per iteration; one benchmark per distinct generator registration. |
| Solver | One solver call; input preparation happens outside the timed region. |
| End to end | Generate fresh input and solve for one implementation. |
| Full run | Execute the complete default selection, generating fresh input for each solver. |

Benchmark every solution variant individually. The full-run benchmark uses the
same default selection and execution order as the runner. It measures a complete
pass directly, rather than adding individual benchmark estimates.

For solver-only benchmarks, immutable solvers may reuse parsed input, provided
they do not change it through interior mutability. Mutable and consuming solvers
receive newly generated input on every iteration through Criterion's batched
setup. They must never receive the previous iteration's modified data. An opt-in
`bench_input = "fresh"` setting covers solvers that mutate through `&T`:

```rust
#[aoc(day4, part2, cached, bench_input = "fresh")]
```

Use bounded batches, black-box inputs and outputs, and exclude answer formatting
from timings. Destruction performed inside a consuming solver is part of that
call's cost; external setup cleanup is not solver work. End-to-end and full-run
benchmarks include cleanup, and the full-run benchmark avoids nested per-phase
timers. Validate generation and execution once before measurement, and abort a
benchmark on an error.

```sh
cargo aoc-harness bench
cargo aoc-harness bench --day 4
cargo aoc-harness bench --day 4 --part 2 --solution flood_fill
```

Criterion's [timing-loop guide](https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html)
and [Bencher API](https://docs.rs/criterion/latest/criterion/struct.Bencher.html)
describe the setup/measurement separation and ownership-sensitive timing loops
this design would use.

## Implementation shape and validation

Use a small runtime crate plus a proc-macro crate, with the public crate
re-exporting the macros. Both the runner and benchmarks consume the same registry
and selection rules. Generated adapters keep parsing and invocation typed, so
the core API does not require `Any`, parsed-data downcasts, or `Clone` bounds.

Before migrating all days, validate:

- A non-`Clone` generated type works with borrowed, mutable, and consuming parts.
- Mutation and consumption cannot affect a later part or benchmark iteration.
- Slice coercions and parsed values borrowing the raw input compile and run.
- Named alternatives and generator overrides resolve deterministically.
- Full-run selection includes each selected part exactly once and charges all
  of its input preparation.
- Existing sample and regression answers remain unchanged, including day 4
  after removing its internal input copy.
