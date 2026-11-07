//! One frozen Session scope; Desktop and Session processes share the store's OS commit lock.
use super::LocalTerminalService;
use crate::terminal_coordinator::LaunchCapture;
use orchbee_capability_node_protocol::{
    DefaultCustomProgram, DefaultLaunchProgram, LaunchPreferences, ProtocolError,
    TerminalLaunchColors, TerminalLaunchPreferences, TerminalLaunchThemes,
};
use orchbee_local_runtime::{UserPreferencesScope, UserPreferencesStore, UserTerminalLaunchIntent};
use orchbee_workbench_protocol::{
    ProtocolVersion, SettingsFailure as Failure, SettingsOperation as Operation,
    SettingsOutcome as Outcome, SettingsRequest, SettingsResult, UserLaunchCommand,
    UserPreferencesResponse,
};

impl LocalTerminalService {
    pub(crate) fn initialize_preferences(
        &self,
        scope: UserPreferencesScope,
    ) -> std::io::Result<()> {
        self.preferences
            .set(UserPreferencesStore::new(scope))
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::AlreadyExists))
    }

    /// Reads one creation's launch intent from the frozen store before the caller takes any lock,
    /// so a saved change reaches only later creations and the dispatched launch and its published
    /// program share this one read. A missing store or program beyond the selection budget
    /// captures `Unavailable`; a refused record uses built-in defaults without repairing it.
    pub(super) fn launch_capture(&self) -> LaunchCapture {
        let Some(store) = self.preferences.get() else {
            return LaunchCapture::unavailable();
        };
        let intent = store.terminal_launch_intent(true).unwrap_or_else(|_| {
            self.configuration.note_unreadable_preferences();
            UserTerminalLaunchIntent {
                program: Some(UserLaunchCommand::SystemShell { login: false }),
                scrollback_bytes: orchbee_workbench_protocol::UserPreferencesValues::default()
                    .scrollback_bytes,
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

    /// Whether this Session records and resumes Agent sessions, read from the frozen store before
    /// the caller takes any lock. A refused record reads as the built-in default, on; a missing
    /// store reads as off.
    #[cfg(windows)]
    pub(crate) fn agent_resume_enabled(&self) -> bool {
        self.preferences.get().is_some_and(|store| {
            store
                .read()
                .map_or(true, |snapshot| snapshot.values.resume_agents_on_restore)
        })
    }

    pub(crate) fn execute_settings(&self, request: SettingsRequest) -> SettingsResult {
        let outcome = match self.workbench_session_matches(&request.session_id) {
            None => Outcome::Refused {
                reason: Failure::Unavailable,
            },
            Some(false) => Outcome::Refused {
                reason: Failure::SessionMismatch,
            },
            Some(true) => self.preferences.get().map_or(
                Outcome::Refused {
                    reason: Failure::Unavailable,
                },
                |store| match store.execute(&request.operation.preferences_request()) {
                    UserPreferencesResponse::Snapshot { snapshot, changed } => {
                        match request.operation {
                            Operation::Open {} => Outcome::Opened {
                                snapshot: *snapshot,
                            },
                            Operation::Save { .. } => Outcome::Saved {
                                snapshot: *snapshot,
                                changed,
                            },
                        }
                    }
                    UserPreferencesResponse::Refused { reason } => Outcome::Refused {
                        reason: reason.into(),
                    },
                    UserPreferencesResponse::OutcomeUnknown {} => Outcome::OutcomeUnknown {},
                },
            ),
        };
        SettingsResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome,
        }
    }
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
