use super::*;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
pub(super) struct Fixture(PathBuf);
impl Fixture {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "orchbee-config-reload-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub(super) fn file(&self) -> PathBuf {
        self.0.join("config.toml")
    }
    pub(super) fn write(&self, text: &str) {
        std::fs::write(self.file(), text).unwrap();
    }
    pub(super) fn settings(&self, explicit: bool) -> UserSettings {
        read_user_settings_at(Some(&self.file()), explicit, |_| None)
            .unwrap()
            .settings
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
pub(super) fn request(action: Action) -> ConfigurationRequest {
    ConfigurationRequest {
        protocol_version: ProtocolVersion,
        request_id: orchbee_workbench_protocol::WorkbenchRequestId::parse(&format!("{:032x}", 1))
            .unwrap(),
        session_id: orchbee_workbench_protocol::SessionId::parse(&format!("{:032x}", 2)).unwrap(),
        action,
    }
}
pub(super) fn current(registry: &Registry) -> ConfigurationSnapshot {
    let Outcome::Current { snapshot } =
        registry.execute(&request(Action::Current), |_| panic!("read cannot apply"))
    else {
        panic!("current publication");
    };
    snapshot
}

#[test]
fn theme_only_publication_advances_revision_and_fatal_reload_retains_the_complete_snapshot() {
    use orchbee_workbench_protocol::ConfigurationThemeName;
    let fixture = Fixture::new();
    fixture.write("[theme]\nname='terminal-native'");
    let registry = Registry::default();
    registry.initialize(&fixture.settings(true)).unwrap();
    let baseline = current(&registry);
    assert_eq!(baseline.theme_name, ConfigurationThemeName::TerminalNative);
    for (index, name, expected) in [
        (1, "one-light", ConfigurationThemeName::OneLight),
        (2, "one-dark", ConfigurationThemeName::OneDark),
    ] {
        fixture.write(&format!("[theme]\nname='{name}'"));
        let Outcome::Reloaded {
            snapshot,
            changed: true,
        } = registry.execute(&request(Action::Reload), |settings| {
            settings.theme.as_str() == name
        })
        else {
            panic!("theme-only candidate must publish")
        };
        assert_eq!(snapshot.revision.value(), index);
        assert_eq!(snapshot.theme_name, expected);
        assert_eq!(snapshot.keys_profile, baseline.keys_profile);
        assert_eq!(snapshot.notification_profile, baseline.notification_profile);
        assert_eq!(
            snapshot.spaces_sidebar_profile,
            baseline.spaces_sidebar_profile
        );
        assert!(snapshot.restart_required.is_empty());
        assert_eq!(current(&registry), snapshot);
    }
    let retained = current(&registry);
    fixture.write("[theme");
    assert!(matches!(
        registry.execute(&request(Action::Reload), |_| panic!("fatal cannot apply")),
        Outcome::Failed {
            reason: Failure::LoadFailed,
            ..
        }
    ));
    assert_eq!(current(&registry), retained);
    fixture.write("[theme]\nname='unrecognized'");
    let Outcome::Reloaded {
        snapshot,
        changed: true,
    } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!("field fallback must publish")
    };
    assert_eq!(snapshot.theme_name, ConfigurationThemeName::TerminalNative);
    assert_eq!(snapshot.revision.value(), retained.revision.value() + 1);
    assert_eq!(snapshot.diagnostics.items[0].field, "theme.name");
}

pub(in crate::local_terminal_service) fn maximum_accepted_source_with_escaped_profiles() -> String {
    let command = "\\".repeat(4096);
    let description = "\\".repeat(512);
    let mut commands = String::new();
    for _ in 0..7 {
        commands.push_str(&format!(
            "[[keys.command]]\ncommand='{command}'\ndescription='{description}'\n"
        ));
    }
    commands.push_str(&format!(
        "[[keys.command]]\ncommand='{}'\n",
        "\\".repeat(279)
    ));
    let rules = [4089, 4000, 4000, 4000]
        .map(|length| format!("{{equals='{}'}}", "\\".repeat(length)))
        .into_iter()
        .collect::<Vec<_>>()
        .join(",");
    let sidebars = format!(
        "[ui.sidebar.agents]\nrows=[[{{token='agent',rules=[{rules}]}}]]\n\
         [ui.sidebar.spaces]\nrows=[[{{token='workspace',rules=[{rules}]}}]]\n"
    );
    let unknown_fields = (0..40)
        .map(|index| format!("'{index:02}'=1\n"))
        .collect::<String>();
    let source = format!("{commands}[ui]\n{unknown_fields}{sidebars}#x\n");
    assert_eq!(source.len(), 64 * 1024);
    source
}

#[test]
fn startup_and_reload_reject_an_accepted_source_whose_composed_response_is_oversized() {
    let fixture = Fixture::new();
    fixture.write(&maximum_accepted_source_with_escaped_profiles());
    let candidate = read_user_settings_at(Some(&fixture.file()), true, |_| None).unwrap();
    assert_eq!(candidate.diagnostics.items().len(), 40);
    let keys_profile = candidate.settings.keys.profile();
    let agent_sidebar_profile = candidate.settings.agent_sidebar.profile();
    let spaces_sidebar_profile = candidate.settings.space_sidebar.profile();
    assert_eq!(keys_profile.len(), 65_534);
    assert_eq!(agent_sidebar_profile.len(), 32_767);
    assert_eq!(spaces_sidebar_profile.len(), 32_752);
    assert!(orchbee_local_runtime::KeySettings::from_profile(&keys_profile).is_ok());
    assert!(
        orchbee_local_runtime::AgentSidebarSettings::from_profile(&agent_sidebar_profile).is_ok()
    );
    assert!(
        orchbee_local_runtime::SpaceSidebarSettings::from_profile(&spaces_sidebar_profile).is_ok()
    );
    let diagnostics = protocol_diagnostics(&candidate.diagnostics).unwrap();
    assert!(diagnostics.validate().is_ok());
    let candidate_snapshot = snapshot(&candidate.settings, diagnostics, 0);
    let publication = Outcome::Current {
        snapshot: candidate_snapshot,
    };
    assert!(publication_result(publication.clone()).validate().is_ok());
    let Err(ContractError::EncodedValueTooLarge { actual, maximum }) =
        preflight_publication(&publication)
    else {
        panic!("composed Current response must exceed its exact wire bound")
    };
    assert!(actual > maximum);
    assert_eq!(
        maximum,
        orchbee_workbench_protocol::MAXIMUM_CONFIGURATION_BYTES
    );

    let startup = Registry::default();
    let error = startup
        .initialize_load(&candidate.settings, &candidate)
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(matches!(
        startup.execute(&request(Action::Current), |_| panic!(
            "uninitialized startup"
        )),
        Outcome::Failed {
            reason: Failure::Unavailable,
            ..
        }
    ));

    let registry = Registry::default();
    registry.initialize(&UserSettings::default()).unwrap();
    let retained = current(&registry);
    let mut applied = false;
    let outcome = registry.execute_with(
        &request(Action::Reload),
        move |_, _| Ok(candidate),
        |_| {
            applied = true;
            true
        },
    );
    assert!(matches!(
        outcome,
        Outcome::Failed {
            reason: Failure::LoadFailed,
            ..
        }
    ));
    assert!(!applied);
    assert_eq!(current(&registry), retained);
}

#[test]
fn sidebar_reload_publishes_valid_profiles_and_defaults_only_invalid_profiles() {
    let fixture = Fixture::new();
    fixture.write(
        "[keys]\nprefix='ctrl+a'\n[ui.sidebar.agents]\nrows=[['state_text','$summary']]\nrow_gap=1\n[ui.sidebar.spaces]\nrows=[['workspace','$summary']]\nrow_gap=3",
    );
    let registry = Registry::default();
    registry.initialize(&fixture.settings(true)).unwrap();
    let Outcome::Reloaded { snapshot, .. } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!()
    };
    let layout =
        orchbee_local_runtime::AgentSidebarSettings::from_profile(&snapshot.agent_sidebar_profile)
            .unwrap();
    assert_eq!(layout.row_gap(), 1);
    assert_eq!(
        layout.rows_for_agent("codex", None).unwrap()[0][1].name(),
        "$summary"
    );
    fixture.write("[keys]\nprefix='ctrl+x'\n[ui.toast]\ndelivery='terminal'\n[ui.sidebar.agents]\nrows=[['invalid-token']]");
    let Outcome::Reloaded {
        snapshot: agent_warning,
        changed: true,
    } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!("field warning remains an admitted publication")
    };
    assert!(agent_warning.keys_profile.contains("ctrl+x"));
    assert_eq!(
        agent_warning.diagnostics.items[0].field,
        "ui.sidebar.agents"
    );
    assert_eq!(
        orchbee_local_runtime::AgentSidebarSettings::from_profile(
            &agent_warning.agent_sidebar_profile
        )
        .unwrap(),
        orchbee_local_runtime::AgentSidebarSettings::default()
    );
    assert_eq!(
        orchbee_local_runtime::SpaceSidebarSettings::from_profile(&snapshot.spaces_sidebar_profile)
            .unwrap()
            .row_gap(),
        3
    );
    fixture.write("[keys]\nprefix='ctrl+x'\n[ui.toast]\ndelivery='terminal'\n[ui.sidebar.agents]\nrows=[]\n[ui.sidebar.spaces]\nrows=[['agent']]");
    let Outcome::Reloaded {
        snapshot: spaces_warning,
        changed: true,
    } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!("field warning remains an admitted publication")
    };
    assert_eq!(
        spaces_warning.diagnostics.items[0].field,
        "ui.sidebar.spaces"
    );
    assert_eq!(current(&registry), spaces_warning);
    fixture.write("[keys]\nprefix='ctrl+x'\n[ui.sidebar.agents]\nrows=[]\nrow_gap=2");
    let Outcome::Reloaded { snapshot: next, .. } =
        registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!()
    };
    assert_eq!(next.revision.value(), spaces_warning.revision.value() + 1);
    assert!(next.diagnostics.items.is_empty());
    assert_eq!(next.diagnostics.omitted_count, 0);
    assert!(next.keys_profile.contains("ctrl+x"));
    assert_eq!(
        orchbee_local_runtime::AgentSidebarSettings::from_profile(&next.agent_sidebar_profile)
            .unwrap()
            .row_gap(),
        2
    );
}

#[test]
fn managed_startup_rereads_one_complete_authoritative_server_baseline() {
    use crate::{local_terminal_service::LocalTerminalService, worker_registry::WorkerRegistry};
    use std::{
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
    fixture.write("[keys]\nprefix='ctrl+a'");
    let startup = fixture.settings(true);
    fixture.write("[keys]\nprefix='ctrl+x'\n[update]\nchannel='invalid'");
    let current = request(Action::Current);
    let owner = Owner(Arc::new(LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Ok(current.session_id.clone()),
    )));
    owner
        .0
        .start_process_observer_with_settings(startup, false)
        .unwrap();
    let Outcome::Current { snapshot } = owner.0.execute_configuration(current).outcome else {
        panic!("current startup configuration")
    };
    assert_eq!(snapshot.revision.value(), 0);
    assert!(snapshot.keys_profile.contains("ctrl+x"));
    assert!(!snapshot.keys_profile.contains("ctrl+a"));
    assert_eq!(snapshot.diagnostics.items.len(), 1);
    assert_eq!(snapshot.diagnostics.items[0].field, "update.channel");
    assert_eq!(snapshot.diagnostics.items[0].reason, "invalid-channel");
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
        Ok(reload.session_id.clone()),
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
    let Outcome::Reloaded {
        snapshot: warning,
        changed: true,
    } = owner.0.execute_configuration(reload.clone()).outcome
    else {
        panic!("invalid shell mode defaults only that leaf")
    };
    assert_eq!(warning.default_shell, "wrong.exe");
    assert_eq!(warning.shell_mode, ConfigurationShellMode::Auto);
    assert_eq!(warning.diagnostics.items[0].field, "terminal.shell_mode");
    let fallback = capture(13);
    assert_eq!(fallback.executable(), "wrong.exe");
    assert_eq!(fallback.mode().as_str(), "auto");
    assert_eq!(current(&owner.0.configuration), warning);
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
    assert_eq!(capture(14), fallback);
    assert_eq!(current(&owner.0.configuration), warning);
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
    let Outcome::Reloaded {
        snapshot: warning,
        changed: true,
    } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!("invalid action override disables that action")
    };
    assert_eq!(warning.diagnostics.items[0].field, "keys.help");
    fixture.write("[keys");
    assert!(matches!(
        registry.execute(&request(Action::Reload), |_| panic!(
            "fatal parse cannot apply"
        )),
        Outcome::Failed {
            reason: Failure::LoadFailed,
            ..
        }
    ));
    assert_eq!(current(&registry), warning);
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
    assert_eq!(current(&registry), warning);
}

#[test]
fn missing_default_resets_keys_without_requiring_restart() {
    let fixture = Fixture::new();
    fixture.write("[terminal]\ndefault_shell='cmd.exe'");
    let registry = Registry::default();
    registry.initialize(&fixture.settings(false)).unwrap();
    std::fs::remove_file(fixture.file()).unwrap();
    let Outcome::Reloaded { snapshot, .. } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!("reset defaults");
    };
    assert!(snapshot.restart_required.is_empty());
    fixture.write("[terminal]\ndefault_shell='cmd.exe'");
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

#[test]
fn reload_releases_committed_state_while_loading_the_configuration_file() {
    let registry = Registry::default();
    let settings = UserSettings::default();
    registry.initialize(&settings).unwrap();
    let candidate = orchbee_local_runtime::UserSettingsLoad {
        settings,
        diagnostics: orchbee_local_runtime::SettingsDiagnostics::default(),
    };
    let outcome = registry.execute_with(
        &request(Action::Reload),
        |_, _| {
            let state = registry
                .0
                .try_lock()
                .expect("configuration file loading must not own committed state");
            assert!(state.as_ref().unwrap().reloading);
            drop(state);
            assert!(matches!(
                registry.execute(&request(Action::Current), |_| panic!()),
                Outcome::Current { .. }
            ));
            Ok(candidate)
        },
        |_| true,
    );
    assert!(matches!(outcome, Outcome::Reloaded { .. }));
    assert!(!registry.0.lock().unwrap().as_ref().unwrap().reloading);
}

#[test]
fn window_title_reload_defaults_invalid_leaf_and_publishes_its_warning() {
    let fixture = Fixture::new();
    fixture.write("[ui]\nwindow_title='{workspace}'");
    let registry = Registry::default();
    registry.initialize(&fixture.settings(true)).unwrap();
    assert_eq!(current(&registry).window_title, "{workspace}");
    fixture.write("[ui]\nwindow_title='{tab}/{pane}'");
    let Outcome::Reloaded { snapshot, .. } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!()
    };
    assert_eq!(snapshot.window_title, "{tab}/{pane}");
    assert!(snapshot.diagnostics.items.is_empty());
    fixture.write("[ui]\nwindow_title='{unknown}'");
    let Outcome::Reloaded { snapshot, .. } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!()
    };
    assert_eq!(
        snapshot.window_title,
        orchbee_local_runtime::DEFAULT_WINDOW_TITLE
    );
    assert_eq!(snapshot.diagnostics.items[0].field, "ui.window_title");
    fixture.write("[ui]\nwindow_title=''");
    let Outcome::Reloaded { snapshot, .. } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!()
    };
    assert!(snapshot.window_title.is_empty());
    assert!(snapshot.diagnostics.items.is_empty());
    fixture.write("[ui]\nwindow_title=2");
    let Outcome::Reloaded { snapshot, .. } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!()
    };
    assert_eq!(
        snapshot.window_title,
        orchbee_local_runtime::DEFAULT_WINDOW_TITLE
    );
    assert_eq!(snapshot.diagnostics.items[0].field, "ui.window_title");
    assert_eq!(snapshot.diagnostics.items[0].reason, "wrong-type");
}

#[test]
fn notification_reload_exposes_policy_and_defaults_invalid_leaves() {
    let fixture = Fixture::new();
    fixture.write("");
    let registry = Registry::default();
    registry.initialize(&fixture.settings(true)).unwrap();
    for delivery in ["orchbee", "terminal", "system", "off"] {
        fixture.write(&format!(
            "[ui.toast]\ndelivery='{delivery}'\nagent_delivery='terminal'\ndelay_seconds=7\n[ui.toast.overlay]\nposition='top-left'\n[ui.sound]\nenabled=true\n[ui.sound.agents]\ncodex='off'"
        ));
        let Outcome::Reloaded { snapshot, .. } =
            registry.execute(&request(Action::Reload), |_| true)
        else {
            panic!()
        };
        let parsed = orchbee_local_runtime::NotificationSettings::from_profile(
            &snapshot.notification_profile,
        )
        .unwrap();
        assert_eq!(parsed.delivery.as_str(), delivery);
        assert_eq!(parsed.agent_delivery.as_str(), "terminal");
        assert_eq!(parsed.delay_seconds, 7);
        assert!(parsed.sound_enabled);
        assert_eq!(
            parsed.agent_sound("codex"),
            orchbee_local_runtime::AgentSoundSetting::Off
        );
        assert_eq!(parsed.position(), "top-left");
        assert!(snapshot.restart_required.is_empty());
    }
    fixture.write("[ui.toast]\ndelivery='bad'");
    let Outcome::Reloaded {
        snapshot: delivery_warning,
        ..
    } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!()
    };
    assert_eq!(
        delivery_warning.diagnostics.items[0].field,
        "ui.toast.delivery"
    );
    assert_eq!(
        orchbee_local_runtime::NotificationSettings::from_profile(
            &delivery_warning.notification_profile
        )
        .unwrap(),
        orchbee_local_runtime::NotificationSettings::default()
    );
    fixture.write("[ui.sound]\nenabled='yes'");
    let Outcome::Reloaded {
        snapshot: sound_warning,
        ..
    } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!()
    };
    assert_eq!(sound_warning.diagnostics.items[0].field, "ui.sound.enabled");
    assert_eq!(current(&registry), sound_warning);
}

#[test]
fn warning_record_changes_advance_revision_when_effective_settings_are_equal() {
    let fixture = Fixture::new();
    fixture.write("");
    let registry = Registry::default();
    registry.initialize(&fixture.settings(true)).unwrap();
    fixture.write("[update]\nchannel='invalid'");
    let Outcome::Reloaded {
        snapshot: warning,
        changed: true,
    } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!()
    };
    assert_eq!(warning.revision.value(), 1);
    assert_eq!(warning.diagnostics.items[0].field, "update.channel");
    fixture.write("");
    let Outcome::Reloaded {
        snapshot: clean,
        changed: true,
    } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!()
    };
    assert_eq!(clean.revision.value(), 2);
    assert!(clean.diagnostics.items.is_empty());
    assert_eq!(clean.keys_profile, warning.keys_profile);
    assert_eq!(clean.notification_profile, warning.notification_profile);
}
