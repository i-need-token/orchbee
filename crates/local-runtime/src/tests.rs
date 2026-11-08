use crate::{
    Cohort, LockAttempt, ModeCheckedLocalSessionRendezvous as Rendezvous, SessionScope,
    private_filesystem as permissions,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

mod child_registration;
mod crash_recovery;
mod durable_sessions;
mod named_catalog;
mod session_name;
mod session_store;

#[test]
fn handoff_pipe_addresses_are_ticket_and_scope_specific_with_bounded_unicode_inputs() {
    use orchbee_local_ipc::LocalListener;
    let root = std::env::temp_dir().join(format!("orchbee-handoff-address-{}", std::process::id()));
    let first = Rendezvous::handoff_endpoint_path(&root.join("first"), [1; 16]);
    let second = Rendezvous::handoff_endpoint_path(&root.join("second"), [1; 16]);
    let next = Rendezvous::handoff_endpoint_path(&root.join("first"), [2; 16]);
    assert_ne!(first, second);
    assert_ne!(first, next);
    assert_eq!(
        first,
        Rendezvous::handoff_endpoint_path(&root.join("first"), [1; 16])
    );
    let long = Rendezvous::handoff_endpoint_path(&root.join("目录".repeat(2048)), [3; 16]);
    assert!(long.to_str().unwrap().len() < 120);
    let listeners = [first, second, next, long].map(|path| LocalListener::bind(path).unwrap());
    drop(listeners);
    assert!(
        !root.exists(),
        "pipe derivation never creates or adopts a filesystem root"
    );
}

#[test]
fn native_admission_rejects_hard_links_junctions_and_non_normalized_paths() {
    use std::os::windows::process::CommandExt;
    let fixture = Fixture::new();
    let original = fixture.0.join("original");
    let alias = fixture.0.join("alias");
    fs::write(&original, b"preserved").unwrap();
    fs::hard_link(&original, &alias).unwrap();
    assert!(permissions::open_file(&alias, false, true).is_err());
    assert_eq!(fs::read(&original).unwrap(), b"preserved");
    assert!(permissions::open_tree(&fixture.0.join(".")).is_err());
    let target = fixture.0.join("target");
    let junction = fixture.0.join("junction");
    fs::create_dir(&target).unwrap();
    let status = std::process::Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command",
            "New-Item -ItemType Junction -Path $env:ORCHBEE_FIXTURE_LINK -Value $env:ORCHBEE_FIXTURE_TARGET -ErrorAction Stop | Out-Null"])
        .env("ORCHBEE_FIXTURE_LINK", &junction).env("ORCHBEE_FIXTURE_TARGET", &target)
        .creation_flags(0x0800_0000).status().unwrap();
    assert!(status.success());
    assert!(permissions::open_tree(&junction).is_err());
    // Remove the junction itself before recursive fixture cleanup.
    fs::remove_dir(&junction).unwrap();
    assert!(target.is_dir());
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "orchbee-runtime-test-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let mut directory = permissions::open_directory(&path, true).unwrap();
        permissions::protect_new(&mut directory).unwrap();
        Self(path)
    }
    fn open(&self, scope: &str) -> Rendezvous {
        Rendezvous::prepare_mode_checked(&self.0, SessionScope::parse(scope).unwrap()).unwrap()
    }
    /// This folder as a data root, whose `sessions/` holds every Session folder.
    fn root(&self) -> crate::DataRoot {
        crate::DataRoot::at(self.0.clone())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let parent = std::env::temp_dir().canonicalize().unwrap();
        let path = self.0.canonicalize().unwrap();
        assert_eq!(path.parent(), Some(parent.as_path()));
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-runtime-test-")
        );
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn obsolete_lifetime_identity_is_refused_without_creating_a_second_lock() {
    let fixture = Fixture::new();
    let session = fixture.open("obsolete-lifetime");
    let runtime = session.runtime_directory().to_path_buf();
    drop(session);
    let obsolete = runtime.join("site-lifetime.lock");
    fs::write(&obsolete, b"retained old owner\n").unwrap();
    for create in [false, true] {
        let scope = SessionScope::parse("obsolete-lifetime").unwrap();
        let result = if create {
            Rendezvous::prepare_mode_checked(&fixture.0, scope)
        } else {
            Rendezvous::validate_mode_checked(&fixture.0, scope)
        };
        assert!(
            matches!(result, Err(crate::RendezvousError::System { source, .. })
            if source.kind() == std::io::ErrorKind::InvalidData)
        );
        assert!(!runtime.join("session-lifetime.lock").exists());
        assert_eq!(fs::read(&obsolete).unwrap(), b"retained old owner\n");
    }
}

#[test]
fn native_lock_contention_marker_exclusivity_and_endpoint_identity() {
    let fixture = Fixture::new();
    let first = fixture.open("first");
    let duplicate = fixture.open("first");
    let second = fixture.open("second");
    assert_ne!(first.client_socket_path(), second.client_socket_path());
    assert_ne!(
        Rendezvous::host_endpoint_path(first.runtime_directory()),
        Rendezvous::host_endpoint_path(second.runtime_directory())
    );
    assert_eq!(first.client_socket_path(), duplicate.client_socket_path());
    let guard = match first.try_acquire_activation_lock().unwrap() {
        LockAttempt::Acquired(guard) => guard,
        LockAttempt::Contended => panic!("unexpected owner"),
    };
    assert!(matches!(
        duplicate.try_acquire_activation_lock().unwrap(),
        LockAttempt::Contended
    ));
    drop(guard);
    assert!(matches!(
        duplicate.try_acquire_activation_lock().unwrap(),
        LockAttempt::Acquired(_)
    ));
    assert!(!first.activation_started().unwrap());
    first.commit_activation().unwrap();
    assert!(duplicate.activation_started().unwrap());
    assert!(duplicate.commit_activation().is_err());
    assert!(!first.client_socket_exists().unwrap());
    let listener = orchbee_local_ipc::LocalListener::bind(first.client_socket_path()).unwrap();
    assert!(duplicate.client_socket_exists().unwrap());
    drop(listener);
    assert!(!first.client_socket_exists().unwrap());
}

#[test]
fn broad_existing_acl_is_rejected_without_repair() {
    use windows_permissions::{
        LocalBox, SecurityDescriptor,
        constants::{SeObjectType, SecurityInformation},
        wrappers,
    };
    let fixture = Fixture::new();
    let session = fixture.open("unsafe");
    let path = session.runtime_directory().to_owned();
    drop(session);
    let mut handle = permissions::open_directory(&path, true).unwrap();
    let sddl = format!(
        "D:P(A;OICI;FA;;;{})(A;OICI;GR;;;WD)",
        orchbee_local_ipc::current_user_sid().unwrap()
    );
    let descriptor: LocalBox<SecurityDescriptor> = sddl.parse().unwrap();
    wrappers::SetSecurityInfo(
        &mut handle,
        SeObjectType::SE_FILE_OBJECT,
        SecurityInformation::Dacl | SecurityInformation::ProtectedDacl,
        None,
        None,
        descriptor.dacl(),
        None,
    )
    .unwrap();
    assert!(permissions::require_private(&handle).is_err());
    assert!(
        Rendezvous::prepare_mode_checked(&fixture.0, SessionScope::parse("unsafe").unwrap())
            .is_err()
    );
    assert!(permissions::require_private(&handle).is_err());
}

#[test]
fn diagnostic_observations_never_create_and_release_the_lifetime_probe() {
    use crate::SessionLifetimeObservation as Observation;
    let fixture = Fixture::new();
    let session = fixture.open("diagnose");
    assert!(!session.activation_lock_exists().unwrap());
    assert_eq!(
        session.observe_session_lifetime_lock().unwrap(),
        Observation::Absent
    );
    assert!(!session.activation_started().unwrap());
    assert_eq!(
        fs::read_dir(session.runtime_directory()).unwrap().count(),
        0
    );
    let held = match session.try_acquire_session_lifetime_lock().unwrap() {
        LockAttempt::Acquired(held) => held,
        LockAttempt::Contended => panic!("new fixture is uncontended"),
    };
    assert_eq!(
        session.observe_session_lifetime_lock().unwrap(),
        Observation::Held
    );
    drop(held);
    assert_eq!(
        session.observe_session_lifetime_lock().unwrap(),
        Observation::Free
    );
    assert!(matches!(
        session.try_acquire_session_lifetime_lock().unwrap(),
        LockAttempt::Acquired(_)
    ));
    let lock_path = session.runtime_directory().join("session-lifetime.lock");
    fs::hard_link(&lock_path, fixture.0.join("lock-alias")).unwrap();
    assert_eq!(
        session.observe_session_lifetime_lock().unwrap(),
        Observation::Unsafe
    );
    session.commit_activation().unwrap();
    fs::write(
        session.runtime_directory().join("activation.started"),
        b"preserved",
    )
    .unwrap();
    assert!(session.activation_started().is_err());
    assert_eq!(
        fs::read(session.runtime_directory().join("activation.started")).unwrap(),
        b"preserved"
    );
}

#[test]
fn completed_activation_is_pinned_idempotent_and_consumed_before_restart() {
    use crate::ActivationState;
    let fixture = Fixture::new();
    let session = fixture.open("stop-cycle");
    let marker = session.runtime_directory().join("activation.started");
    assert_eq!(session.activation_state().unwrap(), ActivationState::Absent);
    assert!(session.retain_active_record().is_err());
    session.commit_activation().unwrap();
    let mut record = session.retain_active_record().unwrap();
    assert!(fs::rename(&marker, session.runtime_directory().join("replaced")).is_err());
    assert!(record.complete([0; 16]).is_err());
    assert!(session.commit_activation().is_err());
    record.complete([1; 16]).unwrap();
    let completed = fs::read(&marker).unwrap();
    record.complete([1; 16]).unwrap();
    assert_eq!(fs::read(&marker).unwrap(), completed);
    assert!(record.complete([2; 16]).is_err());
    assert_eq!(fs::read(&marker).unwrap(), completed);
    assert_eq!(
        session.activation_state().unwrap(),
        ActivationState::Stopped {
            request_id: [1; 16]
        }
    );
    assert!(!session.activation_started().unwrap());
    assert!(session.retain_active_record().is_err());
    // Simulate torn I/O at the same retained file; only its existing owner
    // may finish the same ID, while discovery/start must fail closed.
    fs::write(&marker, b"orchbee-stopped-v1:01").unwrap();
    assert!(session.activation_state().is_err());
    assert!(session.commit_activation().is_err());
    assert!(record.complete([2; 16]).is_err());
    record.complete([1; 16]).unwrap();
    drop(record);
    session.commit_activation().unwrap();
    assert_eq!(session.activation_state().unwrap(), ActivationState::Active);
    assert!(fs::read(&marker).unwrap().is_empty());
    let mut second = session.retain_active_record().unwrap();
    second.complete([2; 16]).unwrap();
    assert_eq!(
        session.activation_state().unwrap(),
        ActivationState::Stopped {
            request_id: [2; 16]
        }
    );
}

#[test]
fn native_cohort_rejects_tampering_and_manifest_version() {
    let fixture = Fixture::new();
    let mut artifacts = serde_json::Map::new();
    for (name, artifact) in [
        ("orchbee", crate::CohortArtifact::Client),
        ("orchbee-server", crate::CohortArtifact::Server),
        ("orchbee-worker", crate::CohortArtifact::Worker),
        ("orchbee-terminal-host", crate::CohortArtifact::TerminalHost),
    ] {
        fs::write(fixture.0.join(artifact.file_name()), name.as_bytes()).unwrap();
        artifacts.insert(
            name.to_owned(),
            format!("{:x}", Sha256::digest(name.as_bytes())).into(),
        );
    }
    artifacts.insert(
        "agent-observation-adapters".to_owned(),
        "a".repeat(64).into(),
    );
    let manifest = serde_json::json!({"schemaVersion": 1, "artifacts": artifacts});
    fs::write(
        fixture.0.join("cohort.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let deadline = || Instant::now() + Duration::from_secs(3);
    assert!(Cohort::load(&fixture.0, deadline()).is_ok());
    let worker = fixture.0.join(crate::CohortArtifact::Worker.file_name());
    fs::write(&worker, b"tampered").unwrap();
    assert!(Cohort::load(&fixture.0, deadline()).is_err());
    fs::write(&worker, b"orchbee-worker").unwrap();
    let mut invalid_version = manifest;
    invalid_version["schemaVersion"] = 2.into();
    fs::write(
        fixture.0.join("cohort.json"),
        serde_json::to_vec(&invalid_version).unwrap(),
    )
    .unwrap();
    assert!(Cohort::load(&fixture.0, deadline()).is_err());
}
