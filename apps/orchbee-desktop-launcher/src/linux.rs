//! The Linux launcher's effects beyond the shared run: the parent check and wait through `/proc`,
//! the sandbox switch, and the failure on standard error, the only report a launcher started from the
//! application menu has, in the locale `LC_ALL`, `LC_MESSAGES` or `LANG` names.
use crate::decision::Failure;
use crate::launch;
use crate::localization;
use std::{
    ffi::OsStr,
    fs, thread,
    time::{Duration, Instant},
};

/// How often the wait looks at the parent.
const WAIT_POLL: Duration = Duration::from_millis(100);

/// The files that say the system restricts unprivileged user namespaces, each with the value that
/// says so: Ubuntu's AppArmor switch, Debian's older switch, and no user namespaces at all. The
/// AppImage's `AppRun` reads the same (apps/desktop/scripts/linux-packages.mjs).
const RESTRICTIONS: [(&str, &str); 3] = [
    (
        "/proc/sys/kernel/apparmor_restrict_unprivileged_userns",
        "1",
    ),
    ("/proc/sys/kernel/unprivileged_userns_clone", "0"),
    ("/proc/sys/user/max_user_namespaces", "0"),
];

/// Runs the launcher with its confirmed parent. Where the system restricts unprivileged user
/// namespaces, Chromium's sandbox cannot start in a window under the person's home, which no
/// AppArmor profile covers, so every window starts with `--no-sandbox` there.
///
/// # Errors
///
/// The failure to report when no window starts.
pub(crate) fn launch() -> Result<(), Failure> {
    let restricted = RESTRICTIONS
        .iter()
        .any(|(file, value)| fs::read_to_string(file).is_ok_and(|text| text.trim() == *value));
    let switches: &[&OsStr] = if restricted {
        &[OsStr::new("--no-sandbox")]
    } else {
        &[]
    };
    launch::launch(Parent::confirmed(), switches)
}

/// Writes `failure` to standard error.
pub(crate) fn show(failure: &Failure) {
    eprintln!("{}", localization::failure(locale(), failure));
}

/// The locale of the first of `LC_ALL`, `LC_MESSAGES` and `LANG` that is set and not empty.
fn locale() -> localization::Locale {
    let tag = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(std::env::var_os)
        .find(|value| !value.is_empty());
    localization::locale(tag.as_deref().and_then(std::ffi::OsStr::to_str))
}

/// The launcher's parent and its start in clock ticks after boot, when it started no later than
/// the launcher.
struct Parent {
    process_id: u32,
    started: u64,
}

impl Parent {
    fn confirmed() -> Option<Self> {
        let process_id = std::os::unix::process::parent_id();
        let started = Status::read(process_id)?.started;
        let own = Status::read(std::process::id())?.started;
        (process_id > 1 && started <= own).then_some(Self {
            process_id,
            started,
        })
    }
}

impl launch::Parent for Parent {
    fn process_id(&self) -> u32 {
        self.process_id
    }

    /// The parent has ended once its `/proc` entry is gone, names a later start, or is a zombie.
    fn wait(&self, limit: Duration) {
        let deadline = Instant::now() + limit;
        while Status::read(self.process_id)
            .is_some_and(|status| status.started == self.started && !status.ended)
            && Instant::now() < deadline
        {
            thread::sleep(WAIT_POLL);
        }
    }
}

/// What `/proc/<pid>/stat` says about one process.
struct Status {
    /// Field 22, the start in clock ticks after boot.
    started: u64,
    /// The state is `Z` (zombie) or `X` (dead).
    ended: bool,
}

impl Status {
    /// The status of `process_id`; none when it cannot be read. The command name in parentheses may
    /// itself hold parentheses and spaces, so the fields are counted after the last `)`.
    fn read(process_id: u32) -> Option<Self> {
        let stat = fs::read_to_string(format!("/proc/{process_id}/stat")).ok()?;
        let mut fields = stat.rsplit_once(')')?.1.split_whitespace();
        let state = fields.next()?;
        // The state is field 3 and the start field 22, so 18 fields lie between them.
        let started = fields.nth(18)?.parse().ok()?;
        Some(Self {
            started,
            ended: matches!(state, "Z" | "X"),
        })
    }
}
