# Select everything with `all`

Implemented in `aoc-harness` 0.6.0 and `aoc-harness-cli` 0.7.0.

Selection flags take `all` instead of separate `--all-*` flags:

| Before | After | Commands |
| --- | --- | --- |
| `--all-days` | `--day all` | run, list, bench, profile |
| `input --all` | `input --day all` | input |
| `--all-solutions` | `--solution all` | run, list, bench, profile |
| — | `--year all` | run, list, bench, profile, input |

`generate` keeps explicit `--year <YEAR>` and `--day <DAYS>`. The old flags are
removed from both the CLI and the runtime, a breaking change for both crates.

## Rules

- `all` cannot be mixed with specific values, so `--day all,3` is an error.
- `--solution <NAME>` still needs `--day` and `--part`; `--solution all` does not.
  `run --solution all` fails when displayed answers disagree, as `--all-solutions`
  did. For `profile`, it profiles every variant rather than only the defaults; for
  `list` and `bench`, which already include every variant, it changes nothing.
- `--input` needs exactly one day and one year, so it rejects `--day all` and
  `--year all`.

## `--year all`

`--year all` covers each registered year in ascending order. It does not change
day selection: without `--day`, run, bench, and profile use each year's latest
registered day, so `--year all --day all` selects everything.

Each year keeps only the requested days it registers, and a year with no matching
solution is skipped. Selection fails only when a requested day matches no year,
or when no year matches at all. `list` without `--year` already behaved this way;
`list --year all` is the same listing.

- `run` reads every year's inputs before running any solution. A failed solution
  stops the run; a disagreement fails after every year has run.
- `run --format json` wraps per-year reports in `{ "outcome", "years" }`, for a
  single year too. See [JSON output for `run`](run-json.md).
- `bench` plans every year, and checks `--baseline` for each, before measuring.
  Reports go to each year's directory. An explicit `--report-dir` becomes the
  parent of those directories when the crate registers several years.
- `profile` records each year separately. With several years, `--output` names a
  directory holding one subdirectory per year.
- `input` reads the years from the `aoc_lib!` declaration and skips years where no
  selected day is available, such as `--day 13` for 2025 or an unreleased event.

## Compatibility

The runtime advertises `select-all-v1`. The CLI forwards `all` selectors to
runtimes with it. For older runtimes, it rewrites `--day all` and `--solution all`
as `--all-days` and `--all-solutions`, so existing solution projects keep working.
`--year all`, and `--day all` combined with `--part`, have no older spelling; the
CLI names the `aoc-harness` 0.6.0 upgrade instead.

An older CLI sends `--all-days` and `--all-solutions`, which a 0.6.0 runtime
rejects. The runtime's unrecognized `select-all-v1` capability already makes
such a CLI suggest upgrading itself.
