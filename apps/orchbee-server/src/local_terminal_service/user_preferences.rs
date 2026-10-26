//! One frozen Session scope; Desktop and Session processes share the store's OS commit lock.
use super::LocalTerminalService;
use orchbee_local_runtime::{UserPreferencesScope, UserPreferencesStore};
use orchbee_workbench_protocol::{
    ProtocolVersion, SettingsFailure as Failure, SettingsOperation as Operation,
    SettingsOutcome as Outcome, SettingsRequest, SettingsResult, UserPreferencesResponse,
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

#[cfg(test)]
mod tests;
