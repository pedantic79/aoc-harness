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
aoc-harness = "0.2.1"
```

It provides the runtime and re-exports `aoc`, `aoc_generator`, `aoc_lib`, and
`aoc_main` from [`aoc-harness-macros`](https://crates.io/crates/aoc-harness-macros).
Import these macros through `aoc_harness` in your solution code.

Run `cargo aoc-harness --help` for commands. Downloads happen only when requested
with `input` or `--download`. They share the user-managed
`$HOME/.adventofcode.session` raw token file with aoc-cli. Personal input files
should stay out of Git.

`cargo aoc-harness profile --day 4 --part 2` records one end-to-end workload with
Samply for approximately 10 seconds. Override `--duration`, `--family`, or
`--profiler` as needed. Supported backends are Samply and Flamegraph wherever
those tools run, Linux `perf`, and macOS `xctrace`. Install the profiler
separately. Debug symbols are enabled automatically; recordings are saved under
the target directory or at a new `--output` path. Use `profile --help` for options.

See the [repository documentation](https://github.com/pedantic79/aoc-harness).
Licensed under MIT OR Apache-2.0.
