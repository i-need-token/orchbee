//! Parses a TZif time zone file (RFC 8536 and RFC 9636) and answers the UTC offset in force at an
//! instant: from its transitions, and past the last one from the POSIX `TZ` rule in its footer.
//! Leap-second records are skipped, so a zone that counts leap seconds reads that many seconds early.

use crate::civil_date::{civil_from_days, days_from_civil};

/// A parsed zone.
#[derive(Debug)]
pub(super) struct Zone {
    /// Transition instants in Unix seconds, strictly ascending, each with the offset it starts.
    transitions: Vec<(i64, i32)>,
    /// The offset of time type 0, which applies before the first transition.
    initial: i32,
    /// The footer rule, which applies from the last transition on, or at every instant of a zone
    /// without transitions; `None` for a version 1 file or an empty footer.
    rule: Option<Rule>,
}

impl Zone {
    /// Parses a whole TZif file; any malformed part, including a footer rule this reader cannot
    /// evaluate, refuses the file rather than guessing an offset.
    pub(super) fn parse(bytes: &[u8]) -> Option<Self> {
        let mut input = bytes;
        let (version, counts) = header(&mut input)?;
        if version == 0 {
            let (transitions, initial) = block(&mut input, &counts, 4)?;
            return Some(Self {
                transitions,
                initial,
                rule: None,
            });
        }
        skip_block(&mut input, &counts)?;
        let (_, counts) = header(&mut input)?;
        let (transitions, initial) = block(&mut input, &counts, 8)?;
        let footer = input.strip_prefix(b"\n")?;
        let end = footer.iter().position(|&byte| byte == b'\n')?;
        let text = footer.get(..end)?;
        let rule = if text.is_empty() {
            None
        } else {
            Some(Rule::parse(text)?)
        };
        Some(Self {
            transitions,
            initial,
            rule,
        })
    }

    /// The offset east of UTC in force at `seconds` after the Unix epoch.
    pub(super) fn offset_at(&self, seconds: i64) -> Option<i32> {
        let passed = self
            .transitions
            .partition_point(|&(instant, _)| instant <= seconds);
        if passed == self.transitions.len()
            && let Some(rule) = &self.rule
        {
            return rule.offset_at(seconds);
        }
        Some(match passed.checked_sub(1) {
            None => self.initial,
            Some(last) => self.transitions.get(last)?.1,
        })
    }

    /// An instant whose local second is exactly `local`, if it is unique.
    #[cfg(unix)]
    pub(super) fn at_local(&self, local: i64) -> Option<i64> {
        let mut offsets = vec![self.initial];
        offsets.extend(self.transitions.iter().map(|&(_, offset)| offset));
        if let Some(rule) = &self.rule {
            offsets.push(rule.standard);
            offsets.extend(rule.daylight.as_ref().map(|daylight| daylight.offset));
        }
        offsets.sort_unstable();
        offsets.dedup();
        let mut result = None;
        for offset in offsets {
            let candidate = local.checked_sub(i64::from(offset))?;
            if self.offset_at(candidate)? == offset && result.replace(candidate).is_some() {
                return None;
            }
        }
        result
    }
}

/// The counts a TZif header declares for the data block after it.
struct Counts {
    utc_indicators: usize,
    standard_indicators: usize,
    leap_seconds: usize,
    transitions: usize,
    types: usize,
    designations: usize,
}

const HEADER: usize = 44;
/// Offsets beyond 25 hours west or 26 hours east are not ones any zone uses.
const OFFSET_RANGE: std::ops::RangeInclusive<i32> = -89_999..=93_599;

fn take<'a>(input: &mut &'a [u8], count: usize) -> Option<&'a [u8]> {
    let (head, rest) = input.split_at_checked(count)?;
    *input = rest;
    Some(head)
}

fn header(input: &mut &[u8]) -> Option<(u8, Counts)> {
    let header = take(input, HEADER)?;
    if header.get(..4)? != b"TZif" {
        return None;
    }
    let count = |index: usize| -> Option<usize> {
        let start = 20 + 4 * index;
        let field = header.get(start..start + 4)?.try_into().ok()?;
        usize::try_from(u32::from_be_bytes(field)).ok()
    };
    let counts = Counts {
        utc_indicators: count(0)?,
        standard_indicators: count(1)?,
        leap_seconds: count(2)?,
        transitions: count(3)?,
        types: count(4)?,
        designations: count(5)?,
    };
    Some((*header.get(4)?, counts))
}

/// Skips the version 1 data block that precedes the 64-bit one.
fn skip_block(input: &mut &[u8], counts: &Counts) -> Option<()> {
    let length = counts
        .transitions
        .checked_mul(5)?
        .checked_add(counts.types.checked_mul(6)?)?
        .checked_add(counts.designations)?
        .checked_add(counts.leap_seconds.checked_mul(8)?)?
        .checked_add(counts.standard_indicators)?
        .checked_add(counts.utc_indicators)?;
    take(input, length).map(|_| ())
}

/// Reads one data block whose times are `time_size` bytes: its transitions and type 0's offset.
fn block(input: &mut &[u8], counts: &Counts, time_size: usize) -> Option<(Vec<(i64, i32)>, i32)> {
    let times = take(input, counts.transitions.checked_mul(time_size)?)?;
    let indices = take(input, counts.transitions)?;
    let types = take(input, counts.types.checked_mul(6)?)?;
    take(input, counts.designations)?;
    take(input, counts.leap_seconds.checked_mul(time_size + 4)?)?;
    take(input, counts.standard_indicators)?;
    take(input, counts.utc_indicators)?;
    let offsets = types
        .chunks_exact(6)
        .map(|record| {
            let offset = i32::from_be_bytes(record.get(..4)?.try_into().ok()?);
            OFFSET_RANGE.contains(&offset).then_some(offset)
        })
        .collect::<Option<Vec<_>>>()?;
    let initial = *offsets.first()?;
    let transitions = times
        .chunks_exact(time_size)
        .zip(indices)
        .map(|(time, &index)| {
            let instant = match time_size {
                8 => i64::from_be_bytes(time.try_into().ok()?),
                _ => i64::from(i32::from_be_bytes(time.try_into().ok()?)),
            };
            Some((instant, *offsets.get(usize::from(index))?))
        })
        .collect::<Option<Vec<_>>>()?;
    transitions
        .windows(2)
        .all(|pair| matches!(pair, [(earlier, _), (later, _)] if earlier < later))
        .then_some((transitions, initial))
}

/// A POSIX `TZ` rule with the RFC 9636 extensions to transition times: hours from -167 to 167.
#[derive(Debug)]
struct Rule {
    /// Standard time's offset east of UTC.
    standard: i32,
    daylight: Option<Daylight>,
}

#[derive(Debug)]
struct Daylight {
    /// Daylight time's offset east of UTC.
    offset: i32,
    /// When daylight time starts, in standard local time.
    start: Change,
    /// When daylight time ends, in daylight local time.
    end: Change,
}

#[derive(Clone, Copy, Debug)]
struct Change {
    day: Day,
    /// Seconds after the local midnight that starts `day`; negative or beyond a day is allowed.
    time: i32,
}

#[derive(Clone, Copy, Debug)]
enum Day {
    /// `Jn`: day 1 to 365, never counting February 29.
    Julian(i64),
    /// `n`: day 0 to 365, counting February 29.
    Ordinal(i64),
    /// `Mm.w.d`: weekday `d` (0 is Sunday) of week `w` of month `m`; week 5 is the last.
    Weekday { month: i64, week: i64, weekday: i64 },
}

impl Rule {
    fn parse(text: &[u8]) -> Option<Self> {
        let mut input = text;
        name(&mut input)?;
        // POSIX offsets count hours west of UTC.
        let standard = -clock(&mut input, 24)?;
        if input.is_empty() {
            return Some(Self {
                standard,
                daylight: None,
            });
        }
        name(&mut input)?;
        let offset = if input.first().is_some_and(|&byte| byte != b',') {
            -clock(&mut input, 24)?
        } else {
            standard.checked_add(3600)?
        };
        // A footer that names daylight time always states when it starts and ends.
        let mut input = input.strip_prefix(b",")?;
        let start = change(&mut input)?;
        let mut input = input.strip_prefix(b",")?;
        let end = change(&mut input)?;
        input.is_empty().then_some(Self {
            standard,
            daylight: Some(Daylight { offset, start, end }),
        })
    }

    fn offset_at(&self, seconds: i64) -> Option<i32> {
        let Some(daylight) = &self.daylight else {
            return Some(self.standard);
        };
        let local = seconds.checked_add(i64::from(self.standard))?;
        let (year, _, _) = civil_from_days(local.div_euclid(86_400));
        let start = daylight
            .start
            .local_instant(year)?
            .checked_sub(i64::from(self.standard))?;
        let end = daylight
            .end
            .local_instant(year)?
            .checked_sub(i64::from(daylight.offset))?;
        // Southern zones start daylight time late in the year and end it early in the next.
        let in_daylight = if start <= end {
            start <= seconds && seconds < end
        } else {
            seconds >= start || seconds < end
        };
        Some(if in_daylight {
            daylight.offset
        } else {
            self.standard
        })
    }
}

impl Change {
    /// This change in `year`, in seconds since the epoch as if local time were UTC.
    fn local_instant(self, year: i64) -> Option<i64> {
        self.day
            .of(year)
            .checked_mul(86_400)?
            .checked_add(i64::from(self.time))
    }
}

impl Day {
    /// Days since 1970-01-01 of this day in `year`.
    fn of(self, year: i64) -> i64 {
        let january_first = days_from_civil(year, 1, 1);
        match self {
            Self::Julian(day) => january_first + day - 1 + i64::from(leap_year(year) && day >= 60),
            Self::Ordinal(day) => january_first + day,
            Self::Weekday {
                month,
                week,
                weekday,
            } => {
                let first = days_from_civil(year, month, 1);
                let next_month = if month == 12 {
                    days_from_civil(year + 1, 1, 1)
                } else {
                    days_from_civil(year, month + 1, 1)
                };
                // 1970-01-01 was a Thursday.
                let first_weekday = (first + 4).rem_euclid(7);
                let mut day = first + (weekday - first_weekday).rem_euclid(7) + (week - 1) * 7;
                while day >= next_month {
                    day -= 7;
                }
                day
            }
        }
    }
}

fn leap_year(year: i64) -> bool {
    year.rem_euclid(4) == 0 && (year.rem_euclid(100) != 0 || year.rem_euclid(400) == 0)
}

/// A zone abbreviation: letters, or `<...>` around letters, digits, `+` and `-`.
fn name(input: &mut &[u8]) -> Option<()> {
    let length = if let Some(quoted) = input.strip_prefix(b"<") {
        let inside = quoted
            .iter()
            .position(|&byte| byte == b'>')
            .filter(|&inside| inside > 0)?;
        let valid = quoted
            .get(..inside)?
            .iter()
            .all(|&byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-'));
        valid.then_some(inside + 2)?
    } else {
        Some(
            input
                .iter()
                .take_while(|byte| byte.is_ascii_alphabetic())
                .count(),
        )
        .filter(|&letters| letters > 0)?
    };
    take(input, length).map(|_| ())
}

/// `[+-]hh[:mm[:ss]]` as signed seconds, with at most `hour_limit` hours.
fn clock(input: &mut &[u8], hour_limit: i32) -> Option<i32> {
    let sign = match input.first() {
        Some(b'-') => -1,
        Some(b'+') => 1,
        _ => 0,
    };
    if sign != 0 {
        take(input, 1)?;
    }
    let hours = number(input, 3).filter(|&hours| hours <= hour_limit)?;
    let mut seconds = hours * 3600;
    for scale in [60, 1] {
        let Some(rest) = input.strip_prefix(b":") else {
            break;
        };
        *input = rest;
        seconds += number(input, 2).filter(|&part| part < 60)? * scale;
    }
    Some(if sign < 0 { -seconds } else { seconds })
}

/// One to `digits` decimal digits.
fn number(input: &mut &[u8], digits: usize) -> Option<i32> {
    let length = input
        .iter()
        .take(digits)
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    let text = take(input, length).filter(|text| !text.is_empty())?;
    Some(
        text.iter()
            .fold(0, |value, &digit| value * 10 + i32::from(digit - b'0')),
    )
}

/// `date[/time]`; the time defaults to 02:00.
fn change(input: &mut &[u8]) -> Option<Change> {
    let day = match input.first()? {
        b'J' => {
            take(input, 1)?;
            Day::Julian(i64::from(
                number(input, 3).filter(|day| (1..=365).contains(day))?,
            ))
        }
        b'M' => {
            take(input, 1)?;
            let month = number(input, 2).filter(|month| (1..=12).contains(month))?;
            *input = input.strip_prefix(b".")?;
            let week = number(input, 1).filter(|week| (1..=5).contains(week))?;
            *input = input.strip_prefix(b".")?;
            let weekday = number(input, 1).filter(|weekday| (0..=6).contains(weekday))?;
            Day::Weekday {
                month: i64::from(month),
                week: i64::from(week),
                weekday: i64::from(weekday),
            }
        }
        _ => Day::Ordinal(i64::from(
            number(input, 3).filter(|day| (0..=365).contains(day))?,
        )),
    };
    let time = match input.strip_prefix(b"/") {
        Some(rest) => {
            *input = rest;
            clock(input, 167)?
        }
        None => 7200,
    };
    Some(Change { day, time })
}

#[cfg(test)]
mod tests;
