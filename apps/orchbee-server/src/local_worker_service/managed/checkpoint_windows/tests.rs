use super::*;
use orchbee_local_runtime::{
    CheckpointRestoreNotice, LockAttempt, SessionCatalog, SessionName, SessionScope,
};
use orchbee_server::WorkbenchAuthority;
use orchbee_workbench_protocol::{SessionId, WorkerId};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

struct Fixture(Option<ModeCheckedLocalSessionRendezvous>);

impl Fixture {
    fn new() -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let scope = SessionScope::parse(&format!(
            "checkpoint-load-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ))
        .unwrap();
        Self(Some(
            ModeCheckedLocalSessionRendezvous::prepare_mode_checked(&std::env::temp_dir(), scope)
                .unwrap(),
        ))
    }

    fn local(&self) -> &Path {
        self.0.as_ref().unwrap().runtime_directory()
    }

    /// The runtime folder doubles as the data root, so every file stays inside the fixture.
    fn root(&self) -> DataRoot {
        DataRoot::at(self.local().to_owned())
    }

    fn start(&self) -> ModeCheckedLocalSessionRendezvous {
        let name = SessionName::parse("migration").unwrap();
        let catalog = SessionCatalog::try_lock(self.local(), &self.root()).unwrap();
        catalog.prepare_activation(&name).unwrap();
        let session = ModeCheckedLocalSessionRendezvous::prepare_mode_checked(
            self.local(),
            SessionScope::parse(name.as_str()).unwrap(),
        )
        .unwrap();
        let LockAttempt::Acquired(_activation) = session.try_acquire_activation_lock().unwrap()
        else {
            panic!("activation owner")
        };
        catalog.begin_activation(&session, [1; 16]).unwrap();
        session.commit_activation().unwrap();
        session
    }

    fn state(&self, session: &ModeCheckedLocalSessionRendezvous) -> PathBuf {
        self.local()
            .join("sessions")
            .join(session.runtime_directory().file_name().unwrap())
    }

    fn catalog(&self, session: &ModeCheckedLocalSessionRendezvous) -> PathBuf {
        self.state(session).join("session.json")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let path = self.local().to_owned();
        drop(self.0.take());
        let path = path.canonicalize().unwrap();
        assert_eq!(
            path.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(&format!("checkpoint-load-{}-", std::process::id()))
        );
        fs::remove_dir_all(path).unwrap();
    }
}

fn retained_files(path: &Path) -> BTreeMap<std::ffi::OsString, Vec<u8>> {
    fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), fs::read(entry.path()).unwrap())
        })
        .collect()
}

fn valid_checkpoint() -> Vec<u8> {
    WorkbenchAuthority::new(SessionId::parse(&format!("{:032x}", 300)).unwrap())
        .unwrap()
        .checkpoint(&WorkerId::parse(&format!("{:032x}", 400)).unwrap())
        .unwrap()
}

#[test]
fn incompatible_checkpoint_loads_a_fallback_or_visible_empty_without_rewriting_copies() {
    for healthy in [false, true] {
        let fixture = Fixture::new();
        let session = fixture.start();
        let mut store = SessionStore::open(
            &fixture.root(),
            &session,
            MAXIMUM_WORKBENCH_CHECKPOINT_BYTES,
        )
        .unwrap();
        if healthy {
            store.publish(&valid_checkpoint()).unwrap();
        }
        let mut unsupported: Value = serde_json::from_slice(&valid_checkpoint()).unwrap();
        unsupported["version"] = json!(7);
        store
            .publish(&serde_json::to_vec(&unsupported).unwrap())
            .unwrap();
        drop(store);
        let before = retained_files(&fixture.state(&session));
        let (store, checkpoint, _record, suppressed) =
            load_from(&fixture.root(), &session).unwrap();
        assert_eq!(checkpoint.is_some(), healthy);
        assert_eq!(suppressed, !healthy);
        assert_eq!(
            session.checkpoint_restore_notice(&fixture.root()).unwrap(),
            Some(if healthy {
                CheckpointRestoreNotice::Recovered
            } else {
                CheckpointRestoreNotice::Empty
            })
        );
        drop(store);
        for (name, bytes) in before {
            assert_eq!(fs::read(fixture.state(&session).join(name)).unwrap(), bytes);
        }
    }
}

#[test]
fn unsupported_catalog_is_rejected_before_pending_store_cleanup() {
    let fixture = Fixture::new();
    let session = fixture.start();
    let mut store = SessionStore::open(
        &fixture.root(),
        &session,
        MAXIMUM_WORKBENCH_CHECKPOINT_BYTES,
    )
    .unwrap();
    store.publish(&valid_checkpoint()).unwrap();
    drop(store);
    let state = fixture.state(&session);
    fs::write(state.join("checkpoint.pending"), b"retained pending").unwrap();
    fs::write(
        state.join("checkpoint.restore-notice.pending"),
        b"retained notice pending",
    )
    .unwrap();
    let catalog_path = fixture.catalog(&session);
    let mut record: Value = serde_json::from_slice(&fs::read(&catalog_path).unwrap()).unwrap();
    record["version"] = json!(6);
    let record = serde_json::to_vec(&record).unwrap();
    fs::write(&catalog_path, &record).unwrap();
    let before = retained_files(&state);
    assert!(load_from(&fixture.root(), &session).is_err());
    assert_eq!(retained_files(&state), before);
    assert_eq!(fs::read(catalog_path).unwrap(), record);
}
