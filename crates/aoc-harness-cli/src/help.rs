use super::BuildOptions;
use clap::{Arg, ArgAction, Command, CommandFactory};

fn flag(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name)
        .long(name)
        .action(ArgAction::SetTrue)
        .help(help)
}

fn value(name: &'static str, value_name: &'static str, help: &'static str) -> Arg {
    Arg::new(name).long(name).value_name(value_name).help(help)
}

fn action_command(action: &'static str) -> Command {
    let (about, examples) = match action {
        "run" => (
            "Run solutions for the latest registered day by default",
            "Examples:\n  aoc-harness run\n  aoc-harness run --days 1,3\n  aoc-harness run --all-days\n  aoc-harness run --day 4 --part 2 --solution flood_fill",
        ),
        "list" => (
            "List all registered solution variants and mark their defaults; selects every day by default",
            "Examples:\n  aoc-harness list\n  aoc-harness list --days 1,3\n  aoc-harness list --day 4 --part 2",
        ),
        "bench" => (
            "Benchmark all solution variants and the default full run; selects every day by default",
            "Examples:\n  aoc-harness bench\n  aoc-harness bench --day 4 --smoke\n  aoc-harness bench --filter '^solver/' --sample-size 20",
        ),
        _ => unreachable!("known command"),
    };

    let mut command = BuildOptions::command()
        .name(action)
        .bin_name(format!("aoc-harness {action}"))
        .about(about)
        .after_help(format!(
            "{examples}\n\nAll options may appear before or after the command."
        ))
        .mut_arg("color", |arg| arg.hide(action != "bench"))
        .next_help_heading("Selection options")
        .arg(
            value(
                "day",
                "N[,N...]",
                "Select days (1-25, comma-separated or repeated; each day runs once)",
            )
            .visible_alias("days")
            .value_delimiter(',')
            .action(ArgAction::Append),
        )
        .arg(flag("all-days", "Select every registered day").conflicts_with("day"))
        .arg(value("part", "N", "Select part 1 or 2 (requires --day)").requires("day"))
        .arg(
            value(
                "solution",
                "NAME",
                "Select a named solution (requires --day and --part)",
            )
            .requires_all(["day", "part"]),
        );

    if action == "run" {
        command = command.arg(
            flag(
                "all-solutions",
                "Run every solution variant in the selected days instead of their defaults",
            )
            .conflicts_with("solution"),
        );
    }
    if action != "list" {
        command = command.arg(
            value(
                "input",
                "PATH",
                "Override input for exactly one selected day",
            )
            .requires("day")
            .conflicts_with("all-days"),
        );
    }
    if action == "bench" {
        command = command
            .next_help_heading("Benchmark options")
            .arg(value(
                "filter",
                "REGEX",
                "Filter Criterion benchmark family names",
            ))
            .arg(
                value("sample-size", "N", "Samples per benchmark (at least 10)")
                    .default_value("100"),
            )
            .arg(value("warm-up-time", "SECONDS", "Warmup time per benchmark").default_value("3"))
            .arg(
                value(
                    "measurement-time",
                    "SECONDS",
                    "Measurement time per benchmark",
                )
                .default_value("5"),
            )
            .arg(flag(
                "smoke",
                "Validate each matching family once without measurements or reports",
            ));
    }

    command
}

pub(super) fn command(action: Option<&str>) -> Command {
    match action {
        Some("run") => action_command("run"),
        Some("list") => action_command("list"),
        Some("bench") => action_command("bench"),
        _ => action_command("run")
            .name("aoc-harness")
            .bin_name("aoc-harness")
            .about("Run a library-only Advent of Code consumer")
            .after_help(
                "Omit the command to run the latest registered day.\nUse 'aoc-harness <COMMAND> --help' for command-specific options and examples.\nAll options may appear before or after the command.",
            )
            .subcommand(action_command("run"))
            .subcommand(action_command("list"))
            .subcommand(action_command("bench")),
    }
}
