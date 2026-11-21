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

#[cfg(test)]
mod tests {
    #[test]
    fn day_input_reads_this_modules_year() {
        let raw = aoc_harness::input!(1);

        assert_eq!(raw, "alpha\nbeta");
    }
}
