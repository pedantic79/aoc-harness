# Releasing

Update crate versions and dependency requirements. Keep the hardcoded versions
in `scripts/validate_packages.py` in sync.

Run from the repository root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --manifest-path fixtures/library/Cargo.toml --lib
cargo test --manifest-path fixtures/parity/Cargo.toml
python3 scripts/validate_packages.py
```

Publish the crates being released in dependency order, skipping unchanged crates:

```sh
cargo publish -p aoc-harness-macros
cargo publish -p aoc-harness
cargo publish -p aoc-harness-cli
```

Update the README's version examples where needed.

Create one GitHub release per batch of crate releases. Use a unique date-based
tag, such as `release-YYYY-MM-DD`, pointing to the commit used for publishing.
List all three crate versions in the release notes, marking each as newly
published or unchanged. The crate versions may differ.
