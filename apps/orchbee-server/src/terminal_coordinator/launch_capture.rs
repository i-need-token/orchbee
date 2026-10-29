//! One creation's launch capture: the Session preferences read once before admission and the
//! System shell of the Worker generation that launches it. The dispatched launch, the launched
//! program published with its binding and whether that launch runs an interactive Shell all derive
//! from this value; it performs no I/O.

use orchbee_capability_node_protocol::{DefaultLaunchProgram, LaunchPreferences, LaunchSelection};
use orchbee_workbench_protocol::PresentationLaunchProgram;

#[derive(Clone, Debug)]
pub(crate) struct LaunchCapture {
    /// What an explicit selection carries: the confirmed scrollback and colors, never a program.
    explicit: LaunchPreferences,
    /// What a default launch carries, including the default profile's program.
    default: LaunchPreferences,
    system_shell: Option<PresentationLaunchProgram>,
}

impl LaunchCapture {
    pub(crate) const fn new(explicit: LaunchPreferences, default: LaunchPreferences) -> Self {
        Self {
            explicit,
            default,
            system_shell: None,
        }
    }

    /// A capture whose every launch the Worker refuses, for requests that never create.
    pub(crate) const fn unavailable() -> Self {
        Self::new(
            LaunchPreferences::Unavailable,
            LaunchPreferences::Unavailable,
        )
    }

    /// The device defaults of an adapter without a Session preference owner.
    #[cfg(test)]
    pub(crate) const fn absent() -> Self {
        Self::new(LaunchPreferences::Absent, LaunchPreferences::Absent)
    }

    /// Adds the System shell file name of the Worker generation the creation targets; only
    /// [`CreateInputs`](super::CreateInputs) supplies it, from that generation's request handle.
    #[must_use]
    pub(super) fn with_system_shell(
        mut self,
        system_shell: Option<PresentationLaunchProgram>,
    ) -> Self {
        self.system_shell = system_shell;
        self
    }

    pub(crate) fn preferences(&self, selection: Option<&LaunchSelection>) -> LaunchPreferences {
        if selection.is_some() {
            self.explicit.clone()
        } else {
            self.default.clone()
        }
    }

    /// The file name of the program a launch with this selection requests: the explicit executable,
    /// else the default profile's custom executable, else this generation's System shell. A default
    /// launch whose capture is unavailable, which the Worker refuses, requests no known program.
    pub(super) fn launch_program(
        &self,
        selection: Option<&LaunchSelection>,
    ) -> Option<PresentationLaunchProgram> {
        if let Some(selection) = selection {
            return PresentationLaunchProgram::from_executable(selection.executable());
        }
        match &self.default {
            LaunchPreferences::Available(preferences) => match preferences.program() {
                Some(DefaultLaunchProgram::Custom(program)) => {
                    PresentationLaunchProgram::from_executable(program.executable())
                }
                Some(DefaultLaunchProgram::SystemShell { .. }) | None => self.system_shell.clone(),
            },
            LaunchPreferences::Absent => self.system_shell.clone(),
            LaunchPreferences::Unavailable => None,
        }
    }

    /// Whether a launch with this selection runs an interactive Shell, judged from the argv it
    /// dispatches: the explicit selection, else the default profile's custom program with its
    /// literal argv. A default launch of the System shell always qualifies.
    pub(super) fn launches_interactive_shell(&self, selection: Option<&LaunchSelection>) -> bool {
        if let Some(selection) = selection {
            return interactive_shell_launch(selection.executable(), selection.arguments());
        }
        match &self.default {
            LaunchPreferences::Available(preferences) => match preferences.program() {
                Some(DefaultLaunchProgram::Custom(program)) => {
                    interactive_shell_launch(program.executable(), program.arguments())
                }
                Some(DefaultLaunchProgram::SystemShell { .. }) | None => true,
            },
            // The Worker refuses an unavailable capture, so its launch never starts.
            LaunchPreferences::Absent | LaunchPreferences::Unavailable => true,
        }
    }
}

/// The Host's interactive Shell launch rule (`interactive_shell.zig`) applied to a dispatched argv:
/// PowerShell with only plain interactive flags, `cmd` with only `/D` or `/Q`, and POSIX shells
/// with only interactive or login flags. Commands, scripts and other programs return false. The
/// PowerShell startup that Orchbee prepares later is never part of the argv, so its exact
/// recognition stays with the Host.
fn interactive_shell_launch(executable: &str, arguments: &[String]) -> bool {
    let filename = executable.rsplit(['/', '\\']).next().unwrap_or(executable);
    let name = match filename.split_at_checked(filename.len().saturating_sub(4)) {
        Some((stem, extension)) if extension.eq_ignore_ascii_case(".exe") => stem,
        _ => filename,
    };
    let flags: &[&str] =
        if name.eq_ignore_ascii_case("pwsh") || name.eq_ignore_ascii_case("powershell") {
            &["-NoLogo", "-NoProfile", "-NoExit", "-Login", "-l"]
        } else if name.eq_ignore_ascii_case("cmd") {
            &["/D", "/Q"]
        } else if ["sh", "bash", "zsh", "fish", "dash", "ksh", "csh", "tcsh"].contains(&name) {
            &["-i", "-l", "-il", "-li", "--login", "--noprofile", "--norc"]
        } else {
            return false;
        };
    arguments
        .iter()
        .all(|argument| flags.iter().any(|flag| argument.eq_ignore_ascii_case(flag)))
}

#[cfg(test)]
mod tests;
