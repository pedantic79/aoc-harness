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
