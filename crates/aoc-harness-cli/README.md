# aoc-harness-cli

The `cargo aoc-harness` / `cargo-aoc-harness` command for annotated Advent of Code
libraries. Provides run, list, Criterion benchmarks, profiling, input acquisition, and safe
day scaffolding. Explicit downloads share the user-managed
`$HOME/.adventofcode.session` raw token file with aoc-cli.

Install from crates.io with `cargo install aoc-harness-cli --locked`.
Run `cargo aoc-harness --help` for commands. Existing execution stays local-only
unless `--download` is requested. Personal input files should stay out of Git.

`cargo aoc-harness profile --day 4 --part 2` records one end-to-end workload with
Samply for approximately 10 seconds. Override `--duration`, `--family`, or
`--profiler` as needed. Supported backends are Samply and Flamegraph wherever
those tools run, Linux `perf`, and macOS `xctrace`. Install the profiler
separately. Debug symbols are enabled automatically; recordings are saved under
the target directory or at a new `--output` path. Use `profile --help` for options.

See the [repository documentation](https://github.com/pedantic79/aoc-harness).
Licensed under MIT OR Apache-2.0.
