//! Runtime shared by typed solution adapters and the generated CLI bridge.

pub use aoc_harness_macros::{aoc, aoc_generator, aoc_lib};
#[doc(hidden)]
pub use criterion;

use clap::{ColorChoice, Parser, ValueEnum};
use std::{
    collections::BTreeMap,
    error::Error as StdError,
    fmt,
    io::IsTerminal,
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct Error {
    message: String,
    pub generator: Duration,
    pub solver: Duration,
}

impl Error {
    pub fn message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            generator: Duration::ZERO,
            solver: Duration::ZERO,
        }
    }

    pub fn phase<E: StdError>(
        day: u8,
        part: Option<u8>,
        name: &str,
        phase: &str,
        error: E,
    ) -> Self {
        Self::message(format!(
            "day {day} part {} [{name}] {phase}: {error}",
            part.map(|p| p.to_string()).unwrap_or_else(|| "*".into())
        ))
    }

    pub fn context(mut self, day: u8, part: u8, name: &str) -> Self {
        self.message = format!("day {day} part {part} [{name}]: {}", self.message);
        self
    }

    pub fn timings(mut self, generator: Duration, solver: Duration) -> Self {
        self.generator = generator;
        self.solver = solver;
        self
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl StdError for Error {}

pub struct Invocation {
    pub answer: Box<dyn fmt::Display>,
    pub generator: Duration,
    pub solver: Duration,
}

#[derive(Clone, Copy)]
pub struct Solution {
    pub day: u8,
    pub part: u8,
    pub name: &'static str,
    pub named: bool,
    pub explicit_default: bool,
    pub run: fn(&str) -> Result<Invocation, Error>,
    /// Executes a complete pass without nested phase timers or answer formatting.
    pub execute: fn(&str) -> Result<(), Error>,
    pub bench: fn(&mut criterion::Criterion, &str),
}

#[derive(Clone, Copy)]
pub struct Generator {
    pub day: u8,
    /// Zero denotes a day-level generator.
    pub part: u8,
    pub name: &'static str,
    pub bench: fn(&mut criterion::Criterion, &str),
    pub check: fn(&str) -> Result<(), Error>,
}

pub struct Registry {
    pub year: u16,
    pub manifest_dir: &'static str,
    pub solutions: &'static [Solution],
    pub generators: &'static [Generator],
}

const fn equal(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }

    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Evaluated by `aoc_lib!` at compile time, after Rust applies cfg attributes.
pub const fn validate(solutions: &[Solution], generators: &[Generator]) {
    let mut i = 0;
    while i < solutions.len() {
        let a = &solutions[i];
        let mut count = 0;
        let mut unnamed = 0;
        let mut defaults = 0;
        let mut j = 0;
        while j < solutions.len() {
            let b = &solutions[j];
            if a.day == b.day && a.part == b.part {
                count += 1;
                if !b.named {
                    unnamed += 1;
                }
                if b.explicit_default {
                    defaults += 1;
                }
                if i != j && equal(a.name, b.name) {
                    panic!("duplicate solution identity (day, part, name)");
                }
            }
            j += 1;
        }
        if defaults > 1 {
            panic!("multiple explicit defaults for one day/part");
        }
        if count > 1 && unnamed == 0 && defaults == 0 {
            panic!("several named solutions: mark one #[aoc(..., default)]");
        }
        i += 1;
    }

    i = 0;
    while i < generators.len() {
        let a = &generators[i];
        let mut j = i + 1;
        while j < generators.len() {
            let b = &generators[j];
            if a.day == b.day && a.part == b.part && equal(a.name, b.name) {
                panic!("duplicate generator identity (day, part, name)");
            }
            j += 1;
        }
        i += 1;
    }
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Action {
    #[default]
    Run,
    List,
    Bench,
}

#[derive(Debug, Parser)]
#[command(
    name = "aoc-harness",
    about = "Run a registered Advent of Code library"
)]
pub struct Options {
    #[arg(value_enum, default_value = "run")]
    pub action: Action,
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=25))]
    pub day: Option<u8>,
    #[arg(long, requires = "day", value_parser = clap::value_parser!(u8).range(1..=2))]
    pub part: Option<u8>,
    #[arg(long, requires_all = ["day", "part"], conflicts_with = "all_solutions")]
    pub solution: Option<String>,
    #[arg(long)]
    pub all_solutions: bool,
    #[arg(long, requires = "day")]
    pub input: Option<PathBuf>,
    /// Regex over Criterion benchmark names.
    #[arg(long)]
    pub filter: Option<String>,
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u64).range(10..))]
    pub sample_size: u64,
    /// Seconds per benchmark.
    #[arg(long, default_value = "3", value_parser = positive_seconds)]
    pub warm_up_time: f64,
    /// Seconds per benchmark.
    #[arg(long, default_value = "5", value_parser = positive_seconds)]
    pub measurement_time: f64,
    /// Execute and validate every benchmark family once, without statistical measurement.
    #[arg(long)]
    pub smoke: bool,
    /// Color benchmark results.
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    pub color: ColorChoice,
    #[arg(long, hide = true)]
    pub report_dir: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            action: Action::Run,
            day: None,
            part: None,
            solution: None,
            all_solutions: false,
            input: None,
            filter: None,
            sample_size: 100,
            warm_up_time: 3.,
            measurement_time: 5.,
            smoke: false,
            color: ColorChoice::Auto,
            report_dir: None,
        }
    }
}

fn positive_seconds(value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n >= 1e-9 && *n <= 86400.)
        .ok_or_else(|| "expected seconds between 0.000000001 and 86400".into())
}

impl Registry {
    pub fn select(&self, options: &Options, variants: bool) -> Result<Vec<&Solution>, Error> {
        let mut candidates: Vec<_> = self
            .solutions
            .iter()
            .filter(|s| {
                options.day.is_none_or(|d| s.day == d)
                    && options.part.is_none_or(|p| s.part == p)
                    && options.solution.as_ref().is_none_or(|n| s.name == n)
            })
            .collect();
        candidates.sort_by_key(|s| (s.day, s.part, s.name));
        if candidates.is_empty() {
            return Err(Error::message(
                "no solutions match the requested day, part, and variant",
            ));
        }

        if !variants && !options.all_solutions && options.solution.is_none() {
            let mut groups: BTreeMap<(u8, u8), Vec<&Solution>> = BTreeMap::new();
            for s in candidates {
                groups.entry((s.day, s.part)).or_default().push(s);
            }
            candidates = groups
                .into_values()
                .map(|group| {
                    group
                        .iter()
                        .find(|s| s.explicit_default)
                        .or_else(|| group.iter().find(|s| !s.named))
                        .copied()
                        .unwrap_or(group[0])
                })
                .collect();
        }
        Ok(candidates)
    }

    pub fn load_inputs(
        &self,
        selected: &[&Solution],
        options: &Options,
    ) -> Result<BTreeMap<u8, String>, Error> {
        let mut inputs = BTreeMap::new();
        for solution in selected {
            if inputs.contains_key(&solution.day) {
                continue;
            }
            let path = options.input.clone().unwrap_or_else(|| {
                PathBuf::from(self.manifest_dir)
                    .join(format!("input/{}/day{}.txt", self.year, solution.day))
            });
            let raw = std::fs::read_to_string(&path).map_err(|e| {
                Error::message(format!(
                    "day {} input {}: {e}",
                    solution.day,
                    path.display()
                ))
            })?;
            inputs.insert(solution.day, raw.trim_end_matches('\n').to_owned());
        }
        Ok(inputs)
    }

    pub fn run(&self, options: &Options) -> Result<(), Error> {
        let selected = self.select(options, false)?;
        let inputs = self.load_inputs(&selected, options)?;
        let mut results = Vec::with_capacity(selected.len());
        let mut failure = None;
        let mut generator = Duration::ZERO;
        let mut solver = Duration::ZERO;

        let start = Instant::now();
        for solution in &selected {
            match (solution.run)(&inputs[&solution.day]) {
                Ok(invocation) => {
                    generator += invocation.generator;
                    solver += invocation.solver;
                    results.push((solution, invocation));
                }
                Err(error) => {
                    generator += error.generator;
                    solver += error.solver;
                    failure = Some(error.context(solution.day, solution.part, solution.name));
                    break;
                }
            }
        }
        let elapsed = start.elapsed();

        println!("AOC {}\n", self.year);
        for (solution, result) in results {
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
        let scope = scope(self.year, options);
        let status = if failure.is_some() {
            "Partial run"
        } else if options.all_solutions {
            "Comparison run"
        } else {
            "Full run"
        };
        println!("{status} ({scope}, {} selected):\n\tgenerator: {generator:?},\n\trunner: {solver:?},\n\ttotal: {elapsed:?}", selected.len());

        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub fn list(&self, options: &Options) -> Result<(), Error> {
        let selected = self.select(options, true)?;
        let mut defaults_options = Options {
            day: options.day,
            part: options.part,
            ..Options::default()
        };
        defaults_options.solution = None;
        let defaults = self.select(&defaults_options, false)?;

        for solution in selected {
            let default = defaults.iter().any(|s| std::ptr::eq(*s, solution));
            println!(
                "{} day {} part {} [{}]{}",
                self.year,
                solution.day,
                solution.part,
                solution.name,
                if default { " (selected default)" } else { "" }
            );
        }
        Ok(())
    }

    pub fn bench(&self, options: &Options) -> Result<(), Error> {
        let selected = self.select(options, true)?;
        // Full-run benches always follow the runner's default selection within the scope.
        let defaults_options = Options {
            day: options.day,
            part: options.part,
            input: options.input.clone(),
            ..Options::default()
        };
        let defaults = self.select(&defaults_options, false)?;
        let mut required = selected.clone();
        required.extend(defaults.iter().copied());
        required.sort_by_key(|s| (s.day, s.part, s.name));
        required.dedup_by_key(|s| (s.day, s.part, s.name));
        let inputs = self.load_inputs(&required, options)?;
        let generators: Vec<_> = self
            .generators
            .iter()
            .filter(|g| {
                options.day.is_none_or(|d| g.day == d)
                    && options.part.is_none_or(|p| g.part == 0 || g.part == p)
                    && options
                        .solution
                        .as_ref()
                        .is_none_or(|n| g.name == "default" || g.name == n)
                    && inputs.contains_key(&g.day)
            })
            .collect();

        // A fallible parser or solver must succeed before Criterion starts measuring.
        for solution in &required {
            (solution.execute)(&inputs[&solution.day])
                .map_err(|e| e.context(solution.day, solution.part, solution.name))?;
        }
        for generator in &generators {
            (generator.check)(&inputs[&generator.day])?;
        }

        let filter = options
            .filter
            .as_ref()
            .map(|s| regex::Regex::new(s))
            .transpose()
            .map_err(|e| Error::message(format!("benchmark filter: {e}")))?;
        let matches = |label: &str| filter.as_ref().is_none_or(|f| f.is_match(label));
        let full_label = format!(
            "full-run/{}",
            scope(self.year, &defaults_options).replace(' ', "-")
        );
        let generator_label = |g: &Generator| {
            if g.part == 0 {
                format!("generator/day{}/{}", g.day, g.name)
            } else {
                format!("generator/day{}/part{}/{}", g.day, g.part, g.name)
            }
        };
        let solution_label =
            |s: &Solution, phase: &str| format!("{phase}/day{}/part{}/{}", s.day, s.part, s.name);
        let count = generators
            .iter()
            .filter(|g| matches(&generator_label(g)))
            .count()
            + selected
                .iter()
                .filter(|s| matches(&solution_label(s, "solver")))
                .count()
            + selected
                .iter()
                .filter(|s| matches(&solution_label(s, "end-to-end")))
                .count()
            + usize::from(matches(&full_label));
        if count == 0 {
            return Err(Error::message("no benchmarks match --filter"));
        }

        if options.smoke {
            for g in generators.iter().filter(|g| matches(&generator_label(g))) {
                (g.check)(&inputs[&g.day])?;
                println!("Smoke: {}", generator_label(g));
            }
            for s in &selected {
                for phase in ["solver", "end-to-end"] {
                    if matches(&solution_label(s, phase)) {
                        (s.execute)(&inputs[&s.day])
                            .map_err(|e| e.context(s.day, s.part, s.name))?;
                        println!("Smoke: {}", solution_label(s, phase));
                    }
                }
            }
            if matches(&full_label) {
                for s in &defaults {
                    (s.execute)(&inputs[&s.day])?;
                }
                println!("Smoke: {full_label}");
            }
            println!("Benchmark smoke passed: {count} families (no statistical reports; omit --smoke to measure)");
            return Ok(());
        }

        let report = options
            .report_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from(self.manifest_dir).join("target/criterion"));
        let mut criterion = criterion::Criterion::default()
            .with_output_color(match options.color {
                ColorChoice::Always => true,
                ColorChoice::Never => false,
                ColorChoice::Auto => {
                    std::io::stdout().is_terminal()
                        && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
                        && std::env::var_os("TERM").is_none_or(|term| term != "dumb")
                }
            })
            .output_directory(&report)
            .sample_size(options.sample_size as usize)
            .warm_up_time(Duration::from_secs_f64(options.warm_up_time))
            .measurement_time(Duration::from_secs_f64(options.measurement_time));
        if let Some(filter) = &options.filter {
            criterion = criterion.with_filter(filter);
        }

        for generator in generators {
            (generator.bench)(&mut criterion, &inputs[&generator.day]);
        }
        for solution in selected {
            (solution.bench)(&mut criterion, &inputs[&solution.day]);
        }
        criterion.bench_function(&full_label, |b| {
            b.iter(|| {
                for solution in &defaults {
                    (solution.execute)(std::hint::black_box(&inputs[&solution.day]))
                        .unwrap_or_else(|e| panic!("{e}"));
                }
            })
        });
        criterion.final_summary();
        println!("Benchmark reports: {}", report.display());
        Ok(())
    }
}

fn scope(year: u16, options: &Options) -> String {
    let mut scope = year.to_string();
    if let Some(day) = options.day {
        scope.push_str(&format!(" day {day}"));
    }
    if let Some(part) = options.part {
        scope.push_str(&format!(" part {part}"));
    }
    if let Some(name) = &options.solution {
        scope.push_str(&format!(" [{name}]"));
    }
    scope
}

pub fn entry(registry: Registry) -> i32 {
    let options = Options::parse();
    let result = match options.action {
        Action::Run => registry.run(&options),
        Action::List => registry.list(&options),
        Action::Bench => registry.bench(&options),
    };
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("error: {error}");
            1
        }
    }
}
