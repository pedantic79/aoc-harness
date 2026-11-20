use aoc_harness::aoc;

// Same day, parts, function names, and variant as 2019, without a generator.
#[aoc(day1, part1)]
fn part1(raw: &str) -> usize {
    raw.lines().count()
}

#[aoc(day1, part2)]
fn part2(raw: &[u8]) -> usize {
    raw.len()
}

#[aoc(day1, part2, fast, default)]
fn fast(raw: &str) -> usize {
    raw.len()
}
