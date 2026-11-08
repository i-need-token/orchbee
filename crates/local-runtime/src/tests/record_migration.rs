//! Persisted data versions (ORC-397): the layout refusal and newest version in
//! `internal/version.json`, migration through a synthetic step with its backup, retention, and the
//! failures that leave a record unchanged.
use super::Fixture;
use crate::{
    DataRoot, LayoutRefusal, PRODUCT_VERSION, VersionedRecord,
    data_root::{create_state_directory, settle_version_record},
    private_filesystem as permissions,
    record_migration::{
        KEPT_UPGRADE_FOLDERS, MigrationStep, RecordOpening, open_with, persist_migration,
        upgrade_folder,
    },
};
use serde_json::{Value, json};
use std::{cell::Cell, fs, io, io::Write, path::Path};

mod stores;

thread_local! {
    /// The steps one test gives one record on its own thread, in place of the shipped none.
    static INJECTED: Cell<Option<(VersionedRecord, &'static [MigrationStep])>> =
        const { Cell::new(None) };
}

/// The steps a test injected for `record` on this thread, if any.
pub(crate) fn injected_steps(record: VersionedRecord) -> Option<&'static [MigrationStep]> {
    INJECTED
        .get()
        .filter(|(injected, _)| *injected == record)
        .map(|(_, steps)| steps)
}

/// Gives `record` `steps` on this thread until the returned guard drops.
fn inject(record: VersionedRecord, steps: &'static [MigrationStep]) -> Injected {
    INJECTED.set(Some((record, steps)));
    Injected
}

struct Injected;
impl Drop for Injected {
    fn drop(&mut self) {
        INJECTED.set(None);
    }
}

fn write_private(path: &Path, bytes: &[u8]) {
    let _pins = create_state_directory(path.parent().unwrap()).unwrap();
    let _ = fs::remove_file(path);
    let mut file = permissions::open_file(path, true, true).unwrap();
    file.write_all(bytes).unwrap();
}

fn folders(path: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

// The proposal's first rule: a build refuses a root whose layout is higher than its own, before it
// reads or creates anything else, and never rewrites the record that refused it.
#[test]
fn a_higher_or_unreadable_layout_is_refused_and_left_unchanged() {
    let fixture = Fixture::new();
    let root = fixture.root();
    assert_eq!(root.admit_layout(), Ok(()), "a root without the record");

    let newer = br#"{"layout":2,"newestVersion":"9.0.0","moved":["state"]}"#;
    write_private(&root.version_file(), newer);
    assert_eq!(
        root.admit_layout(),
        Err(LayoutRefusal {
            layout: Some(2),
            newest_version: Some("9.0.0".into())
        })
    );
    assert!(settle_version_record(&root).is_err());
    assert_eq!(fs::read(root.version_file()).unwrap(), newer);
    assert_eq!(folders(&root.internal()), ["version.json"]);

    for unreadable in [
        &b"broken"[..],
        br#"{"layout":"1"}"#,
        br#"{"newestVersion":"1.0.0"}"#,
    ] {
        write_private(&root.version_file(), unreadable);
        assert_eq!(
            root.admit_layout(),
            Err(LayoutRefusal {
                layout: None,
                newest_version: None
            })
        );
    }
}

// `newestVersion` names the newest Orchbee that used the root, so it only rises, and entries a
// later release added survive the rewrite.
#[test]
fn the_newest_version_only_rises_and_keeps_later_entries() {
    let fixture = Fixture::new();
    let older = DataRoot::at(fixture.0.join("older"));
    write_private(
        &older.version_file(),
        br#"{"layout":1,"newestVersion":"0.0.1-alpha","added":{"kept":true}}"#,
    );
    settle_version_record(&older).unwrap();
    let record: Value = serde_json::from_slice(&fs::read(older.version_file()).unwrap()).unwrap();
    assert_eq!(
        record,
        json!({"layout":1,"newestVersion":PRODUCT_VERSION,"added":{"kept":true}})
    );
    assert_eq!(older.admit_layout(), Ok(()));

    let newer = DataRoot::at(fixture.0.join("newer"));
    let bytes = br#"{"layout":1,"newestVersion":"999.0.0"}"#;
    write_private(&newer.version_file(), bytes);
    settle_version_record(&newer).unwrap();
    assert_eq!(fs::read(newer.version_file()).unwrap(), bytes);
}

/// Version 1 to 2 of a synthetic record: `name` becomes `title`.
fn rename_step(mut value: Value) -> Option<Value> {
    let object = value.as_object_mut()?;
    let name = object.remove("name")?;
    object.insert("title".into(), name);
    object.insert("version".into(), 2.into());
    Some(value)
}

fn admits_title(bytes: &[u8]) -> bool {
    serde_json::from_slice::<Value>(bytes).is_ok_and(|value| {
        value.get("version") == Some(&json!(2))
            && value.get("title").is_some_and(Value::is_string)
            && value.as_object().is_some_and(|object| object.len() == 2)
    })
}

const STEPS: &[MigrationStep] = &[MigrationStep {
    from: 1,
    apply: rename_step,
}];

// The opening rules: an earlier record migrates only when every step applies and the result passes
// the current strict parser; a newer one is never touched; a record without an integer version is
// the store's ordinary invalid record.
#[test]
fn a_synthetic_step_migrates_only_into_a_record_the_parser_admits() {
    let original = br#"{"version":1,"name":"work"}"#;
    match open_with(original, 2, STEPS, admits_title) {
        RecordOpening::Migrated { from: 1, bytes } => {
            let value: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value, json!({"version":2,"title":"work"}));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        open_with(br#"{"version":2,"title":"work"}"#, 2, STEPS, admits_title),
        RecordOpening::Current
    );
    assert_eq!(
        open_with(br#"{"version":3,"other":0}"#, 2, STEPS, admits_title),
        RecordOpening::Newer { version: 3 }
    );
    for failing in [
        // The step finds no `name`.
        &br#"{"version":1,"title":"work"}"#[..],
        // The step applies, but the result keeps a field the current parser refuses.
        br#"{"version":1,"name":"work","extra":0}"#,
        // No step leads from version 0.
        br#"{"version":0,"name":"work"}"#,
    ] {
        let version = serde_json::from_slice::<Value>(failing).unwrap()["version"]
            .as_u64()
            .unwrap();
        assert_eq!(
            open_with(failing, 2, STEPS, admits_title),
            RecordOpening::NotUpdated { version }
        );
    }
    // A step that does not advance the version is a failed step.
    let stalled: &[MigrationStep] = &[MigrationStep {
        from: 1,
        apply: Some,
    }];
    assert_eq!(
        open_with(original, 2, stalled, |_| true),
        RecordOpening::NotUpdated { version: 1 }
    );
    for unversioned in [
        &b"broken"[..],
        br#"{"title":"work"}"#,
        br#"{"version":"2"}"#,
    ] {
        assert_eq!(
            open_with(unversioned, 2, STEPS, admits_title),
            RecordOpening::Unversioned
        );
    }
}

// A persisted migration copies the original into the upgrade folder at its relative path before
// the store replaces it, and a failed replacement leaves the original where it was.
#[test]
fn a_migration_backs_up_the_original_before_replacing_it() {
    let fixture = Fixture::new();
    let root = fixture.root();
    let record = root.state().join("synthetic.json");
    let original = br#"{"version":1,"name":"work"}"#;
    write_private(&record, original);
    let RecordOpening::Migrated { bytes, .. } = open_with(original, 2, STEPS, admits_title) else {
        panic!("the synthetic step applies");
    };

    let failed = persist_migration(
        &root,
        &record,
        original,
        || Err(io::ErrorKind::Other.into()),
    );
    assert!(failed.is_err());
    assert_eq!(fs::read(&record).unwrap(), original);

    let copy = persist_migration(&root, &record, original, || fs::write(&record, &bytes)).unwrap();
    assert_eq!(fs::read(&record).unwrap(), bytes);
    assert_eq!(fs::read(&copy).unwrap(), original);
    let upgrade = folders(&root.backups());
    assert_eq!(upgrade.len(), 1, "one product version shares one folder");
    assert!(upgrade[0].ends_with(&format!("-upgrade-{PRODUCT_VERSION}")));
    assert_eq!(
        copy,
        root.backups()
            .join(&upgrade[0])
            .join("internal")
            .join("state")
            .join("synthetic.json")
    );
}

// Only the three newest upgrade folders stay, a version reuses its folder whatever the day, and a
// reset folder is never deleted automatically.
#[test]
fn upgrade_folders_keep_the_newest_three_and_reset_folders_stay() {
    let fixture = Fixture::new();
    let root = fixture.root();
    let backups = root.backups();
    let record = root.state().join("synthetic.json");
    let reset = crate::back_up_for_reset(&root, "reset-synthetic", &record, b"first").unwrap();
    let second = crate::back_up_for_reset(&root, "reset-synthetic", &record, b"second").unwrap();
    assert_ne!(reset.parent(), second.parent());
    assert!(crate::back_up_for_reset(&root, "upgrade-0.1.0", &record, b"x").is_err());

    for (date, version) in [
        ("2026-01-01", "0.9.0"),
        ("2026-02-01", "0.10.0"),
        ("2026-02-01", "0.10.1"),
        ("2026-03-01", "1.0.0"),
    ] {
        upgrade_folder(&backups, date, version).unwrap();
    }
    let reused = upgrade_folder(&backups, "2026-04-01", "0.10.1").unwrap();
    assert!(reused.ends_with("2026-02-01-upgrade-0.10.1"));
    let kept: Vec<String> = folders(&backups)
        .into_iter()
        .filter(|name| name.contains("-upgrade-"))
        .collect();
    assert_eq!(kept.len(), KEPT_UPGRADE_FOLDERS);
    assert_eq!(
        kept,
        [
            "2026-02-01-upgrade-0.10.0",
            "2026-02-01-upgrade-0.10.1",
            "2026-03-01-upgrade-1.0.0"
        ]
    );
    assert_eq!(fs::read(&reset).unwrap(), b"first");
    assert_eq!(fs::read(&second).unwrap(), b"second");
}
