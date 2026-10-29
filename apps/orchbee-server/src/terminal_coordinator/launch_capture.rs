//! One creation's launch capture: the Session preferences read once before admission and the
//! System shell of the Worker generation that launches it. The dispatched launch and the launched
//! program published with its binding both derive from this value; it performs no I/O.

use orchbee_capability_node_protocol::{
    DefaultCustomProgram, DefaultLaunchProgram, LaunchPreferences, LaunchSelection,
};
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

    /// Adds the System shell file name of the Worker generation whose request handle the caller
    /// read under the same registry guard.
    #[must_use]
    pub(crate) fn with_system_shell(
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

    /// The custom program a default launch with this capture dispatches with its literal argv.
    /// Explicit selections, System shell defaults and unavailable captures have none.
    pub(super) fn custom_default(
        &self,
        selection: Option<&LaunchSelection>,
    ) -> Option<DefaultCustomProgram> {
        let (None, LaunchPreferences::Available(preferences)) = (selection, &self.default) else {
            return None;
        };
        match preferences.program() {
            Some(DefaultLaunchProgram::Custom(program)) => Some(program.clone()),
            Some(DefaultLaunchProgram::SystemShell { .. }) | None => None,
        }
    }
}

#[cfg(test)]
mod tests;
