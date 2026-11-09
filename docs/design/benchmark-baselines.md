# Compare benchmarks against a named baseline

Implemented.

Use a named baseline to compare several edits to a solver against the same
starting measurement. Both operations belong on `cargo aoc-harness bench`.

```sh
# Measure the original solver.
cargo aoc-harness bench -d 4 -p 2 --save-baseline before

# After each edit, compare the same workload with the original measurement.
cargo aoc-harness bench -d 4 -p 2 --baseline before
```

## Behavior

- `--save-baseline NAME` saves Criterion measurements for the selected workloads.
  Saving the same name again replaces those workloads' reference measurements.
- `--baseline NAME` measures the current code and reports Criterion's timing
  change for each selected workload. Comparisons preserve the saved reference.
  If a selected workload has no measurement under that name, exit unsuccessfully,
  identify the missing baseline and workload, and suggest `--save-baseline NAME`.
- Both flags use the package's existing Criterion report directory and honor
  the existing day, part, solution, family, and workload filters. A name is a
  single directory component within each workload's results. Reject path traversal
  and names that collide with Criterion's working directories.
- Accept one baseline operation per measured run. Combining the flags or using
  either with `--smoke` is a usage error.

## Validation

Use a short measured benchmark to save a baseline, then compare twice and check
that its samples and estimates are identical after both comparisons. Also cover
a comparison that selects a workload absent from the saved baseline.
