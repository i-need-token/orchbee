//! Records follow the window: a stored sample of every format a release shipped keeps opening
//! through today's opener. `record-samples/` holds one sample per released format of each record
//! this crate opens, named `<record>-v<version>`, so a format change adds a step and a sample and
//! removes neither; the README's persisted data versions say when a sample is replaced instead.
//! Every sample of one Session shares one activation.
use super::super::{Fixture, write_private};
use crate::{VersionedRecord, data_root::create_state_directory, durable_sessions::Directory};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

const SCOPE: &str = "work";

fn samples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tests/record_migration/record-samples")
}

/// The `version` a JSON sample carries.
fn version(bytes: &[u8]) -> u64 {
    serde_json::from_slice::<Value>(bytes).unwrap()["version"]
        .as_u64()
        .unwrap()
}

/// The suite that keeps each versioned record's samples, beside the store that opens it.
const fn samples_owner(record: VersionedRecord) -> &'static str {
    match record {
        VersionedRecord::PluginRegistry | VersionedRecord::SessionLifecycle => "local-runtime",
        VersionedRecord::Machines | VersionedRecord::TuiPosition => "workbench-client",
    }
}

/// Stores a Session record sample as the Session record of `fixture`'s scope and opens it. A
/// Session record names the runtime folder of the machine that wrote it, so the sample is pointed
/// at the fixture's, whose endpoints the Session uses here.
fn open_session(fixture: &Fixture, bytes: &[u8]) -> bool {
    let mut record: Value = serde_json::from_slice(bytes).unwrap();
    assert!(record["runtime"].is_string());
    record["runtime"] = fixture
        .open(SCOPE)
        .runtime_directory()
        .to_str()
        .unwrap()
        .into();
    let directory = Directory::create(&fixture.root()).unwrap();
    let _pins = create_state_directory(&directory.folder(SCOPE)).unwrap();
    write_private(
        directory.record_file(SCOPE).unwrap(),
        serde_json::to_vec(&record).unwrap(),
    )
    .unwrap();
    directory.read(SCOPE).unwrap().is_some()
}

/// The newest Session record sample, which the restore notice samples belong to.
fn current_session() -> Vec<u8> {
    let current = VersionedRecord::SessionLifecycle.current_version();
    fs::read(samples().join(format!("session-v{current}.json"))).unwrap()
}

// Every stored sample opens to a usable record through the opener its process uses, and each
// versioned record this crate opens has a sample of its current version.
#[test]
fn every_stored_sample_opens_through_todays_opener() {
    let mut versioned = Vec::new();
    for entry in fs::read_dir(samples()).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        let bytes = fs::read(samples().join(&name)).unwrap();
        let fixture = Fixture::new();
        let root = fixture.root();
        let opened = match name.rsplit_once("-v").map(|(record, _)| record) {
            Some("session") => {
                versioned.push((VersionedRecord::SessionLifecycle, version(&bytes)));
                open_session(&fixture, &bytes)
            }
            Some("restore-notice") => {
                assert!(open_session(&fixture, &current_session()));
                let directory = Directory::create(&root).unwrap();
                write_private(
                    directory
                        .folder(SCOPE)
                        .join(crate::checkpoint_restore_notice::FILE),
                    &bytes,
                )
                .unwrap();
                fixture
                    .open(SCOPE)
                    .checkpoint_restore_notice(&root)
                    .unwrap()
                    .is_some()
            }
            Some("plugin-registry") => {
                versioned.push((VersionedRecord::PluginRegistry, version(&bytes)));
                open_plugin_registry(&root, &bytes)
            }
            _ => panic!("unknown sample {name}"),
        };
        assert!(opened, "{name} opens");
    }
    for record in VersionedRecord::ALL {
        if samples_owner(record) == "local-runtime" {
            assert!(
                versioned.contains(&(record, record.current_version())),
                "{record:?} has no sample of its current version"
            );
        }
    }
}

#[cfg(windows)]
fn open_plugin_registry(root: &crate::DataRoot, bytes: &[u8]) -> bool {
    let registry = crate::PluginRegistry::in_data_root(root).unwrap();
    let _pins = create_state_directory(registry.root()).unwrap();
    write_private(
        registry
            .root()
            .join(VersionedRecord::PluginRegistry.file_name()),
        bytes,
    )
    .unwrap();
    !registry.list(None).unwrap().plugins.is_empty()
}

/// The registry is not ported to macOS and Linux, where it refuses as unsupported.
#[cfg(not(windows))]
fn open_plugin_registry(_: &crate::DataRoot, _: &[u8]) -> bool {
    true
}
