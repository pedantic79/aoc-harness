use crate::__aoc_runtime::{aoc, aoc_generator};
use std::io;

#[aoc_generator(day@DAY@)]
fn parse(raw: &str) -> Vec<&str> {
    raw.lines().collect()
}

#[aoc(day@DAY@, part1)]
fn part1(_input: &[&str]) -> Result<usize, io::Error> {
    Err(io::Error::other("day @DAY@ part 1 is not implemented"))
}

#[aoc(day@DAY@, part2)]
fn part2(_input: &[&str]) -> Result<usize, io::Error> {
    Err(io::Error::other("day @DAY@ part 2 is not implemented"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "supply an example and its expected answer, then implement part 1"]
    fn example_part1() {
        let input = parse(""); // TODO: add the puzzle's example.

        let answer = part1(&input).unwrap();

        assert_eq!(answer, 0); // TODO: replace with the expected answer.
    }

    #[test]
    #[ignore = "supply an example and its expected answer, then implement part 2"]
    fn example_part2() {
        let input = parse(""); // TODO: add the puzzle's example.

        let answer = part2(&input).unwrap();

        assert_eq!(answer, 0); // TODO: replace with the expected answer.
    }
}
