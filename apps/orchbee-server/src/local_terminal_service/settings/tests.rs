use super::*;
use crate::{
    workbench_service::{WorkbenchExecution, WorkbenchPolicy, WorkbenchService},
    worker_registry::WorkerRegistry,
};
use orchbee_capability_node_protocol::LaunchSelection;
use orchbee_workbench_protocol::{WorkbenchRequest, WorkbenchResponse};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
impl Fixture {
    fn file(&self) -> PathBuf {
        self.0.join("config.toml")
    }
    fn load(&self) -> orchbee_local_runtime::UserSettingsLoad {
        orchbee_local_runtime::read_user_settings_at(Some(&self.file()), false, |_| None).unwrap()
    }
}

fn private_root() -> Fixture {
    // The product reaches this folder through the canonical data root; macOS's temporary folder
    // sits behind /var.
    #[cfg(unix)]
    let parent = std::env::temp_dir().canonicalize().unwrap();
    #[cfg(windows)]
    let parent = std::env::temp_dir();
    let root = parent.join(format!(
        "orchbee-session-settings-{}-{}",
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
    Fixture(root)
}

#[test]
fn settings_dispatch_checks_permission_and_session_before_file_access() {
    let fixture = private_root();
    let service = Arc::new(LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Ok(
            orchbee_workbench_protocol::SessionId::parse("11111111111111111111111111111111")
                .unwrap(),
        ),
    ));
    service.install_configuration(&fixture.load()).unwrap();
    assert!(service.install_configuration(&fixture.load()).is_err());
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
        dispatch(false, session, json!({"kind":"read"})),
        Outcome::refused(SettingsRefusal::Denied)
    );
    assert_eq!(
        dispatch(
            true,
            "33333333333333333333333333333333",
            json!({"kind":"read"})
        ),
        Outcome::refused(SettingsRefusal::SessionMismatch)
    );
    assert!(!fixture.file().exists());
    let Outcome::Current { snapshot } = dispatch(true, session, json!({"kind":"read"})) else {
        panic!("read");
    };
    assert_eq!(
        snapshot.file,
        orchbee_workbench_protocol::SettingsFile::Missing {}
    );
    let request = json!({"kind":"edit", "changes":[{"key":"appearance.mode","value":"dark"}]});
    assert_eq!(
        dispatch(false, session, request),
        Outcome::refused(SettingsRefusal::Denied)
    );
    // An admitted edit reaches the editor, which refuses an unknown key before opening any file.
    let unknown = json!({"kind":"edit", "changes":[{"key":"appearance.unknown","value":true}]});
    assert!(matches!(
        dispatch(true, session, unknown),
        Outcome::Refused {
            reason: SettingsRefusal::UnknownKey,
            ..
        }
    ));
    assert!(!fixture.file().exists());
}

#[test]
fn launch_capture_reads_the_session_settings_and_refuses_before_configuration() {
    let fixture = private_root();
    let service = LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Ok(
            orchbee_workbench_protocol::SessionId::parse("11111111111111111111111111111111")
                .unwrap(),
        ),
    );
    let executable = if cfg!(windows) {
        "C:/tools/shell.exe"
    } else {
        "/bin/sh"
    };
    let explicit = LaunchSelection::new(executable.into(), Vec::new()).unwrap();
    let captured = |selection| match service.launch_capture().preferences(selection) {
        LaunchPreferences::Available(captured) => Some(captured),
        LaunchPreferences::Unavailable => None,
        LaunchPreferences::Absent => panic!("a Session always states its capture"),
    };
    assert_eq!(captured(None), None);
    assert_eq!(captured(Some(&explicit)), None);

    std::fs::write(fixture.file(), "[appearance]\nmode = \"dark\"\n").unwrap();
    service.install_configuration(&fixture.load()).unwrap();
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
    // One read serves both: an explicit selection keeps the colors and scrollback, never a program.
    let explicit = captured(Some(&explicit)).unwrap();
    assert_eq!(explicit.program(), None);
    assert_eq!(explicit.colors(), default.colors());
    assert_eq!(explicit.scrollback_bytes(), default.scrollback_bytes());
}
