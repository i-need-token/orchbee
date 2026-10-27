use super::*;
use crate::{
    workbench_service::{WorkbenchExecution, WorkbenchPolicy, WorkbenchService},
    worker_registry::WorkerRegistry,
};
use orchbee_capability_node_protocol::{
    LaunchSelection, LeaseExpiresAtUnixMilliseconds, LeaseId, OperationId, TerminalFence,
    TerminalId, TerminalLaunchMode, TerminalOperationAuthorization, TerminalOperationLease,
    WorkerGeneration, WorkerId,
};
use orchbee_workbench_protocol::{
    PreferenceKey, UserPreferencesPatch, UserPreferencesRequest, WorkbenchRequest,
    WorkbenchResponse,
};
use serde_json::{Value, json};
use std::{
    num::NonZeroU64,
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

fn private_root() -> Scope {
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
    Scope(root)
}

#[test]
fn settings_dispatch_checks_permission_and_session_before_shared_store_access() {
    let fixture = private_root();
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

#[test]
fn launch_dispatch_captures_the_frozen_scope_and_refuses_without_a_store() {
    let fixture = private_root();
    let service = LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Ok(
            orchbee_workbench_protocol::SessionId::parse("11111111111111111111111111111111")
                .unwrap(),
        ),
    );
    let authorization = TerminalOperationAuthorization::new(
        TerminalId::from_bytes([1; 16]).unwrap(),
        OperationId::from_bytes([2; 16]).unwrap(),
        WorkerId::from_bytes([3; 16]).unwrap(),
        WorkerGeneration::new(NonZeroU64::MIN),
        TerminalFence::new(NonZeroU64::MIN),
        TerminalOperationLease::new(
            LeaseId::from_bytes([4; 16]).unwrap(),
            LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::MAX),
        ),
        orchbee_capability_node_protocol::TERMINAL_LAUNCH_CAPABILITY_VERSION,
    );
    let launch = |selection| {
        AuthorizedTerminalLaunch::new(authorization, None, TerminalLaunchMode::Deferred, selection)
    };
    let captured = |selection| match service
        .with_launch_preferences(&launch(selection))
        .unwrap()
        .preferences()
    {
        LaunchPreferences::Available(captured) => Some(captured.clone()),
        LaunchPreferences::Unavailable => None,
        LaunchPreferences::Absent => panic!("a Session always states its capture"),
    };
    assert_eq!(captured(None), None);

    let scope = UserPreferencesScope::for_configuration(&fixture.0.join("config.toml")).unwrap();
    service.initialize_preferences(scope.clone()).unwrap();
    let saved = UserPreferencesStore::new(scope).execute(&UserPreferencesRequest::Update {
        source: "missing".into(),
        changes: UserPreferencesPatch::set(PreferenceKey::ApplicationTheme, json!("dark")).unwrap(),
    });
    assert!(matches!(saved, UserPreferencesResponse::Snapshot { .. }));
    let default = captured(None).unwrap();
    assert_eq!(
        default.program(),
        Some(&DefaultLaunchProgram::SystemShell { login: false })
    );
    assert_eq!(
        default.colors(),
        Some(TerminalLaunchColors {
            foreground: 0xd4d4d4,
            background: 0x1e1e1e,
            dark: true,
        })
    );
    assert_eq!(default.scrollback_bytes(), 10_000_000);
    let executable = if cfg!(windows) {
        "C:/tools/shell.exe"
    } else {
        "/bin/sh"
    };
    let explicit = LaunchSelection::new(executable.into(), Vec::new()).unwrap();
    assert_eq!(captured(Some(explicit)).unwrap().program(), None);
}
