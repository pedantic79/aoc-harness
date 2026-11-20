mod day01;

#[path = "puzzles/second.rs"]
mod day02;

#[cfg(feature = "bonus")]
mod day03 {
    #[aoc_harness::aoc(day3, part1)]
    fn part1(raw: &str) -> usize {
        raw.len()
    }
}
