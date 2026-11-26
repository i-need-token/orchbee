//! Catalog downloads run the operating system's own curl as one bounded child per attempt, through
//! the platform's owned process adapter. Every build passes curl the same arguments and makes the
//! same three attempts under the same limits.
use super::{CatalogUrl, Failure};
use std::{sync::atomic::AtomicBool, time::Instant};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

/// curl's arguments before the URL: no configuration file, an HTTP error status fails, no progress
/// output but its own error line, at most five HTTP or HTTPS redirects, five seconds to connect,
/// fifteen for the whole transfer, and no body larger than the rule-cache limit.
const CURL_ARGUMENTS: [&str; 18] = [
    "--disable",
    "--fail",
    "--silent",
    "--show-error",
    "--location",
    "--max-redirs",
    "5",
    "--proto",
    "=http,https",
    "--proto-redir",
    "=http,https",
    "--connect-timeout",
    "5",
    "--max-time",
    "15",
    "--max-filesize",
    "262144",
    "--url",
];
const _: () = assert!(super::LIMIT == 262_144);

/// Up to three attempts, each within fifteen seconds and the update's own deadline, 250 ms apart.
/// Only a transfer failure, including an attempt that ran out of its own time, is retried.
pub(in crate::agent_screen::updating) fn text(
    source: &CatalogUrl,
    stop: &AtomicBool,
    deadline: Instant,
) -> Result<String, Failure> {
    use super::cancelled;
    use std::{thread, time::Duration};
    let program = platform::curl()?;
    let mut result = Err(Failure::Fetch);
    for attempt in 0..3 {
        cancelled(stop, deadline)?;
        result = platform::once(
            &program,
            source,
            stop,
            deadline.min(Instant::now() + Duration::from_secs(15)),
        );
        if matches!(result, Err(Failure::Deadline)) && cancelled(stop, deadline).is_ok() {
            result = Err(Failure::Fetch);
        }
        if !matches!(result, Err(Failure::Fetch)) {
            break;
        }
        if attempt < 2 {
            let retry = deadline.min(Instant::now() + Duration::from_millis(250));
            while Instant::now() < retry {
                cancelled(stop, deadline)?;
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
    result
}
