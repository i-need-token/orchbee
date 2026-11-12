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
    pub(super) fn load(&self, explicit: bool) -> UserSettingsLoad {
        read_user_settings_at(Some(&self.file()), explicit, |_| None).unwrap()
    }
    pub(super) fn settings(&self, explicit: bool) -> UserSettings {
        self.load(explicit).settings
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
        "[appearance.sidebar.agents]\nrows=[[{{token='agent',rules=[{rules}]}}]]\n\
         [appearance.sidebar.workspaces]\nrows=[[{{token='workspace',rules=[{rules}]}}]]\n"
    );
    let unknown_fields = (0..40)
        .map(|index| format!("'{index:02}'=1\n"))
        .collect::<String>();
    let source = format!("{commands}[appearance]\n{unknown_fields}{sidebars}#x\n");
    assert!(source.len() as u64 <= orchbee_local_runtime::MAXIMUM_USER_SETTINGS_BYTES);
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
    let error = startup.initialize_load(&candidate).unwrap_err();
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
        "[keys]\nprefix='ctrl+a'\n[appearance.sidebar.agents]\nrows=[['state_text','$summary']]\nrow_gap=1\n[appearance.sidebar.workspaces]\nrows=[['workspace','$summary']]\nrow_gap=3",
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
    fixture.write("[keys]\nprefix='ctrl+x'\n[notifications]\ncli_delivery='terminal'\n[appearance.sidebar.agents]\nrows=[['invalid-token']]");
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
        "appearance.sidebar.agents"
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
    fixture.write("[keys]\nprefix='ctrl+x'\n[notifications]\ncli_delivery='terminal'\n[appearance.sidebar.agents]\nrows=[]\n[appearance.sidebar.workspaces]\nrows=[['agent']]");
    let Outcome::Reloaded {
        snapshot: spaces_warning,
        changed: true,
    } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!("field warning remains an admitted publication")
    };
    assert_eq!(
        spaces_warning.diagnostics.items[0].field,
        "appearance.sidebar.workspaces"
    );
    assert_eq!(current(&registry), spaces_warning);
    fixture.write("[keys]\nprefix='ctrl+x'\n[appearance.sidebar.agents]\nrows=[]\nrow_gap=2");
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
fn managed_startup_publishes_its_single_read_as_revision_zero() {
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
    fixture.write("[keys]\nprefix='ctrl+x'\n[update]\nchannel='invalid'");
    let startup = fixture.load(true);
    let current = request(Action::Current);
    let owner = Owner(Arc::new(LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Ok(current.session_id.clone()),
    )));
    owner
        .0
        .start_process_observer_with_configuration(startup, false)
        .unwrap();
    let Outcome::Current { snapshot } = owner.0.execute_configuration(current).outcome else {
        panic!("current startup configuration")
    };
    assert_eq!(snapshot.revision.value(), 0);
    assert!(snapshot.keys_profile.contains("ctrl+x"));
    assert_eq!(snapshot.diagnostics.items.len(), 1);
    assert_eq!(snapshot.diagnostics.items[0].field, "update.channel");
    assert_eq!(snapshot.diagnostics.items[0].reason, "invalid-channel");
}

#[test]
fn real_session_sampler_publishes_edits_and_failed_apply_retains_the_snapshot() {
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
    let reload = request(Action::Reload);
    let owner = Owner(Arc::new(LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Ok(reload.session_id.clone()),
    )));
    owner
        .0
        .start_process_observer_with_configuration(fixture.load(true), false)
        .unwrap();
    // No explicit reload: the Session's sampler publishes the edit.
    fixture.write("[keys]\nprefix='ctrl+d'\n");
    let deadline = Instant::now() + Duration::from_secs(5);
    let snapshot = loop {
        if let Outcome::Current { snapshot } = owner
            .0
            .execute_configuration(request(Action::Current))
            .outcome
            && snapshot.keys_profile.contains("ctrl+d")
        {
            break snapshot;
        }
        assert!(Instant::now() < deadline, "sampled publication");
        std::thread::sleep(Duration::from_millis(20));
    };
    // The fixture's startup read ignores the environment, so the sampler's first real-environment
    // observation may publish its own revision before the edit.
    assert!(snapshot.revision.value() >= 1);
    assert!(matches!(
        owner.0.execute_configuration(reload.clone()).outcome,
        Outcome::Reloaded { snapshot: ref explicit, changed: false } if *explicit == snapshot
    ));
    owner
        .0
        .stop_process_observer_before(Instant::now() + Duration::from_secs(3))
        .unwrap();
    fixture.write("[keys]\nprefix='ctrl+x'");
    assert!(matches!(
        owner.0.execute_configuration(reload).outcome,
        Outcome::Failed { .. }
    ));
    assert_eq!(current(&owner.0.configuration), snapshot);
}

#[test]
fn reload_publishes_valid_candidates_and_retains_previous_state_on_load_or_apply_failure() {
    let fixture = Fixture::new();
    fixture.write("[keys]\nprefix='ctrl+a'");
    let registry = Registry::default();
    registry.initialize(&fixture.settings(true)).unwrap();
    assert_eq!(current(&registry).revision.value(), 0);
    fixture.write("[keys]\nprefix='ctrl+d'\n[agent_detection]\nauto_update=false");
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
        let mut lock = registry.state.lock().unwrap();
        assert!(matches!(
            registry.execute(&request(Action::Reload), |_| panic!()),
            Outcome::Failed {
                reason: Failure::Busy,
                ..
            }
        ));
        lock.as_mut().unwrap().snapshot.revision = WorkbenchRevision::new(u64::MAX);
    }
    fixture.write("[agent_detection]\nauto_update=false");
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
        syntax: None,
        unreadable: None,
    };
    let outcome = registry.execute_with(
        &request(Action::Reload),
        |_, _| {
            let state = registry
                .state
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
    assert!(!registry.state.lock().unwrap().as_ref().unwrap().reloading);
}

#[test]
fn window_title_reload_defaults_invalid_leaf_and_publishes_its_warning() {
    let fixture = Fixture::new();
    fixture.write("[appearance]\nwindow_title='{workspace}'");
    let registry = Registry::default();
    registry.initialize(&fixture.settings(true)).unwrap();
    assert_eq!(current(&registry).window_title, "{workspace}");
    fixture.write("[appearance]\nwindow_title='{tab}/{panel}'");
    let Outcome::Reloaded { snapshot, .. } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!()
    };
    assert_eq!(snapshot.window_title, "{tab}/{panel}");
    assert!(snapshot.diagnostics.items.is_empty());
    fixture.write("[appearance]\nwindow_title='{unknown}'");
    let Outcome::Reloaded { snapshot, .. } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!()
    };
    assert_eq!(
        snapshot.window_title,
        orchbee_local_runtime::DEFAULT_WINDOW_TITLE
    );
    assert_eq!(
        snapshot.diagnostics.items[0].field,
        "appearance.window_title"
    );
    fixture.write("[appearance]\nwindow_title=''");
    let Outcome::Reloaded { snapshot, .. } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!()
    };
    assert!(snapshot.window_title.is_empty());
    assert!(snapshot.diagnostics.items.is_empty());
    fixture.write("[appearance]\nwindow_title=2");
    let Outcome::Reloaded { snapshot, .. } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!()
    };
    assert_eq!(
        snapshot.window_title,
        orchbee_local_runtime::DEFAULT_WINDOW_TITLE
    );
    assert_eq!(
        snapshot.diagnostics.items[0].field,
        "appearance.window_title"
    );
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
            "[notifications]\ncli_delivery='{delivery}'\nagent_reminder='when-hidden'\nsound=true\ndelay_seconds=7\nposition='top-left'\n[notifications.sound_by_agent]\ncodex='off'"
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
        // Automatic Agent delivery and the global sound switch are shared settings that Clients
        // read from the settings snapshot, so the file's values do not reach this profile.
        let defaults = orchbee_local_runtime::NotificationSettings::default();
        assert_eq!(
            parsed.agent_delivery.as_str(),
            defaults.agent_delivery.as_str()
        );
        assert_eq!(parsed.delay_seconds, 7);
        assert_eq!(parsed.sound_enabled, defaults.sound_enabled);
        assert_eq!(
            parsed.agent_sound("codex"),
            orchbee_local_runtime::AgentSoundSetting::Off
        );
        assert_eq!(parsed.position(), "top-left");
    }
    fixture.write("[notifications]\ncli_delivery='bad'");
    let Outcome::Reloaded {
        snapshot: delivery_warning,
        ..
    } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!()
    };
    assert_eq!(
        delivery_warning.diagnostics.items[0].field,
        "notifications.cli_delivery"
    );
    assert_eq!(
        orchbee_local_runtime::NotificationSettings::from_profile(
            &delivery_warning.notification_profile
        )
        .unwrap(),
        orchbee_local_runtime::NotificationSettings::default()
    );
    fixture.write("[notifications]\nsound='yes'");
    let Outcome::Reloaded {
        snapshot: sound_warning,
        ..
    } = registry.execute(&request(Action::Reload), |_| true)
    else {
        panic!()
    };
    assert_eq!(
        sound_warning.diagnostics.items[0].field,
        "notifications.sound"
    );
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

fn settings(registry: &Registry) -> SettingsSnapshot {
    let SettingsOutcome::Current { snapshot } = registry.settings_current() else {
        panic!("current settings");
    };
    *snapshot
}

fn sampled(
    path: Option<&std::path::Path>,
    explicit: bool,
) -> Result<UserSettingsLoad, SettingsError> {
    read_user_settings_at(path, explicit, |_| None)
}

fn watched(registry: &Registry) -> std::sync::Arc<orchbee_server::SessionChangeSignal> {
    let signal = std::sync::Arc::new(orchbee_server::SessionChangeSignal::default());
    registry.changes.watch(&signal);
    signal
}

fn set(key: &str, value: orchbee_workbench_protocol::SettingValue) -> SettingChange {
    SettingChange::Set {
        key: orchbee_workbench_protocol::SettingPath::parse(key).unwrap(),
        value,
    }
}

fn value(
    snapshot: &SettingsSnapshot,
    key: &str,
) -> Option<orchbee_workbench_protocol::SettingScalar> {
    snapshot
        .values
        .get(&orchbee_workbench_protocol::SettingPath::parse(key).unwrap())
        .cloned()
        .flatten()
}

/// Edits through the real editor, with the fixture as the data root.
fn edit_in(
    fixture: &Fixture,
) -> impl FnOnce(Option<&std::path::Path>, &SettingChanges) -> Result<bool, SettingsEditError> + '_
{
    move |path, changes| {
        let path = path.map(|path| path.as_os_str().to_owned());
        orchbee_local_runtime::edit_settings(changes, |name| match name {
            "ORCHBEE_HOME" => Some(fixture.0.clone().into_os_string()),
            "ORCHBEE_CONFIG_FILE" => path.clone(),
            _ => None,
        })
    }
}

#[test]
fn an_edit_against_an_unparsable_file_writes_nothing_and_answers_its_position() {
    let fixture = Fixture::new();
    fixture.write("[terminal]\nfocus_new_panel = true\n[keys\n");
    let startup = fixture.load(true);
    let syntax = startup.syntax.unwrap();
    let registry = Registry::default();
    registry.initialize_load(&startup).unwrap();
    let signal = watched(&registry);
    let before = std::fs::read(fixture.file()).unwrap();
    let (outcome, output_off) = registry.edit_with(
        &[set(
            "terminal.focus_new_panel",
            orchbee_workbench_protocol::SettingValue::Boolean(false),
        )],
        edit_in(&fixture),
        sampled,
        |_| panic!("nothing was written"),
    );
    assert!(!output_off);
    assert_eq!(
        outcome,
        SettingsOutcome::Refused {
            reason: SettingsRefusal::SyntaxError,
            key: None,
            detail: Some("invalid-toml".into()),
            line: Some(u32::try_from(syntax.line).unwrap()),
            column: Some(u32::try_from(syntax.column).unwrap()),
        }
    );
    assert_eq!(std::fs::read(fixture.file()).unwrap(), before);
    assert_eq!(signal.revision(), 0);
}

#[test]
fn a_refused_edit_publishes_nothing() {
    let fixture = Fixture::new();
    fixture.write("[terminal]\nfocus_new_panel = true\n");
    let registry = Registry::default();
    registry.initialize_load(&fixture.load(true)).unwrap();
    let signal = watched(&registry);
    let (configuration, published) = (current(&registry), settings(&registry));
    let (outcome, _) = registry.edit_with(
        &[set(
            "terminal.focus_new_panel",
            orchbee_workbench_protocol::SettingValue::Text("yes".into()),
        )],
        edit_in(&fixture),
        sampled,
        |_| panic!("nothing was written"),
    );
    assert!(matches!(
        outcome,
        SettingsOutcome::Refused {
            reason: SettingsRefusal::InvalidValue,
            key: Some(ref key),
            ..
        } if key.as_str() == "terminal.focus_new_panel"
    ));
    assert_eq!(
        std::fs::read_to_string(fixture.file()).unwrap(),
        "[terminal]\nfocus_new_panel = true\n"
    );
    assert_eq!(signal.revision(), 0);
    assert_eq!(current(&registry), configuration);
    assert_eq!(settings(&registry), published);
}

#[test]
fn one_committed_edit_publishes_exactly_one_snapshot_with_a_higher_revision() {
    let fixture = Fixture::new();
    fixture.write("[terminal]\nfocus_new_panel = true\n");
    let registry = Registry::default();
    registry.initialize_load(&fixture.load(true)).unwrap();
    let signal = watched(&registry);
    let before = settings(&registry);
    let (outcome, _) = registry.edit_with(
        &[set(
            "terminal.focus_new_panel",
            orchbee_workbench_protocol::SettingValue::Boolean(false),
        )],
        edit_in(&fixture),
        sampled,
        |_| true,
    );
    let SettingsOutcome::Edited {
        snapshot,
        changed: true,
    } = outcome
    else {
        panic!("edited: {outcome:?}");
    };
    assert!(snapshot.revision.value() > before.revision.value());
    assert_eq!(
        value(&snapshot, "terminal.focus_new_panel"),
        Some(orchbee_workbench_protocol::SettingScalar::Boolean(false))
    );
    assert_eq!(settings(&registry), *snapshot);
    assert!(!current(&registry).focus_new_panel);
    assert_eq!(signal.revision(), 1);
    // The sampler then finds the file it would publish already published.
    let mut sampler = Sampler::default();
    registry.sample(&mut sampler, Instant::now(), sampled, |_| {
        panic!("equal candidate")
    });
    assert_eq!(signal.revision(), 1);
    assert_eq!(settings(&registry), *snapshot);
}

#[test]
fn an_external_edit_is_observed_and_published() {
    let fixture = Fixture::new();
    fixture.write("[keys]\nprefix='ctrl+a'");
    let registry = Registry::default();
    registry.initialize_load(&fixture.load(true)).unwrap();
    let signal = watched(&registry);
    let mut sampler = Sampler::default();
    let at = |samples: u32| Instant::now() + SAMPLE_INTERVAL * samples;
    registry.sample(&mut sampler, at(0), sampled, |_| panic!("equal candidate"));
    assert_eq!(signal.revision(), 0);
    // Lengths differ so coarse modification times cannot hide an edit.
    fixture.write("[keys]\nprefix='ctrl+d'\n[appearance]\nmode='dark'\n");
    // Refused application, such as busy manifest maintenance, retries the same file.
    registry.sample(&mut sampler, at(1), sampled, |_| false);
    assert_eq!(signal.revision(), 0);
    registry.sample(&mut sampler, at(2), sampled, |_| true);
    assert_eq!(signal.revision(), 1);
    let configuration = current(&registry);
    let published = settings(&registry);
    assert!(configuration.keys_profile.contains("ctrl+d"));
    assert_eq!(configuration.revision.value(), 1);
    assert_eq!(published.revision.value(), 1);
    assert_eq!(
        value(&published, "appearance.mode"),
        Some(orchbee_workbench_protocol::SettingScalar::Text(
            "dark".into()
        ))
    );
}

#[test]
fn the_last_valid_values_are_served_during_a_syntax_error() {
    let fixture = Fixture::new();
    fixture.write("[appearance]\nmode = 'dark'\n");
    let registry = Registry::default();
    registry.initialize_load(&fixture.load(true)).unwrap();
    let signal = watched(&registry);
    let configuration = current(&registry);
    let mut sampler = Sampler::default();
    let at = |samples: u32| Instant::now() + SAMPLE_INTERVAL * samples;
    registry.sample(&mut sampler, at(0), sampled, |_| panic!("equal candidate"));
    fixture.write("[appearance\nmode = 'light'\n");
    let syntax = fixture.load(true).syntax.unwrap();
    registry.sample(&mut sampler, at(1), sampled, |_| {
        panic!("a file that does not parse is never applied")
    });
    assert_eq!(signal.revision(), 1);
    let broken = settings(&registry);
    assert_eq!(
        broken.file,
        SettingsFile::SyntaxError {
            reason: orchbee_workbench_protocol::SettingsSyntaxReason::InvalidToml,
            line: u32::try_from(syntax.line).unwrap(),
            column: u32::try_from(syntax.column).unwrap(),
        }
    );
    assert_eq!(
        value(&broken, "appearance.mode"),
        Some(orchbee_workbench_protocol::SettingScalar::Text(
            "dark".into()
        ))
    );
    assert!(
        broken
            .set
            .iter()
            .any(|key| key.as_str() == "appearance.mode")
    );
    assert_eq!(current(&registry), configuration);
    assert!(matches!(
        registry.execute(&request(Action::Reload), |_| panic!("not applied")),
        Outcome::Failed {
            reason: Failure::LoadFailed,
            ..
        }
    ));
}
