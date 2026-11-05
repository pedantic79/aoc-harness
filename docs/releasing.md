# Release preparation

The three crates have coordinated `0.1.0` versions, versioned path dependencies,
repository metadata, crate READMEs, complete MIT/Apache-2.0 licenses, and explicit
package contents including the scaffold template. No publication is assumed or
performed by implementation or validation.

Run from the checkout with cached third-party dependencies:

```sh
export CARGO_TARGET_DIR="$PWD/target"
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --manifest-path fixtures/library/Cargo.toml --lib
cargo test --manifest-path fixtures/parity/Cargo.toml
python3 scripts/validate_packages.py
```

`validate_packages.py` packages in dependency order, unpacks each archive, and
checks those sources in an isolated workspace. A local patch registry supplies
only the unpacked first-party crates, so versioned dependencies resolve without
requiring unpublished versions on crates.io. It installs the packaged command,
checks help/version outside Cargo, runs a versioned library consumer through the
installed CLI, compiles generated source, and exercises library-forwarding and
standalone binaries. It writes `target/package-validation/validation.json`.
It does not claim that versions or names already exist in the public registry.

Release separately in this order: `aoc-harness-macros`, `aoc-harness`, then
`aoc-harness-cli`. Keep versions and dependency requirements coordinated; check
registry availability and let each dependency become available before publishing
the next crate. After publication exists, replace checkout installation examples
with `cargo install aoc-harness-cli --locked` and
`aoc-harness = "0.1.0"`, using the actual released version. Publication and name
reservation require a separate explicit release request.

The CLI bridge depends only on the consumer, preserving that consumer's runtime
version. Ordinary run/list/bench remain compatible with older runtimes.
`--download` requires the `input-plan-v1` capability from `__describe --protocol 1`
and fails with an upgrade diagnostic before network access when unavailable.
Explicit `input --year --day` and generation remain independent of that protocol.

The API change normalizes one outer `Option` on generators and solvers. Consumers
passing an optional parsed value should add `output = "value"` to the generator.
Custom Borrow projections require `input = "borrow"`. These changes and the
registration-discovery boundary must be included in release notes.
