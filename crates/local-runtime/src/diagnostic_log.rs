//! The diagnostic log (ORC-397): the Client, Server, Worker and Desktop Main append whole JSON Lines
//! records to `logs/<YYYY-MM-DD>.jsonl` in the data root, named by local date. A record holds the
//! time, level, process role, PID, event and at most eight bounded fields.
//!
//! Every event name and text value is a `&'static str` token from a closed grammar, and the other
//! values are counts and flags, so a record cannot carry terminal content, typed input, prompts,
//! command arguments, environment or configuration values, file contents or credentials. A process
//! logs only after its entry point installs its role; library code that tests exercise writes
//! nothing until then. Writing is best effort and never changes what the caller does.
//!
//! When a process opens a day's file it removes daily files dated more than six days earlier. A
//! day's file stops growing at [`DAILY_LOG_LIMIT_BYTES`] after one `log-capped` record. Writers
//! hold the day's file without write sharing while they append, so a Rust writer and Desktop
//! Main's exclusive open admit one record at a time. Logging is Windows only until the POSIX data
//! root lands (ORC-328).
//!
//! Cross-runtime contract: search for `daily-diagnostic-log` before changing the record shape, a
//! limit, the retention rule or the file names; Desktop Main writes the same records.
use std::{
    io,
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(not(windows))]
mod unsupported;
#[cfg(not(windows))]
use unsupported as platform;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;
#[cfg(test)]
mod tests;

/// The size a day's file never exceeds; the `log-capped` record fits inside it.
pub const DAILY_LOG_LIMIT_BYTES: u64 = 7 * 1024 * 1024;
/// The days a process keeps when it opens a day's file: that day and the six before it.
pub const RETAINED_LOG_DAYS: i64 = 7;
/// The record that closes a day's file once the next record would pass the limit.
const CAPPED_EVENT: &str = "log-capped";
const MAXIMUM_FIELDS: usize = 8;
const MAXIMUM_TOKEN: usize = 64;
const MAXIMUM_FIELD_NAME: usize = 32;
const FILE_SUFFIX: &str = ".jsonl";

/// The process that wrote a record. Desktop Main writes `desktop-main` from its own writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogRole {
    Client,
    Server,
    Worker,
}
impl LogRole {
    const fn token(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Server => "server",
            Self::Worker => "worker",
        }
    }
}
const ROLES: [&str; 4] = ["client", "server", "worker", "desktop-main"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}
impl LogLevel {
    const fn token(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}
const LEVELS: [&str; 3] = ["info", "warning", "error"];

/// One bounded field value. A text value is a compile-time token; a token outside the grammar
/// (letters, digits, `.`, `_`, `-`, at most 64 bytes) is written as `invalid`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogValue {
    Token(&'static str),
    Count(u64),
    Flag(bool),
}
impl From<io::ErrorKind> for LogValue {
    fn from(kind: io::ErrorKind) -> Self {
        use io::ErrorKind as Kind;
        Self::Token(match kind {
            Kind::NotFound => "not-found",
            Kind::PermissionDenied => "permission-denied",
            Kind::AlreadyExists => "already-exists",
            Kind::WouldBlock => "would-block",
            Kind::InvalidInput => "invalid-input",
            Kind::InvalidData => "invalid-data",
            Kind::TimedOut => "timed-out",
            Kind::Interrupted => "interrupted",
            Kind::Unsupported => "unsupported",
            Kind::UnexpectedEof => "unexpected-eof",
            Kind::BrokenPipe => "broken-pipe",
            Kind::ConnectionRefused => "connection-refused",
            Kind::ConnectionReset => "connection-reset",
            Kind::ConnectionAborted => "connection-aborted",
            Kind::OutOfMemory => "out-of-memory",
            _ => "other",
        })
    }
}
/// A field name is camelCase ASCII of at most 32 bytes.
pub type LogField = (&'static str, LogValue);

static ROLE: OnceLock<LogRole> = OnceLock::new();

/// Names this process's role and enables its log records. Only an executable's entry point calls
/// it, and only once; a later call is ignored.
pub fn install_diagnostic_log(role: LogRole) {
    let _ = ROLE.set(role);
}

/// Appends one record when this process installed its role, and does nothing otherwise.
pub fn log_event(level: LogLevel, event: &'static str, fields: &[LogField]) {
    let Some(role) = ROLE.get() else {
        return;
    };
    let stamp = Stamp::now();
    let record = encode(
        stamp.unix_milliseconds,
        level.token(),
        role.token(),
        event,
        fields,
    );
    let capped = encode(
        stamp.unix_milliseconds,
        LogLevel::Warning.token(),
        role.token(),
        CAPPED_EVENT,
        &[],
    );
    let _ = platform::append(&record, &capped, stamp);
}

/// The daily files of the last seven days, each holding only the lines that are well-formed
/// records, with the count of lines left out. The destructive request log is not included.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct RecentDiagnosticLogs {
    pub files: Vec<(String, Vec<u8>)>,
    pub omitted_lines: u64,
}

/// Reads this environment's recent daily files without creating anything.
pub fn recent_diagnostic_logs(root: &crate::DataRoot) -> io::Result<RecentDiagnosticLogs> {
    let today = Stamp::now().day;
    let mut recent = RecentDiagnosticLogs::default();
    for day in (today - (RETAINED_LOG_DAYS - 1))..=today {
        let name = file_name(day);
        let Some(bytes) = platform::read_daily(&root.logs().join(&name))? else {
            continue;
        };
        let (kept, omitted) = well_formed_lines(&bytes);
        recent.omitted_lines += omitted;
        recent.files.push((name, kept));
    }
    Ok(recent)
}

/// The instant of one record and the local day that names its file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Stamp {
    unix_milliseconds: u64,
    /// Days since 1970-01-01 of the local date, or of the UTC date when the host offers no local
    /// clock.
    day: i64,
}
impl Stamp {
    fn now() -> Self {
        let unix_milliseconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
            });
        Self::at(unix_milliseconds)
    }
    fn at(unix_milliseconds: u64) -> Self {
        let day = crate::local_clock(unix_milliseconds).map_or_else(
            || i64::try_from(unix_milliseconds / 86_400_000).unwrap_or(0),
            |local| {
                days_from_civil(
                    i64::from(local.year),
                    i64::from(local.month),
                    i64::from(local.day),
                )
            },
        );
        Self {
            unix_milliseconds,
            day,
        }
    }
}

fn file_name(day: i64) -> String {
    let (year, month, date) = civil_from_days(day);
    format!("{year:04}-{month:02}-{date:02}{FILE_SUFFIX}")
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's `days_from_civil`).
pub(crate) fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

pub(crate) fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    (year_of_era + era * 400 + i64::from(month <= 2), month, day)
}

/// `YYYY-MM-DDTHH:MM:SS.mmmZ` in UTC.
fn timestamp(unix_milliseconds: u64) -> String {
    let seconds = unix_milliseconds / 1000;
    let (year, month, day) = civil_from_days(i64::try_from(seconds / 86_400).unwrap_or(0));
    let second_of_day = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        second_of_day / 3600,
        second_of_day / 60 % 60,
        second_of_day % 60,
        unix_milliseconds % 1000,
    )
}

fn is_token(value: &str) -> bool {
    (1..=MAXIMUM_TOKEN).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_field_name(name: &str) -> bool {
    (1..=MAXIMUM_FIELD_NAME).contains(&name.len())
        && name.as_bytes()[0].is_ascii_lowercase()
        && name.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

/// One record line. Tokens need no escaping: the grammar admits no quote, backslash or control
/// character.
fn encode(
    unix_milliseconds: u64,
    level: &str,
    role: &str,
    event: &'static str,
    fields: &[LogField],
) -> Vec<u8> {
    let event = if is_token(event) { event } else { "invalid" };
    let mut body = String::new();
    for (name, value) in fields
        .iter()
        .filter(|(name, _)| is_field_name(name))
        .take(MAXIMUM_FIELDS)
    {
        if !body.is_empty() {
            body.push(',');
        }
        let value = match value {
            LogValue::Token(token) if is_token(token) => format!("\"{token}\""),
            LogValue::Token(_) => "\"invalid\"".to_owned(),
            LogValue::Count(count) => count.to_string(),
            LogValue::Flag(flag) => flag.to_string(),
        };
        body.push_str(&format!("\"{name}\":{value}"));
    }
    format!(
        "{{\"time\":\"{}\",\"level\":\"{level}\",\"role\":\"{role}\",\"pid\":{},\"event\":\"{event}\",\"fields\":{{{body}}}}}\n",
        timestamp(unix_milliseconds),
        std::process::id(),
    )
    .into_bytes()
}

/// Keeps each line that is exactly a record this grammar writes, from any writer, and counts the
/// rest, so a bundle never carries text a writer did not produce.
fn well_formed_lines(bytes: &[u8]) -> (Vec<u8>, u64) {
    let mut kept = Vec::new();
    let mut omitted = 0;
    for line in bytes.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        if well_formed(line) {
            kept.extend_from_slice(line);
            kept.push(b'\n');
        } else {
            omitted += 1;
        }
    }
    (kept, omitted)
}

fn well_formed(line: &[u8]) -> bool {
    let Ok(serde_json::Value::Object(record)) = serde_json::from_slice(line) else {
        return false;
    };
    let text = |key: &str| record.get(key).and_then(serde_json::Value::as_str);
    record.len() == 6
        && text("time").is_some_and(well_formed_time)
        && text("level").is_some_and(|level| LEVELS.contains(&level))
        && text("role").is_some_and(|role| ROLES.contains(&role))
        && record
            .get("pid")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|pid| pid <= u64::from(u32::MAX))
        && text("event").is_some_and(is_token)
        && record
            .get("fields")
            .and_then(serde_json::Value::as_object)
            .is_some_and(|fields| {
                fields.len() <= MAXIMUM_FIELDS
                    && fields.iter().all(|(name, value)| {
                        is_field_name(name)
                            && match value {
                                serde_json::Value::String(token) => is_token(token),
                                serde_json::Value::Number(number) => number.is_u64(),
                                serde_json::Value::Bool(_) => true,
                                _ => false,
                            }
                    })
            })
}

fn well_formed_time(time: &str) -> bool {
    let bytes = time.as_bytes();
    bytes.len() == 24
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            10 => *byte == b'T',
            13 | 16 => *byte == b':',
            19 => *byte == b'.',
            23 => *byte == b'Z',
            _ => byte.is_ascii_digit(),
        })
}
