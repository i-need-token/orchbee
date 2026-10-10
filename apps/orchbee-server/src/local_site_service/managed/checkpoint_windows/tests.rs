use super::*;
use orchbee_local_site_runtime::{
    LockAttempt, SiteScope, WindowsCheckpointRestoreNotice, WindowsNamedSiteCatalog,
    WindowsSiteName,
};
use orchbee_server::WorkbenchAuthority;
use orchbee_workbench_protocol::{SiteId, WorkerId};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

struct Fixture(Option<ModeCheckedLocalSiteRendezvous>);

impl Fixture {
    fn new() -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let scope = SiteScope::parse(&format!(
            "checkpoint-load-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ))
        .unwrap();
        Self(Some(
            ModeCheckedLocalSiteRendezvous::prepare_mode_checked(&std::env::temp_dir(), scope)
                .unwrap(),
        ))
    }

    fn local(&self) -> &Path {
        self.0.as_ref().unwrap().runtime_directory()
    }

    fn start(&self) -> ModeCheckedLocalSiteRendezvous {
        let name = WindowsSiteName::parse("migration").unwrap();
        let scope = name.scope_for_user(&orchbee_local_ipc::current_user_sid().unwrap());
        let catalog = WindowsNamedSiteCatalog::try_lock(self.local(), self.local()).unwrap();
        catalog.prepare_activation(&scope, Some(&name)).unwrap();
        let site = ModeCheckedLocalSiteRendezvous::prepare_mode_checked(
            self.local(),
            SiteScope::parse(&scope).unwrap(),
        )
        .unwrap();
        let LockAttempt::Acquired(activation) = site.try_acquire_activation_lock().unwrap() else {
            panic!("activation owner")
        };
        site.register_session_name(&activation, &name).unwrap();
        catalog.begin_activation(&site, [1; 16]).unwrap();
        site.commit_activation().unwrap();
        site
    }

    fn state(&self, site: &ModeCheckedLocalSiteRendezvous) -> PathBuf {
        self.local().join("Orchbee/native-sessions").join(
            ModeCheckedLocalSiteRendezvous::host_endpoint_id(site.runtime_directory()),
        )
    }

    fn catalog(&self, site: &ModeCheckedLocalSiteRendezvous) -> PathBuf {
        self.local()
            .join("Orchbee/session-catalog")
            .join(ModeCheckedLocalSiteRendezvous::host_endpoint_id(
                self.local(),
            ))
            .join(format!(
                "{}.json",
                site.runtime_directory()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
            ))
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
    WorkbenchAuthority::new(SiteId::parse(&format!("{:032x}", 300)).unwrap())
        .unwrap()
        .checkpoint(&WorkerId::parse(&format!("{:032x}", 400)).unwrap())
        .unwrap()
}

fn historical_at_capacity() -> Vec<u8> {
    let id = |number: usize| format!("{number:032x}");
    serde_json::to_vec(&json!({
        "schema":"orchbee.workbench.checkpoint", "version":5, "lastIdentity":id(260),
        "navigation": {"standaloneTerminalContextId":id(1), "projects":[{
            "projectId":id(2), "label":"P",
            "directories":(0..256).map(|index| json!({"directoryId":id(index+3),
                "path":format!("C:/{}", "a".repeat(if index < 122 {3984} else {3983}))})).collect::<Vec<_>>(),
            "workspaces":[{"workspaceId":id(259), "label":"W", "primaryDirectoryId":id(3), "terminalContextId":id(260)}]
        }]},
        "terminalContexts":([1,260].map(|number| json!({"terminalContextId":id(number), "groups":[], "tabs":[]}))),
        "profiles":[], "agentSessions":[], "terminalDirectories":[], "managedNames":[]
    })).unwrap()
}

#[test]
fn historical_capacity_refuses_server_load_without_fallback_notice_or_rewrite() {
    let historical = historical_at_capacity();
    let valid = valid_checkpoint();
    for (plain, payloads) in [
        (true, vec![&historical]),
        (false, vec![&historical]),
        (false, vec![&valid, &historical]),
        (false, vec![&historical, &valid]),
    ] {
        let fixture = Fixture::new();
        let site = fixture.start();
        let mut store =
            WindowsSessionStore::open(fixture.local(), &site, MAXIMUM_WORKBENCH_CHECKPOINT_BYTES)
                .unwrap();
        let has_fallback = payloads.len() > 1;
        if plain {
            fs::write(fixture.state(&site).join("checkpoint.json"), &historical).unwrap();
        } else {
            for payload in payloads {
                store.publish(payload).unwrap();
            }
        }
        // Negative control: the former load callback loses this typed refusal and selects
        // another checkpoint or Empty. The real Server load below must refuse instead.
        let prior_behavior = store
            .read_validated(|bytes| WorkbenchCheckpoint::parse(bytes).ok())
            .unwrap();
        assert_eq!(prior_behavior.checkpoint.is_some(), has_fallback);
        assert_eq!(
            prior_behavior.notice,
            Some(if has_fallback {
                WindowsCheckpointRestoreNotice::Recovered
            } else {
                WindowsCheckpointRestoreNotice::Empty
            })
        );
        drop(store);
        let state = fixture.state(&site);
        let before = retained_files(&state);
        let catalog = fs::read(fixture.catalog(&site)).unwrap();
        let marker = fs::read(site.runtime_directory().join("activation.started")).unwrap();
        let error = load_from(fixture.local(), &site)
            .err()
            .expect("migration limit must stop Server startup");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(matches!(
            error
                .get_ref()
                .and_then(|cause| cause.downcast_ref::<WorkbenchCheckpointError>()),
            Some(WorkbenchCheckpointError::HistoricalMigrationTooLarge {
                version: 5,
                actual: 1_048_677,
                maximum: 1_048_576
            })
        ));
        assert_eq!(retained_files(&state), before);
        assert_eq!(fs::read(fixture.catalog(&site)).unwrap(), catalog);
        assert_eq!(
            fs::read(site.runtime_directory().join("activation.started")).unwrap(),
            marker
        );
        assert!(!state.join("checkpoint.restore-notice").exists());
    }
}

#[test]
fn ordinary_corruption_still_loads_a_fallback_or_visible_empty_without_rewriting_copies() {
    for healthy in [false, true] {
        let fixture = Fixture::new();
        let site = fixture.start();
        let mut store =
            WindowsSessionStore::open(fixture.local(), &site, MAXIMUM_WORKBENCH_CHECKPOINT_BYTES)
                .unwrap();
        if healthy {
            store.publish(&valid_checkpoint()).unwrap();
        }
        store.publish(b"{}").unwrap();
        drop(store);
        let before = retained_files(&fixture.state(&site));
        let (store, checkpoint, _record, suppressed) = load_from(fixture.local(), &site).unwrap();
        assert_eq!(checkpoint.is_some(), healthy);
        assert_eq!(suppressed, !healthy);
        assert_eq!(
            site.checkpoint_restore_notice(fixture.local()).unwrap(),
            Some(if healthy {
                WindowsCheckpointRestoreNotice::Recovered
            } else {
                WindowsCheckpointRestoreNotice::Empty
            })
        );
        drop(store);
        for (name, bytes) in before {
            assert_eq!(fs::read(fixture.state(&site).join(name)).unwrap(), bytes);
        }
    }
}

#[test]
fn unsupported_catalog_is_rejected_before_pending_store_cleanup() {
    let fixture = Fixture::new();
    let site = fixture.start();
    let mut store =
        WindowsSessionStore::open(fixture.local(), &site, MAXIMUM_WORKBENCH_CHECKPOINT_BYTES)
            .unwrap();
    store.publish(&valid_checkpoint()).unwrap();
    drop(store);
    let state = fixture.state(&site);
    fs::write(state.join("checkpoint.pending"), b"retained pending").unwrap();
    fs::write(
        state.join("checkpoint.restore-notice.pending"),
        b"retained notice pending",
    )
    .unwrap();
    let catalog_path = fixture.catalog(&site);
    let mut record: Value = serde_json::from_slice(&fs::read(&catalog_path).unwrap()).unwrap();
    record["version"] = json!(6);
    let record = serde_json::to_vec(&record).unwrap();
    fs::write(&catalog_path, &record).unwrap();
    let before = retained_files(&state);
    assert!(load_from(fixture.local(), &site).is_err());
    assert_eq!(retained_files(&state), before);
    assert_eq!(fs::read(catalog_path).unwrap(), record);
}
