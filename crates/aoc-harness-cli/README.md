# aoc-harness-cli

The `cargo aoc-harness` / `cargo-aoc-harness` command for annotated Advent of Code
libraries. Provides run, list, Criterion benchmarks, input acquisition, and safe
day scaffolding. Explicit downloads share the user-managed
`$HOME/.adventofcode.session` raw token file with aoc-cli.

From a checkout, install with `cargo install --path crates/aoc-harness-cli --locked`.
Run `cargo aoc-harness --help` for commands. Existing execution stays local-only
unless `--download` is requested. Personal input files should stay out of Git.

See the [repository documentation](https://github.com/pedantic79/aoc-harness).
Licensed under MIT OR Apache-2.0. Publication is a separate release operation.
