use super::BuildOptions;
use clap::{Arg, ArgAction, Command, CommandFactory, FromArgMatches};
use std::{ffi::OsString, path::PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Run,
    List,
    Bench,
    Input,
    Generate,
    Credentials,
}

impl Action {
    pub fn name(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::List => "list",
            Self::Bench => "bench",
            Self::Input => "input",
            Self::Generate => "generate",
            Self::Credentials => "credentials",
        }
    }
}

pub(crate) struct Request {
    pub build: BuildOptions,
    pub action: Action,
    pub runtime: Vec<OsString>,
    pub days: Vec<u8>,
    pub year: Option<u16>,
    pub all: bool,
    pub generate: bool,
    pub dry_run: bool,
    pub download: bool,
    pub session_file: Option<PathBuf>,
    pub open: bool,
    pub smoke: bool,
}

fn flag(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name)
        .long(name)
        .action(ArgAction::SetTrue)
        .help(help)
}

fn value(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name).long(name).help(help)
}

fn action_command(action: Action) -> Command {
    let name = action.name();
    if action == Action::Credentials {
        return Command::new(name)
            .about("Inspect the user-managed session source without printing its value")
            .subcommand_required(true)
            .arg(
                value(
                    "session-file",
                    "Read this file instead of the environment or home file",
                )
                .global(true)
                .value_parser(clap::value_parser!(PathBuf)),
            )
            .subcommand(
                Command::new("status").about("Validate the selected session locally; no HTTP"),
            );
    }
    let about = match action {
        Action::Run => "Run solutions for the latest registered day by default",
        Action::List => "List every registered day and mark default variants",
        Action::Bench => "Benchmark the latest registered day, including variants and the full run",
        Action::Input => "Download missing puzzle inputs without compiling solutions",
        Action::Generate => {
            "Create day modules without credentials, network access, or compilation"
        }
        _ => unreachable!(),
    };
    let execution = matches!(action, Action::Run | Action::List | Action::Bench);
    let mut command = BuildOptions::command().name(name)
        .bin_name(format!("cargo aoc-harness {name}"))
        .about(about).version(env!("CARGO_PKG_VERSION"))
        .mut_arg("color", |arg| arg.hide(action != Action::Bench))
        .after_help(format!("Examples:\n  cargo aoc-harness {name} --day 4\n  cargo aoc-harness {name} --days 1,3\n\nAll options may appear before or after the command."))
        .next_help_heading("Selection options")
        .arg(value("day", "Days 1-25, numeric or dayN, comma-separated or repeated")
            .short('d').visible_alias("days").value_delimiter(',').action(ArgAction::Append)
            .value_parser(aoc_harness::parse_day));
    if execution {
        command = command
            .arg(flag("all-days", "Select every registered day").conflicts_with("day"))
            .arg(
                value("part", "Part 1 or 2, numeric or partN")
                    .requires("day")
                    .value_parser(aoc_harness::parse_part),
            )
            .arg(value("solution", "Named solution").requires_all(["day", "part"]));
    }
    if action == Action::Run {
        command =
            command.arg(flag("all-solutions", "Run every variant").conflicts_with("solution"));
    }
    if matches!(action, Action::Run | Action::Bench) {
        command = command
            .arg(flag(
                "debug-info",
                "Include debug symbols without changing optimization or stripping them",
            ))
            .arg(
                value("input", "Alternate input for exactly one selected day")
                    .short('i')
                    .value_parser(clap::value_parser!(PathBuf))
                    .requires("day")
                    .conflicts_with_all(["all-days", "download"]),
            )
            .arg(flag(
                "download",
                "Fetch missing default inputs before executing",
            ))
            .arg(
                value("session-file", "Override the shared session source")
                    .value_parser(clap::value_parser!(PathBuf))
                    .requires("download"),
            );
    }
    if matches!(action, Action::Run | Action::Input) {
        let mut arg = flag(
            "generate",
            "Create missing day modules using the shared scaffolder",
        );
        if action == Action::Run {
            arg = arg.requires("day");
        }
        command = command.arg(arg);
    }
    if action == Action::Input {
        command = command
            .arg(
                value("year", "Event year; defaults to the current EST year")
                    .value_parser(clap::value_parser!(u16).range(2015..)),
            )
            .arg(
                flag("all", "Download all released days of a known event")
                    .requires("year")
                    .conflicts_with("day"),
            )
            .arg(
                value("session-file", "Override the shared session source")
                    .value_parser(clap::value_parser!(PathBuf)),
            )
            .arg(
                flag(
                    "dry-run",
                    "Preview generated source without downloading or writing",
                )
                .requires("generate"),
            );
    }
    if action == Action::Generate {
        command = command.mut_arg("day", |arg| arg.required(true)).arg(flag(
            "dry-run",
            "Print proposed paths and a diff without writing",
        ));
    }
    if action == Action::Bench {
        command = command
            .next_help_heading("Benchmark options")
            .arg(value("filter", "Regex over Criterion family names"))
            .arg(
                value("sample-size", "Samples per benchmark (at least 10)")
                    .value_parser(clap::value_parser!(u64).range(10..)),
            )
            .arg(value("warm-up-time", "Warmup seconds").value_parser(seconds))
            .arg(value("measurement-time", "Measurement seconds").value_parser(seconds))
            .arg(flag(
                "smoke",
                "Validate families without measurements or reports",
            ))
            .arg(
                flag("open", "Open the successful measured HTML report")
                    .short('o')
                    .conflicts_with("smoke"),
            );
    }
    command
}

fn seconds(value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 1e-9 && *v <= 86400.)
        .ok_or_else(|| "expected seconds between 0.000000001 and 86400".into())
}

const ACTIONS: [Action; 6] = [
    Action::Run,
    Action::List,
    Action::Bench,
    Action::Input,
    Action::Generate,
    Action::Credentials,
];

// Use the same clap schema to identify option values while moving a command to
// the front. This preserves before/after options and values named "run" or "bench".
pub(crate) fn parse(mut args: Vec<OsString>) -> Result<Request, clap::Error> {
    let schemas: Vec<_> = ACTIONS.into_iter().map(action_command).collect();
    let mut takes_value = std::collections::HashSet::new();
    for schema in &schemas {
        for arg in schema.get_arguments() {
            if arg.get_action().takes_values() {
                if let Some(long) = arg.get_long() {
                    takes_value.insert(format!("--{long}"));
                }
                if let Some(aliases) = arg.get_visible_aliases() {
                    for alias in aliases {
                        takes_value.insert(format!("--{alias}"));
                    }
                }
                if let Some(short) = arg.get_short() {
                    takes_value.insert(format!("-{short}"));
                }
            }
        }
    }
    let mut action = Action::Run;
    let mut position = None;
    let mut skip = false;
    for (index, arg) in args.iter().enumerate() {
        if skip {
            skip = false;
            continue;
        }
        let text = arg.to_string_lossy();
        if takes_value.contains(text.as_ref()) {
            skip = true;
            continue;
        }
        if let Some(found) = ACTIONS.iter().find(|a| a.name() == text) {
            action = *found;
            position = Some(index);
            break;
        }
    }
    if args.first().is_some_and(|a| a == "help") {
        if let Some(index) = position {
            args.remove(index);
        }
        args.remove(0);
        args.push("--help".into());
    } else if let Some(index) = position {
        args.remove(index);
    }
    let overview = position.is_none() && args.iter().any(|a| a == "--help" || a == "-h");
    let mut schema = action_command(action);
    if position.is_none() {
        schema = schema.name("aoc-harness").bin_name("cargo aoc-harness");
    }
    if overview {
        schema = schema.name("aoc-harness").bin_name("cargo aoc-harness")
            .after_help("Omit the command to run the latest registered day. All options may appear before or after the command.");
        for command in schemas {
            schema = schema.subcommand(command);
        }
    }
    let matches =
        schema.try_get_matches_from(std::iter::once(OsString::from("aoc-harness")).chain(args))?;
    let get_flag = |name| {
        matches
            .try_get_one::<bool>(name)
            .ok()
            .flatten()
            .copied()
            .unwrap_or(false)
    };
    let get_path = |name| matches.try_get_one::<PathBuf>(name).ok().flatten().cloned();
    let mut days: Vec<u8> = matches
        .try_get_many::<u8>("day")
        .ok()
        .flatten()
        .into_iter()
        .flatten()
        .copied()
        .collect();
    days.sort_unstable();
    days.dedup();
    let mut build = if action == Action::Credentials {
        BuildOptions::default()
    } else {
        BuildOptions::from_arg_matches(&matches)?
    };
    if build.profile.is_empty() {
        build.profile = "release".into();
    }
    build.debug_info = get_flag("debug-info");
    let mut session_file = get_path("session-file");
    if let Some(("status", status)) = matches.subcommand() {
        session_file = status.get_one::<PathBuf>("session-file").cloned();
    }
    let mut runtime: Vec<OsString> = vec![action.name().into()];
    if !days.is_empty() {
        runtime.push("--day".into());
        runtime.push(
            days.iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(",")
                .into(),
        );
    }
    for name in ["all-days", "all-solutions", "smoke"] {
        if get_flag(name) {
            runtime.push(format!("--{name}").into());
        }
    }
    if let Some(part) = matches.try_get_one::<u8>("part").ok().flatten() {
        runtime.push("--part".into());
        runtime.push(part.to_string().into());
    }
    for name in [
        "sample-size",
        "warm-up-time",
        "measurement-time",
        "solution",
        "filter",
        "input",
    ] {
        if let Some(values) = matches.try_get_raw(name).ok().flatten() {
            runtime.push(format!("--{name}").into());
            runtime.extend(values.map(OsString::from));
        }
    }
    if let Some(filter) = matches.try_get_one::<String>("filter").ok().flatten() {
        // Validate syntax before any Cargo build, source writes, or HTTP.
        regex::Regex::new(filter).map_err(|e| {
            clap::Error::raw(
                clap::error::ErrorKind::ValueValidation,
                format!("benchmark filter: {e}"),
            )
        })?;
    }
    if get_path("input").is_some() && days.len() != 1 {
        return Err(clap::Error::raw(
            clap::error::ErrorKind::ArgumentConflict,
            "--input requires exactly one selected day",
        ));
    }
    Ok(Request {
        build,
        action,
        runtime,
        days,
        year: matches.try_get_one::<u16>("year").ok().flatten().copied(),
        all: get_flag("all"),
        generate: get_flag("generate"),
        dry_run: get_flag("dry-run"),
        download: get_flag("download"),
        session_file,
        open: get_flag("open"),
        smoke: get_flag("smoke"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_aliases_lower_to_older_runtime_numeric_arguments() {
        let request = parse(
            ["-d", "day4", "run", "--part", "part2", "-i", "bench"]
                .map(OsString::from)
                .to_vec(),
        )
        .unwrap();

        assert_eq!(
            request.runtime,
            ["run", "--day", "4", "--part", "2", "--input", "bench"].map(OsString::from)
        );
    }
}
