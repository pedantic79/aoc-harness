use aoc_harness::{aoc, aoc_generator};

#[aoc_generator(day2)]
fn parse(raw: &str) -> Vec<(u32, u32)> {
    let numbers = crate::common::numbers(raw);
    numbers.chunks(2).map(|pair| (pair[0], pair[1])).collect()
}

#[aoc(day2, part1)]
fn part1(pairs: &[(u32, u32)]) -> u32 {
    pairs.iter().map(|(a, b)| a * b).sum()
}
