//! The settings the Session serves and edits, and the readers that take their values from the
//! configuration owner's last valid settings.
use super::LocalTerminalService;
use crate::terminal_coordinator::LaunchCapture;
use orchbee_capability_node_protocol::{
    DefaultCustomProgram, DefaultLaunchProgram, LaunchPreferences, ProtocolError,
    TerminalLaunchColors, TerminalLaunchPreferences, TerminalLaunchThemes,
};
use orchbee_local_runtime::UserTerminalLaunchIntent;
use orchbee_workbench_protocol::{
    ProtocolVersion, SettingsOutcome as Outcome, SettingsRefusal, SettingsRequest, SettingsResult,
    UserLaunchCommand, UserPreferencesValues,
};

impl LocalTerminalService {
    /// Reads one creation's launch intent from the last valid settings before the caller takes
    /// any lock, so a committed change reaches only later creations and the dispatched launch and
    /// its published program share this one read. Before startup installs configuration, or for a
    /// program beyond the selection budget, it captures `Unavailable`.
    pub(super) fn launch_capture(&self) -> LaunchCapture {
        self.configuration
            .with_preferences(launch_capture)
            .unwrap_or_else(LaunchCapture::unavailable)
    }

    /// Whether this Session records and resumes Agent sessions, read before the caller takes any
    /// lock. Before startup installs configuration it reads as off.
    #[cfg(windows)]
    pub(crate) fn agent_resume_enabled(&self) -> bool {
        self.configuration
            .with_preferences(|values| values.resume_agents_on_restore)
            .unwrap_or(false)
    }

    pub(crate) fn execute_settings(&self, request: SettingsRequest) -> SettingsResult {
        let outcome = match self.workbench_session_matches(&request.session_id) {
            None => Outcome::refused(SettingsRefusal::Unavailable),
            Some(false) => Outcome::refused(SettingsRefusal::SessionMismatch),
            Some(true) => self.settings_outcome(&request.operation),
        };
        SettingsResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome,
        }
    }
}

/// The launch facts `values` give: the default profile's program for a creation that names none,
/// and the scrollback, colors and themes every creation takes. Values whose default profile or
/// colors cannot be captured launch the system shell with built-in defaults.
pub(super) fn launch_capture(values: &UserPreferencesValues) -> LaunchCapture {
    let intent = UserTerminalLaunchIntent::capture(values, true).unwrap_or_else(|_| {
        UserTerminalLaunchIntent {
            program: Some(UserLaunchCommand::SystemShell { login: false }),
            scrollback_bytes: UserPreferencesValues::default().scrollback_bytes,
            colors: None,
            themes: Default::default(),
        }
    });
    let explicit = UserTerminalLaunchIntent {
        program: None,
        scrollback_bytes: intent.scrollback_bytes,
        colors: intent.colors,
        themes: intent.themes.clone(),
    };
    let capture = |intent| {
        launch_preferences(intent)
            .map_or(LaunchPreferences::Unavailable, LaunchPreferences::Available)
    };
    LaunchCapture::new(capture(explicit), capture(intent))
}

fn launch_preferences(
    intent: UserTerminalLaunchIntent,
) -> Result<TerminalLaunchPreferences, ProtocolError> {
    let program = match intent.program {
        None => None,
        Some(UserLaunchCommand::SystemShell { login }) => {
            Some(DefaultLaunchProgram::SystemShell { login })
        }
        Some(UserLaunchCommand::CustomExecutable {
            executable,
            arguments,
        }) => Some(DefaultLaunchProgram::Custom(DefaultCustomProgram::new(
            executable, arguments,
        )?)),
    };
    let colors = intent.colors.map(|colors| {
        let (foreground, background) = colors.rgb();
        TerminalLaunchColors {
            foreground,
            background,
            dark: colors.dark(),
        }
    });
    let themes = TerminalLaunchThemes::new(
        intent.themes.light().to_owned(),
        intent.themes.dark().to_owned(),
    )?;
    TerminalLaunchPreferences::new(intent.scrollback_bytes, colors, themes, program)
}

#[cfg(test)]
mod tests;
