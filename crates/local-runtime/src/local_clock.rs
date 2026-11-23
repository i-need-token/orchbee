//! Converts between Unix-millisecond instants and the host's local wall clock, applying the
//! system time zone's rules in force at that instant. Windows asks the system; macOS and Linux read
//! the zone file `/etc/localtime` links to and, like Windows, ignore `TZ`. A host whose zone cannot
//! be read offers no local clock, so a caller shows only what it can derive without one.

#[cfg(unix)]
mod system_zone;
#[cfg(any(unix, test))]
mod tzif;

/// A local date and minute.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalClock {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
}

/// The local wall clock at `unix_milliseconds`, or `None` when the host cannot convert it.
#[must_use]
pub fn local_clock(unix_milliseconds: u64) -> Option<LocalClock> {
    platform(unix_milliseconds)
}

/// The start of a local minute on this host. A skipped or repeated minute has no unique instant.
#[must_use]
pub fn unix_milliseconds_at_local(clock: LocalClock) -> Option<u64> {
    let local = local_seconds(clock)?;
    u64::try_from(at_local(local)?).ok()?.checked_mul(1000)
}

fn local_seconds(clock: LocalClock) -> Option<i64> {
    use crate::diagnostic_log::{civil_from_days, days_from_civil};

    let (year, month, day) = (
        i64::from(clock.year),
        i64::from(clock.month),
        i64::from(clock.day),
    );
    if year < 1970 || !(1..=12).contains(&month) || clock.hour >= 24 || clock.minute >= 60 {
        return None;
    }
    let days = days_from_civil(year, month, day);
    (civil_from_days(days) == (year, month, day))
        .then_some(days * 86_400 + i64::from(clock.hour) * 3600 + i64::from(clock.minute) * 60)
}

#[cfg(windows)]
fn at_local(local: i64) -> Option<i64> {
    // The Windows zone switches between standard and DST offsets. Sample on
    // both sides of a possible transition and require an exact round trip.
    const TWO_DAYS: i64 = 2 * 86_400;
    let clock_at = |instant: i64| platform(u64::try_from(instant).ok()?.checked_mul(1000)?);
    let mut result = None;
    for reference in [local - TWO_DAYS, local + TWO_DAYS] {
        let offset = local_seconds(clock_at(reference)?)? - reference;
        let candidate = local - offset;
        if local_seconds(clock_at(candidate)?)? == local {
            if result.is_some_and(|known| known != candidate) {
                return None;
            }
            result = Some(candidate);
        }
    }
    result
}

#[cfg(unix)]
fn at_local(local: i64) -> Option<i64> {
    system_zone::at_local(local)
}

#[cfg(not(any(windows, unix)))]
const fn at_local(_local: i64) -> Option<i64> {
    None
}

/// 100-nanosecond intervals from 1601-01-01 to the Unix epoch.
#[cfg(windows)]
const UNIX_EPOCH_AS_FILETIME: u64 = 116_444_736_000_000_000;

#[cfg(windows)]
fn platform(unix_milliseconds: u64) -> Option<LocalClock> {
    let intervals = unix_milliseconds
        .checked_mul(10_000)?
        .checked_add(UNIX_EPOCH_AS_FILETIME)?;
    let universal = winsafe::FileTimeToSystemTime(&winsafe::FILETIME::from(intervals)).ok()?;
    let local = winsafe::SystemTimeToTzSpecificLocalTime(None, &universal).ok()?;
    Some(LocalClock {
        year: local.wYear,
        month: local.wMonth,
        day: local.wDay,
        hour: local.wHour,
        minute: local.wMinute,
    })
}

#[cfg(unix)]
fn platform(unix_milliseconds: u64) -> Option<LocalClock> {
    let seconds = i64::try_from(unix_milliseconds / 1000).ok()?;
    wall_clock(seconds, system_zone::offset_at(seconds)?)
}

#[cfg(not(any(windows, unix)))]
const fn platform(_unix_milliseconds: u64) -> Option<LocalClock> {
    None
}

/// The civil date and minute `offset` seconds east of UTC at `seconds` after the Unix epoch.
#[cfg(unix)]
fn wall_clock(seconds: i64, offset: i32) -> Option<LocalClock> {
    let local = seconds.checked_add(i64::from(offset))?;
    let (year, month, day) = crate::diagnostic_log::civil_from_days(local.div_euclid(86_400));
    let second_of_day = local.rem_euclid(86_400);
    Some(LocalClock {
        year: u16::try_from(year).ok()?,
        month: u16::try_from(month).ok()?,
        day: u16::try_from(day).ok()?,
        hour: u16::try_from(second_of_day / 3600).ok()?,
        minute: u16::try_from(second_of_day % 3600 / 60).ok()?,
    })
}
