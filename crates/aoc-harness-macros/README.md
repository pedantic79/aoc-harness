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
aoc-harness = "0.5.0"
```

Import the macros through `aoc_harness` in your solution code:

```rust,ignore
use aoc_harness::{aoc, aoc_generator, aoc_lib, aoc_main};
```

Registration discovery scans direct attributes in the selected library or binary
module tree. Generated adapters preserve borrowed lifetimes and ownership modes.
The `year` given to `aoc_lib!` or `aoc_main!` must be a constant `u16`
expression; registrations are scoped to their event year at compile time.
Literals and `const` items such as `year = EVENT_YEAR` work; an expression that
can only be evaluated at run time doesn't compile.

`aoc_lib! { years = { 2019 => y2019, 2025 => y2025 }, default_year = 2025 }`
registers several events in one crate, and `cargo aoc-harness --year` selects one.
`#[aoc]` and `#[aoc_generator]` are written the same way inside each year
module; they take the year from the module they are in, so two years can reuse
the same day, part, and function names. The keys and `default_year` are integer
literals, 2015 or later. Duplicate years, mapped modules that repeat or nest,
an unregistered `default_year`, combining `year` with `years`, and registrations
outside every mapped module are compile errors. Standalone `aoc_main!` binaries
support one year.

See the [repository documentation](https://github.com/pedantic79/aoc-harness).
Licensed under MIT OR Apache-2.0.
