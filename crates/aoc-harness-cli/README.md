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
aoc-harness = "0.3.0"
```

It provides the runtime and re-exports `aoc`, `aoc_generator`, `aoc_lib`, and
`aoc_main` from [`aoc-harness-macros`](https://crates.io/crates/aoc-harness-macros).
Import these macros through `aoc_harness` in your solution code.

Run `cargo aoc-harness --help` for commands. Downloads happen only when requested
with `input` or `--download`. They share the user-managed
`$HOME/.adventofcode.session` raw token file with aoc-cli. Personal input files
should stay out of Git.

Select multiple days with `--day 1,3` or `--day 1 --day 3`; `-d` is the short form
of `--day`. Use `--all-days` to select every registered day for `run`, `list`,
`bench`, or `profile`.

Select a part with `--part 1` or `-p 1`, for example
`cargo aoc-harness run -d 1 -p 2`. Workspace package selection uses `--package`.

`cargo aoc-harness bench` benchmarks solvers by default. Use `--family` to select
any combination of `generator`, `solver`, `end-to-end`, and `full-run`, for example
`cargo aoc-harness bench --family generator,solver` or
`cargo aoc-harness bench --family solver --family full-run`. `--filter` narrows
the selected families; `--smoke` validates them without statistical measurement.
Use `bench --help` for options.

`cargo aoc-harness profile --day 4 --part 2` records an end-to-end workload with
Samply for approximately 10 seconds. Profiling shares the benchmark command's
`--family` and `--filter` selection, including comma-separated values and repeated
`--family` flags. Each selected workload gets its own recording; `--duration`
applies to each workload. With multiple workloads, `--output PATH` selects a new
directory containing recordings organized by workload name. Override
`--duration` or `--profiler` as needed. Supported backends are Samply and Flamegraph
wherever those tools run, Linux `perf`, and macOS `xctrace`. Install the profiler
separately. Debug symbols are enabled automatically; recordings are saved under
the target directory or at a new `--output` path. Use `profile --help` for options.

See the [repository documentation](https://github.com/pedantic79/aoc-harness).
Licensed under MIT OR Apache-2.0.
