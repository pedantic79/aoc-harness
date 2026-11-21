use crate::common::numbers;
use aoc_harness::{aoc, aoc_generator};

// Private to this year; 2025 day 1 takes raw input under the same names.
struct Masses(Vec<u32>);

fn fuel(mass: u32) -> u32 {
    (mass / 3).saturating_sub(2)
}

#[aoc_generator(day1)]
fn parse(raw: &str) -> Masses {
    Masses(numbers(raw))
}

#[aoc(day1, part1)]
fn part1(masses: &Masses) -> u32 {
    masses.0.iter().map(|&m| fuel(m)).sum()
}

#[aoc(day1, part2)]
fn part2(masses: &Masses) -> u32 {
    let total = |mut mass: u32| {
        let mut sum = 0;
        while fuel(mass) > 0 {
            mass = fuel(mass);
            sum += mass;
        }
        sum
    };
    masses.0.iter().map(|&m| total(m)).sum()
}

#[aoc(day1, part2, fast, default)]
fn fast(masses: &Masses) -> u32 {
    part2(masses)
}

#[cfg(feature = "bonus")]
#[aoc(day1, part1, bonus)]
fn bonus(masses: &Masses) -> u32 {
    part1(masses) + 1000
}

#[cfg(test)]
mod tests {
    #[test]
    fn named_year_input_reads_this_years_file() {
        let raw = aoc_harness::input!(2019, 1);

        assert_eq!(raw, "12\n14\n1969");
    }

    #[test]
    fn day_input_reads_this_modules_year() {
        let raw = aoc_harness::input!(1);

        assert_eq!(raw, "12\n14\n1969");
    }
}
