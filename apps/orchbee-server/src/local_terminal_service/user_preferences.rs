//! One frozen Site scope; Desktop and Site processes share the store's OS commit lock.
use super::LocalTerminalService;
use orchbee_local_site_runtime::{UserPreferencesScope, UserPreferencesStore};
use orchbee_workbench_protocol::{
    NativeSettingsFailure as Failure, NativeSettingsOperation as Operation,
    NativeSettingsOutcome as Outcome, NativeSettingsRequest, NativeSettingsResult, ProtocolVersion,
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

    pub(crate) fn execute_settings(&self, request: NativeSettingsRequest) -> NativeSettingsResult {
        let outcome = match self.workbench_site_matches(&request.site_id) {
            None => Outcome::Refused {
                reason: Failure::Unavailable,
            },
            Some(false) => Outcome::Refused {
                reason: Failure::SiteMismatch,
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
        NativeSettingsResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            site_id: request.site_id,
            outcome,
        }
    }
}

#[cfg(test)]
mod tests;
