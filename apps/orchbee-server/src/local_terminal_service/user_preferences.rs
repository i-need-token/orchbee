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
mod tests {
    use super::*;
    use crate::{
        workbench_service::{WorkbenchExecution, WorkbenchPolicy, WorkbenchService},
        worker_registry::WorkerRegistry,
    };
    use orchbee_workbench_protocol::{WorkbenchRequest, WorkbenchResponse};
    use serde_json::{Value, json};
    use std::{
        path::PathBuf,
        sync::{Arc, Mutex},
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Scope(PathBuf);
    impl Drop for Scope {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn settings_dispatch_checks_permission_and_site_before_shared_store_access() {
        let root = std::env::temp_dir().join(format!(
            "orchbee-site-preferences-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let fixture = Scope(root);
        let scope =
            UserPreferencesScope::for_configuration(&fixture.0.join("config.toml")).unwrap();
        let service = Arc::new(LocalTerminalService::from_identity(
            Arc::new(Mutex::new(WorkerRegistry::new())),
            Ok(
                orchbee_workbench_protocol::SiteId::parse("11111111111111111111111111111111")
                    .unwrap(),
            ),
        ));
        service.initialize_preferences(scope.clone()).unwrap();
        assert!(service.initialize_preferences(scope.clone()).is_err());
        let dispatch = |allowed, site, operation: Value| {
            let workbench = WorkbenchService::new(
                WorkbenchPolicy {
                    read: allowed,
                    manage_navigation: allowed,
                    associate_directory: false,
                    create_terminal: false,
                    manage_terminal_lifecycle: false,
                },
                service.clone(),
            );
            let request: NativeSettingsRequest = serde_json::from_value(json!({"protocolVersion": 1,
                "requestId": "22222222222222222222222222222222", "siteId": site, "operation": operation})).unwrap();
            request.validate().unwrap();
            let WorkbenchExecution::Response(WorkbenchResponse::Settings(result)) =
                workbench.execute(WorkbenchRequest::Settings(request.clone()))
            else {
                panic!("settings receipt");
            };
            assert_eq!(result.request_id, request.request_id);
            result.outcome
        };
        let site = "11111111111111111111111111111111";
        assert_eq!(
            dispatch(false, site, json!({"kind":"open"})),
            Outcome::Refused {
                reason: Failure::Denied
            }
        );
        assert_eq!(
            dispatch(
                true,
                "33333333333333333333333333333333",
                json!({"kind":"open"})
            ),
            Outcome::Refused {
                reason: Failure::SiteMismatch
            }
        );
        assert!(!scope.directory().exists());
        let Outcome::Opened { snapshot } = dispatch(true, site, json!({"kind":"open"})) else {
            panic!("open");
        };
        let request = json!({"kind":"save", "source": snapshot.source, "changes":{"applicationTheme":"dark"}});
        assert_eq!(
            dispatch(false, site, request.clone()),
            Outcome::Refused {
                reason: Failure::Denied
            }
        );
        let Outcome::Saved {
            snapshot: saved,
            changed: true,
        } = dispatch(true, site, request.clone())
        else {
            panic!("save");
        };
        assert_eq!(
            dispatch(true, site, request),
            Outcome::Refused {
                reason: Failure::SourceChanged
            }
        );
        let external = UserPreferencesStore::new(scope)
            .execute(&orchbee_workbench_protocol::UserPreferencesRequest::Read {});
        assert!(
            matches!(external, UserPreferencesResponse::Snapshot { snapshot, .. } if snapshot == saved)
        );
    }
}
