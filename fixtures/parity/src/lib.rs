use aoc_harness::{aoc, aoc_generator};
use std::{
    borrow::{Borrow, BorrowMut},
    cell::Cell,
    io,
};

pub struct Owner<'a> {
    lines: Vec<&'a str>,
    visited: Cell<bool>,
}

impl<'a> Borrow<[&'a str]> for Owner<'a> {
    fn borrow(&self) -> &[&'a str] {
        &self.lines
    }
}

impl<'a> BorrowMut<[&'a str]> for Owner<'a> {
    fn borrow_mut(&mut self) -> &mut [&'a str] {
        &mut self.lines
    }
}

impl Borrow<Cell<bool>> for Owner<'_> {
    fn borrow(&self) -> &Cell<bool> {
        &self.visited
    }
}

#[aoc_generator(day1)]
fn parse<'a>(raw: &'a str) -> std::option::Option<Owner<'a>> {
    if raw == "no parser value" {
        std::thread::sleep(std::time::Duration::from_millis(1));
        return None;
    }
    Some(Owner {
        lines: raw.lines().collect(),
        visited: Cell::new(false),
    })
}

#[aoc(day1, part1, input = "borrow")]
fn shared<'a>(lines: &[&'a str]) -> Option<usize> {
    if lines.first() == Some(&"no solver value") {
        return None;
    }
    Some(lines.len())
}

#[aoc(day1, part1, interior, input = "borrow", bench_input = "fresh")]
fn interior(visited: &Cell<bool>) -> Option<usize> {
    assert!(
        !visited.replace(true),
        "shared projection reused mutated state"
    );
    Some(1)
}

#[aoc(day1, part2, input = "borrow")]
fn mutable<'a>(lines: &mut [&'a str]) -> Option<usize> {
    assert_eq!(lines[0], "alpha", "mutable projection reused state");
    lines[0] = "changed";
    Some(lines.len())
}

#[aoc(day1, part2, owned)]
fn owned(owner: Owner<'_>) -> Option<usize> {
    Some(owner.lines.len())
}

#[aoc_generator(day2, output = "value")]
fn optional_value(raw: &str) -> Option<&str> {
    if raw == "none" {
        None
    } else {
        Some(raw)
    }
}

#[aoc(day2, part1)]
fn deliberately_optional(value: &Option<&str>) -> usize {
    value.map(str::len).unwrap_or(99)
}

#[aoc_generator(day3)]
fn one_layer(raw: &str) -> Result<Option<&str>, io::Error> {
    Ok(Some(raw))
}

#[aoc(day3, part1)]
fn nested_value(value: &Option<&str>) -> usize {
    value.unwrap().len()
}

mod display_wrapper {
    use super::*;

    struct Option<T>(T);

    impl<T: std::fmt::Display> std::fmt::Display for Option<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.0.fmt(f)
        }
    }

    #[aoc(day4, part1, output = "value")]
    fn literal(raw: &str) -> Option<usize> {
        Option(raw.len())
    }
}

#[cfg(feature = "bonus")]
#[path = "extra.rs"]
mod extra;

aoc_harness::aoc_lib! { year = 2025 }

#[cfg(test)]
mod tests {
    use super::*;
    use aoc_harness::Options;

    #[test]
    fn options_projections_and_one_layer_normalization() {
        let registry = __aoc_registry();
        let selected = registry
            .select(
                &Options {
                    day: vec![1],
                    all_solutions: true,
                    ..Default::default()
                },
                false,
            )
            .unwrap();

        for solution in selected {
            let result = (solution.run)("alpha\nbeta").unwrap();

            assert!(["1", "2"].contains(&result.answer.to_string().as_str()));
            (solution.execute)("alpha\nbeta").unwrap();
        }
        for (day, raw, answer) in [(2, "none", "99"), (3, "abc", "3"), (4, "abcd", "4")] {
            let selected = registry
                .select(
                    &Options {
                        day: vec![day],
                        ..Default::default()
                    },
                    false,
                )
                .unwrap();
            let result = (selected[0].run)(raw).unwrap();

            assert_eq!(result.answer.to_string(), answer);
        }
    }

    #[test]
    fn none_reports_phase_and_preserves_partial_timings() {
        let registry = __aoc_registry();
        let selected = registry
            .select(
                &Options {
                    day: vec![1],
                    part: Some(1),
                    ..Default::default()
                },
                false,
            )
            .unwrap();

        let parser_error = (selected[0].run)("no parser value").err().unwrap();
        let solver_error = (selected[0].run)("no solver value").err().unwrap();

        assert!(parser_error
            .to_string()
            .contains("generator: returned None"));
        assert!(parser_error.generator >= std::time::Duration::from_millis(1));
        assert_eq!(parser_error.solver, std::time::Duration::ZERO);
        assert!(solver_error.to_string().contains("solver: returned None"));
        assert!(solver_error.solver > std::time::Duration::ZERO);
    }
}
