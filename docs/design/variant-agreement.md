# Check variant agreement with `run --all-solutions`

Implemented.

`cargo aoc-harness run --all-solutions` will automatically check whether the
selected implementations of each puzzle part give the same answer. The existing
flag already runs each variant against the same raw input and prints its answer
and timings. Today the command can succeed when those answers differ.

## Comparison behavior

Group completed results by day and part. After the full-run timer stops, format
each answer once with `Display`. Use that same string for reporting and exact
comparison, including whitespace and line breaks. A group with one implementation
requires no comparison.

If every group agrees and execution succeeds, exit successfully. If any group
disagrees, finish the selected comparisons, show the usual answers and timings,
identify each disagreeing day and part with its variant names and answers, and
exit with a nonzero status. All variants participate equally, including the
default. A disagreement alone gives no basis for choosing a correct answer.
Generator and solver errors still follow the existing partial-run behavior.

## Example and regression coverage

The [library fixture](../../fixtures/library/src/lib.rs) has two useful cases:

| Run from the fixture | Expected outcome |
| --- | --- |
| `cargo aoc-harness run -d 1 -p 1 --all-solutions` | Both variants return `6`. The command succeeds. |
| `cargo aoc-harness run -d 1 -p 2 --all-solutions` | The unnamed, `mutable`, and `owned` variants return `12`, `306`, and `12`. The command reports the disagreement and exits unsuccessfully. |

The comparison also works on an alternate input supplied with `--input`.
Consumer regression tests remain the place to record known answers. Those tests
can catch a shared bug even when every variant returns the same wrong answer.

## Compatibility

Scripts that use `run --all-solutions` will receive a failure status for a
disagreement that previously produced a successful exit. Treat this as a breaking
change to the documented CLI behavior when preparing the release.
