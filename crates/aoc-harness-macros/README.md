# aoc-harness-macros

This package implements the `aoc`, `aoc_generator`, `aoc_lib`, and `aoc_main`
procedural macros for [`aoc-harness`](https://crates.io/crates/aoc-harness).

Install [`aoc-harness-cli`](https://crates.io/crates/aoc-harness-cli) to get the
`cargo aoc-harness` command for running, benchmarking, profiling, downloading
inputs, and generating day modules:

```sh
cargo install aoc-harness-cli --locked
```

Then add `aoc-harness` to your solution project's `Cargo.toml`. It provides the
runtime, includes this macros package automatically, and re-exports all four
macros:

```toml
[dependencies]
aoc-harness = "0.4.1"
```

Import the macros through `aoc_harness` in your solution code:

```rust,ignore
use aoc_harness::{aoc, aoc_generator, aoc_lib, aoc_main};
```

Registration discovery scans direct attributes in the selected library or binary
module tree. Generated adapters preserve borrowed lifetimes and ownership modes.

See the [repository documentation](https://github.com/pedantic79/aoc-harness).
Licensed under MIT OR Apache-2.0.
