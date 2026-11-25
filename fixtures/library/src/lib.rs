//! Synthetic, library-only consumer: no runner or benchmark targets.

use aoc_harness::{aoc, aoc_generator};

mod numbers {
    use super::*;
    use std::{cell::Cell, io};

    // Deliberately does not implement Clone.
    pub struct Numbers {
        values: Vec<i64>,
        visited: Cell<bool>,
    }

    #[aoc_generator(day1)]
    fn parse(raw: &str) -> Result<Numbers, std::num::ParseIntError> {
        let values = raw
            .split_whitespace()
            .map(str::parse)
            .collect::<Result<_, _>>()?;
        Ok(Numbers {
            values,
            visited: Cell::new(false),
        })
    }

    #[aoc_generator(day1, part2)]
    fn twice(raw: &str) -> Result<Numbers, std::num::ParseIntError> {
        let mut numbers = parse(raw)?;
        for value in &mut numbers.values {
            *value *= 2;
        }
        Ok(numbers)
    }

    #[aoc_generator(day1, part2, mutable)]
    fn hundreds(raw: &str) -> Result<Numbers, std::num::ParseIntError> {
        let mut numbers = parse(raw)?;
        for value in &mut numbers.values {
            *value += 100;
        }
        Ok(numbers)
    }

    #[aoc(day1, part1)]
    fn borrowed(numbers: &Numbers) -> i64 {
        numbers.values.iter().sum()
    }

    #[aoc(day1, part1, cached, bench_input = "fresh")]
    fn interior_mutation(numbers: &Numbers) -> i64 {
        assert!(
            !numbers.visited.replace(true),
            "input reused after interior mutation"
        );
        numbers.values.iter().sum()
    }

    #[aoc(day1, part2)]
    fn unnamed(numbers: &Numbers) -> i64 {
        numbers.values.iter().sum()
    }

    #[aoc(day1, part2, owned, default)]
    fn consuming(numbers: Numbers) -> Result<i64, io::Error> {
        if numbers.values.contains(&-2) {
            return Err(io::Error::other("negative sentinel"));
        }
        Ok(numbers.values.into_iter().sum())
    }

    #[aoc(day1, part2, mutable)]
    fn mutate(numbers: &mut Numbers) -> i64 {
        assert!(!numbers.visited.replace(true), "mutated input reused");
        let sum = numbers.values.iter().sum();
        numbers.values.clear();
        sum
    }
}

mod raw {
    use super::*;

    #[aoc(day2, part1)]
    fn text(raw: &str) -> usize {
        raw.len()
    }

    #[aoc(day2, part2, bytes)]
    fn bytes(raw: &[u8]) -> usize {
        raw.len()
    }
}

mod borrowed_input {
    use super::*;

    #[aoc_generator(day3)]
    fn lines(raw: &str) -> Vec<&str> {
        raw.lines().collect()
    }

    #[cfg(feature = "bonus")]
    #[aoc_generator(day3, part1)]
    fn doubled_lines(raw: &str) -> Vec<&str> {
        raw.lines().chain(raw.lines()).collect()
    }

    #[aoc(day3, part1)]
    fn borrowed_slice(lines: &[&str]) -> usize {
        lines.len()
    }

    #[aoc(day3, part2)]
    fn mutable_slice(lines: &mut [&str]) -> usize {
        assert_eq!(lines[0], "alpha", "input reused after mutation");
        lines[0] = "changed";
        lines.len()
    }
}

#[cfg(feature = "bonus")]
mod bonus {
    use super::*;

    #[aoc(day5, part1, only)]
    fn only(raw: &str) -> usize {
        raw.len() + 100
    }
}

aoc_harness::aoc_lib! { year = 2025 }

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::{Duration, Instant},
    };

    static GENERATIONS: AtomicUsize = AtomicUsize::new(0);
    static CLEANUPS: AtomicUsize = AtomicUsize::new(0);

    struct Guard;

    impl Drop for Guard {
        fn drop(&mut self) {
            std::thread::sleep(Duration::from_millis(3));
            CLEANUPS.fetch_add(1, Ordering::SeqCst);
        }
    }

    pub struct State {
        text: String,
        _guard: Guard,
    }

    #[aoc_generator(day20)]
    fn slow_parse(raw: &str) -> State {
        GENERATIONS.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(2));
        State {
            text: raw.to_owned(),
            _guard: Guard,
        }
    }

    #[aoc(day20, part1)]
    fn owned(state: State) -> usize {
        state.text.len()
    }

    #[aoc(day20, part2)]
    fn mutable(state: &mut State) -> usize {
        assert_eq!(state.text, "fresh");
        state.text.clear();
        state.text.len()
    }

    mod fallible_alias {
        use super::*;
        use std::io::{Error, Result};

        #[aoc_generator(day23)]
        fn parse(raw: &str) -> Result<Vec<&str>> {
            if raw == "bad" {
                return Err(Error::other("bad input"));
            }

            Ok(raw.lines().collect())
        }

        #[aoc(day23, part1)]
        fn solve(lines: &[&str]) -> Result<usize> {
            Ok(lines.len())
        }
    }

    /// Records which generator prepared a solution's input.
    pub struct Source(&'static str);

    mod generator_ranking {
        use super::*;

        #[aoc_generator(day24)]
        fn day(_: &str) -> Source {
            Source("day")
        }

        #[aoc_generator(day24, part2)]
        fn part(_: &str) -> Source {
            Source("part")
        }

        #[aoc_generator(day24, _, fast)]
        fn name(_: &str) -> Source {
            Source("name")
        }

        #[aoc_generator(day24, part1, fast)]
        fn exact(_: &str) -> Source {
            Source("exact")
        }

        #[aoc(day24, part1)]
        fn part1(source: &Source) -> &'static str {
            source.0
        }

        #[aoc(day24, part1, fast)]
        fn part1_fast(source: &Source) -> &'static str {
            source.0
        }

        #[aoc(day24, part1, slow)]
        fn part1_slow(source: &Source) -> &'static str {
            source.0
        }

        #[aoc(day24, part2)]
        fn part2(source: &Source) -> &'static str {
            source.0
        }

        #[aoc(day24, part2, fast)]
        fn part2_fast(source: &Source) -> &'static str {
            source.0
        }

        #[aoc(day24, part2, slow)]
        fn part2_slow(source: &Source) -> &'static str {
            source.0
        }
    }

    mod shared_generator {
        use super::*;

        #[aoc_generator(day25)]
        fn day(_: &str) -> Source {
            Source("day")
        }

        #[aoc_generator(day25, part1, sorted)]
        #[aoc_generator(day25, part2, heap)]
        fn shared(_: &str) -> Source {
            Source("shared")
        }

        #[aoc(day25, part1)]
        fn part1(source: &Source) -> &'static str {
            source.0
        }

        #[aoc(day25, part1, sorted)]
        fn part1_sorted(source: &Source) -> &'static str {
            source.0
        }

        #[aoc(day25, part2)]
        fn part2(source: &Source) -> &'static str {
            source.0
        }

        #[aoc(day25, part2, heap)]
        fn part2_heap(source: &Source) -> &'static str {
            source.0
        }
    }

    mod generator_by_name {
        use super::*;

        #[aoc_generator(day22)]
        fn day(_: &str) -> Source {
            Source("day")
        }

        #[aoc_generator(day22, part1, alpha)]
        fn alpha(_: &str) -> Source {
            Source("alpha")
        }

        #[aoc_generator(day22, _, beta)]
        fn beta(_: &str) -> Source {
            Source("beta")
        }

        #[aoc(day22, part1)]
        fn part1(source: &Source) -> &'static str {
            source.0
        }

        #[aoc(day22, part1, alpha)]
        fn part1_alpha(source: &Source) -> &'static str {
            source.0
        }

        #[aoc(day22, part1, alpha_1, generator = alpha)]
        fn part1_alpha_1(source: &Source) -> &'static str {
            source.0
        }

        #[aoc(day22, part1, alpha_2, generator = alpha)]
        fn part1_alpha_2(source: &Source) -> &'static str {
            source.0
        }

        #[aoc(day22, part1, strict, generator = beta)]
        fn part1_strict(source: &Source) -> &'static str {
            source.0
        }

        #[aoc(day22, part2, generator = beta)]
        fn part2(source: &Source) -> &'static str {
            source.0
        }

        #[aoc(day22, part2, own, generator = own)]
        fn part2_own(source: &Source) -> &'static str {
            source.0
        }

        #[aoc_generator(day22, part2, own)]
        fn own(_: &str) -> Source {
            Source("own")
        }
    }

    #[test]
    fn solutions_select_generators_by_registration_name() {
        let registry = __aoc_registry();
        let options = aoc_harness::Options {
            day: vec![22],
            ..Default::default()
        };

        let chosen: Vec<_> = registry
            .select(&options, true)
            .unwrap()
            .into_iter()
            .map(|s| (s.part, s.name, (s.run)("").unwrap().answer.to_string()))
            .collect();

        let expected = [
            (1, "alpha", "alpha"),
            (1, "alpha_1", "alpha"),
            (1, "alpha_2", "alpha"),
            (1, "default", "day"),
            (1, "strict", "beta"),
            (2, "default", "beta"),
            (2, "own", "own"),
        ];
        let expected: Vec<_> = expected
            .into_iter()
            .map(|(part, name, source)| (part, name, source.to_owned()))
            .collect();
        assert_eq!(chosen, expected);
    }

    #[test]
    fn stacked_generator_attributes_share_one_function() {
        let registry = __aoc_registry();
        let options = aoc_harness::Options {
            day: vec![25],
            ..Default::default()
        };

        let chosen: Vec<_> = registry
            .select(&options, true)
            .unwrap()
            .into_iter()
            .map(|s| (s.part, s.name, (s.run)("").unwrap().answer.to_string()))
            .collect();

        let expected = [
            (1, "default", "day"),
            (1, "sorted", "shared"),
            (2, "default", "day"),
            (2, "heap", "shared"),
        ];
        let expected: Vec<_> = expected
            .into_iter()
            .map(|(part, name, source)| (part, name, source.to_owned()))
            .collect();
        assert_eq!(chosen, expected);
    }

    #[test]
    fn generators_rank_name_over_part_over_day() {
        let registry = __aoc_registry();
        let options = aoc_harness::Options {
            day: vec![24],
            ..Default::default()
        };

        let chosen: Vec<_> = registry
            .select(&options, true)
            .unwrap()
            .into_iter()
            .map(|s| (s.part, s.name, (s.run)("").unwrap().answer.to_string()))
            .collect();

        let expected = [
            (1, "default", "day"),
            (1, "fast", "exact"),
            (1, "slow", "day"),
            (2, "default", "part"),
            (2, "fast", "name"),
            (2, "slow", "part"),
        ];
        let expected: Vec<_> = expected
            .into_iter()
            .map(|(part, name, source)| (part, name, source.to_owned()))
            .collect();
        assert_eq!(chosen, expected);
    }

    #[test]
    fn day_input_uses_the_registered_year_and_removes_the_final_newline() {
        let raw: String = aoc_harness::input!(2);

        assert_eq!(raw, "hello");
    }

    #[test]
    fn imported_result_alias_and_borrowed_parsed_values_work() {
        let registry = __aoc_registry();
        let options = aoc_harness::Options {
            day: vec![23],
            ..Default::default()
        };
        let selected = registry.select(&options, false).unwrap();

        let result = (selected[0].run)("one\ntwo").unwrap();
        let error = (selected[0].run)("bad").err().unwrap();

        assert_eq!(result.answer.to_string(), "2");
        assert!(error.to_string().contains("generator: bad input"));
    }

    #[test]
    fn typed_preparation_and_timings_include_each_generation_and_cleanup() {
        let registry = __aoc_registry();
        let options = aoc_harness::Options {
            day: vec![20],
            ..Default::default()
        };
        let selected = registry.select(&options, false).unwrap();
        GENERATIONS.store(0, Ordering::SeqCst);
        CLEANUPS.store(0, Ordering::SeqCst);

        let start = Instant::now();
        let results: Vec<_> = selected.iter().map(|s| (s.run)("fresh").unwrap()).collect();
        let elapsed = start.elapsed();
        let generation: Duration = results.iter().map(|r| r.generator).sum();

        assert_eq!(results[0].answer.to_string(), "5");
        assert_eq!(results[1].answer.to_string(), "0");
        assert_eq!(GENERATIONS.load(Ordering::SeqCst), 2);
        assert_eq!(CLEANUPS.load(Ordering::SeqCst), 2);
        assert!(generation >= Duration::from_millis(4));
        assert!(results[0].solver >= Duration::from_millis(3));
        assert!(elapsed >= generation + Duration::from_millis(6));

        for solution in selected {
            (solution.execute)("fresh").unwrap();
        }

        assert_eq!(GENERATIONS.load(Ordering::SeqCst), 4);
        assert_eq!(CLEANUPS.load(Ordering::SeqCst), 4);
    }
}
