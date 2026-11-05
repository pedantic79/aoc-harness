# Proposed harness API

Status: implementation specification. The existing harness and solutions are
unchanged. Consumer projects need only an annotated solution library; the
Cargo subcommand generates executable and benchmark entry points.

Keep the attribute-based workflow from `cargo-aoc`, add ownership-aware solver
arguments, and generate both a runner and benchmarks from the same registrations.
Deliver the runtime, macros, CLI, and benchmarks together in the first usable
version.

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
and no implicit `Clone` requirement. `input = "borrow"` explicitly selects
`Borrow<T>` / `BorrowMut<T>` on a generated owner for reference arguments,
including unsized targets and borrowed lifetimes. It requires a generator;
ordinary deref/slice coercions remain the default. Shared projections with
interior mutation can request `bench_input = "fresh"`.

Functions remain ordinary Rust functions that tests can call directly. Answers
must implement `Display + 'static` (for example, numbers or owned strings), so
the runner can retain them for reporting after execution. This restriction is on
answers, not parsed input. Generators and solvers may also return
`Result<T, E>` with an error implementing `std::error::Error`, or `Option<T>`.
The harness unwraps exactly one outer wrapper; `None` is an ordinary phase error.
Use `output = "value"` on either annotation to retain wrappers as ordinary values;
existing deliberately optional parsed values need this migration annotation.
Recognition uses the final path segment `Option` or `Result`, without inferring
arbitrary aliases. An error identifies
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
manifest directory. Also accept zero-padded day filenames such as `day01.txt`,
preferring `day1.txt` when both exist. Match the current harness's removal of
trailing `\n` characters; preserve other whitespace. An explicit input path can
override the default for a single-day run.

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

The library declaration supplies the registry used by the Cargo subcommand:

```rust
// src/lib.rs, after the day modules
aoc_harness::aoc_lib! { year = 2025 }
```

The CLI package supplies the `cargo-aoc-harness` executable, invoked through
Cargo's subcommand convention:

```sh
cargo aoc-harness run
cargo aoc-harness run --days 1,4
cargo aoc-harness run --all-days
cargo aoc-harness run --day 4 --part 2
cargo aoc-harness run --day 4 --part 2 --solution flood_fill
cargo aoc-harness run --day 4 --all-solutions
cargo aoc-harness list
cargo aoc-harness bench
cargo aoc-harness bench --day 4 --part 2
cargo aoc-harness run --day 4
```

`run`, including when the command is omitted, and `bench` select the highest
registered day by default. Select multiple days with `--day 1,4` (alias `--days`)
or repeated `--day` flags. `--all-days` restores the full-year selection and
conflicts with `--day`. Repeated days execute only once, in day order. An input
override requires exactly one selected day. `list` selects all days by default
and accepts the same day filters. Normal runs and benchmarks use release
optimization by default.

Discover the consumer package with `cargo metadata --format-version 1`. Support
`--manifest-path`, `--package`, Cargo features, and an explicit build profile;
require package selection when a workspace root is ambiguous. Read library target
names from Cargo metadata, including custom library names and renamed packages.

Generate a small bridge crate under the Cargo target directory, link the consumer
library, and call its exported harness entry points. Use the registered metadata
from compiled code to list and select solutions. The consumer writes no
`main.rs`, benchmark source, or `[[bench]]` target. Keep the consumer manifest and
source files unchanged during ordinary execution; only explicit generation edits
source. The [parity design](design/cargo-aoc-feature-parity.md) specifies input
acquisition, scaffolding, and the versioned metadata-only execution plan.

The generated bridge must use the consumer library's runtime API, avoiding
incompatible duplicate runtime versions. Give it its own workspace boundary and
respect the consumer's build configuration, target directory, features, and
requested profile. Read Cargo's JSON build messages to locate the executable.
Let Cargo check dependency freshness on each invocation so changed solutions
cannot silently run a stale bridge. Forward diagnostics and child-process exit
status, and preserve arguments as process arguments rather than shell text.

Cargo documents the metadata, build-message, and subcommand mechanisms in its
[external-tools guide](https://doc.rust-lang.org/cargo/reference/external-tools.html).

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

`cargo aoc-harness bench` builds and runs a generated benchmark entry point from the
same library registrations used by `run` and `list`. Benchmark support is part of
the runtime's default feature set, so the default consumer setup needs no
benchmark-specific development dependencies or feature declarations. Use
Criterion and generate these benchmark families:

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

Expose benchmark filtering, sample size, warmup time, and measurement time, with
an explicit smoke-test mode for automated verification. Print the report location
after a successful benchmark run. Input paths remain relative to the consumer
manifest, including when the generated bridge is elsewhere.

Criterion's [timing-loop guide](https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html)
and [Bencher API](https://docs.rs/criterion/latest/criterion/struct.Bencher.html)
describe the setup/measurement separation and ownership-sensitive timing loops
this design would use.

## Implementation shape and validation

Use three workspace crates: `aoc-harness` for the runtime, `aoc-harness-macros`
for proc macros, and `aoc-harness-cli` for the Cargo executable.
The public runtime crate re-exports the macros. Run, list, and bench consume the
same registry and selection rules. Generated adapters keep parsing and
invocation typed, so the core API does not require `Any`, parsed-data downcasts,
or `Clone` bounds.

Before migrating all days, validate:

- A non-`Clone` generated type works with borrowed, mutable, and consuming parts.
- Mutation and consumption cannot affect a later part or benchmark iteration.
- Slice coercions and parsed values borrowing the raw input compile and run.
- Named alternatives and generator overrides resolve deterministically.
- Full-run selection includes each selected part exactly once and charges all
  of its input preparation.
- Existing sample and regression answers remain unchanged, including day 4
  after removing its internal input copy.
- A library-only consumer runs, lists variants, and benchmarks through both CLI
  invocation forms without user-written runner or benchmark entry points.
- CLI execution preserves consumer source and manifest files, handles paths with
  spaces and custom library names, and resolves packages in a Cargo workspace.
- Editing a consumer solution changes subsequent CLI output; generator failures,
  solver failures, unknown selections, missing inputs, and Cargo build failures
  return nonzero exit status with an actionable explanation.


## Additional entry points and CLI operations

`aoc_main! { lib = crate_path }` forwards a consumer-owned binary to a library
registry. `aoc_main! { year = 2025 }` declares a standalone binary registry;
Cargo target identity determines its source, with `source = "relative/path.rs"`
as an explicit fallback. Both use the same runtime options, input loading,
selection, benchmarks, source tracking, and compile-time validation as `aoc_lib!`.
Direct executables do not download. The Cargo subcommand remains library-only.

The CLI also implements `credentials status`, `input`, and `generate`, with
explicit `--download` composition on run/bench and `--generate` composition on
input/run. Sessions come from an explicit file, nonblank
`ADVENT_OF_CODE_SESSION`, or the shared `$HOME/.adventofcode.session`; status
and acquisitions never mutate or print a token. Help/version work without Cargo.
`bench --open` opens a successful measured report, and `--debug-info` enables
symbols without changing the selected profile's optimization. See the README
for commands, migration details, calendars, and storage guarantees.
