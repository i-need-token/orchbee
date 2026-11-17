//! The registry's compatibility rules, judged on this checkout: no generation leaves before its
//! compatibility window closes, and no sealed fixture changes.

use std::{
    fmt::Write as _,
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use orchbee_connection_handshake::GenerationRange;
use sha2::{Digest, Sha256};

use crate::{
    registry::{Contract, Registry, Supersession, repository},
    support::TestResult,
};

/// The shortest time a superseded generation stays supported after its superseding release.
const WINDOW_DAYS: i64 = 90;
/// The contracts whose superseded generations stay supported in every 1.x release.
const KEPT_THROUGH_MAJOR_VERSION: [&str; 3] = ["keeper", "obhr", "worker-host"];

#[test]
fn no_generation_leaves_inside_its_compatibility_window() -> TestResult {
    let registry = Registry::load()?;
    let today = i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() / 86_400)?;
    let violations: Vec<String> = registry
        .contracts()
        .flat_map(|(name, generations, superseded)| {
            window_violations(name, generations, superseded, today)
        })
        .collect();
    assert!(violations.is_empty(), "{}", violations.join("\n"));
    Ok(())
}

#[test]
fn the_window_check_counts_ninety_days_from_the_superseding_release() -> TestResult {
    // 2027-01-01 lies exactly 90 days before this day.
    let today = civil_days("2027-04-01").ok_or("2027-04-01 is a date")?;
    let admitted = |name: &str, entry: &str| -> Result<bool, serde_json::Error> {
        let contract: Contract = serde_json::from_str(entry)?;
        Ok(window_violations(name, contract.generations, &contract.superseded, today).is_empty())
    };
    let entry = |lowest: u16, highest: u16, superseded_on: &str| {
        format!(
            r#"{{"generations":{{"lowest":{lowest},"highest":{highest}}},"superseded":[{{"generation":1,"supersededIn":"1.2.0","supersededOn":"{superseded_on}"}}]}}"#
        )
    };

    assert!(admitted("obtf", &entry(2, 2, "2027-01-01"))?);
    assert!(!admitted("obtf", &entry(2, 2, "2027-01-02"))?, "day 89");
    assert!(
        !admitted(
            "obtf",
            r#"{"generations":{"lowest":2,"highest":2},"superseded":[]}"#
        )?,
        "a dropped generation names its supersession"
    );
    for kept in ["keeper", "obhr", "worker-host"] {
        assert!(
            !admitted(kept, &entry(2, 2, "2026-01-01"))?,
            "{kept} keeps its superseded generations"
        );
    }
    assert!(
        !admitted("obtf", &entry(1, 1, "2027-01-01"))?,
        "only a generation below the highest is superseded"
    );
    assert!(
        !admitted("obtf", &entry(1, 2, "2027-02-29"))?,
        "a supersession date names a real day"
    );
    Ok(())
}

#[test]
fn sealed_fixtures_keep_their_bytes() -> TestResult {
    let registry = Registry::load()?;
    let repository = repository();
    let mut violations = Vec::new();
    for pair in registry.frozen_fixtures.windows(2) {
        if let [earlier, later] = pair
            && earlier.path >= later.path
        {
            violations.push(format!(
                "frozen fixture {} must follow {} in strictly bytewise order",
                later.path, earlier.path
            ));
        }
    }
    for fixture in &registry.frozen_fixtures {
        let path = &fixture.path;
        let plain = path
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
            && !path.contains('\\');
        if !plain {
            violations.push(format!(
                "frozen fixture {path} is not a plain relative path"
            ));
            continue;
        }
        let file = path
            .split('/')
            .fold(repository.clone(), |file, component| file.join(component));
        match fs::symlink_metadata(&file) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => {
                violations.push(format!("frozen fixture {path} is not a regular file"));
                continue;
            }
            Err(error) => {
                violations.push(format!("frozen fixture {path}: {error}"));
                continue;
            }
        }
        if let Some(sealed) = &fixture.sha256 {
            let mut actual = String::with_capacity(64);
            for byte in Sha256::digest(fs::read(&file)?) {
                write!(actual, "{byte:02x}")?;
            }
            if actual != *sealed {
                violations.push(format!(
                    "{path} changed after a release sealed it: its SHA-256 is {actual}, sealed \
                     {sealed}. A frozen fixture never changes; new cases go into a new file \
                     (schemas/connection/README.md#frozen-fixtures)."
                ));
            }
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
    Ok(())
}

/// The ways one contract's supersessions break the registry rules on `today`, counted in days
/// since 1970-01-01 UTC: an entry that is out of order, not below the highest generation, or
/// dated on no real day, and a generation below the lowest that left without a supersession,
/// within the window, or from a contract that keeps every superseded generation.
fn window_violations(
    name: &str,
    generations: GenerationRange,
    superseded: &[Supersession],
    today: i64,
) -> Vec<String> {
    let mut violations = Vec::new();
    for (index, entry) in superseded.iter().enumerate() {
        let generation = entry.generation.get();
        if let Some(previous) = index.checked_sub(1).and_then(|index| superseded.get(index))
            && previous.generation >= entry.generation
        {
            violations.push(format!(
                "{name}: superseded generation {generation} must follow {} in ascending order",
                previous.generation.get()
            ));
        }
        if entry.generation >= generations.highest() {
            violations.push(format!(
                "{name}: generation {generation} is superseded, but it is not below the highest \
                 supported generation {}",
                generations.highest().get()
            ));
        }
        if civil_days(&entry.superseded_on).is_none() {
            violations.push(format!(
                "{name}: generation {generation} has supersededOn {}, which is not a YYYY-MM-DD \
                 date",
                entry.superseded_on
            ));
        }
    }
    for dropped in 1..generations.lowest().get() {
        if KEPT_THROUGH_MAJOR_VERSION.contains(&name) {
            violations.push(format!(
                "{name}: generation {dropped} is dropped, but a superseded {name} generation stays \
                 supported in every 1.x release"
            ));
            continue;
        }
        let Some(entry) = superseded
            .iter()
            .find(|entry| entry.generation.get() == dropped)
        else {
            violations.push(format!(
                "{name}: generation {dropped} is dropped without a superseded entry"
            ));
            continue;
        };
        if let Some(day) = civil_days(&entry.superseded_on)
            && today < day + WINDOW_DAYS
        {
            violations.push(format!(
                "{name}: generation {dropped} is dropped within {WINDOW_DAYS} days of its \
                 supersession in {} on {}",
                entry.superseded_in.as_str(),
                entry.superseded_on
            ));
        }
    }
    violations
}

/// Days since 1970-01-01 of a proleptic Gregorian `YYYY-MM-DD` date, or `None` when `text` names
/// no real day.
fn civil_days(text: &str) -> Option<i64> {
    let number = |start: usize, end: usize| {
        text.get(start..end)?
            .bytes()
            .try_fold(0_i64, |value, byte| {
                byte.is_ascii_digit()
                    .then(|| value * 10 + i64::from(byte - b'0'))
            })
    };
    if text.len() != 10 || text.get(4..5) != Some("-") || text.get(7..8) != Some("-") {
        return None;
    }
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if !(1..=month_days).contains(&day) {
        return None;
    }
    // Howard Hinnant's days_from_civil, with years starting in March.
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
}
