//! Converts a Unix-millisecond instant to the host's local wall clock for display, applying the
//! system time zone's rules in force at that instant. Windows asks the system; macOS and Linux read
//! the zone file `/etc/localtime` links to and, like Windows, ignore `TZ`. A host whose zone cannot
//! be read offers no local clock, so a caller shows only what it can derive without one.

#[cfg(unix)]
mod system_zone;
#[cfg(any(unix, test))]
mod tzif;

/// A local date and minute, for display only.
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
