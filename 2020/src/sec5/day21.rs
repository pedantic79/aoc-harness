use std::{collections::{BTreeMap, HashMap, HashSet}, convert::TryFrom};

use itertools::Itertools;

use crate::utils::collections::{Intersections, ToLookup};
#[derive(Debug)]
struct Line<'a> {
    ingredients: HashSet<&'a str>,
    allergens: Vec<&'a str>,
}
impl<'a> TryFrom<&'a str> for Line<'a> {
    type Error = ();

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        let mut w = value.split_whitespace().peekable();
        let mut ingredients = HashSet::new();
        let mut allergens = Vec::new();
        while w.peek() != Some(&"(contains") {
            ingredients.insert(w.next().ok_or(())?);
        }
        assert_eq!(w.next().unwrap(), "(contains");
        for allergen in w {
            allergens.push(allergen.trim_end_matches(|c| "),".contains(c)));
        }
        Ok(Self { ingredients, allergens })
    }
}
fn get_map<'a>(lines: &[Line<'a>]) -> HashMap<&'a str, HashSet<&'a str>> {
    let allergen_to_collections: HashMap<&str, Vec<&HashSet<&str>>> = lines
        .iter()
        .flat_map(|l| l.allergens.iter().map(move |a| (*a, &l.ingredients)))
        .collect_lookup();
    allergen_to_collections
        .into_iter()
        .map(|(k, v)| (k, v.into_iter().intersections()))
        .collect::<HashMap<_, _>>()
}

#[aoc(day21, part1)]
pub fn p1(input: &str) -> usize {
    let lines = input.lines().map(Line::try_from).collect::<Result<Vec<_>,_>>().unwrap();
    let options = get_map(&lines);
    let could_have_allergen: HashSet<&str> = options.values().flat_map(HashSet::iter).copied().collect();
    lines
        .into_iter()
        .flat_map(|x| {
            x.ingredients
                .into_iter()
                .filter(|&x| !could_have_allergen.contains(x))
        })
        .count()
}

#[aoc(day21, part2)]
pub fn p2(input: &str) -> String {
    let lines = input.lines().map(Line::try_from).collect::<Result<Vec<_>,_>>().unwrap();
    let mut options = get_map(&lines);
    let mut known_allergen: BTreeMap<&str, &str> = BTreeMap::new();
    while !options.is_empty() {
        //find an allergen with only one option and remove it from other sets.
        let (&allergen, singleton_ingredient_set) = options.iter().find(|x| x.1.len() == 1).expect("Stuck while reducing options");
        let &ingredient = singleton_ingredient_set.iter().next().unwrap();
        known_allergen.insert(allergen, ingredient);
        options.remove(allergen);
        for x in options.values_mut() {
            x.remove(ingredient);
        }
    }
    known_allergen.values().join(",")
}
