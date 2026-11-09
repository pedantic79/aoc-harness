# JSON output for `run`

Implemented.

`cargo aoc-harness run --format json` will emit one JSON report on standard
output. Plain `run` keeps its human-readable output. The first consumer is the
[2025 validation script](../../scripts/validate_2025.py), which currently parses
terminal output with a regular expression that accepts only unsigned numeric
answers from unnamed implementations.

## Report contents

- Include the event year and an ordered result for each completed implementation.
  Each result identifies the day, part, and variant and includes the displayed
  answer as a string. Preserve whitespace and line breaks, including in answers
  that look like numbers.
- Report per-result generator and solver durations and run totals in integer
  nanoseconds. Format answers and serialize the report after the full-run timer
  stops.
- Include the run outcome. On a generator or solver error, report the completed
  results and identify the failed implementation and phase. With `--all-solutions`,
  include any [variant disagreements](variant-agreement.md). The exit status follows
  the same rules as the human-readable run.
- Keep build messages, download progress, and output printed by solution code
  separate from the JSON report so a script can parse stdout directly.

## Validation

Update the compatibility script to read actual answers from the report by day
and part. Exercise a named variant, a negative answer, a multiline answer,
and a partial run. A solver that prints a diagnostic should still produce a
parseable report.
