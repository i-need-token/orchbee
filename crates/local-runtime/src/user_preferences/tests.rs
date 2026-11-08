use super::*;
use orchbee_workbench_protocol::{UserLaunchCommand, UserPreferencesPatch};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
static SERIAL: AtomicU64 = AtomicU64::new(0);

#[test]
fn new_terminal_defaults_capture_shared_changes_without_mutating_existing_facts() {
    let fixture = Fixture::new();
    assert!(
        fixture
            .store
            .terminal_launch_intent(false)
            .unwrap()
            .colors
            .is_none(),
        "a fresh System + Theme preference does not invent terminal colors"
    );
    let executable = std::env::current_exe()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let profiles = json!({ "version": 1, "defaultProfileId": "custom", "profiles": [
        { "id": "custom", "name": "Shared launch", "command": {
            "kind": "custom-executable", "executable": &executable,
            "arguments": ["one argument", "literal && $ value", ""]
        }}
    ]});
    let snapshot = fixture.update("missing", PreferenceKey::LaunchProfiles, profiles);
    let snapshot = fixture.update(
        &snapshot.source,
        PreferenceKey::ApplicationTheme,
        json!("light"),
    );
    let before = fixture.store.terminal_launch_intent(true).unwrap();
    assert_eq!(
        before.program,
        Some(UserLaunchCommand::CustomExecutable {
            executable,
            arguments: vec![
                "one argument".into(),
                "literal && $ value".into(),
                String::new()
            ],
        })
    );
    assert_eq!(before.scrollback_bytes, 10_000_000);
    let other_process = UserPreferencesStore::new(fixture.store.scope().clone());
    accepted(other_process.execute(&UserPreferencesRequest::Update {
        source: snapshot.source,
        changes: UserPreferencesPatch::set(PreferenceKey::ScrollbackBytes, json!(0)).unwrap(),
    }));
    let after = fixture.store.terminal_launch_intent(true).unwrap();
    assert_eq!(after.scrollback_bytes, 0);
    assert_eq!(before.scrollback_bytes, 10_000_000);
    assert_eq!(before.program, after.program);
    let explicit = fixture.store.terminal_launch_intent(false).unwrap();
    assert!(explicit.program.is_none());
    assert_eq!(explicit.scrollback_bytes, 0);
    let saved = accepted(fixture.store.execute(&UserPreferencesRequest::Read {}));
    let dark = accepted(fixture.store.execute(&UserPreferencesRequest::Update {
        source: saved.source,
        changes: UserPreferencesPatch::set(PreferenceKey::ApplicationTheme, json!("dark")).unwrap(),
    }));
    let next = fixture.store.terminal_launch_intent(false).unwrap();
    assert_eq!(next.colors.unwrap().rgb(), (0xd4d4d4, 0x1e1e1e));
    assert_eq!(before.colors.unwrap().rgb(), (0x141414, 0xfcfcfc));
    assert_eq!(after.colors, before.colors);
    accepted(fixture.store.execute(&UserPreferencesRequest::Update {
        source: dark.source,
        changes:
            UserPreferencesPatch::set(PreferenceKey::ApplicationTheme, json!("system")).unwrap(),
    }));
    assert!(
        fixture
            .store
            .terminal_launch_intent(false)
            .unwrap()
            .colors
            .is_none()
    );
    fs::write(
        fixture.store.scope().directory().join(RECORD_NAME),
        b"invalid",
    )
    .unwrap();
    assert!(matches!(
        fixture.store.terminal_launch_intent(false),
        Err(PreferencesFailure::InvalidSource)
    ));
}

#[test]
fn duplicate_claims_in_a_durable_record_are_not_silently_coalesced() {
    let record = json!({ "version": 1, "values": UserPreferencesValues::default(),
        "claimed": ["locale", "locale"] });
    assert!(serde_json::from_value::<Record>(record).is_err());
}

struct Fixture {
    root: PathBuf,
    store: UserPreferencesStore,
}
impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = crate::test_temporary_directory().join(format!(
            "orchbee-preferences-{}-{stamp}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let store = UserPreferencesStore::new(UserPreferencesScope::for_data_root(
            &crate::DataRoot::at(root.clone()),
        ));
        Self { root, store }
    }
    fn update(
        &self,
        source: &str,
        key: PreferenceKey,
        value: serde_json::Value,
    ) -> UserPreferencesSnapshot {
        accepted(self.store.execute(&UserPreferencesRequest::Update {
            source: source.into(),
            changes: UserPreferencesPatch::set(key, value).unwrap(),
        }))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn accepted(response: UserPreferencesResponse) -> UserPreferencesSnapshot {
    match response {
        UserPreferencesResponse::Snapshot { snapshot, .. } => *snapshot,
        response => panic!("unexpected response: {response:?}"),
    }
}

#[test]
fn reads_do_not_create_storage_and_data_roots_are_isolated() {
    let fixture = Fixture::new();
    let before = fixture.store.read().unwrap();
    assert_eq!(before.source, "missing");
    assert_eq!(before.values, UserPreferencesValues::default());
    assert!(!fixture.store.scope.directory.exists());
    fixture.update(&before.source, PreferenceKey::SoundEnabled, json!(true));
    let other = UserPreferencesStore::new(UserPreferencesScope::for_data_root(
        &crate::DataRoot::at(fixture.root.join("other")),
    ));
    assert_eq!(other.read().unwrap().source, "missing");
}

#[test]
fn a_stale_writer_cannot_erase_the_other_clients_commit() {
    let fixture = Fixture::new();
    let old = fixture.store.read().unwrap();
    let committed = fixture.update(&old.source, PreferenceKey::SoundEnabled, json!(true));
    let response = fixture.store.execute(&UserPreferencesRequest::Update {
        source: old.source,
        changes: UserPreferencesPatch::set(PreferenceKey::CopyOnSelect, json!(false)).unwrap(),
    });
    assert_eq!(
        response,
        UserPreferencesResponse::Refused {
            reason: PreferencesFailure::SourceChanged
        }
    );
    assert_eq!(fixture.store.read().unwrap(), committed);
}

#[test]
fn explicit_reset_records_the_requested_default_values() {
    let fixture = Fixture::new();
    let old = fixture.store.read().unwrap();
    let reset = accepted(fixture.store.execute(&UserPreferencesRequest::Update {
        source: old.source,
        changes: UserPreferencesPatch::reset([
            PreferenceKey::ApplicationTheme,
            PreferenceKey::SoundEnabled,
        ]),
    }));
    assert!(!reset.values.sound_enabled);
    assert_eq!(
        reset.values.application_theme,
        orchbee_workbench_protocol::ApplicationAppearance::System
    );
    assert!(reset.claimed.contains(&PreferenceKey::ApplicationTheme));
    assert!(reset.claimed.contains(&PreferenceKey::SoundEnabled));
}

#[test]
fn invalid_sources_and_invalid_changes_never_repair_or_replace_data() {
    let fixture = Fixture::new();
    let initial = fixture.store.read().unwrap();
    let first = fixture.update(&initial.source, PreferenceKey::SoundEnabled, json!(true));
    let record = fixture.store.scope.directory.join(RECORD_NAME);
    let original = fs::read(&record).unwrap();
    let mut invalid_values = first.values.clone();
    invalid_values.scrollback_bytes = 1_000_000_001;
    let invalid = UserPreferencesPatch::between(&first.values, &invalid_values);
    assert_eq!(
        fixture.store.execute(&UserPreferencesRequest::Update {
            source: first.source,
            changes: invalid
        }),
        UserPreferencesResponse::Refused {
            reason: PreferencesFailure::InvalidChanges
        }
    );
    assert_eq!(fs::read(&record).unwrap(), original);
    fs::write(&record, b"{broken").unwrap();
    assert_eq!(
        fixture.store.execute(&UserPreferencesRequest::Read {}),
        UserPreferencesResponse::Refused {
            reason: PreferencesFailure::InvalidSource
        }
    );
    assert_eq!(
        fixture.store.execute(&UserPreferencesRequest::Update {
            source: "missing".into(),
            changes: UserPreferencesPatch::set(PreferenceKey::SoundEnabled, json!(true)).unwrap()
        }),
        UserPreferencesResponse::Refused {
            reason: PreferencesFailure::InvalidSource
        }
    );
    assert_eq!(fs::read(&record).unwrap(), b"{broken");
}

#[test]
fn reading_a_current_record_preserves_values_source_and_bytes() {
    use sha2::Digest;

    let fixture = Fixture::new();
    let values = UserPreferencesValues {
        application_theme: orchbee_workbench_protocol::ApplicationAppearance::Light,
        ..UserPreferencesValues::default()
    };
    let record = Record {
        version: 1,
        values,
        claimed: BTreeSet::from([PreferenceKey::SoundEnabled]),
    };
    let mut original = serde_json::to_vec_pretty(&record).unwrap();
    original.push(b'\n');
    let path = fixture.store.scope.directory.join(RECORD_NAME);
    fs::create_dir_all(&fixture.store.scope.directory).unwrap();
    fs::write(&path, &original).unwrap();

    let observed = fixture.store.read().unwrap();
    assert_eq!(
        observed.values.application_theme,
        orchbee_workbench_protocol::ApplicationAppearance::Light
    );
    assert_eq!(
        observed.claimed,
        BTreeSet::from([PreferenceKey::SoundEnabled])
    );
    assert_eq!(
        observed.source,
        format!("{:x}", sha2::Sha256::digest(&original))
    );
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn a_record_without_theme_fields_reads_with_their_defaults() {
    let fixture = Fixture::new();
    let record = Record {
        version: 1,
        values: UserPreferencesValues {
            sound_enabled: true,
            ..UserPreferencesValues::default()
        },
        claimed: BTreeSet::from([PreferenceKey::SoundEnabled]),
    };
    let mut document = serde_json::to_value(&record).unwrap();
    let values = document["values"].as_object_mut().unwrap();
    values.remove("lightTheme").unwrap();
    values.remove("darkTheme").unwrap();
    values.remove("tuiColorSource").unwrap();
    let original = serde_json::to_vec_pretty(&document).unwrap();
    let path = fixture.store.scope.directory.join(RECORD_NAME);
    fs::create_dir_all(&fixture.store.scope.directory).unwrap();
    fs::write(&path, &original).unwrap();

    let observed = fixture.store.read().unwrap();
    assert!(observed.values.sound_enabled);
    assert_eq!(observed.values.light_theme, "orchbee-light");
    assert_eq!(observed.values.dark_theme, "orchbee-dark");
    assert_eq!(
        observed.values.tui_color_source,
        orchbee_workbench_protocol::TuiColorSource::Theme
    );
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn explicitly_claimed_application_appearances_are_preserved() {
    use orchbee_workbench_protocol::ApplicationAppearance as Appearance;

    for appearance in [Appearance::Light, Appearance::Dark, Appearance::System] {
        let fixture = Fixture::new();
        let values = UserPreferencesValues {
            application_theme: appearance,
            ..UserPreferencesValues::default()
        };
        let record = Record {
            version: 1,
            values,
            claimed: BTreeSet::from([PreferenceKey::ApplicationTheme]),
        };
        let original = serde_json::to_vec(&record).unwrap();
        fs::create_dir_all(&fixture.store.scope.directory).unwrap();
        fs::write(fixture.store.scope.directory.join(RECORD_NAME), &original).unwrap();

        let snapshot = fixture.store.read().unwrap();
        assert_eq!(snapshot.values.application_theme, appearance);
        assert!(snapshot.claimed.contains(&PreferenceKey::ApplicationTheme));
        assert_eq!(
            fs::read(fixture.store.scope.directory.join(RECORD_NAME)).unwrap(),
            original
        );
    }
}

#[test]
fn a_competing_os_lock_refuses_without_stealing_or_leaving_a_pending_record() {
    let fixture = Fixture::new();
    let held = storage::Writer::acquire(&fixture.store.scope).unwrap();
    let request = UserPreferencesRequest::Update {
        source: "missing".into(),
        changes: UserPreferencesPatch::set(PreferenceKey::SoundEnabled, json!(true)).unwrap(),
    };
    assert_eq!(
        fixture.store.execute(&request),
        UserPreferencesResponse::Refused {
            reason: PreferencesFailure::Busy
        }
    );
    assert!(!fixture.store.scope.directory.join(RECORD_NAME).exists());
    drop(held);
    assert!(
        accepted(fixture.store.execute(&request))
            .values
            .sound_enabled
    );
}

#[test]
fn publication_rechecks_the_source_after_candidate_preparation() {
    let fixture = Fixture::new();
    let before = fixture.store.read().unwrap();
    fixture.update(&before.source, PreferenceKey::SoundEnabled, json!(true));
    let mut writer = storage::Writer::acquire(&fixture.store.scope).unwrap();
    let observed = writer.read().unwrap().unwrap();
    let external = b"external editor owns these bytes";
    fs::write(fixture.store.scope.directory.join(RECORD_NAME), external).unwrap();
    assert_eq!(
        writer.commit(Some(&observed), b"unpublished candidate"),
        Err(CommitError::Refused(PreferencesFailure::SourceChanged))
    );
    assert_eq!(writer.read().unwrap().unwrap(), external);
}

#[cfg(windows)]
#[test]
fn shared_preferences_ignore_configuration_bytes_before_first_save() {
    let fixture = Fixture::new();
    let configuration = fixture.root.join("generated.toml");
    let store = UserPreferencesStore::new(UserPreferencesScope::for_data_root(
        &crate::DataRoot::at(fixture.root.clone()),
    ));

    let without_file = store.read().unwrap();
    assert_eq!(without_file.source, "missing");
    assert_eq!(without_file.values, UserPreferencesValues::default());
    assert_eq!(
        without_file.values.application_theme,
        orchbee_workbench_protocol::ApplicationAppearance::System
    );
    assert_eq!(
        without_file.values.terminal_display_theme,
        orchbee_workbench_protocol::TerminalDisplayAppearance::Theme
    );
    assert_eq!(
        without_file.values.agent_notification_mode,
        orchbee_workbench_protocol::AgentReminderMode::Never
    );

    fs::write(&configuration, crate::default_user_settings_toml()).unwrap();
    let with_generated_file = store.read().unwrap();
    assert_eq!(with_generated_file.values, UserPreferencesValues::default());
    assert_eq!(with_generated_file.values, without_file.values);
    assert_eq!(with_generated_file.source, "missing");
    fs::write(&configuration, "invalid TOML remains independent").unwrap();
    let unchanged = store.read().unwrap();
    assert_eq!(unchanged, without_file);
    let saved = accepted(store.execute(&UserPreferencesRequest::Update {
        source: unchanged.source,
        changes: UserPreferencesPatch::set(PreferenceKey::SoundEnabled, json!(true)).unwrap(),
    }));
    assert!(saved.values.sound_enabled);
    assert_eq!(
        fs::read_to_string(&configuration).unwrap(),
        "invalid TOML remains independent"
    );
}

#[test]
fn only_explicit_reset_all_may_replace_a_refused_record_and_cannot_clobber_a_repair() {
    let fixture = Fixture::new();
    fixture.update("missing", PreferenceKey::ApplicationTheme, json!("dark"));
    let file = fixture.store.scope().directory().join("preferences.json");
    fs::write(&file, b"{refused record}").unwrap();
    let reset = UserPreferencesRequest::Update {
        source: "unreadable".into(),
        changes: UserPreferencesPatch::reset(PreferenceKey::ALL),
    };
    let partial = fixture.store.execute(&UserPreferencesRequest::Update {
        source: "unreadable".into(),
        changes: UserPreferencesPatch::reset([PreferenceKey::ApplicationTheme]),
    });
    assert!(matches!(partial, UserPreferencesResponse::Refused { .. }));
    assert_eq!(fs::read(&file).unwrap(), b"{refused record}");
    let repaired = accepted(fixture.store.execute(&reset));
    assert_eq!(repaired.values, UserPreferencesValues::default());
    assert_eq!(repaired.claimed.len(), PreferenceKey::ALL.len());
    assert_ne!(repaired.source, "unreadable");
    assert_eq!(fixture.store.read().unwrap(), repaired);
    assert_eq!(
        fixture.store.execute(&reset),
        UserPreferencesResponse::Refused {
            reason: PreferencesFailure::SourceChanged,
        }
    );
}
