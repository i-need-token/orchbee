use crate::{
    LockAttempt, ModeCheckedLocalSiteRendezvous as Rendezvous, NativeCohort, SiteScope,
    windows_permissions as permissions,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

mod session_store;
mod site_name;

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
        Rendezvous::prepare_mode_checked(&self.0, SiteScope::parse(scope).unwrap()).unwrap()
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
    let site = fixture.open("unsafe");
    let path = site.runtime_directory().to_owned();
    drop(site);
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
        Rendezvous::prepare_mode_checked(&fixture.0, SiteScope::parse("unsafe").unwrap()).is_err()
    );
    assert!(permissions::require_private(&handle).is_err());
}

#[test]
fn diagnostic_observations_never_create_and_release_the_lifetime_probe() {
    use crate::SiteLifetimeObservation as Observation;
    let fixture = Fixture::new();
    let site = fixture.open("diagnose");
    assert!(!site.activation_lock_exists().unwrap());
    assert_eq!(
        site.observe_site_lifetime_lock().unwrap(),
        Observation::Absent
    );
    assert!(!site.activation_started().unwrap());
    assert_eq!(fs::read_dir(site.runtime_directory()).unwrap().count(), 0);
    let held = match site.try_acquire_site_lifetime_lock().unwrap() {
        LockAttempt::Acquired(held) => held,
        LockAttempt::Contended => panic!("new fixture is uncontended"),
    };
    assert_eq!(
        site.observe_site_lifetime_lock().unwrap(),
        Observation::Held
    );
    drop(held);
    assert_eq!(
        site.observe_site_lifetime_lock().unwrap(),
        Observation::Free
    );
    assert!(matches!(
        site.try_acquire_site_lifetime_lock().unwrap(),
        LockAttempt::Acquired(_)
    ));
    let lock_path = site.runtime_directory().join("site-lifetime.lock");
    fs::hard_link(&lock_path, fixture.0.join("lock-alias")).unwrap();
    assert_eq!(
        site.observe_site_lifetime_lock().unwrap(),
        Observation::Unsafe
    );
    site.commit_activation().unwrap();
    fs::write(
        site.runtime_directory().join("activation.started"),
        b"preserved",
    )
    .unwrap();
    assert!(site.activation_started().is_err());
    assert_eq!(
        fs::read(site.runtime_directory().join("activation.started")).unwrap(),
        b"preserved"
    );
}

#[test]
fn completed_activation_is_pinned_idempotent_and_consumed_before_restart() {
    use crate::ActivationState;
    let fixture = Fixture::new();
    let site = fixture.open("stop-cycle");
    let marker = site.runtime_directory().join("activation.started");
    assert_eq!(site.activation_state().unwrap(), ActivationState::Absent);
    assert!(site.retain_active_record().is_err());
    site.commit_activation().unwrap();
    let mut record = site.retain_active_record().unwrap();
    assert!(fs::rename(&marker, site.runtime_directory().join("replaced")).is_err());
    assert!(record.complete([0; 16]).is_err());
    assert!(site.commit_activation().is_err());
    record.complete([1; 16]).unwrap();
    let completed = fs::read(&marker).unwrap();
    record.complete([1; 16]).unwrap();
    assert_eq!(fs::read(&marker).unwrap(), completed);
    assert!(record.complete([2; 16]).is_err());
    assert_eq!(fs::read(&marker).unwrap(), completed);
    assert_eq!(
        site.activation_state().unwrap(),
        ActivationState::Stopped {
            request_id: [1; 16]
        }
    );
    assert!(!site.activation_started().unwrap());
    assert!(site.retain_active_record().is_err());
    // Simulate torn I/O at the same retained file; only its existing owner
    // may finish the same ID, while discovery/start must fail closed.
    fs::write(&marker, b"orchbee-stopped-v1:01").unwrap();
    assert!(site.activation_state().is_err());
    assert!(site.commit_activation().is_err());
    assert!(record.complete([2; 16]).is_err());
    record.complete([1; 16]).unwrap();
    drop(record);
    site.commit_activation().unwrap();
    assert_eq!(site.activation_state().unwrap(), ActivationState::Active);
    assert!(fs::read(&marker).unwrap().is_empty());
    let mut second = site.retain_active_record().unwrap();
    second.complete([2; 16]).unwrap();
    assert_eq!(
        site.activation_state().unwrap(),
        ActivationState::Stopped {
            request_id: [2; 16]
        }
    );
}

#[test]
fn native_cohort_rejects_tampering_and_manifest_version() {
    let fixture = Fixture::new();
    let mut artifacts = serde_json::Map::new();
    for name in [
        "orchbee",
        "orchbee-server",
        "orchbee-worker",
        "orchbee-terminal-host",
    ] {
        fs::write(fixture.0.join(format!("{name}.exe")), name.as_bytes()).unwrap();
        artifacts.insert(
            name.to_owned(),
            format!("{:x}", Sha256::digest(name.as_bytes())).into(),
        );
    }
    artifacts.insert(
        "agent-observation-adapters".to_owned(),
        "a".repeat(64).into(),
    );
    let manifest = serde_json::json!({"schemaVersion": 2, "artifacts": artifacts});
    fs::write(
        fixture.0.join("cohort.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let deadline = || Instant::now() + Duration::from_secs(3);
    assert!(NativeCohort::load(&fixture.0, deadline()).is_ok());
    fs::write(fixture.0.join("orchbee-worker.exe"), b"tampered").unwrap();
    assert!(NativeCohort::load(&fixture.0, deadline()).is_err());
    fs::write(fixture.0.join("orchbee-worker.exe"), b"orchbee-worker").unwrap();
    let mut invalid_version = manifest;
    invalid_version["schemaVersion"] = 1.into();
    fs::write(
        fixture.0.join("cohort.json"),
        serde_json::to_vec(&invalid_version).unwrap(),
    )
    .unwrap();
    assert!(NativeCohort::load(&fixture.0, deadline()).is_err());
}
