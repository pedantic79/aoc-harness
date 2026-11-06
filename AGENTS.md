# Repository guidance

## Code readability

Use blank lines as paragraphs in code: separate logical phases such as setup,
validation, work, and result handling. Keep closely related statements together,
including an operation and its error handling. In tests, separate setup, actions,
and assertions.

## README audiences and consistency

When changing any README or user-facing behavior, review all four READMEs and
update each affected document:

| README | Where readers see it | Purpose |
| --- | --- | --- |
| [README.md](README.md) | GitHub repository | Full getting-started guide and user documentation. |
| [crates/aoc-harness/README.md](crates/aoc-harness/README.md) | `aoc-harness` on crates.io | Runtime dependency and re-exported macros for solution projects. |
| [crates/aoc-harness-cli/README.md](crates/aoc-harness-cli/README.md) | `aoc-harness-cli` on crates.io | CLI installation and the `cargo aoc-harness` command. |
| [crates/aoc-harness-macros/README.md](crates/aoc-harness-macros/README.md) | `aoc-harness-macros` on crates.io | Procedural macros used through the runtime's re-exports. |

Each crate README must work as an independent entry point for crates.io readers:
explain the package's role, show CLI installation and runtime dependency setup,
and link to the other packages and full repository documentation as relevant.

Keep setup guidance consistent: install `aoc-harness-cli`, add `aoc-harness` as
the solution project's dependency, and import macros through `aoc_harness`.
The runtime includes `aoc-harness-macros` automatically and re-exports its macros.

Before completing a change, check shared installation instructions, dependency
versions, command names, macro imports, and feature descriptions across all four
READMEs. Tailor the level of detail to each audience.
