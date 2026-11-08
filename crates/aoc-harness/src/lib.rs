//! Runtime shared by typed solution adapters and the generated CLI bridge.

pub use aoc_harness_macros::{aoc, aoc_generator, aoc_lib, aoc_main};
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

    pub fn no_value(day: u8, part: Option<u8>, name: &str, phase: &str) -> Self {
        Self::message(format!(
            "day {day} part {} [{name}] {phase}: returned None",
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
    Profile,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum ProfileFamily {
    Generator,
    Solver,
    #[default]
    EndToEnd,
    FullRun,
}

impl ProfileFamily {
    fn prefix(self) -> &'static str {
        match self {
            Self::Generator => "generator/",
            Self::Solver => "solver/",
            Self::EndToEnd => "end-to-end/",
            Self::FullRun => "full-run/",
        }
    }
}

#[derive(Clone, Debug, Parser)]
#[command(
    name = "aoc-harness",
    about = "Run a registered Advent of Code library"
)]
pub struct Options {
    #[arg(value_enum, default_value = "run")]
    pub action: Action,
    /// Select days (comma-separated or repeated); run, bench, and profile default to the latest registered day.
    #[arg(short = 'd', long, value_delimiter = ',', value_parser = parse_day)]
    pub day: Vec<u8>,
    /// Select every registered day instead of the latest day.
    #[arg(long, conflicts_with = "day")]
    pub all_days: bool,
    #[arg(short = 'p', long, requires = "day", value_parser = parse_part)]
    pub part: Option<u8>,
    #[arg(long, requires_all = ["day", "part"], conflicts_with = "all_solutions")]
    pub solution: Option<String>,
    #[arg(long)]
    pub all_solutions: bool,
    #[arg(short = 'i', long, requires = "day", conflicts_with = "all_days")]
    pub input: Option<PathBuf>,
    /// Regex narrowing the selected benchmark or profiling families by workload name.
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
    /// Execute and validate selected benchmark families once, without statistical measurement.
    #[arg(long)]
    pub smoke: bool,
    /// Workload families, comma-separated or repeated (bench default: solver; profile default: end-to-end).
    #[arg(long, value_enum, value_delimiter = ',')]
    pub family: Vec<ProfileFamily>,
    /// Approximate profiling seconds per workload, excluding compilation; minimum one second.
    #[arg(long, default_value = "10", value_parser = profile_seconds)]
    pub duration: f64,
    #[arg(long, hide = true)]
    pub profile_status: Option<PathBuf>,
    #[arg(long, hide = true)]
    pub profile_working_dir: Option<PathBuf>,
    #[arg(long, hide = true)]
    pub profile_workload: Option<String>,
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
            day: Vec::new(),
            all_days: false,
            part: None,
            solution: None,
            all_solutions: false,
            input: None,
            filter: None,
            sample_size: 100,
            warm_up_time: 3.,
            measurement_time: 5.,
            smoke: false,
            family: Vec::new(),
            duration: 10.,
            profile_status: None,
            profile_working_dir: None,
            profile_workload: None,
            color: ColorChoice::Auto,
            report_dir: None,
        }
    }
}

/// Parse numeric and cargo-aoc-style selectors.
pub fn parse_day(value: &str) -> Result<u8, String> {
    selector(value, "day", 25)
}

pub fn parse_part(value: &str) -> Result<u8, String> {
    selector(value, "part", 2)
}

fn selector(value: &str, prefix: &str, max: u8) -> Result<u8, String> {
    value
        .strip_prefix(prefix)
        .unwrap_or(value)
        .parse::<u8>()
        .ok()
        .filter(|n| (1..=max).contains(n))
        .ok_or_else(|| format!("expected {prefix} in 1..={max}"))
}

/// Prefer the unpadded input, falling back only when it is absent.
/// Permission and decoding failures must never be treated as cache misses.
pub fn input_path(manifest: &std::path::Path, year: u16, day: u8) -> std::io::Result<PathBuf> {
    let path = manifest.join(format!("input/{year}/day{day}.txt"));
    match std::fs::symlink_metadata(&path) {
        Ok(_) => Ok(path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let padded = path.with_file_name(format!("day{day:02}.txt"));
            match std::fs::symlink_metadata(&padded) {
                Ok(_) => Ok(padded),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(path),
                Err(e) => Err(e),
            }
        }
        Err(e) => Err(e),
    }
}

pub struct ExecutionPlan<'a> {
    pub selected: Vec<&'a Solution>,
    pub defaults: Vec<&'a Solution>,
    pub required: Vec<&'a Solution>,
    pub generators: Vec<&'a Generator>,
    filter: Option<regex::Regex>,
    full_label: String,
    families: Vec<ProfileFamily>,
    profiling: bool,
    workload: Option<String>,
}

impl ExecutionPlan<'_> {
    fn matches(&self, label: &str) -> bool {
        self.families.iter().any(|f| label.starts_with(f.prefix()))
            && self.filter.as_ref().is_none_or(|f| f.is_match(label))
            && self.workload.as_ref().is_none_or(|w| w == label)
    }

    fn labels(&self) -> Vec<String> {
        self.generators
            .iter()
            .map(|g| generator_label(g))
            .chain(
                self.selected
                    .iter()
                    .flat_map(|s| [solution_label(s, "solver"), solution_label(s, "end-to-end")]),
            )
            .chain(std::iter::once(self.full_label.clone()))
            .filter(|label| !label.is_empty() && self.matches(label))
            .collect()
    }
}

fn generator_label(g: &Generator) -> String {
    if g.part == 0 {
        format!("generator/day{}/{}", g.day, g.name)
    } else {
        format!("generator/day{}/part{}/{}", g.day, g.part, g.name)
    }
}

fn solution_label(s: &Solution, phase: &str) -> String {
    format!("{phase}/day{}/part{}/{}", s.day, s.part, s.name)
}

fn positive_seconds(value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n >= 1e-9 && *n <= 86400.)
        .ok_or_else(|| "expected seconds between 0.000000001 and 86400".into())
}

pub fn profile_seconds(value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n >= 1. && *n <= 86400.)
        .ok_or_else(|| "expected profiling seconds between 1 and 86400".into())
}

impl Registry {
    fn selected_days(&self, options: &Options) -> Vec<u8> {
        let mut days = options.day.clone();
        if days.is_empty()
            && !options.all_days
            && matches!(
                options.action,
                Action::Run | Action::Bench | Action::Profile
            )
        {
            days.extend(self.solutions.iter().map(|s| s.day).max());
        }

        days.sort_unstable();
        days.dedup();
        days
    }

    pub fn select(&self, options: &Options, variants: bool) -> Result<Vec<&Solution>, Error> {
        let days = self.selected_days(options);
        if options.input.is_some() && days.len() != 1 {
            return Err(Error::message("--input requires exactly one selected day"));
        }

        let mut candidates: Vec<_> = self
            .solutions
            .iter()
            .filter(|s| {
                (days.is_empty() || days.contains(&s.day))
                    && options.part.is_none_or(|p| s.part == p)
                    && options.solution.as_ref().is_none_or(|n| s.name == n)
            })
            .collect();
        candidates.sort_by_key(|s| (s.day, s.part, s.name));
        if candidates.is_empty()
            || days
                .iter()
                .any(|day| !candidates.iter().any(|s| s.day == *day))
        {
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
            let path = match &options.input {
                Some(path) => path.clone(),
                None => input_path(
                    std::path::Path::new(self.manifest_dir),
                    self.year,
                    solution.day,
                )
                .map_err(|e| Error::message(format!("day {} input path: {e}", solution.day)))?,
            };
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
        let plan = self.plan(options)?;
        let selected = plan.selected;
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
        let scope = self.scope(options);
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
        let defaults_options = Options {
            solution: None,
            all_solutions: false,
            ..options.clone()
        };
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

    pub fn plan(&self, options: &Options) -> Result<ExecutionPlan<'_>, Error> {
        if !matches!(options.action, Action::Bench | Action::Profile) {
            let selected = self.select(options, matches!(options.action, Action::List))?;
            return Ok(ExecutionPlan {
                required: selected.clone(),
                selected,
                defaults: Vec::new(),
                generators: Vec::new(),
                filter: None,
                full_label: String::new(),
                families: Vec::new(),
                profiling: false,
                workload: None,
            });
        }

        let profiling = matches!(options.action, Action::Profile);
        let families = if options.family.is_empty() {
            vec![if profiling {
                ProfileFamily::EndToEnd
            } else {
                ProfileFamily::Solver
            }]
        } else {
            options.family.clone()
        };
        let selected = self.select(options, !profiling)?;
        // Full-run benches always follow the runner's default selection within the scope.
        let defaults_options = Options {
            solution: None,
            all_solutions: false,
            ..options.clone()
        };
        let defaults = self.select(&defaults_options, false)?;
        let mut required = selected.clone();
        required.extend(defaults.iter().copied());
        required.sort_by_key(|s| (s.day, s.part, s.name));
        required.dedup_by_key(|s| (s.day, s.part, s.name));

        let generators: Vec<_> = self
            .generators
            .iter()
            .filter(|g| {
                options.part.is_none_or(|p| g.part == 0 || g.part == p)
                    && options
                        .solution
                        .as_ref()
                        .is_none_or(|n| g.name == "default" || g.name == n)
                    && required.iter().any(|s| s.day == g.day)
            })
            .collect();

        let filter = options
            .filter
            .as_ref()
            .map(|s| regex::Regex::new(s))
            .transpose()
            .map_err(|e| Error::message(format!("benchmark filter: {e}")))?;
        let full_label = format!(
            "full-run/{}",
            self.scope(&defaults_options).replace(' ', "-")
        );
        let mut plan = ExecutionPlan {
            selected,
            defaults,
            required,
            generators,
            filter,
            full_label,
            families,
            profiling,
            workload: options.profile_workload.clone(),
        };
        let labels = plan.labels();
        let count = labels.len();
        if count == 0 {
            return Err(Error::message(
                "no benchmarks match --filter or the selected families",
            ));
        }

        plan.generators
            .retain(|g| labels.contains(&generator_label(g)));
        plan.selected.retain(|s| {
            labels.contains(&solution_label(s, "solver"))
                || labels.contains(&solution_label(s, "end-to-end"))
                || plan.generators.iter().any(|g| g.day == s.day)
        });
        if !labels.contains(&plan.full_label) {
            plan.defaults.clear();
        }
        if plan.selected.is_empty() {
            plan.selected = plan.defaults.clone();
        }
        plan.required = plan.selected.clone();
        plan.required.extend(plan.defaults.iter().copied());
        plan.required.sort_by_key(|s| (s.day, s.part, s.name));
        plan.required.dedup_by_key(|s| (s.day, s.part, s.name));

        Ok(plan)
    }

    pub fn describe(&self, options: &Options) -> Result<serde_json::Value, Error> {
        let plan = self.plan(options)?;
        let mut days: Vec<_> = plan.required.iter().map(|s| s.day).collect();
        days.sort_unstable();
        days.dedup();
        let identity =
            |s: &&Solution| serde_json::json!({"day": s.day, "part": s.part, "name": s.name});

        Ok(serde_json::json!({
            "protocol": 1, "capabilities": ["input-plan-v1", "profiling-v1"], "year": self.year,
            "manifest_dir": self.manifest_dir, "action": match options.action {
                Action::Run => "run", Action::Bench => "bench", Action::List => "list", Action::Profile => "profile",
            },
            "selection": plan.selected.iter().map(identity).collect::<Vec<_>>(),
            "defaults": plan.defaults.iter().map(identity).collect::<Vec<_>>(),
            "required_input_days": days,
            "profile_workloads": if matches!(options.action, Action::Profile) { plan.labels() } else { Vec::new() },
        }))
    }

    pub fn bench(&self, options: &Options) -> Result<(), Error> {
        let plan = self.plan(options)?;
        let inputs = self.load_inputs(&plan.required, options)?;

        // Validate before Criterion measures, after planning has validated filters.
        for solution in &plan.required {
            if plan.matches(&solution_label(solution, "solver"))
                || plan.matches(&solution_label(solution, "end-to-end"))
                || (plan.matches(&plan.full_label)
                    && plan.defaults.iter().any(|s| std::ptr::eq(*s, *solution)))
            {
                (solution.execute)(&inputs[&solution.day])
                    .map_err(|e| e.context(solution.day, solution.part, solution.name))?;
            }
        }
        for generator in plan
            .generators
            .iter()
            .filter(|g| plan.matches(&generator_label(g)))
        {
            (generator.check)(&inputs[&generator.day])?;
        }

        let matches = |label: &str| plan.matches(label);
        let selected = &plan.selected;
        let defaults = &plan.defaults;
        let generators = &plan.generators;
        let full_label = &plan.full_label;
        let labels = plan.labels();
        let count = labels.len();

        if options.smoke {
            if plan.profiling {
                for label in &labels {
                    println!("Profiling workload validated: {label}");
                }
                return Ok(());
            }

            for g in generators.iter().filter(|g| matches(&generator_label(g))) {
                (g.check)(&inputs[&g.day])?;
                println!("Smoke: {}", generator_label(g));
            }
            for s in selected {
                for phase in ["solver", "end-to-end"] {
                    if matches(&solution_label(s, phase)) {
                        (s.execute)(&inputs[&s.day])
                            .map_err(|e| e.context(s.day, s.part, s.name))?;
                        println!("Smoke: {}", solution_label(s, phase));
                    }
                }
            }
            if matches(full_label) {
                for s in defaults {
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
        if plan.profiling {
            // Criterion 0.5 exposes this setter as a hidden API. Keep the
            // version-specific profiling integration here, beside configuration.
            criterion = criterion.profile_time(Some(Duration::from_secs_f64(options.duration)));
        }
        let selected_labels: Vec<_> = labels.iter().map(|label| regex::escape(label)).collect();
        criterion = criterion.with_filter(format!("^(?:{})$", selected_labels.join("|")));

        for generator in generators {
            (generator.bench)(&mut criterion, &inputs[&generator.day]);
        }
        for solution in selected {
            (solution.bench)(&mut criterion, &inputs[&solution.day]);
        }
        criterion.bench_function(full_label, |b| {
            b.iter(|| {
                for solution in defaults {
                    (solution.execute)(std::hint::black_box(&inputs[&solution.day]))
                        .unwrap_or_else(|e| panic!("{e}"));
                }
            })
        });
        if !plan.profiling {
            criterion.final_summary();
            println!("Benchmark reports: {}", report.display());
        }
        Ok(())
    }

    fn scope(&self, options: &Options) -> String {
        let mut scope = self.year.to_string();
        let days = self.selected_days(options);
        match days.as_slice() {
            [] => {}
            [day] => scope.push_str(&format!(" day {day}")),
            _ => {
                let days: Vec<_> = days.iter().map(u8::to_string).collect();
                scope.push_str(&format!(" days {}", days.join(",")));
            }
        }
        if let Some(part) = options.part {
            scope.push_str(&format!(" part {part}"));
        }
        if let Some(name) = &options.solution {
            scope.push_str(&format!(" [{name}]"));
        }
        scope
    }
}

pub fn entry(registry: Registry) -> i32 {
    let mut args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == "__describe") {
        if args.get(2).is_none_or(|arg| arg != "--protocol")
            || args.get(3).is_none_or(|arg| arg != "1")
        {
            eprintln!("error: unsupported describe protocol; expected --protocol 1");
            return 1;
        }
        args.drain(1..4);
        let options = match Options::try_parse_from(args) {
            Ok(options) => options,
            Err(error) => {
                let _ = error.print();
                return error.exit_code();
            }
        };
        return match registry.describe(&options) {
            Ok(description) => {
                println!("{description}");
                0
            }
            Err(error) => {
                eprintln!("error: {error}");
                1
            }
        };
    }

    let options = Options::parse_from(args);
    let result = if matches!(options.action, Action::Profile) {
        options
            .profile_working_dir
            .as_ref()
            .map_or(Ok(()), |directory| {
                std::env::set_current_dir(directory)
                    .map_err(|e| Error::message(format!("set profiling workload directory: {e}")))
            })
    } else {
        Ok(())
    };
    let result = result.and_then(|()| match options.action {
        Action::Run => registry.run(&options),
        Action::List => registry.list(&options),
        Action::Bench => registry.bench(&options),
        Action::Profile => registry.bench(&options),
    });
    let result = result.and_then(|()| {
        if matches!(options.action, Action::Profile) && !options.smoke {
            if let Some(path) = &options.profile_status {
                std::fs::write(path, b"ok\n").map_err(|e| {
                    Error::message(format!("write profiling completion status: {e}"))
                })?;
            }
        }
        Ok(())
    });
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("error: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p_selects_parts_for_runtime_entry_points() {
        for action in ["run", "list", "bench", "profile"] {
            let options =
                Options::try_parse_from(["aoc-harness", action, "-d", "1", "-p", "part2"]).unwrap();

            assert_eq!(options.part, Some(2));
        }
    }

    #[test]
    fn day_selection_accepts_single_comma_separated_and_repeated_days() {
        let cases: &[(&[&str], &[u8])] = &[
            (&["run", "--day", "4"], &[4]),
            (&["run", "--day", "1,4"], &[1, 4]),
            (&["--day", "1", "run", "--day", "4"], &[1, 4]),
            (&["run", "--day=1,4"], &[1, 4]),
        ];

        for (args, expected) in cases {
            let args = std::iter::once("aoc-harness").chain(args.iter().copied());

            let options = Options::try_parse_from(args).unwrap();

            assert_eq!(options.day, *expected);
            assert!(!options.all_days);
        }
    }

    #[test]
    fn day_selection_rejects_out_of_range_days_and_conflicting_selectors() {
        let cases: &[&[&str]] = &[
            &["run", "--days", "1,4"],
            &["run", "--day", "0"],
            &["run", "--day", "1,26"],
            &["run", "--day", "1,4", "--all-days"],
            &["run", "--all-days", "--input", "input.txt"],
        ];

        for args in cases {
            let input = std::iter::once("aoc-harness").chain(args.iter().copied());

            let result = Options::try_parse_from(input);

            assert!(result.is_err(), "unexpectedly accepted {args:?}");
        }
    }
}

#[cfg(test)]
mod plan_tests {
    use super::*;

    fn never_run(_: &str) -> Result<Invocation, Error> {
        panic!("describe ran a solver")
    }
    fn never_execute(_: &str) -> Result<(), Error> {
        panic!("describe executed a solver")
    }
    fn never_bench(_: &mut criterion::Criterion, _: &str) {
        panic!("describe benchmarked")
    }
    const fn solution(day: u8, name: &'static str, named: bool) -> Solution {
        Solution {
            day,
            part: 1,
            name,
            named,
            explicit_default: false,
            run: never_run,
            execute: never_execute,
            bench: never_bench,
        }
    }
    static SOLUTIONS: [Solution; 3] = [
        solution(1, "default", false),
        solution(2, "default", false),
        solution(2, "variant", true),
    ];

    #[test]
    fn describe_validates_plans_without_input_or_user_work() {
        let registry = Registry {
            year: 2025,
            manifest_dir: "/no/such/consumer",
            solutions: &SOLUTIONS,
            generators: &[],
        };
        let run = Options::default();
        let bench = Options {
            action: Action::Bench,
            day: vec![2],
            part: Some(1),
            solution: Some("variant".into()),
            ..Default::default()
        };

        let latest = registry.describe(&run).unwrap();
        let variants = registry.describe(&bench).unwrap();

        assert_eq!(latest["required_input_days"], serde_json::json!([2]));
        assert_eq!(latest["selection"][0]["name"], "default");
        assert_eq!(variants["selection"][0]["name"], "variant");
        assert_eq!(variants["defaults"], serde_json::json!([]));
        assert_eq!(registry.plan(&bench).unwrap().required.len(), 1);
        assert!(registry
            .describe(&Options {
                filter: Some("[".into()),
                ..bench.clone()
            })
            .is_err());
        assert!(registry
            .describe(&Options {
                filter: Some("no-family".into()),
                ..bench
            })
            .is_err());
        assert!(registry
            .describe(&Options {
                day: vec![25],
                ..run
            })
            .is_err());
    }

    #[test]
    fn profiling_selects_default_workloads_and_only_their_required_inputs() {
        let registry = Registry {
            year: 2025,
            manifest_dir: "/no/such/consumer",
            solutions: &SOLUTIONS,
            generators: &[],
        };
        let options = Options {
            action: Action::Profile,
            family: vec![ProfileFamily::Solver],
            ..Default::default()
        };

        let latest = registry.describe(&options).unwrap();
        let filtered = registry
            .describe(&Options {
                all_days: true,
                filter: Some("day1/".into()),
                ..options.clone()
            })
            .unwrap();
        let multiple = registry
            .describe(&Options {
                all_days: true,
                ..options.clone()
            })
            .unwrap();
        let full = registry
            .describe(&Options {
                all_days: true,
                family: vec![ProfileFamily::FullRun],
                ..options
            })
            .unwrap();

        assert_eq!(
            latest["profile_workloads"],
            serde_json::json!(["solver/day2/part1/default"])
        );
        assert_eq!(latest["selection"].as_array().unwrap().len(), 1);
        assert_eq!(filtered["required_input_days"], serde_json::json!([1]));
        assert_eq!(
            multiple["profile_workloads"],
            serde_json::json!(["solver/day1/part1/default", "solver/day2/part1/default"])
        );
        assert_eq!(
            full["profile_workloads"],
            serde_json::json!(["full-run/2025"])
        );
        assert_eq!(full["required_input_days"], serde_json::json!([1, 2]));
    }

    #[test]
    fn families_and_filters_select_the_same_workloads_for_benchmarks_and_profiles() {
        static GENERATORS: [Generator; 1] = [Generator {
            day: 2,
            part: 0,
            name: "default",
            bench: never_bench,
            check: never_execute,
        }];
        let registry = Registry {
            year: 2025,
            manifest_dir: "/no/such/consumer",
            solutions: &SOLUTIONS,
            generators: &GENERATORS,
        };

        for action in ["bench", "profile"] {
            let options = |args: &[&str]| {
                Options::try_parse_from(
                    [
                        "aoc-harness",
                        action,
                        "--day",
                        "2",
                        "--part",
                        "1",
                        "--solution",
                        "variant",
                    ]
                    .into_iter()
                    .chain(args.iter().copied()),
                )
                .unwrap()
            };
            let default = registry.plan(&options(&[])).unwrap().labels();

            assert_eq!(
                default,
                [format!(
                    "{}/day2/part1/variant",
                    if action == "bench" {
                        "solver"
                    } else {
                        "end-to-end"
                    }
                )]
            );

            let combined = registry
                .plan(&options(&[
                    "--family",
                    "generator,solver",
                    "--family",
                    "end-to-end,full-run",
                    "--family",
                    "solver",
                ]))
                .unwrap()
                .labels();

            assert_eq!(
                combined,
                [
                    "generator/day2/default",
                    "solver/day2/part1/variant",
                    "end-to-end/day2/part1/variant",
                    "full-run/2025-day-2-part-1",
                ]
            );

            let filtered = registry
                .plan(&options(&[
                    "--family",
                    "generator,solver",
                    "--filter",
                    "^generator/",
                ]))
                .unwrap()
                .labels();

            assert_eq!(filtered, ["generator/day2/default"]);
            assert!(registry
                .plan(&options(&["--family", "solver", "--filter", "^generator/"]))
                .is_err());
        }
    }
}
