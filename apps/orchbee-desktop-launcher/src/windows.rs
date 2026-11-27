//! The Windows launcher's effects beyond the shared run: the parent check and wait, the message box,
//! and the Windows display language.
use crate::decision::Failure;
use crate::launch;
use crate::localization::{self, Locale};
use std::time::Duration;
use winsafe::{self as w, co, prelude::*};

/// The registry values holding the Windows display language: the person's choice when they made
/// one, then the system's, as Windows caches it for the person.
const DISPLAY_LANGUAGE: [(&str, &str); 2] = [
    (r"Control Panel\Desktop", "PreferredUILanguages"),
    (
        r"Control Panel\Desktop\MuiCached",
        "MachinePreferredUILanguages",
    ),
];

/// Runs the launcher with its parent, opened for the wait.
///
/// # Errors
///
/// The failure to show when no window starts.
pub(crate) fn launch() -> Result<(), Failure> {
    launch::launch(Parent::confirmed(), &[])
}

/// Shows `failure` in a message box in the Windows display language. Nothing else can report it:
/// the launcher has no console, and a message box that cannot open leaves only the exit code.
pub(crate) fn show(failure: &Failure) {
    let text = localization::failure(display_locale(), failure);
    let _ = w::HWND::NULL.MessageBox(&text, localization::CAPTION, co::MB::OK | co::MB::ICONERROR);
}

/// The launcher's parent, open for the wait, when it started no later than the launcher.
struct Parent {
    process_id: u32,
    process: w::guard::CloseHandleGuard<w::HPROCESS>,
}

impl Parent {
    fn confirmed() -> Option<Self> {
        let own = std::process::id();
        let mut snapshot =
            w::HPROCESSLIST::CreateToolhelp32Snapshot(co::TH32CS::SNAPPROCESS, None).ok()?;
        let process_id = snapshot
            .iter_processes()
            .filter_map(Result::ok)
            .find(|entry| entry.th32ProcessID == own)?
            .th32ParentProcessID;
        let process = w::HPROCESS::OpenProcess(
            co::PROCESS::QUERY_LIMITED_INFORMATION | co::PROCESS::SYNCHRONIZE,
            false,
            process_id,
        )
        .ok()?;
        let (parent_started, ..) = process.GetProcessTimes().ok()?;
        let (own_started, ..) = w::HPROCESS::GetCurrentProcess().GetProcessTimes().ok()?;
        (u64::from(parent_started) <= u64::from(own_started)).then_some(Self {
            process_id,
            process,
        })
    }
}

impl launch::Parent for Parent {
    fn process_id(&self) -> u32 {
        self.process_id
    }

    fn wait(&self, limit: Duration) {
        let milliseconds = u32::try_from(limit.as_millis()).unwrap_or(u32::MAX);
        let _ = self.process.WaitForSingleObject(Some(milliseconds));
    }
}

/// The locale of the first Windows display language the registry names, or `en-US` without one.
fn display_locale() -> Locale {
    let tag = DISPLAY_LANGUAGE.iter().find_map(|(key, value)| {
        let found = w::HKEY::CURRENT_USER
            .RegGetValue(Some(key), Some(value), co::RRF::RT_REG_MULTI_SZ)
            .ok()?;
        match found {
            w::RegistryValue::MultiSz(languages) => {
                languages.into_iter().find(|language| !language.is_empty())
            }
            _ => None,
        }
    });
    localization::locale(tag.as_deref())
}
