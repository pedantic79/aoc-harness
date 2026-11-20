# aoc-harness-cli

This package contains the `cargo-aoc-harness` executable, which Cargo exposes as
`cargo aoc-harness`. It runs and lists solutions from annotated Advent of Code
libraries, provides Criterion benchmarks and profiling, downloads inputs, and
generates day modules.

Install the CLI from [crates.io](https://crates.io/crates/aoc-harness-cli):

```sh
cargo install aoc-harness-cli --locked
```

Use [`aoc-harness`](https://crates.io/crates/aoc-harness) in your solution
project's `Cargo.toml`:

```toml
[dependencies]
aoc-harness = "0.5.0"
```

It provides the runtime and re-exports `aoc`, `aoc_generator`, `aoc_lib`, and
`aoc_main` from [`aoc-harness-macros`](https://crates.io/crates/aoc-harness-macros).
Import these macros through `aoc_harness` in your solution code.

Run `cargo aoc-harness --help` for commands. Downloads happen only when requested
with `input` or `--download`. They share the user-managed
`$HOME/.adventofcode.session` raw token file with aoc-cli. Personal input files
should stay out of Git. `input -d 1` and `input --all` use the selected package's
`aoc_lib!` year, even when downloading in a later calendar year, without compiling
unfinished solvers.

A crate that registers several events with `aoc_lib! { years = { ... } }` selects
one with `--year` on `run`, `list`, `bench`, `profile`, `input`, and `generate`.
Without it, commands use the only registered year or `default_year`, and plain
`list` shows every year. `--year` must name a registered year, including in
single-year crates, and requires an `aoc-harness` 0.5.0 or later runtime.

Inputs are saved to `input/{year}/day{day}.txt` beside the solution package's
`Cargo.toml`. Set `input-dir` in that manifest to keep them elsewhere, for
example in a private repository. Relative paths start at the manifest, not the
invocation directory, and `run`, `bench`, `profile`, and `input!` read from the
same place:

```toml
[package.metadata.aoc-harness]
input-dir = "../advent-of-code-input"
```

The solution package needs `aoc-harness` 0.4.2 or later for `input-dir`; older
runtimes keep reading `input/` even though the CLI downloads to the configured
directory.

`cargo aoc-harness generate -d 2` creates `src/day02.rs` and adds `mod day02;`
before `aoc_lib!` in `src/lib.rs`. In a multi-year crate, `generate --year 2019`
writes the day beside the year's module (`src/y2019/mod.rs` or `src/y2019.rs`)
and declares it there. New module names use two-digit day numbers,
unless the library root already declares only unpadded single-digit modules
such as `mod day4;`. Days whose declaration and file already match are skipped;
an undeclared day file or a mismatched declaration stops generation before
anything is written. Add `--dry-run` to preview the changes.

Save a UTF-8 starter module at `templates/day.rs` in your solution package.
`cargo aoc-harness generate -d 4` picks it up automatically, replacing `@DAY@`
with `4` and `@YEAR@` with the selected year from that package's `aoc_lib!`.
Use `let raw = aoc_harness::input!(@DAY@);` in a template's regression test
to read that day's local puzzle input as a `String`.

To use a different default, set `day-template` in the solution package's
`Cargo.toml`. Relative paths start at that manifest.

```toml
[package.metadata.aoc-harness]
day-template = "starters/puzzle.rs"
```

An explicit `--template PATH` takes priority and resolves relative to the
invocation directory. The CLI uses its bundled template when no override,
manifest setting, or `templates/day.rs` is present. For scripts that used
`input --generate` or `run --generate`, call `generate` as a separate step.
See [Custom day templates](https://github.com/pedantic79/aoc-harness#custom-day-templates)
for a complete example.

Select multiple days with `--day 1,3` or `--day 1 --day 3`; `-d` is the short form
of `--day`. Use `--all-days` to select every registered day for `run`, `list`,
`bench`, or `profile`.

Select a part with `--part 1` or `-p 1`, for example
`cargo aoc-harness run -d 1 -p 2`. Workspace package selection uses `--package`.

Use `run --all-solutions` to compare all variants' displayed answers by day and
part. Any disagreement, including whitespace or line breaks, makes the command
fail after completing the selection. `run --format json` emits one report with
the event year, ordered results, string answers, integer nanosecond timings,
outcome, partial failures, and disagreements. Build, download, and solution
output goes to stderr so stdout is directly parseable. Plain `run` keeps its
human-readable output.

`cargo aoc-harness bench` benchmarks solvers by default. Use `--family` to select
any combination of `generator`, `solver`, `end-to-end`, and `full-run`, for example
`cargo aoc-harness bench --family generator,solver` or
`cargo aoc-harness bench --family solver --family full-run`. `--filter` narrows
the selected families; `--smoke` executes each workload exactly once without
statistical measurement or reports. Use `--save-baseline NAME` to save selected
measurements and `--baseline NAME` to compare without replacing them. Missing
workload references fail with instructions to save them. Choose one baseline
operation per measured run; neither can be combined with `--smoke`. Names are
single directory components excluding `base`, `new`, `change`, `report`, and `profile`.
Reports go to `<target-dir>/criterion/<package>/`; a crate that registers several
years uses a `<year>/` subdirectory, so each year keeps its own reports and
baselines. The summary names the event year. Use `bench --help` for options.

`cargo aoc-harness profile --day 4 --part 2` records an end-to-end workload with
Samply for approximately 10 seconds. Profiling shares the benchmark command's
`--family` and `--filter` selection, including comma-separated values and repeated
`--family` flags. Each selected workload gets its own recording; `--duration`
applies to each workload. With multiple workloads, `--output PATH` selects a new
directory containing recordings organized by workload name. Override
`--duration` or `--profiler` as needed. Supported backends are Samply and Flamegraph
wherever those tools run, Linux `perf`, and macOS `xctrace`. Install the profiler
separately. Debug symbols are enabled automatically; recordings are saved under
`<target-dir>/aoc-harness/profiles/<package>/` (with a `<year>/` subdirectory in a
multi-year crate) or at a new `--output` path, used as given. Use `profile --help` for options.

See the [repository documentation](https://github.com/pedantic79/aoc-harness).
Licensed under MIT OR Apache-2.0.
