use crate::{Error, Invocation, Options, Registry, RunFormat, Solution};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

struct Completed<'a> {
    solution: &'a Solution,
    answer: String,
    generator: Duration,
    solver: Duration,
}

fn disagreements(results: &[Completed<'_>]) -> Vec<Value> {
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for result in results {
        groups
            .entry((result.solution.day, result.solution.part))
            .or_default()
            .push(result);
    }

    groups.into_iter().filter_map(|((day, part), group)| {
        group.iter().any(|r| r.answer != group[0].answer).then(|| json!({
            "day": day, "part": part,
            "variants": group.iter().map(|r| json!({"variant": r.solution.name, "answer": r.answer})).collect::<Vec<_>>()
        }))
    }).collect()
}

/// One year's completed run: its report, and the error that ends it.
struct YearRun {
    report: Value,
    error: Option<Error>,
    failed: bool,
}

/// Run each scoped year in order and report every year together.
pub(crate) fn run(scopes: &[(&Registry, Options)], options: &Options) -> Result<(), Error> {
    let mut prepared = Vec::with_capacity(scopes.len());
    for (registry, options) in scopes {
        let plan = registry.plan(options)?;
        let inputs = registry.load_inputs(&plan.selected, options)?;
        prepared.push((*registry, options, plan.selected, inputs));
    }

    let mut reports = Vec::with_capacity(prepared.len());
    let mut error = None;
    for (index, (registry, options, selected, inputs)) in prepared.into_iter().enumerate() {
        if index > 0 && options.format == RunFormat::Human {
            println!();
        }

        let year = registry.run_year(options, &selected, &inputs);
        reports.push(year.report);
        if let Some(year_error) = year.error {
            error.get_or_insert(year_error);
        }
        if year.failed {
            break;
        }
    }

    if options.format == RunFormat::Json {
        let outcome = ["failed", "disagreement"]
            .into_iter()
            .find(|outcome| reports.iter().any(|r| r["outcome"] == *outcome))
            .unwrap_or("success");
        let report = json!({ "outcome": outcome, "years": reports });
        let text = serde_json::to_string(&report)
            .map_err(|e| Error::message(format!("serialize run report: {e}")))?;

        if let Some(path) = &options.run_report {
            std::fs::write(path, format!("{text}\n"))
                .map_err(|e| Error::message(format!("write run report: {e}")))?;
        } else {
            println!("{text}");
        }
    }

    error.map_or(Ok(()), Err)
}

impl Registry {
    /// Run one year's selected solutions, printing its human report when
    /// requested and returning its JSON report.
    fn run_year(
        &self,
        options: &Options,
        selected: &[&Solution],
        inputs: &BTreeMap<u8, String>,
    ) -> YearRun {
        let mut invocations = Vec::with_capacity(selected.len());
        let mut failure = None;
        let mut generator = Duration::ZERO;
        let mut solver = Duration::ZERO;

        let start = Instant::now();
        for solution in selected {
            match (solution.run)(&inputs[&solution.day]) {
                Ok(invocation) => {
                    generator += invocation.generator;
                    solver += invocation.solver;
                    invocations.push((*solution, invocation));
                }
                Err(error) => {
                    generator += error.generator;
                    solver += error.solver;
                    failure = Some((
                        *solution,
                        error.context(solution.day, solution.part, solution.name),
                    ));
                    break;
                }
            }
        }
        let elapsed = start.elapsed();

        // Display may be expensive or have side effects. Format exactly once,
        // outside the full-run timer, and use that string everywhere.
        let results: Vec<_> = invocations
            .into_iter()
            .map(
                |(
                    solution,
                    Invocation {
                        answer,
                        generator,
                        solver,
                    },
                )| Completed {
                    solution,
                    answer: answer.to_string(),
                    generator,
                    solver,
                },
            )
            .collect();
        let disagreements = if options.all_solutions() {
            disagreements(&results)
        } else {
            Vec::new()
        };
        let outcome = if failure.is_some() {
            "failed"
        } else if !disagreements.is_empty() {
            "disagreement"
        } else {
            "success"
        };

        let report = json!({
            "year": self.year, "outcome": outcome,
            "results": results.iter().map(|r| json!({
                "day": r.solution.day, "part": r.solution.part, "variant": r.solution.name,
                "answer": r.answer, "generator_ns": r.generator.as_nanos(), "solver_ns": r.solver.as_nanos(),
            })).collect::<Vec<_>>(),
            "totals": { "generator_ns": generator.as_nanos(), "solver_ns": solver.as_nanos(), "total_ns": elapsed.as_nanos() },
            "failure": failure.as_ref().map(|(s, e)| json!({
                "day": s.day, "part": s.part, "variant": s.name, "phase": e.phase, "message": e.to_string(),
            })),
            "disagreements": disagreements,
        });

        if options.format == RunFormat::Human {
            println!("AOC {}\n", self.year);
            for result in &results {
                let solution = result.solution;
                let variant = if solution.named {
                    format!(" - {}", solution.name)
                } else {
                    String::new()
                };
                println!(
                    "Day {} - Part {}{}: {}\n\tgenerator: {:?},\n\trunner: {:?}\n",
                    solution.day,
                    solution.part,
                    variant,
                    result.answer,
                    result.generator,
                    result.solver
                );
            }
            let scope = self.scope(options);
            let status = if failure.is_some() {
                "Partial run"
            } else if options.all_solutions() {
                "Comparison run"
            } else {
                "Full run"
            };
            println!("{status} ({scope}, {} selected):\n\tgenerator: {generator:?},\n\trunner: {solver:?},\n\ttotal: {elapsed:?}", selected.len());
            for group in &disagreements {
                println!(
                    "Variant disagreement: day {} part {}",
                    group["day"], group["part"]
                );
                for variant in group["variants"].as_array().unwrap() {
                    println!(
                        "  [{}]: {:?}",
                        variant["variant"].as_str().unwrap(),
                        variant["answer"].as_str().unwrap()
                    );
                }
            }
        }

        let failed = failure.is_some();
        let error = match failure {
            Some((_, error)) => Some(error),
            None if !disagreements.is_empty() => Some(Error::message("variant answers disagree")),
            None => None,
        };
        YearRun {
            report,
            error,
            failed,
        }
    }
}

// A separate process lets all solution output (including native writes and
// Display implementations) stream to stderr while stdout contains only JSON.
pub(crate) fn isolated(args: &[OsString]) -> i32 {
    let run = || -> Result<i32, Box<dyn std::error::Error>> {
        let report = tempfile::NamedTempFile::new()?;
        let status = Command::new(std::env::current_exe()?)
            .args(&args[1..])
            .arg("--run-report")
            .arg(report.path())
            .stdout(Stdio::from(std::io::stderr()))
            .status()?;
        let bytes = std::fs::read(report.path())?;
        if !bytes.is_empty() {
            use std::io::Write;
            std::io::stdout().lock().write_all(&bytes)?;
        }
        Ok(status.code().unwrap_or(1))
    };

    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: JSON run: {error}");
            1
        }
    }
}
