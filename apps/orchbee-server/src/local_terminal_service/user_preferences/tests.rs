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
fn settings_dispatch_checks_permission_and_session_before_shared_store_access() {
    let root = std::env::temp_dir().join(format!(
        "orchbee-session-preferences-{}-{}",
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
    let scope = UserPreferencesScope::for_configuration(&fixture.0.join("config.toml")).unwrap();
    let service = Arc::new(LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Ok(
            orchbee_workbench_protocol::SessionId::parse("11111111111111111111111111111111")
                .unwrap(),
        ),
    ));
    service.initialize_preferences(scope.clone()).unwrap();
    assert!(service.initialize_preferences(scope.clone()).is_err());
    let dispatch = |allowed, session, operation: Value| {
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
        let request: SettingsRequest = serde_json::from_value(json!({"protocolVersion": 1,
            "requestId": "22222222222222222222222222222222", "sessionId": session, "operation": operation})).unwrap();
        request.validate().unwrap();
        let WorkbenchExecution::Response(WorkbenchResponse::Settings(result)) =
            workbench.execute(WorkbenchRequest::Settings(request.clone()))
        else {
            panic!("settings receipt");
        };
        assert_eq!(result.request_id, request.request_id);
        result.outcome
    };
    let session = "11111111111111111111111111111111";
    assert_eq!(
        dispatch(false, session, json!({"kind":"open"})),
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
            reason: Failure::SessionMismatch
        }
    );
    assert!(!scope.directory().exists());
    let Outcome::Opened { snapshot } = dispatch(true, session, json!({"kind":"open"})) else {
        panic!("open");
    };
    let request =
        json!({"kind":"save", "source": snapshot.source, "changes":{"applicationTheme":"dark"}});
    assert_eq!(
        dispatch(false, session, request.clone()),
        Outcome::Refused {
            reason: Failure::Denied
        }
    );
    let Outcome::Saved {
        snapshot: saved,
        changed: true,
    } = dispatch(true, session, request.clone())
    else {
        panic!("save");
    };
    assert_eq!(
        dispatch(true, session, request),
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
