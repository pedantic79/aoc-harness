mod private {
    #[aoc_harness::aoc_generator(day1)]
    fn parse(raw: &str) -> Option<Vec<&str>> {
        Some(raw.lines().collect())
    }

    #[aoc_harness::aoc(day1, part1)]
    fn solve(lines: &[&str]) -> Option<usize> {
        Some(lines.len())
    }
}

aoc_harness::aoc_main! { year = 2025 }
