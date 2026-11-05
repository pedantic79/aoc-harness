# aoc-harness

An ownership-aware Rust Advent of Code harness. Annotate a solution **library**;
the CLI builds its runner and Criterion benchmarks automatically. Consumer-owned
executables are also supported with `aoc_main!`. Parsed input
can be borrowed, mutated, or consumed, and never needs to implement `Clone` or
`'static`.

## Install and try the complete example

From this checkout:

```sh
export CARGO_TARGET_DIR="$PWD/target"
cargo install --path crates/aoc-harness-cli --root "$PWD/target/install" --locked
export PATH="$PWD/target/install/bin:$PATH"

cargo aoc-harness list --manifest-path fixtures/library/Cargo.toml
cargo aoc-harness run --manifest-path fixtures/library/Cargo.toml
cargo aoc-harness run --manifest-path fixtures/library/Cargo.toml --all-days
cargo aoc-harness run --manifest-path fixtures/library/Cargo.toml --day 1,3
cargo aoc-harness run --manifest-path fixtures/library/Cargo.toml --day 1
cargo aoc-harness run --manifest-path fixtures/library/Cargo.toml --day 1 --part 2 --solution mutable
cargo aoc-harness run --manifest-path fixtures/library/Cargo.toml --day 1 --all-solutions
cargo aoc-harness bench --manifest-path fixtures/library/Cargo.toml --smoke
cargo aoc-harness bench --manifest-path fixtures/library/Cargo.toml --day 1 \
  --sample-size 10 --warm-up-time 0.05 --measurement-time 0.1
```

`fixtures/library` contains synthetic inputs only. Its custom library target is
`holiday_solutions`, while its package is `winter-library`. Day 1 demonstrates a
non-`Clone` struct, consuming and mutable variants, explicit defaults, three
generator scopes, and interior mutation with fresh benchmark input. Day 2 uses
raw text and bytes. Day 3 returns `Vec<&str>` and supplies borrowed and mutable
slices. `--features bonus` enables another solution and a conditional generator
override. Neither executable source nor benchmark source belongs in the consumer.

The example's latest-day answers are `2`, `2`; `--all-days` produces
`6`, `12`, `5`, `5`, `2`, `2`. The named mutable variant returns `306`; its
generator is more specific than the part override.

Run output follows cargo-aoc's familiar layout. For example, `run --day 1`
produces the following structure (timings vary):

```text
AOC 2025

Day 1 - Part 1: 6
        generator: 8µs,
        runner: 150ns

Day 1 - Part 2 - owned: 12
        generator: 7µs,
        runner: 150ns

Full run (2025 day 1, 2 selected):
        generator: 15µs,
        runner: 300ns,
        total: 16µs
```

`runner` means the solver call. The footer's generator and runner values are
totals; `total` is the elapsed full run. Unnamed solutions omit the variant label,
and named solutions retain their names even when selected as defaults.

## Create a consumer

Create a library with `cargo new --lib my-solutions`, and add the runtime as a
normal dependency, using the absolute path to this checkout:

```toml
[dependencies]
aoc-harness = { path = "/path/to/aoc-harness/crates/aoc-harness" }
```

Put solutions in `src/day01.rs`:

```rust
use aoc_harness::{aoc, aoc_generator};

pub struct Numbers(Vec<u64>); // No Clone required.

#[aoc_generator(day1)]
fn parse(raw: &str) -> Result<Numbers, std::num::ParseIntError> {
    let numbers = raw.split_whitespace().map(str::parse).collect::<Result<_, _>>()?;
    Ok(Numbers(numbers))
}

#[aoc(day1, part1)]
fn borrowed(numbers: &Numbers) -> u64 {
    numbers.0.iter().sum()
}

#[aoc(day1, part2)]
fn consumed(numbers: Numbers) -> u64 {
    numbers.0.into_iter().sum()
}

#[aoc(day1, part2, sorted)]
fn mutable(numbers: &mut Numbers) -> u64 {
    numbers.0.sort_unstable();
    numbers.0.iter().sum()
}
```

Declare the module and the year in `src/lib.rs`:

```rust
mod day01;

aoc_harness::aoc_lib! { year = 2025 }
```

Add `input/2025/day1.txt`, then run `cargo aoc-harness run` from the consumer directory.
Omitting `run` has the same behavior. Inputs are relative to the **consumer
manifest**, even though the executable lives elsewhere. Zero-padded names such as
`day01.txt` are also supported; `day1.txt` takes precedence when both exist.
Trailing `\n` characters are removed; spaces, carriage returns, and other
whitespace are preserved.
`--input PATH` overrides the file for exactly one selected `--day`; an explicit
relative path is relative to the invocation directory.

Functions remain callable directly by ordinary Rust tests. Answers must implement
`Display + 'static`. Generators and solvers may return `Option<T>` or
`Result<T, E>` where `E`
implements `std::error::Error`; error text identifies the solution and phase, and
the command returns a failure status. Error types may borrow from the input.

## Registrations and selection

Generator precedence is variant, then part, then day:

```rust,ignore
#[aoc_generator(day4)]
#[aoc_generator(day4, part2)]
#[aoc_generator(day4, part2, flood_fill)]
```

These annotate separate functions. Without a generator, `&str` and `&[u8]`
solvers receive the raw input. Generators can also accept either raw form.
`Vec<T>` to `&[T]` / `&mut [T]` uses ordinary Rust coercion. Every invocation
generates a fresh value, including two parts using the same day parser.

`run` selects exactly one implementation per day/part: first an explicit
`#[aoc(day4, part2, flood_fill, default)]`, otherwise the unnamed implementation,
otherwise the only registered implementation. Several named alternatives require
an explicit default. Duplicate identities, duplicate generators, and conflicting
defaults fail compilation. `list` shows all variants and marks the selected
defaults. Solutions execute sequentially in day/part/name order.

`run` and `bench` default to the highest registered day, including registrations
enabled by features. Use `--day 1,3` (also `--days 1,3`) or repeat
`--day 1 --day 3` to select multiple days. Repeated days run once, in day order.
`--all-days` selects every registered day and cannot be combined with `--day`.
`list` continues to select every day by default and accepts the same explicit day
options.

```sh
cargo aoc-harness run --all-days
cargo aoc-harness run --days 1,3
cargo aoc-harness run --day 4 --part 2 --solution flood_fill
cargo aoc-harness run --day 4 --all-solutions
cargo aoc-harness list --day 4
```

Comparison totals are labeled **Comparison run**. Filtered footers identify their
scope. A failed solver or generator stops the run, reports completed answers and
partial timings, and never prints a successful full-run total.

## Cargo integration

Use `cargo aoc-harness --help` for the command overview. `cargo aoc-harness run --help`,
`list --help`, and `bench --help` each show their own options and examples. Help
works outside a consumer project without invoking Cargo or building a bridge.

`cargo aoc-harness` supports:

```sh
cargo aoc-harness run --manifest-path ../solutions/Cargo.toml --package my-solutions
cargo aoc-harness run --features fast,simd --no-default-features
cargo aoc-harness list --all-features --profile dev
cargo aoc-harness run --profile my-custom-profile --target-dir "build directory"
```

The default profile is `release`. `--debug-info` on run/bench requests full debug
symbols and disables stripping for the selected profile and its package/build
overrides, without changing optimization or consumer files. `--profile NAME`
continues to select a Cargo profile. `--version` / `-V` works outside a project;
`-d day4` and `--part part2` are selector conveniences. `-p` still selects a Cargo
package. The CLI uses `cargo metadata --format-version 1`
and requires `--package` when the workspace's default package selection is
ambiguous. Custom library names and source paths, renamed runtime dependencies,
and workspace-inherited runtime dependencies work. Features apply to the selected
consumer. `--features package/feature` is also accepted for that package.

Bridge source, manifests, and lockfiles are generated below
`<target-dir>/aoc-harness/`. The bridge has its own workspace and depends only on
the consumer, so it uses that library's runtime version. Cargo configuration,
target directory, target, requested features, workspace profiles, patches, and
replacements are carried into the build. Explicit `--manifest-path` discovery
loads configuration from that manifest's directory. `--config`, `--offline`, and
`--locked` are available; locked builds require an up-to-date consumer lockfile.
The bridge seeds resolution from that lockfile without changing it.

Run, list, and bench invoke Cargo to check freshness. Executables are
located using Cargo JSON artifact messages; paths and child arguments never pass
through a shell. Cargo diagnostics and nonzero child exit codes are propagated.
Ordinary execution preserves the consumer's sources, manifest, and lockfile.
Only explicit generation commands edit source files.

## Acquire inputs and start a day

```sh
cargo aoc-harness credentials status
cargo aoc-harness input --year 2025 --day 4
cargo aoc-harness input --year 2025 --all
cargo aoc-harness generate --day 4 --dry-run
cargo aoc-harness generate --day 4
cargo aoc-harness input --year 2025 --day 4 --generate
cargo aoc-harness run --day 4 --generate --download
cargo aoc-harness bench --day 4 --download --open
```

Downloads are explicit: `input` fetches inputs, and `run`/`bench` fetch missing
inputs only with `--download`. Run/bench select their year and days from compiled
registrations using a versioned input plan, including feature-dependent latest
days. All acquisitions finish before generators, solvers, validation, or timers
start. An older runtime can still run normally; `--download` asks for an upgrade
when it cannot describe its input plan. Explicit `input` needs Cargo metadata
but never compiles solutions, so broken or unimplemented solvers do not block it.

The CLI reads the user-managed `$HOME/.adventofcode.session` file shared with
[aoc-cli](https://github.com/scarvalhojr/aoc-cli). Put only the raw session token
in that file and use owner-only permissions (`chmod 600` on Unix). The harness
never creates, modifies, or prints it. `credentials status` reports the selected
source and local format validation; it does not check expiration with AoC.

Resolution order is an explicit `--session-file PATH`, then nonblank
`ADVENT_OF_CODE_SESSION`, then the home file. Relative explicit paths are relative
to the invocation directory. Whitespace around the token is trimmed; the remaining
value must be nonempty ASCII hexadecimal text, with no fixed length. A malformed
selected source is an error. There are no project-local, TOML, legacy, or config
folder credential fallbacks. `--session-file` on run/bench requires `--download`.
Fully cached acquisitions do not load credentials or initialize HTTP.

Input paths use the selected manifest directory and prefer `dayN.txt` over
`dayNN.txt`, even for empty files. Existing files are validated and preserved;
only missing inputs are downloaded. Requests use fixed `https://adventofcode.com`
URLs, a session cookie, a project User-Agent, bounded timeouts, no redirects, and
no automatic retries. Responses must be UTF-8 text, without HTML/login content,
and at most **16 MiB**. New files preserve response bytes and terminal newlines,
and use atomic publication without overwriting a concurrent writer. Days are
processed sequentially; authentication and rate-limit errors stop the batch.
Other failures are collected. Any failed input makes the command unsuccessful,
while successful downloads remain cached. Keep personal inputs out of Git.

Explicit input downloads accept repeated/comma-separated `--day` / `--days`.
`--all` requires `--year`. The known calendars have 25 days for 2015–2024 and
12 for 2025. For a current event, only released days are selected; future inputs
are rejected locally. Unknown years require explicit days until their calendar
is verified. Dates use fixed EST (UTC-5). Without a day, `input` selects today
only inside the selected year's known December release window; otherwise supply
`--day`. `--offline` prevents both Cargo and AoC traffic and permits cached inputs.
`--input` / `-i` conflicts with `--download` and never becomes a download target.

Generation works offline and creates `dayN.rs` beside the actual library source,
including a custom `[lib].path`, then inserts `mod dayN;` before `aoc_lib!`.
The template has a parser, both solvers returning clear not-implemented errors,
and ignored example placeholders. It imports through the runtime alias exported
by `aoc_lib!`, so renamed dependencies work. Repeating generation is a no-op.
`--dry-run` prints a diff without source writes, builds, or downloads.

The scaffolder preflights the whole batch, preserves existing matching padded
modules, crate documentation, line endings, and root permissions, and refuses
ambiguous inline/cfg/path modules, mismatched files, and source symlinks. It
rechecks source contents before replacement and rolls back unchanged new files
if the root edit fails. Concurrent changes are reported for manual resolution.
Manifests and lockfiles remain untouched. `input --generate` preflights first,
fetches inputs, then applies source changes only after successful acquisition.
`run --generate` requires explicit days and applies the same plan before building;
if requested, downloading then follows the newly compiled registry selection.

## Optional returns and custom projections

Generators and solvers can return `Option<T>` as well as `Result<T, E>`.
`Some`/`Ok` supplies the value; `None` is a contextual harness error identifying
the phase, day, part, and variant. Failures preserve attempted phase timings and
stop execution. Benchmark prevalidation and measurement loops use the same
normalization. Exactly one outer wrapper is interpreted: `Result<Option<T>, E>`
produces an ordinary `Option<T>` parsed value. Successful answers still require
`Display + 'static`; parsed values can borrow input and need not implement Clone.

**Migration:** generators that deliberately return optional parsed values must
add `#[aoc_generator(day4, output = "value")]`. Both annotations accept
`output = "value"` to disable normalization. Only final path segments named
`Option` or `Result` are recognized; arbitrary aliases are not inferred.

Ordinary borrowing, slices, and deref coercions remain the default. For an owner
with a custom projection, opt into `Borrow<T>` or `BorrowMut<T>` explicitly:

```rust,ignore
#[aoc(day4, part1, input = "borrow")]
fn projected(grid: &GridView) -> usize { grid.cells.len() }

#[aoc(day4, part2, input = "borrow")]
fn projected_mut(grid: &mut GridView) -> usize { /* modify a fresh owner */ 0 }
```

The generator's owner must implement the corresponding trait. Unsized targets
and borrowed lifetimes are supported. The option requires a generator and a
reference argument. Mutable projections get a fresh owner in every invocation
and benchmark iteration. For interior mutation through a shared projection,
combine it with `bench_input = "fresh"`.

## Own an executable

A library can also supply `src/main.rs`:

```rust,ignore
aoc_harness::aoc_main! { lib = your_library }
```

Or place annotations directly in a standalone binary and finish its root with
`aoc_harness::aoc_main! { year = 2025 }`. Cargo's binary identity selects
`src/main.rs`, `src/bin/name.rs`, `src/bin/name/main.rs`, or an explicit
`[[bin]].path`. Where identity is unavailable or ambiguous, supply
`source = "relative/path.rs"` in the standalone declaration. Paths stay within
the manifest directory. Declare one entry macro at the crate root.

Both forms use the runtime's existing run/list/bench options and exit codes:
`cargo run --release -- --all-days` or
`cargo run --release -- bench --day 4 --smoke`. Inputs load at runtime from the
consumer's manifest directory; direct executables remain local-only.
The Cargo subcommand continues to require a library target. Both entry forms
share the same direct-annotation discovery boundary described below.

## Timings and automatic benchmarks

The runner loads files before timing and prints answers afterward:

| Measurement | Included work |
| --- | --- |
| Generator duration | One actual generator call, including construction of parsed input. Zero for raw-input solvers. |
| Solver duration | The typed solver call. Destruction inside a consuming solver belongs to that call. |
| Generator / solver totals | Every invocation actually performed, including repeated parsing and failed invocations. |
| Full run | One elapsed interval around the selected pass, including generation, solving, parsed-input cleanup, answer retention, and dispatch. |

Compilation, input file I/O, answer formatting, and harness output are outside the
full-run interval. Code inside a user function, including its own logging, remains
part of that function's cost. Full run is an elapsed measurement, not an addition
of phase estimates.

Criterion is included in the runtime's normal dependencies. `bench` uses the same
compiled registrations and registers:

| Family | Measurement and preparation |
| --- | --- |
| `generator/dayN/default` or `generator/dayN/partP/name` | One parser call per iteration; each distinct registration appears once. Day-level parsers use `default`; part and variant overrides include their part number. Output cleanup is outside measurement. |
| `solver/dayN/partP/name` | One solver call; preparation and external cleanup are outside measurement. |
| `end-to-end/dayN/partP/name` | Fresh generation, solving, and cleanup for that variant. |
| `full-run/YEAR[-day-N[-part-P]]` | A complete pass through the scope's default solutions, including all parsing and cleanup. No nested phase timers. |

Benchmarks include every solution variant in scope. `--solution` restricts the
individual solver/end-to-end variants; the full-run family continues to represent
the default selection within the requested day/part. Generator families include
applicable day, part, and variant registrations in that scope, including parsers
shadowed by more specific registrations.

Immutable solver benchmarks reuse one prepared value. Mutable solvers use
Criterion's `iter_batched_ref`, and consuming solvers use `iter_batched`, with
`BatchSize::PerIteration` to bound memory. Inputs and outputs are black-boxed,
and answers are never formatted in measurement loops. For mutation through an
immutable reference, request fresh input explicitly:

```rust,ignore
#[aoc(day4, part2, cached, bench_input = "fresh")]
fn cached(grid: &Grid) -> usize { /* ... */ }
```

This uses fresh batched preparation as well. End-to-end and full-run loops always
prepare fresh input. Before measuring, all required generators and solvers are
validated once; a subsequent measurement-loop error aborts the process.

```sh
cargo aoc-harness bench --day 4 --part 2 --solution flood_fill
cargo aoc-harness bench --filter '^solver/' --sample-size 20 \
  --warm-up-time 0.1 --measurement-time 0.2
cargo aoc-harness bench --all-days
cargo aoc-harness bench --smoke
```

`--filter` is a regex over family names. Sample size is at least 10; times are
positive seconds. Smoke mode executes each matching family once without
statistical measurement or report generation. It is separate from a real
shortened Criterion run, which exercises the batched measurement loops.

In a terminal, the CLI shows one updating progress line with the benchmark's name
and phase, such as `Benchmark generator/day12/default: measuring...`. Each score
prints as soon as it is available, with the progress line cleared first so
progress messages do not remain between scores. Redirected output contains one
`Running benchmarks...` notice and streamed results, without Criterion's
per-benchmark warmup, sampling, and analysis messages. Warnings and errors are
forwarded immediately. If a benchmark fails, completed output remains visible,
the output is marked incomplete, and the command exits unsuccessfully. Smoke
mode continues to list the families it checks.

Benchmark results use Criterion's colors when stdout is a terminal. Redirected
output stays plain by default.
Use `--color always` to preserve colors through a pipe or `--color never` to
disable them. Automatic color respects `NO_COLOR` and `TERM=dumb`; an explicit
color choice takes precedence.

Reports go to `<target-dir>/criterion/<consumer-package>/`, printed after a
successful measurement. `bench --open` / `-o` opens that successful report; it
conflicts with `--smoke` and never opens after a failed benchmark. An unavailable
report or opener failure returns nonzero and distinguishes it from measurement
failure. Open `report/index.html` for the combined HTML report;
individual benchmark directories contain `new/estimates.json`, samples, and
plots. Estimates are machine- and build-dependent; use release mode for useful
comparisons. Bounded batching adds overhead for very short solver calls.

A local release validation on 2026-11-01 used 10 samples, 10 ms warmup, and
20 ms measurement time. Representative mean estimates from the synthetic
fixture were:

| Benchmark | Mean estimate |
| --- | --- |
| `generator/day1/default` | 32 ns |
| `solver/day1/part2/owned` | 17 ns |
| `full-run/2025-day-1` | 47 ns |

The [generated HTML report](target/criterion/winter-library/report/index.html)
is available after local validation. These short runs verify the measurement
pipeline; longer runs are appropriate for performance decisions.

The implementation follows Cargo's [external tools protocol](https://doc.rust-lang.org/cargo/reference/external-tools.html)
and Criterion's [timing-loop guidance](https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html).

## Validation

```sh
export CARGO_TARGET_DIR="$PWD/target"
cargo fmt --all --check
cargo fmt --manifest-path fixtures/library/Cargo.toml --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --manifest-path fixtures/library/Cargo.toml --lib
cargo test --manifest-path fixtures/parity/Cargo.toml
python3 scripts/validate_packages.py
python3 scripts/validate_2025.py --cli target/install/bin/cargo-aoc-harness
```

The workspace end-to-end test installs both commands into an isolated directory,
copies the synthetic consumer into a path with spaces, checks normal and named
runs, default precedence, both invocation forms, feature forwarding, custom
profiles, workspace discovery, custom library paths, Cargo configuration,
renamed/inherited dependencies, lockfiles, fresh rebuilds, source preservation,
compile-time diagnostics, failed input/generation/solving/builds, smoke families,
and real shortened Criterion measurements for every ownership mode. Fixture
unit tests check imported `Result` aliases, private nested modules, borrowed
parsed values, repeated generation, consuming destruction, cleanup, and phase
versus elapsed timing using a deliberately non-`Clone` state.

The compatibility script copies the attached 2025 library beneath ignored
`target/compatibility-2025`, adapts its harness imports, and symlinks its local
inputs. Only the isolated copy changes day 4 part 2 to take its tuple by value and
remove `inputs.to_vec()`. It compares all **23 registered parts** against the
existing regression constants, runs all **49 sample/regression tests**, and
smokes day 4 benchmarks. Source and input hashes verify that the original
repository is unchanged. The script writes `validation.json` and `run.stdout.log`
under that ignored directory; no personal inputs are checked in.

## Implementation choices and limits

Typed adapters contain all parser and solver calls. The runtime stores only
metadata and function pointers, with no `Any`, parsed-input downcasts, or `Clone`
bounds. Answers alone are retained as boxed `Display` values for reporting.
Generated marker types hold adapter methods implemented in each function's own
module, so private nested modules and private parsed types remain usable.

`aoc_lib!` discovers direct `aoc` and `aoc_generator` attributes in the library's
Rust module tree, including inline modules, normal module files, and `#[path]`
modules. It tracks those files with `include_str!` dependencies, so editing nested
modules invalidates the registry. Rust applies ordinary `#[cfg]` conditions before
compile-time registry validation, including conditional generator precedence.
This avoids proc-macro global state and expansion-order assumptions.

Registrations synthesized by another macro, hidden behind `include!`, imported
under different attribute names, or introduced through `cfg_attr` are not
discovered. Use direct annotations and `#[cfg]` on functions/modules. Fallible return types must spell their final path segment `Result` or `Option`
(including `std::io::Result<T>` and `std::option::Option<T>`); arbitrary aliases
are not inferred. `output = "value"` keeps an outer wrapper as an ordinary value. Registered
functions take one argument and are synchronous, safe, and free of generic type
or const parameters. Lifetime parameters and parsed values borrowing raw input
are supported. Benchmarks require a runnable native target. This first version
is installed and consumed from the checkout; no crates.io publication is assumed.


The parity fixture and workflow tests additionally cover optional returns,
custom Borrow/BorrowMut projections, both executable forms, cache-only execution,
protocol errors, generation composition, synthetic HTTP redirects/statuses/body
limits/timeouts, read-only credentials, and debug-profile overrides. Network
tests use a private localhost server and synthetic tokens; no test contacts AoC
or reads the user's session file. The package validation script checks unpacked
archives, an installed CLI, generated templates, and versioned consumer
dependencies through a temporary local patch registry, with no checkout paths.
See [release preparation](docs/releasing.md) for the separate publication step.
