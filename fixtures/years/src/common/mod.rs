//! Utilities shared by every year. Holds no registrations.

pub fn numbers(raw: &str) -> Vec<u32> {
    raw.split_whitespace()
        .map(|n| n.parse().expect("number"))
        .collect()
}
