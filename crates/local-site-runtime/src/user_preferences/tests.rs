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
    for main_first in [false, true] {
        let fixture = Fixture::new();
        let main = UserPreferencesRequest::ImportDesktop {
            part: DesktopPreferencesPart::Main,
            changes: UserPreferencesPatch::set(PreferenceKey::ScrollbackBytes, json!(4096))
                .unwrap(),
        };
        let renderer = UserPreferencesRequest::ImportDesktop {
            part: DesktopPreferencesPart::Renderer,
            changes: UserPreferencesPatch::set(PreferenceKey::SoundEnabled, json!(true)).unwrap(),
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
        assert_eq!(accepted(fixture.store.execute(first)), complete);
        assert_eq!(accepted(fixture.store.execute(second)), complete);
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
        let root = std::env::temp_dir().join(format!(
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
        changes: UserPreferencesPatch::reset([PreferenceKey::SoundEnabled]),
    }));
    assert!(!reset.values.sound_enabled);
    assert!(reset.claimed.contains(&PreferenceKey::SoundEnabled));
    let imported = accepted(
        fixture
            .store
            .execute(&UserPreferencesRequest::ImportDesktop {
                part: DesktopPreferencesPart::Renderer,
                changes: serde_json::from_value(
                    json!({"soundEnabled": true, "copyOnSelect": false}),
                )
                .unwrap(),
            }),
    );
    assert!(!imported.values.sound_enabled);
    assert!(!imported.values.copy_on_select);
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
    let fixture = Fixture::new();
    let native = "# keep me\n[theme]\nname='one-dark'\n[ui.toast]\nagent_delivery='off'\n[ui.sound]\nenabled=true\n";
    fs::write(&fixture.store.scope.configuration, native).unwrap();
    let read = fixture.store.read().unwrap();
    assert!(read.values.sound_enabled);
    let imported = accepted(
        fixture
            .store
            .execute(&UserPreferencesRequest::ImportDesktop {
                part: DesktopPreferencesPart::Renderer,
                changes: serde_json::from_value(
                    json!({"soundEnabled": false, "applicationTheme": "light", "locale": "zh-CN"}),
                )
                .unwrap(),
            }),
    );
    assert!(imported.values.sound_enabled);
    assert_eq!(
        imported.values.application_theme,
        orchbee_workbench_protocol::ApplicationAppearance::Dark
    );
    assert_eq!(
        imported.values.locale,
        orchbee_workbench_protocol::PreferencesLocale::ZhCn
    );
    assert_eq!(
        fs::read_to_string(&fixture.store.scope.configuration).unwrap(),
        native
    );
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
