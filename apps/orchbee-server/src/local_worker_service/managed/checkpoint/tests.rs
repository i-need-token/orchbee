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

/// The shared temporary folder. macOS reaches its per-user one through a symlinked path, so the
/// fixture uses the canonical `/private/tmp`, as the local runtime's own tests do.
fn temporary() -> PathBuf {
    if cfg!(target_os = "macos") {
        PathBuf::from("/private/tmp")
    } else {
        std::env::temp_dir()
    }
}

struct Fixture(Option<ModeCheckedLocalSessionRendezvous>);

impl Fixture {
    fn new() -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let base = temporary().join(format!(
            "checkpoint-load-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        // Create the fixture folder first so runtime admission judges a folder the fixture owns
        // rather than `%TEMP%`, where sandboxes may add non-inheritable traverse entries.
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        }
        #[cfg(windows)]
        fs::create_dir(&base).unwrap();
        let scope = SessionScope::parse("base").unwrap();
        Self(Some(
            ModeCheckedLocalSessionRendezvous::prepare_mode_checked(&base, scope).unwrap(),
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
        let path = self.local().parent().unwrap().to_owned();
        drop(self.0.take());
        let path = path.canonicalize().unwrap();
        assert_eq!(
            path.parent(),
            Some(temporary().canonicalize().unwrap().as_path())
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

/// A checkpoint of an earlier format is upgraded before anything reads it: the slot as it stood is
/// first copied into this version's upgrade folder, where the held version finds it after a
/// fallback, then the upgrade is published, and the next start reads that unchanged.
#[test]
fn an_earlier_checkpoint_is_backed_up_before_its_upgrade_is_published() {
    let fixture = Fixture::new();
    let session = fixture.start();
    let slot = fixture.state(&session).join("checkpoint.json");
    let original = fs::read(samples().join("checkpoint-v1.bin")).unwrap();
    write_private(&slot, &original);
    let (store, upgraded, _record, _) = load_from(&fixture.root(), &session).unwrap();
    drop(store);
    let upgraded = upgraded.unwrap();
    assert!(upgraded.upgrade(&[7; 16]).unwrap().is_none());
    let folders: Vec<_> = fs::read_dir(fixture.root().backups())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    let [folder] = folders.as_slice() else {
        panic!("one upgrade folder: {folders:?}")
    };
    let relative = slot.strip_prefix(fixture.local()).unwrap();
    assert_eq!(fs::read(folder.join(relative)).unwrap(), original);

    let (_store, again, _record, _) = load_from(&fixture.root(), &session).unwrap();
    let worker = WorkerId::parse(&format!("{:032x}", 400)).unwrap();
    let navigation = |checkpoint: WorkbenchCheckpoint| {
        checkpoint
            .reconstruct(SessionId::parse(&format!("{:032x}", 300)).unwrap(), &worker)
            .unwrap()
            .snapshot()
            .navigation
            .clone()
    };
    assert_eq!(navigation(again.unwrap()), navigation(upgraded));
}

fn samples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/local_worker_service/managed/checkpoint/tests/record-samples")
}

/// Places a stored file as the session store leaves it: readable by its owner alone, which the
/// loader requires on POSIX.
fn write_private(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
}

/// Records follow the window: `tests/record-samples/` keeps, for every format a release shipped, a
/// stored checkpoint slot `checkpoint-v<version>.bin` and the kept terminal output it names,
/// `terminal-output-v<version>.bin`, as the local runtime's record samples describe. Each slot
/// restores through the loader without a recovery notice, and the restored Presentation replays
/// the kept output.
#[test]
fn every_stored_sample_restores_its_checkpoint_and_kept_output() {
    let names: Vec<String> = fs::read_dir(samples())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    let mut outputs: std::collections::BTreeSet<&String> = names
        .iter()
        .filter(|name| name.starts_with("terminal-output-"))
        .collect();
    let mut restored = 0;
    for name in &names {
        let Some(version) = name
            .strip_prefix("checkpoint-")
            .and_then(|rest| rest.strip_suffix(".bin"))
        else {
            assert!(
                name.starts_with("terminal-output-"),
                "unknown sample {name}"
            );
            continue;
        };
        let fixture = Fixture::new();
        let session = fixture.start();
        let state = fixture.state(&session);
        write_private(
            &state.join("checkpoint.json"),
            &fs::read(samples().join(name)).unwrap(),
        );
        let (store, checkpoint, _record, _) = load_from(&fixture.root(), &session).unwrap();
        let checkpoint = checkpoint.unwrap_or_else(|| panic!("{name} restores"));
        assert_eq!(
            session.checkpoint_restore_notice(&fixture.root()).unwrap(),
            None,
            "{name}"
        );
        let worker = WorkerId::parse(&format!("{:032x}", 400)).unwrap();
        let authority = checkpoint
            .reconstruct(SessionId::parse(&format!("{:032x}", 300)).unwrap(), &worker)
            .unwrap();
        // The kept output sample is the file this slot sample names, framed under that number.
        let output_name = format!("terminal-output-{version}.bin");
        fs::create_dir(state.join("terminal-output")).unwrap();
        let output = crate::terminal_output::TerminalOutput::new(std::sync::Arc::new(
            std::sync::Mutex::new(store),
        ));
        let mut replayed = 0;
        for target in authority.restoration_targets(&worker).unwrap() {
            if let Some(token) = target.restored_output() {
                write_private(
                    &state.join("terminal-output").join(format!("{token}.bin")),
                    &fs::read(samples().join(&output_name)).unwrap(),
                );
                assert!(
                    output.restored(target.presentation_id(), token).is_some(),
                    "{output_name}"
                );
                replayed += 1;
            }
        }
        assert_eq!(replayed, 1, "{name} names its kept output");
        outputs.remove(&output_name);
        restored += 1;
    }
    assert!(restored > 0, "a checkpoint sample");
    let current = serde_json::from_slice::<Value>(&valid_checkpoint()).unwrap()["version"].clone();
    assert!(
        names.contains(&format!("checkpoint-v{current}.bin")),
        "a sample of the checkpoint version {current} the writer writes"
    );
    assert!(
        outputs.is_empty(),
        "kept output samples without a checkpoint: {outputs:?}"
    );
}
