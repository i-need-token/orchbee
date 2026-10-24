use super::*;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    path: PathBuf,
    _filesystem_lock: std::sync::MutexGuard<'static, ()>,
}
impl Fixture {
    fn new() -> Self {
        let guard = crate::rendezvous::tests::FILESYSTEM_TEST_LOCK
            .lock()
            .unwrap();
        let path = crate::test_temporary_directory().join(format!(
            "ob-cohort-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let mut artifacts = serde_json::Map::new();
        for artifact in ARTIFACTS {
            fs::write(path.join(artifact.file_name()), b"native fixture\n").unwrap();
            fs::set_permissions(
                path.join(artifact.file_name()),
                fs::Permissions::from_mode(0o700),
            )
            .unwrap();
            artifacts.insert(
                artifact.file_name().into(),
                serde_json::Value::String(format!("{:x}", Sha256::digest(b"native fixture\n"))),
            );
        }
        // The declared adapters digest is a controlled dummy: the fixture
        // stages no sidecar directory, proving admission stays lazy.
        artifacts.insert(
            OBSERVATION_ADAPTERS.into(),
            serde_json::Value::String(format!("{:x}", Sha256::digest(b"fixture adapters\n"))),
        );
        fs::write(
            path.join("cohort.json"),
            serde_json::to_vec(&serde_json::json!({
                "schemaVersion": 1, "artifacts": artifacts,
            }))
            .unwrap(),
        )
        .unwrap();
        // NativeCohort rejects a manifest carrying unsafe mode bits (0644 and
        // 0640 pass; group-or-other-writable does not); the process umask is
        // not guaranteed to clear group-write.
        fs::set_permissions(path.join("cohort.json"), fs::Permissions::from_mode(0o600)).unwrap();
        Self {
            path,
            _filesystem_lock: guard,
        }
    }
    fn load(&self) -> io::Result<NativeCohort> {
        NativeCohort::load(&self.path, Instant::now() + Duration::from_secs(2))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _removed = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn exact_cohort_verifies_every_role_and_has_a_canonical_identity() {
    let fixture = Fixture::new();
    let cohort = fixture.load().unwrap();
    assert_eq!(
        cohort.identity(),
        "29de6d3b37e92387bc577a3f127e354b2fb13c8f1df445a4e6644e8d320a1a5b"
    );
    assert_eq!(
        cohort.executable(NativeArtifact::Server),
        fixture.path.join("orchbee-server")
    );
    fs::write(fixture.path.join("orchbee-terminal-host"), b"mixed Host").unwrap();
    assert_eq!(
        fixture.load().unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn observation_adapters_sidecar_is_declared_and_lazily_admitted() {
    let fixture = Fixture::new();
    let cohort = fixture.load().unwrap();
    let sidecar = cohort.observation_adapters_sidecar();
    assert_eq!(
        sidecar.directory(),
        fixture.path.join("agent-observation-adapters").as_path()
    );
    assert_eq!(
        sidecar.declared_payload_sha256(),
        format!("{:x}", Sha256::digest(b"fixture adapters\n"))
    );
    // The fixture stages no sidecar at all; startup verification still
    // succeeds — the payload is admitted lazily by its first consumer.
    assert!(!sidecar.directory().exists());

    // A damaged sidecar — even a file where the directory belongs or garbage
    // content — still does not block cohort admission.
    fs::write(
        fixture.path.join("agent-observation-adapters"),
        b"not a capsule",
    )
    .unwrap();
    let cohort = fixture.load().unwrap();
    assert_eq!(
        cohort
            .observation_adapters_sidecar()
            .declared_payload_sha256(),
        format!("{:x}", Sha256::digest(b"fixture adapters\n"))
    );
}

#[test]
fn incompatible_and_incomplete_manifests_fail_typed() {
    // Unsupported versions reject even when the complete current shape is supplied.
    for version in [serde_json::json!(2), serde_json::json!(3)] {
        let fixture = Fixture::new();
        let manifest = fixture.path.join("cohort.json");
        let mut encoded: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        encoded["schemaVersion"] = version;
        fs::write(&manifest, encoded.to_string()).unwrap();
        let error = fixture.load().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            error.to_string(),
            "incompatible native cohort manifest version"
        );
    }
    {
        let fixture = Fixture::new();
        let manifest = fixture.path.join("cohort.json");
        let mut encoded: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        encoded["schemaVersion"] = serde_json::json!(2);
        encoded["artifacts"]
            .as_object_mut()
            .unwrap()
            .remove("agent-observation-adapters");
        fs::write(&manifest, encoded.to_string()).unwrap();
        let error = fixture.load().unwrap_err();
        assert_eq!(
            error.to_string(),
            "incompatible native cohort manifest version"
        );
    }
    // A current manifest missing the required adapters member, or declaring a
    // malformed digest, is an invalid manifest — never defaulted.
    for mutate in [
        |manifest: &mut serde_json::Value| {
            manifest["artifacts"]
                .as_object_mut()
                .unwrap()
                .remove("agent-observation-adapters");
        },
        |manifest: &mut serde_json::Value| {
            manifest["artifacts"]["agent-observation-adapters"] = serde_json::json!("not-a-hash");
        },
    ] {
        let fixture = Fixture::new();
        let manifest = fixture.path.join("cohort.json");
        let mut encoded: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        mutate(&mut encoded);
        fs::write(&manifest, encoded.to_string()).unwrap();
        let error = fixture.load().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(error.to_string(), "invalid native development cohort");
    }
}
#[test]
fn missing_symlink_hardlink_and_writable_artifacts_fail_admission() {
    for violation in [
        "missing",
        "symlink",
        "hardlink",
        "writable",
        "not-executable",
    ] {
        let fixture = Fixture::new();
        let artifact = fixture.path.join("orchbee-worker");
        match violation {
            "missing" => fs::remove_file(&artifact).unwrap(),
            "symlink" => {
                fs::remove_file(&artifact).unwrap();
                symlink("orchbee", &artifact).unwrap();
            }
            "hardlink" => fs::hard_link(&artifact, fixture.path.join("alias")).unwrap(),
            "writable" => {
                fs::set_permissions(&artifact, fs::Permissions::from_mode(0o720)).unwrap()
            }
            _ => fs::set_permissions(&artifact, fs::Permissions::from_mode(0o600)).unwrap(),
        }
        assert!(fixture.load().is_err(), "{violation}");
    }
}

#[test]
fn diagnostic_names_only_a_missing_artifact_open() {
    let fixture = Fixture::new();
    fs::remove_file(fixture.path.join(NativeArtifact::Server.file_name())).unwrap();
    let error =
        NativeCohort::load_with_diagnostic(&fixture.path, Instant::now() + Duration::from_secs(2))
            .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert_eq!(
        error.missing_artifact().map(NativeArtifact::file_name),
        Some(NativeArtifact::Server.file_name())
    );

    let fixture = Fixture::new();
    fs::remove_file(fixture.path.join("cohort.json")).unwrap();
    let error =
        NativeCohort::load_with_diagnostic(&fixture.path, Instant::now() + Duration::from_secs(2))
            .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(error.missing_artifact().is_none());
}

#[test]
fn malformed_duplicate_unknown_and_oversized_manifests_fail_closed() {
    for encoded in [
        b"{}".to_vec(),
        b"{\"schemaVersion\":1,\"schemaVersion\":1,\"artifacts\":{}}".to_vec(),
        b"{\"schemaVersion\":1,\"artifacts\":{},\"extra\":true}".to_vec(),
        vec![b' '; MANIFEST_LIMIT as usize + 1],
    ] {
        let fixture = Fixture::new();
        fs::write(fixture.path.join("cohort.json"), encoded).unwrap();
        assert_eq!(
            fixture.load().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
#[test]
fn expired_deadline_and_unsafe_directory_fail_before_artifact_admission() {
    let fixture = Fixture::new();
    assert_eq!(
        NativeCohort::load(&fixture.path, Instant::now())
            .unwrap_err()
            .kind(),
        io::ErrorKind::TimedOut
    );
    fs::set_permissions(&fixture.path, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(fixture.load().is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn exact_artifact_hash_and_mode_do_not_admit_an_extended_acl() {
    let fixture = Fixture::new();
    let file = File::open(fixture.path.join("orchbee-worker")).unwrap();
    rustix::fs::fsetxattr(
        &file,
        "system.posix_acl_access",
        &crate::linux_permissions::tests::named_acl(7),
        rustix::fs::XattrFlags::empty(),
    )
    .unwrap();
    assert_eq!(
        file.metadata().unwrap().permissions().mode() & 0o7777,
        0o700
    );
    assert_eq!(
        fixture.load().unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
}
