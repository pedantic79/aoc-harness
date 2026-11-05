use chrono::{DateTime, Datelike, FixedOffset, TimeZone, Utc};

pub(crate) struct EventCalendar;

impl EventCalendar {
    fn length(year: u16) -> Option<u8> {
        match year {
            2015..=2024 => Some(25),
            2025 => Some(12),
            _ => None,
        }
    }
}

pub(crate) fn select(
    year: Option<u16>,
    days: &[u8],
    all: bool,
    now: DateTime<Utc>,
) -> Result<(u16, Vec<u8>), String> {
    let est = now.with_timezone(&FixedOffset::west_opt(5 * 3600).unwrap());
    let year = year.unwrap_or(est.year() as u16);
    let mut days = days.to_vec();
    if all {
        let length = EventCalendar::length(year)
            .ok_or("unknown event calendar; use an explicit --day or --days list")?;
        days = (1..=length)
            .filter(|day| released(year, *day, now))
            .collect();
        if days.is_empty() {
            return Err("event has not released any inputs yet".into());
        }
    } else if days.is_empty() {
        let length =
            EventCalendar::length(year).ok_or("unknown event calendar; supply --day or --days")?;
        if est.year() != i32::from(year) || est.month() != 12 || est.day() > u32::from(length) {
            return Err(
                "outside the selected event's December release window; supply --day or --days"
                    .into(),
            );
        }
        days.push(est.day() as u8);
    }
    days.sort_unstable();
    days.dedup();
    validate(year, &days, now)?;
    Ok((year, days))
}

fn released(year: u16, day: u8, now: DateTime<Utc>) -> bool {
    Utc.with_ymd_and_hms(i32::from(year), 12, u32::from(day), 5, 0, 0)
        .single()
        .is_some_and(|release| now >= release)
}

pub(crate) fn validate(year: u16, days: &[u8], now: DateTime<Utc>) -> Result<(), String> {
    let current = now
        .with_timezone(&FixedOffset::west_opt(5 * 3600).unwrap())
        .year();
    if year < 2015 || i32::from(year) > current {
        return Err(format!("event year must be between 2015 and {current} EST"));
    }
    for &day in days {
        let max = EventCalendar::length(year).unwrap_or(25);
        if day == 0 || day > max {
            return Err(format!("day {day} is outside the {year} event (1-{max})"));
        }
        if !released(year, day, now) {
            return Err(format!(
                "{year} day {day} has not been released (midnight EST)"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(value: &str) -> DateTime<Utc> {
        value.parse().unwrap()
    }

    #[test]
    fn fixed_est_boundaries_and_year_specific_calendars() {
        let before = at("2025-12-01T04:59:59Z");
        let first = at("2025-12-01T05:00:00Z");
        let after = at("2026-11-05T00:00:00Z");

        assert!(select(None, &[], false, before).is_err());
        assert_eq!(select(None, &[], false, first).unwrap(), (2025, vec![1]));
        assert_eq!(select(Some(2025), &[], true, first).unwrap().1, [1]);
        assert_eq!(select(Some(2025), &[], true, after).unwrap().1.len(), 12);
        assert_eq!(select(Some(2024), &[], true, after).unwrap().1.len(), 25);
        assert!(select(Some(2025), &[13], false, after).is_err());
        assert!(select(Some(2026), &[], true, after).is_err());
        assert!(select(Some(2026), &[1], false, after).is_err());
        assert!(select(Some(2025), &[], false, after).is_err());
        assert!(validate(2014, &[1], after).is_err());
        assert!(validate(2027, &[1], after).is_err());
        assert_eq!(
            select(Some(2026), &[3, 1, 3], false, at("2026-12-04T05:00:00Z"))
                .unwrap()
                .1,
            [1, 3]
        );
    }
}
