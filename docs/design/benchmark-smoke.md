# Execute each selected benchmark workload once in smoke mode

Implemented.

`cargo aoc-harness bench --smoke` checks that the selected workloads can run.
Today a single solver benchmark invokes the solver twice. The benchmark setup
validates the solver, then the smoke branch runs it again to print the result.

The smoke run itself should perform validation. Execute each selected workload
once and report success for that workload after it completes. A generator or
solver error fails the command. Smoke mode does no statistical measurement and
writes no Criterion report.

## Expected invocation counts

For a selection containing one solver implementation:

| Selected families | Expected solver calls |
| --- | --- |
| `solver` | One call. |
| `solver,end-to-end` | Two calls, one for each selected workload. |
| `generator` | Zero calls. Each selected generator runs once. |

For `full-run`, execute one complete pass through the selected default
implementations. A solver selected both individually and within that pass runs
once in each workload. A regression test should count invocations inside the
solver so it catches duplicate execution even if only one status line is printed.
