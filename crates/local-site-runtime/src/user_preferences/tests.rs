use super::*;
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
            .terminal_launch_defaults(false)
            .unwrap()
            .colors
            .is_none(),
        "a fresh System + Host preference does not invent terminal colors"
    );
    let executable = std::env::current_exe()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let profiles = json!({ "version": 1, "defaultProfileId": "custom", "profiles": [
        { "id": "custom", "name": "Shared launch", "command": {
            "kind": "custom-executable", "executable": executable,
            "arguments": ["one argument", "literal && $ value", ""]
        }}
    ]});
    let snapshot = fixture.update("missing", PreferenceKey::LaunchProfiles, profiles);
    let snapshot = fixture.update(
        &snapshot.source,
        PreferenceKey::ApplicationTheme,
        json!("light"),
    );
    let before = fixture.store.terminal_launch_defaults(true).unwrap();
    assert_eq!(
        before.launch.as_ref().unwrap().arguments,
        ["one argument", "literal && $ value", ""]
    );
    assert_eq!(before.scrollback_bytes, 10_000_000);
    let other_process = UserPreferencesStore::new(fixture.store.scope().clone());
    accepted(other_process.execute(&UserPreferencesRequest::Update {
        source: snapshot.source,
        changes: UserPreferencesPatch::set(PreferenceKey::ScrollbackBytes, json!(0)).unwrap(),
    }));
    let after = fixture.store.terminal_launch_defaults(true).unwrap();
    assert_eq!(after.scrollback_bytes, 0);
    assert_eq!(before.scrollback_bytes, 10_000_000);
    assert_eq!(before.launch, after.launch);
    let explicit = fixture.store.terminal_launch_defaults(false).unwrap();
    assert!(explicit.launch.is_none());
    assert_eq!(explicit.scrollback_bytes, 0);
    let saved = accepted(fixture.store.execute(&UserPreferencesRequest::Read {}));
    let dark = accepted(fixture.store.execute(&UserPreferencesRequest::Update {
        source: saved.source,
        changes: UserPreferencesPatch::set(PreferenceKey::ApplicationTheme, json!("dark")).unwrap(),
    }));
    let next = fixture.store.terminal_launch_defaults(false).unwrap();
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
            .terminal_launch_defaults(false)
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
        fixture.store.terminal_launch_defaults(false),
        Err(PreferencesFailure::InvalidSource)
    ));
}

#[test]
fn duplicate_claims_in_a_durable_record_are_not_silently_coalesced() {
    let record = json!({ "version": 1, "values": UserPreferencesValues::default(),
        "claimed": ["locale", "locale"], "desktopImported": true, "desktopMainImported": true });
    assert!(serde_json::from_value::<Record>(record).is_err());
}

#[test]
fn main_and_renderer_legacy_imports_each_run_once_in_either_order() {
    use orchbee_workbench_protocol::ApplicationAppearance as Appearance;

    for (serialized_appearance, expected_appearance) in [
        ("light", Appearance::Light),
        ("dark", Appearance::Dark),
        ("system", Appearance::System),
    ] {
        for main_first in [false, true] {
            let fixture = Fixture::new();
            let main = UserPreferencesRequest::ImportDesktop {
                part: DesktopPreferencesPart::Main,
                changes: UserPreferencesPatch::set(PreferenceKey::ScrollbackBytes, json!(4096))
                    .unwrap(),
            };
            let renderer = UserPreferencesRequest::ImportDesktop {
                part: DesktopPreferencesPart::Renderer,
                changes: serde_json::from_value(json!({
                    "soundEnabled": true,
                    "applicationTheme": serialized_appearance
                }))
                .unwrap(),
            };
            let (first, second) = if main_first {
                (&main, &renderer)
            } else {
                (&renderer, &main)
            };
            accepted(fixture.store.execute(first));
            let complete = accepted(fixture.store.execute(second));
            assert!(complete.desktop_imported && complete.desktop_main_imported);
            assert_eq!(complete.values.scrollback_bytes, 4096);
            assert!(complete.values.sound_enabled);
            assert_eq!(complete.values.application_theme, expected_appearance);
            assert!(complete.claimed.contains(&PreferenceKey::ApplicationTheme));
            assert_eq!(accepted(fixture.store.execute(first)), complete);
            assert_eq!(accepted(fixture.store.execute(second)), complete);
        }
    }
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
        let store = UserPreferencesStore::new(
            UserPreferencesScope::for_configuration(&root.join("config.toml")).unwrap(),
        );
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
        UserPreferencesResponse::Snapshot { snapshot, .. } => snapshot,
        response => panic!("unexpected response: {response:?}"),
    }
}

#[test]
fn reads_do_not_create_storage_and_explicit_configuration_files_are_isolated() {
    let fixture = Fixture::new();
    let before = fixture.store.read().unwrap();
    assert_eq!(before.source, "missing");
    assert_eq!(before.values, UserPreferencesValues::default());
    assert!(!fixture.store.scope.directory.exists());
    fixture.update(&before.source, PreferenceKey::SoundEnabled, json!(true));
    let other = UserPreferencesStore::new(
        UserPreferencesScope::for_configuration(&fixture.root.join("other.toml")).unwrap(),
    );
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
fn delayed_import_cannot_resurrect_a_value_after_an_explicit_reset() {
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
    let imported = accepted(
        fixture
            .store
            .execute(&UserPreferencesRequest::ImportDesktop {
                part: DesktopPreferencesPart::Renderer,
                changes: serde_json::from_value(json!({
                    "soundEnabled": true,
                    "copyOnSelect": false,
                    "applicationTheme": "light"
                }))
                .unwrap(),
            }),
    );
    assert!(!imported.values.sound_enabled);
    assert!(!imported.values.copy_on_select);
    assert_eq!(
        imported.values.application_theme,
        orchbee_workbench_protocol::ApplicationAppearance::System
    );
    assert!(imported.claimed.contains(&PreferenceKey::ApplicationTheme));
    let repeat = accepted(
        fixture
            .store
            .execute(&UserPreferencesRequest::ImportDesktop {
                part: DesktopPreferencesPart::Renderer,
                changes: UserPreferencesPatch::set(PreferenceKey::ApplicationTheme, json!("dark"))
                    .unwrap(),
            }),
    );
    assert_eq!(repeat, imported);
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
        fixture
            .store
            .execute(&UserPreferencesRequest::ImportDesktop {
                part: DesktopPreferencesPart::Renderer,
                changes: UserPreferencesPatch::default()
            }),
        UserPreferencesResponse::Refused {
            reason: PreferencesFailure::InvalidSource
        }
    );
    assert_eq!(fs::read(&record).unwrap(), b"{broken");
}

#[test]
fn unclaimed_light_defaults_to_system_without_changing_the_read_token_or_bytes() {
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
        desktop_imported: false,
        desktop_main_imported: false,
    };
    let mut original = serde_json::to_vec_pretty(&record).unwrap();
    original.push(b'\n');
    let path = fixture.store.scope.directory.join(RECORD_NAME);
    fs::create_dir_all(&fixture.store.scope.directory).unwrap();
    fs::write(&path, &original).unwrap();

    let observed = fixture.store.read().unwrap();
    assert_eq!(
        observed.values.application_theme,
        orchbee_workbench_protocol::ApplicationAppearance::System
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
fn unclaimed_light_is_persisted_as_system_only_by_an_accepted_cas_update() {
    let fixture = Fixture::new();
    let values = UserPreferencesValues {
        application_theme: orchbee_workbench_protocol::ApplicationAppearance::Light,
        ..UserPreferencesValues::default()
    };
    let record = Record {
        version: 1,
        values,
        claimed: BTreeSet::new(),
        desktop_imported: false,
        desktop_main_imported: false,
    };
    let mut original = serde_json::to_vec_pretty(&record).unwrap();
    original.push(b'\n');
    let path = fixture.store.scope.directory.join(RECORD_NAME);
    fs::create_dir_all(&fixture.store.scope.directory).unwrap();
    fs::write(&path, &original).unwrap();

    let observed = fixture.store.read().unwrap();
    let committed = accepted(fixture.store.execute(&UserPreferencesRequest::Update {
        source: observed.source,
        changes: UserPreferencesPatch::set(PreferenceKey::CopyOnSelect, json!(false)).unwrap(),
    }));

    assert_eq!(
        committed.values.application_theme,
        orchbee_workbench_protocol::ApplicationAppearance::System
    );
    assert!(!committed.values.copy_on_select);
    assert_eq!(
        committed.claimed,
        BTreeSet::from([PreferenceKey::CopyOnSelect])
    );
    let stored: Record = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(
        stored.values.application_theme,
        orchbee_workbench_protocol::ApplicationAppearance::System
    );
    assert_eq!(
        stored.claimed,
        BTreeSet::from([PreferenceKey::CopyOnSelect])
    );
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
            desktop_imported: false,
            desktop_main_imported: false,
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
    let request = UserPreferencesRequest::ImportDesktop {
        part: DesktopPreferencesPart::Renderer,
        changes: UserPreferencesPatch::default(),
    };
    assert_eq!(
        fixture.store.execute(&request),
        UserPreferencesResponse::Refused {
            reason: PreferencesFailure::Busy
        }
    );
    assert!(!fixture.store.scope.directory.join(RECORD_NAME).exists());
    drop(held);
    assert!(accepted(fixture.store.execute(&request)).desktop_imported);
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
fn native_explicit_leaves_win_migration_without_rewriting_the_toml() {
    use orchbee_workbench_protocol::ApplicationAppearance as Appearance;

    for (legacy_theme, expected_appearance) in [
        ("terminal-native", Appearance::System),
        ("one-light", Appearance::Light),
        ("one-dark", Appearance::Dark),
    ] {
        let fixture = Fixture::new();
        let native = format!(
            "# keep me\n[theme]\nname='{legacy_theme}'\n[ui.toast]\nagent_delivery='off'\n[ui.sound]\nenabled=true\n"
        );
        fs::write(&fixture.store.scope.configuration, &native).unwrap();
        let read = fixture.store.read().unwrap();
        assert!(read.values.sound_enabled);
        assert_eq!(read.values.application_theme, expected_appearance);
        assert!(read.claimed.contains(&PreferenceKey::ApplicationTheme));
        let imported = accepted(
            fixture
                .store
                .execute(&UserPreferencesRequest::ImportDesktop {
                    part: DesktopPreferencesPart::Renderer,
                    changes: serde_json::from_value(json!({
                        "soundEnabled": false,
                        "applicationTheme": "light",
                        "locale": "zh-CN"
                    }))
                    .unwrap(),
                }),
        );
        assert!(imported.values.sound_enabled);
        assert_eq!(imported.values.application_theme, expected_appearance);
        assert!(imported.claimed.contains(&PreferenceKey::ApplicationTheme));
        assert_eq!(
            imported.values.locale,
            orchbee_workbench_protocol::PreferencesLocale::ZhCn
        );
        assert_eq!(
            fs::read_to_string(&fixture.store.scope.configuration).unwrap(),
            native
        );
    }
}

#[cfg(windows)]
#[test]
fn generated_windows_configuration_preserves_missing_file_shared_preferences() {
    let fixture = Fixture::new();
    let configuration = fixture.root.join("generated.toml");
    let store =
        UserPreferencesStore::new(UserPreferencesScope::for_configuration(&configuration).unwrap());

    let without_file = store.read().unwrap();
    assert_eq!(without_file.source, "missing");
    assert_eq!(without_file.values, UserPreferencesValues::default());
    assert_eq!(
        without_file.values.application_theme,
        orchbee_workbench_protocol::ApplicationAppearance::System
    );
    assert_eq!(
        without_file.values.terminal_display_theme,
        orchbee_workbench_protocol::TerminalDisplayAppearance::Host
    );
    assert_eq!(
        without_file.values.agent_notification_mode,
        orchbee_workbench_protocol::AgentReminderMode::Never
    );

    fs::write(&configuration, crate::default_windows_user_settings_toml()).unwrap();
    let with_generated_file = store.read().unwrap();
    assert_eq!(with_generated_file.values, UserPreferencesValues::default());
    assert_eq!(with_generated_file.values, without_file.values);
}

#[cfg(windows)]
#[test]
fn a_legacy_edit_before_first_save_invalidates_the_observed_source() {
    let fixture = Fixture::new();
    fs::write(
        &fixture.store.scope.configuration,
        "[ui.sound]\nenabled=false\n",
    )
    .unwrap();
    let old = fixture.store.read().unwrap();
    fs::write(
        &fixture.store.scope.configuration,
        "[ui.sound]\nenabled=true\n",
    )
    .unwrap();
    assert_eq!(
        fixture.store.execute(&UserPreferencesRequest::Update {
            source: old.source,
            changes: UserPreferencesPatch::set(PreferenceKey::CopyOnSelect, json!(false)).unwrap()
        }),
        UserPreferencesResponse::Refused {
            reason: PreferencesFailure::SourceChanged
        }
    );
    assert!(fixture.store.read().unwrap().values.sound_enabled);
    assert!(!fixture.store.scope.directory.join(RECORD_NAME).exists());
}
