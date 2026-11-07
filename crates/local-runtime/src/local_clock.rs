//! Converts a Unix-millisecond instant to the host's local wall clock for display. On Windows it
//! applies the time zone rules in force at that instant; elsewhere no local clock is offered, so a
//! caller shows only what it can derive without one.

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

#[cfg(not(windows))]
const fn platform(_unix_milliseconds: u64) -> Option<LocalClock> {
    None
}
