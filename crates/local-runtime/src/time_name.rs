//! File names that carry a UTC time, `YYYY-MM-DDTHH-MM-SSZ`, followed by `-2`, `-3` and so on when
//! a name with that time already holds different content.
use crate::diagnostic_log::{civil_from_days, days_from_civil};

const LENGTH: usize = "YYYY-MM-DDTHH-MM-SSZ".len();

/// The name of `seconds` since the Unix epoch; `number` 1 is the plain time and every later one
/// appends `-<number>`.
pub(crate) fn format(seconds: u64, number: u32) -> String {
    let days = i64::try_from(seconds / 86_400).unwrap_or(i64::MAX);
    let (year, month, day) = civil_from_days(days);
    let second = seconds % 86_400;
    let time = format!(
        "{year:04}-{month:02}-{day:02}T{:02}-{:02}-{:02}Z",
        second / 3600,
        second / 60 % 60,
        second % 60,
    );
    if number <= 1 {
        time
    } else {
        format!("{time}-{number}")
    }
}

/// The second and number of a name [`format`] spells exactly; any other text is not such a name.
pub(crate) fn parse(text: &str) -> Option<(u64, u32)> {
    let time = text.get(..LENGTH)?;
    let number = match text.get(LENGTH..)? {
        "" => 1,
        suffix => {
            let digits = suffix.strip_prefix('-')?;
            if digits.starts_with('0') || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            digits.parse::<u32>().ok().filter(|number| *number >= 2)?
        }
    };
    let field = |range: std::ops::Range<usize>| -> Option<i64> {
        let digits = time.get(range)?;
        digits
            .bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| digits.parse().ok())
            .flatten()
    };
    let (year, month, day) = (field(0..4)?, field(5..7)?, field(8..10)?);
    let (hour, minute, second) = (field(11..13)?, field(14..16)?, field(17..19)?);
    if year < 1970 || !(1..=12).contains(&month) || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let days = u64::try_from(days_from_civil(year, month, day)).ok()?;
    let seconds = days * 86_400 + u64::try_from(hour * 3600 + minute * 60 + second).ok()?;
    // Only the exact spelling counts, which also refuses a day its month does not have.
    (format(seconds, number) == text).then_some((seconds, number))
}
