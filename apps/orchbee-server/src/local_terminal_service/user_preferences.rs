//! One frozen Session scope; Desktop and Session processes share the store's OS commit lock.
use super::LocalTerminalService;
use orchbee_capability_node_protocol::{
    AuthorizedTerminalLaunch, CapabilityNodeMessage, DefaultCustomProgram, DefaultLaunchProgram,
    LaunchPreferences, ProtocolError, TerminalLaunchColors, TerminalLaunchPreferences,
    encode_message,
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

    /// The Session's preference scope follows the configuration file from the Server's single
    /// startup read, or the environment when that read resolved no file. A composition root
    /// freezes it before any terminal launches, restored terminals included.
    #[cfg(windows)]
    pub(crate) fn preferences_scope(
        configuration: &orchbee_local_runtime::UserSettingsLoad,
    ) -> std::io::Result<UserPreferencesScope> {
        match configuration.settings.path.as_deref() {
            Some(path) => UserPreferencesScope::for_configuration(path),
            None => UserPreferencesScope::from_environment(|name| std::env::var_os(name)),
        }
        .map_err(|_| std::io::Error::other("Settings scope is unavailable"))
    }

    /// Attaches this creation's launch intent, read from the frozen store at dispatch so a saved
    /// change reaches only later creations. A missing store, failed read or program beyond the
    /// selection budget sends `Unavailable`; `None` means the capture would exceed the frame.
    pub(super) fn with_launch_preferences(
        &self,
        launch: &AuthorizedTerminalLaunch,
    ) -> Option<AuthorizedTerminalLaunch> {
        let preferences = self
            .preferences
            .get()
            .and_then(|store| store.terminal_launch_intent(launch.launch().is_none()).ok())
            .and_then(|intent| launch_preferences(intent).ok())
            .map_or(LaunchPreferences::Unavailable, LaunchPreferences::Available);
        let launch = launch.clone().with_preferences(preferences).ok()?;
        encode_message(&CapabilityNodeMessage::AuthorizedTerminalLaunch(
            launch.clone(),
        ))
        .ok()?;
        Some(launch)
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
                            Operation::Open {} => Outcome::Opened { snapshot },
                            Operation::Save { .. } => Outcome::Saved { snapshot, changed },
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
    TerminalLaunchPreferences::new(intent.scrollback_bytes, colors, program)
}

#[cfg(test)]
mod tests;
