//! The remote helper `orchbee --machine-link`: the acceptor of one `machine-link` connection on
//! standard input and output, carrying several Sessions' byte streams, routed commands, and Session
//! starts. `docs/machines.md#link-helper` owns the ownership, bounds, and ends kept here; the
//! contract in `schemas/machine` owns every byte.
mod carrier;
mod channels;
mod state;

use crate::{cli::Output, localization::Locale, tui_capabilities::Capability};

/// The helper's whole command line after the program name, in either spelling: a hub of a release
/// up to 0.1.0-beta.24 sends only the earlier one.
#[cfg(windows)]
const HELPER: orchbee_release_records::CrossBuildName = orchbee_release_records::CrossBuildName {
    current: "--machine-link",
    earlier: "--machine-link-v1",
};

/// Admits the helper's token before anything reads Orchbee state; any other command line answers
/// `None` at once. A token at Medium integrity or below without enabled Administrators continues as
/// the helper. An elevated one, as Win32-OpenSSH gives an administrator, relaunches this executable
/// with a standard token, which stays in sshd's Job, and exits with the child's status; it never
/// serves the link itself. A relaunch that fails, or a status above 255, ends with status 5 and a
/// `link-rights` record.
#[cfg(windows)]
pub(crate) fn admit_rights() -> Option<std::process::ExitCode> {
    use orchbee_local_runtime::RelaunchFailure;
    use std::{ffi::OsStr, process::ExitCode};

    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().is_none_or(|first| !HELPER.is(&first)) || arguments.next().is_some() {
        return None;
    }
    let failure = match orchbee_local_runtime::token_rights() {
        Ok(rights) if rights.is_standard() => return None,
        Ok(_) => {
            match orchbee_local_runtime::relaunch_with_standard_rights(OsStr::new(HELPER.current)) {
                Ok(status) => {
                    if let Ok(status) = u8::try_from(status) {
                        return Some(ExitCode::from(status));
                    }
                    crate::diagnostic_events::link_child_failed(status);
                    return Some(ExitCode::from(carrier::REFUSED));
                }
                Err(failure) => failure,
            }
        }
        Err(_) => RelaunchFailure::Derive,
    };
    crate::diagnostic_events::link_rights_refused(failure);
    Some(ExitCode::from(carrier::REFUSED))
}

/// Serves one link until it ends. The status is 0 for an ended link and 5 for a refused start, a
/// refused `OBHS` frame, or a violation; nothing but `OBHS` and `OBMX` bytes reaches standard
/// output. A host without saved machines answers unsupported before reading a byte. The helper
/// writes no text; routed commands carry their own locale.
pub(crate) fn serve(locale: Locale) -> Output {
    if Capability::SavedMachines.require().is_err() {
        return crate::cli::unsupported_platform(None, locale);
    }
    #[cfg(windows)]
    crate::diagnostic_events::link_started(orchbee_local_runtime::token_rights().ok());
    Output {
        status: carrier::serve(),
        stdout: Vec::new(),
        stderr: String::new(),
        finalization: None,
    }
}
