use super::*;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "orchbee-config-reload-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self) -> PathBuf {
        self.0.join("config.toml")
    }
    fn write(&self, text: &str) {
        std::fs::write(self.file(), text).unwrap();
    }
    fn settings(&self, explicit: bool) -> WindowsUserSettings {
        read_windows_user_settings_at(Some(&self.file()), explicit, |_| None).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let target = self.0.canonicalize().unwrap();
        assert!(target.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(
            self.0
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-config-reload-")
        );
        std::fs::remove_dir_all(target).unwrap();
    }
}
fn request(action: Action) -> ConfigurationRequest {
    ConfigurationRequest {
        protocol_version: ProtocolVersion,
        request_id: orchbee_workbench_protocol::WorkbenchRequestId::parse(&format!("{:032x}", 1))
            .unwrap(),
        site_id: orchbee_workbench_protocol::SiteId::parse(&format!("{:032x}", 2)).unwrap(),
        action,
    }
}
fn current(registry: &Registry) -> ConfigurationSnapshot {
    let Outcome::Current { snapshot } =
        registry.execute(&request(Action::Current), |_| panic!("read cannot apply"))
    else {
        panic!("current publication");
    };
    snapshot
}

#[test]
fn real_configuration_owner_updates_coordinator_and_invalid_reload_retains_live_defaults() {
    use crate::{
        local_terminal_service::LocalTerminalService,
        terminal_coordinator::{PreparedOperation, WorkerPlacement},
        worker_registry::WorkerRegistry,
    };
    use orchbee_capability_node_protocol::{
        CapabilityNodeMessage, LeaseExpiresAtUnixMilliseconds, WorkerGeneration, WorkerId,
    };
    use orchbee_client_server_protocol::{
        ClientMessage, RequestId, StartMode, TerminalCreateRequest,
    };
    use std::{
        num::NonZeroU64,
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };
    struct Owner(Arc<LocalTerminalService>);
    impl Drop for Owner {
        fn drop(&mut self) {
            self.0
                .stop_process_observer_before(Instant::now() + Duration::from_secs(3))
                .unwrap();
        }
    }
    let fixture = Fixture::new();
    fixture.write("[terminal]\ndefault_shell='old.exe'");
    let reload = request(Action::Reload);
    let owner = Owner(Arc::new(LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Ok(reload.site_id.clone()),
    )));
    owner
        .0
        .start_process_observer_with_settings(fixture.settings(true), false)
        .unwrap();
    let capture = |id: u128| {
        let PreparedOperation::Dispatch(pending) = owner.0.coordinator.lock().unwrap().prepare(
            ClientMessage::TerminalCreate(TerminalCreateRequest::new(
                RequestId::parse(&format!("{id:032x}")).unwrap(),
                None,
                None,
                StartMode::Deferred,
                None,
            )),
            Some(WorkerPlacement {
                worker_id: WorkerId::from_bytes([9; 16]).unwrap(),
                generation: WorkerGeneration::new(NonZeroU64::MIN),
            }),
            Some(LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::MAX)),
        ) else {
            panic!("prepared");
        };
        let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = pending.command() else {
            panic!("launch");
        };
        launch.default_shell().unwrap().clone()
    };
    let old = capture(11);
    assert_eq!(old.executable(), "old.exe");
    fixture.write("[terminal]\ndefault_shell='new.exe'\nshell_mode='login'");
    let Outcome::Reloaded { snapshot, .. } = owner.0.execute_configuration(reload.clone()).outcome
    else {
        panic!("reload");
    };
    assert!(snapshot.restart_required.is_empty());
    let new = capture(12);
    assert_eq!(snapshot.default_shell, "new.exe");
    assert_eq!(snapshot.shell_mode, ConfigurationShellMode::Login);
    assert_eq!(new.executable(), "new.exe");
    assert_eq!(new.mode().as_str(), "login");
    fixture.write("[terminal]\ndefault_shell='wrong.exe'\nshell_mode='invalid'");
    assert!(matches!(
        owner.0.execute_configuration(reload.clone()).outcome,
        Outcome::Failed {
            reason: Failure::LoadFailed,
            ..
        }
    ));
    assert_eq!(capture(13), new);
    assert_eq!(current(&owner.0.configuration), snapshot);
    assert_eq!(old.executable(), "old.exe");
    fixture.write("[terminal]\ndefault_shell='blocked.exe'");
    owner
        .0
        .stop_process_observer_before(Instant::now() + Duration::from_secs(3))
        .unwrap();
    assert!(matches!(
        owner.0.execute_configuration(reload).outcome,
        Outcome::Failed { .. }
    ));
    assert_eq!(capture(14), new);
    assert_eq!(current(&owner.0.configuration), snapshot);
}

#[test]
fn reload_publishes_valid_candidates_and_retains_previous_state_on_load_or_apply_failure() {
    let fixture = Fixture::new();
    fixture.write("[keys]\nprefix='ctrl+a'");
    let registry = Registry::default();
    registry.initialize(&fixture.settings(true)).unwrap();
    assert_eq!(current(&registry).revision.value(), 0);
    fixture.write("[keys]\nprefix='ctrl+d'\n[update]\nmanifest_check=false\n[terminal]\ndefault_shell='cmd.exe'");
    let Outcome::Reloaded {
        snapshot,
        changed: true,
    } = registry.execute(&request(Action::Reload), |settings| {
        !settings.manifest_check
    })
    else {
        panic!("reload");
    };
    assert_eq!(snapshot.revision.value(), 1);
    assert!(snapshot.restart_required.is_empty());
    assert!(snapshot.keys_profile.contains("ctrl+d"));
    assert!(matches!(
        registry.execute(&request(Action::Reload), |_| true),
        Outcome::Reloaded { changed: false, .. }
    ));
    fixture.write("[keys]\nprefix='ctrl+x'");
    assert!(matches!(
        registry.execute(&request(Action::Reload), |_| false),
        Outcome::Failed {
            reason: Failure::Unavailable,
            ..
        }
    ));
    assert_eq!(current(&registry), snapshot);
    fixture.write("[keys]\nhelp='ctrl+c'");
    assert!(matches!(
        registry.execute(&request(Action::Reload), |_| panic!("invalid cannot apply")),
        Outcome::Failed {
            reason: Failure::LoadFailed,
            ..
        }
    ));
    assert_eq!(current(&registry), snapshot);
    std::fs::remove_file(fixture.file()).unwrap();
    assert!(matches!(
        registry.execute(&request(Action::Reload), |_| panic!(
            "missing explicit cannot apply"
        )),
        Outcome::Failed {
            reason: Failure::LoadFailed,
            ..
        }
    ));
    assert_eq!(current(&registry), snapshot);
}

#[test]
fn missing_default_resets_keys_and_restart_requirements_compare_against_startup() {
    let fixture = Fixture::new();
    fixture.write("[terminal]\ndefault_shell='cmd.exe'\n[session]\nresume_agents_on_restore=false");
    let registry = Registry::default();
    registry.initialize(&fixture.settings(false)).unwrap();
    std::fs::remove_file(fixture.file()).unwrap();
    let Outcome::Reloaded { snapshot, .. } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!("reset defaults");
    };
    assert_eq!(snapshot.restart_required, [Restart::ResumeAgents]);
    fixture.write("[terminal]\ndefault_shell='cmd.exe'\n[session]\nresume_agents_on_restore=false");
    let Outcome::Reloaded { snapshot, .. } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!("return to startup");
    };
    assert!(snapshot.restart_required.is_empty());
}

#[test]
fn busy_exhaustion_and_uninitialized_owner_never_apply_settings() {
    let registry = Registry::default();
    assert!(matches!(
        registry.execute(&request(Action::Reload), |_| panic!()),
        Outcome::Failed {
            reason: Failure::Unavailable,
            ..
        }
    ));
    let fixture = Fixture::new();
    fixture.write("");
    registry.initialize(&fixture.settings(true)).unwrap();
    {
        let mut lock = registry.0.lock().unwrap();
        assert!(matches!(
            registry.execute(&request(Action::Reload), |_| panic!()),
            Outcome::Failed {
                reason: Failure::Busy,
                ..
            }
        ));
        lock.as_mut().unwrap().snapshot.revision = WorkbenchRevision::new(u64::MAX);
    }
    fixture.write("[update]\nmanifest_check=false");
    assert!(matches!(
        registry.execute(&request(Action::Reload), |_| panic!()),
        Outcome::Failed {
            reason: Failure::RevisionExhausted,
            ..
        }
    ));
}
