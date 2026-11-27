# Use the project year for puzzle inputs

Implemented.

`cargo aoc-harness input` must use the event year declared by the selected
solution package in `aoc_lib!`. Today, omitting `--year` makes the CLI use the
current calendar year. In November 2026, `input -d 1 --offline` in the 2025
library fixture reports that 2026 day 1 has yet to be released, even though the
2025 input is cached.

## Expected behavior

- `input -d 1` uses `input/2025/day1.txt` for a project declaring `year = 2025`,
  including when invoked in a later year. `input --all` uses that same event.
- If the CLI cannot determine the declared year, it reports the problem before
  looking up credentials or fetching inputs. The diagnostic should identify the
  selected package and its library source.
- The clock still determines which puzzles have been released and whether a
  default day can be selected during the event.
- Input acquisition must keep working while solvers are unfinished. The macro
  accepts a Rust expression for `year`, so year resolution must account for
  declarations such as `year = EVENT_YEAR` as well as integer literals.

## Remove `--year`

Remove `--year` from `cargo aoc-harness input`. The selected project's declaration
supplies the year for every input request, including `--all`. Passing `--year`
will produce the usual unknown-argument error.

Scripts can migrate by dropping the flag and its value. For example, in a 2025
solution project, `cargo aoc-harness input --year 2025 --all` becomes
`cargo aoc-harness input --all`. Removing a documented flag is a breaking CLI
change.

## Reintroduce `--year` to select a registered event

Superseded in part by multi-year crates (#1, #4). `aoc_lib!` can register
several events with `years = { YEAR => module, ... }`, so `input` again accepts
`--year`, with select-only semantics:

- `--year` names one of the years the declaration registers. It never overrides
  or relabels the declared year, which was the reason for the removal above.
- An unregistered year is an error, including in single-year crates, where
  `--year` must equal the evaluated `year = EXPR`. The error is reported before
  credentials are looked up, inputs are requested, or files are written.
- Without `--year`, `input` uses the only registered year or `default_year`; with
  several years and no default, it lists the registered years and asks for one.
  The clock never selects the year.
- `--all` downloads that event's released days only.
- The CLI reads `years` and `default_year` by parsing the library source, so
  input acquisition still works while solvers do not compile. `year = EXPR` keeps
  the existing probe, compiled only when the year must be known.

## Select every day or year with `all`

Superseded in part by [`all` selectors](all-selectors.md). `input --all` became
`input --day all`, and `input --year all` covers every registered year.
