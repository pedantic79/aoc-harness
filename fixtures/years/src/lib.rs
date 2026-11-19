//! Synthetic multi-year consumer: two event years register the same days,
//! parts, function names, and variants without colliding.

pub mod common;

mod y2019;
mod y2025;

aoc_harness::aoc_lib! {
    years = {
        2019 => y2019,
        2025 => y2025,
    },
    default_year = 2025,
}

#[cfg(test)]
mod tests {
    use super::*;
    use aoc_harness::Options;

    fn identities(year: u16) -> Vec<(u8, u8, &'static str)> {
        let registry = __aoc_events().registry(Some(year)).unwrap();
        let mut identities: Vec<_> = registry
            .solutions
            .iter()
            .map(|s| (s.day, s.part, s.name))
            .collect();
        identities.sort_unstable();
        identities
    }

    fn answer(year: u16, day: u8, part: u8, name: &str, raw: &str) -> String {
        let registry = __aoc_events().registry(Some(year)).unwrap();
        let options = Options {
            day: vec![day],
            part: Some(part),
            solution: Some(name.into()),
            ..Default::default()
        };
        let selected = registry.select(&options, false).unwrap();

        (selected[0].run)(raw).unwrap().answer.to_string()
    }

    #[test]
    fn each_year_has_its_own_registry_and_the_default_is_selected() {
        let events = __aoc_events();

        let default = __aoc_registry();

        assert_eq!(events.years(), [2019, 2025]);
        assert_eq!(events.default_year, Some(2025));
        assert_eq!(default.year, 2025);
        assert_eq!(events.registry(Some(2019)).unwrap().year, 2019);
        assert!(events
            .registry(Some(2018))
            .err()
            .unwrap()
            .to_string()
            .contains("event year 2018 is not registered; registered years: 2019, 2025"));
    }

    #[test]
    fn identical_identities_stay_within_their_year() {
        let mut expected_2019 = vec![(1, 1, "default"), (1, 2, "default"), (1, 2, "fast")];
        let mut expected_2025 = vec![
            (1, 1, "default"),
            (1, 2, "default"),
            (1, 2, "fast"),
            (2, 1, "default"),
        ];
        if cfg!(feature = "bonus") {
            expected_2019.push((1, 1, "bonus"));
            expected_2019.sort_unstable();
            expected_2025.push((3, 1, "default"));
        }

        assert_eq!(identities(2019), expected_2019);
        assert_eq!(identities(2025), expected_2025);
    }

    #[test]
    fn solutions_use_only_their_own_years_generators() {
        let raw = "12\n14\n1969";

        assert_eq!(answer(2019, 1, 1, "default", raw), "658");
        assert_eq!(answer(2019, 1, 2, "fast", raw), "970");
        assert_eq!(answer(2025, 1, 1, "default", raw), "3");
        assert_eq!(answer(2025, 1, 2, "default", raw), "10");
        assert_eq!(answer(2025, 2, 1, "default", "3 4\n5 6"), "42");
    }

    #[test]
    fn named_year_input_reads_each_years_file() {
        let older = aoc_harness::input!(2019, 1);
        let newer = aoc_harness::input!(2025, 1,);

        assert_eq!(older, "12\n14\n1969");
        assert_eq!(newer, "alpha\nbeta");
    }

    #[test]
    #[should_panic(expected = "event year 2018 is not registered")]
    fn named_year_input_rejects_unregistered_years() {
        aoc_harness::input!(2018, 1);
    }
}
