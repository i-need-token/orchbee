//! `config check` coverage at the function boundary: every RendezvousError
//! class, every socket inspection outcome, the cohort report, and the
//! locale row — real filesystem only inside each test's scratch directory.

use super::*;
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;

fn scratch(tag: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "obcli-config-check-{}-{tag}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir(&directory).expect("create scratch base");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("chmod scratch base");
    directory
}

const BASE: &str = "/ob-config-test-base";
const SCOPE_DIR: &str = "/ob-config-test-base/orchbee-native-1000";

fn classify(error: RendezvousError) -> (CheckReport, CheckReport) {
    classify_paths(&Err(error), Path::new(BASE), Path::new(SCOPE_DIR))
}

#[test]
fn unsafe_owner_mode_and_type_fail_with_established_presence() {
    // Inspected invalid entries prove presence: exists:true + unsafe.
    for error in [
        RendezvousError::UnexpectedOwner {
            path: BASE.into(),
            expected: 1000,
            actual: 0,
        },
        RendezvousError::UnexpectedMode {
            path: BASE.into(),
            expected: 0o700,
            actual: 0o777,
        },
        RendezvousError::NotDirectory { path: BASE.into() },
        RendezvousError::DirectorySymlink { path: BASE.into() },
    ] {
        let repr = format!("{error:?}");
        let (base, directory) = classify(error);
        assert_eq!(base.status, "fail", "{repr}");
        assert_eq!(base.details["reason"], json!("unsafe"), "{repr}");
        assert_eq!(base.details["exists"], json!(true), "{repr}");
        assert_eq!(base.details["path"], json!(BASE), "{repr}");
        assert_eq!(directory.status, "not-applicable", "{repr}");
        assert!(directory.details.is_empty(), "{repr}");
    }

    // The same verdict at the scope directory flips which check fails.
    let (base, directory) = classify(RendezvousError::UnexpectedMode {
        path: SCOPE_DIR.into(),
        expected: 0o700,
        actual: 0o750,
    });
    assert_eq!(base.status, "pass");
    assert_eq!(base.details["exists"], json!(true));
    assert_eq!(directory.status, "fail");
    assert_eq!(directory.details["reason"], json!("unsafe"));
    assert_eq!(directory.details["exists"], json!(true));
    assert_eq!(directory.details["path"], json!(SCOPE_DIR));
}

#[test]
fn invalid_paths_fail_without_an_established_exists() {
    for error in [
        RendezvousError::RuntimeBaseNotAbsolute {
            path: "relative".into(),
        },
        RendezvousError::RuntimeBaseNotNormalized {
            path: "/tmp/../tmp".into(),
        },
        RendezvousError::InvalidSiteScope,
    ] {
        let repr = format!("{error:?}");
        let (base, directory) = classify(error);
        assert_eq!(base.status, "fail", "{repr}");
        assert_eq!(base.details["reason"], json!("invalid-path"), "{repr}");
        // `exists` appears only when the path state was established.
        assert!(base.details.get("exists").is_none(), "{repr}");
        assert_eq!(directory.status, "not-applicable", "{repr}");
        assert!(directory.details.is_empty(), "{repr}");
    }
}

#[test]
fn absent_paths_report_exists_false_io_errors_omit_it() {
    // An established absence is a fact: exists:false + reason absent.
    let (base, _) = classify(RendezvousError::System {
        operation: "test",
        path: BASE.into(),
        source: std::io::Error::from(std::io::ErrorKind::NotFound),
    });
    assert_eq!(base.status, "fail");
    assert_eq!(base.details["reason"], json!("absent"));
    assert_eq!(base.details["exists"], json!(false));

    // The same absence at the scope directory is not-applicable, not a
    // fault — but the established fact is still reported.
    let (base, directory) = classify(RendezvousError::System {
        operation: "test",
        path: SCOPE_DIR.into(),
        source: std::io::Error::from(std::io::ErrorKind::NotFound),
    });
    assert_eq!(base.status, "pass");
    assert_eq!(directory.status, "not-applicable");
    assert_eq!(directory.details["reason"], json!("absent"));
    assert_eq!(directory.details["exists"], json!(false));

    // A genuine I/O failure establishes nothing: `exists` is omitted.
    for source in [
        std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        std::io::Error::from_raw_os_error(5), // EIO
    ] {
        let (base, directory) = classify(RendezvousError::System {
            operation: "test",
            path: BASE.into(),
            source,
        });
        assert_eq!(base.status, "fail");
        assert_eq!(base.details["reason"], json!("io-error"));
        assert!(base.details.get("exists").is_none());
        assert_eq!(directory.status, "not-applicable");
        assert!(directory.details.is_empty());
    }
}

#[test]
fn non_utf8_paths_encode_as_path_bytes_hex() {
    // The machine field is exact bytes, never a lossy display string.
    let (key, value) = path_detail(Path::new("/tmp/utf8"));
    assert_eq!(key, "path");
    assert_eq!(value, json!("/tmp/utf8"));
    let raw = std::ffi::OsStr::from_bytes(b"/tmp/\xff\xfe");
    let (key, value) = path_detail(Path::new(raw));
    assert_eq!(key, "pathBytesHex");
    let hex: String = raw
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(value, json!(hex));

    // The same field lands in check details when the base itself is
    // non-UTF-8.
    let weird = PathBuf::from(raw);
    let (base, _) = classify_paths(
        &Err(RendezvousError::UnexpectedOwner {
            path: weird.clone(),
            expected: 1000,
            actual: 0,
        }),
        &weird,
        Path::new(SCOPE_DIR),
    );
    assert_eq!(base.status, "fail");
    assert_eq!(base.details["pathBytesHex"], json!(hex));
    assert!(base.details.get("path").is_none());
}

#[test]
fn cohort_report_maps_verifier_outcomes() {
    // Pass requires a real validated cohort: stage the four artifacts
    // plus a hash-true manifest, all owner-private.
    let cohort = scratch("cohort-pass");
    let mut artifacts = serde_json::Map::new();
    for name in [
        "orchbee",
        "orchbee-server",
        "orchbee-worker",
        "orchbee-terminal-host",
    ] {
        let path = cohort.join(name);
        fs::write(&path, format!("stand-in for {name}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let digest = Sha256::digest(fs::read(&path).unwrap());
        artifacts.insert(name.into(), format!("{digest:x}").into());
    }
    // A controlled dummy digest stands in for the adapters payload — the
    // fixture stages no sidecar; cohort admission stays lazy.
    artifacts.insert(
        "agent-observation-adapters".into(),
        format!("{:x}", Sha256::digest(b"staged stand-in adapters\n")).into(),
    );
    let manifest = cohort.join("cohort.json");
    fs::write(
        &manifest,
        serde_json::json!({"schemaVersion": 1, "artifacts": artifacts}).to_string(),
    )
    .unwrap();
    fs::set_permissions(&manifest, fs::Permissions::from_mode(0o600)).unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    let report = cohort_report(NativeCohort::load(&cohort, deadline));
    assert_eq!(report.status, "pass");
    assert_eq!(report.details["verified"], json!(true));
    fs::remove_dir_all(&cohort).ok();

    // A missing manifest is an invalid-cohort failure, not a crash.
    let empty = scratch("cohort-empty");
    let report = cohort_report(NativeCohort::load(&empty, deadline));
    assert_eq!(report.status, "fail");
    assert_eq!(report.details["verified"], json!(false));
    assert_eq!(report.details["reason"], json!("invalid-cohort"));
    fs::remove_dir_all(&empty).ok();

    // An already-spent deadline reports `deadline` before any I/O.
    let report = cohort_report(NativeCohort::load(
        Path::new("/nonexistent"),
        Instant::now(),
    ));
    assert_eq!(report.status, "fail");
    assert_eq!(report.details["reason"], json!("deadline"));
}

#[test]
fn locale_check_reports_the_selected_locale() {
    let report = locale_check();
    assert_eq!(report.status, "pass");
    assert!(
        ["en-US", "zh-CN"].contains(&report.details["locale"].as_str().unwrap()),
        "locale: {}",
        report.details["locale"]
    );
    assert!(
        ["LC_ALL", "LC_MESSAGES", "LANG", "default"]
            .contains(&report.details["source"].as_str().unwrap()),
        "source: {}",
        report.details["source"]
    );
}

#[test]
fn socket_entry_maps_every_inspection_outcome() {
    let path = Path::new("/tmp/oc-test/orchbee-native-1000/client.sock");
    // Compliant socket: present + pass, aggregate pass bit.
    let (entry, present, fail) = socket_entry(Some(&Ok(true)), path);
    assert_eq!(entry["presence"], json!("present"));
    assert_eq!(entry["status"], json!("pass"));
    assert!(!entry.contains_key("reason"));
    assert!(present && !fail);

    // Established absence: absent + not-applicable.
    let (entry, present, fail) = socket_entry(Some(&Ok(false)), path);
    assert_eq!(entry["presence"], json!("absent"));
    assert_eq!(entry["status"], json!("not-applicable"));
    assert!(!present && !fail);

    // The synthesized invalid-entry verdict stays present + fail + unsafe.
    let (entry, present, fail) = socket_entry(
        Some(&Err(std::io::Error::from(
            std::io::ErrorKind::PermissionDenied,
        ))),
        path,
    );
    assert_eq!(entry["presence"], json!("present"));
    assert_eq!(entry["status"], json!("fail"));
    assert_eq!(entry["reason"], json!("unsafe"));
    assert!(!present && fail);

    // A real fstatat failure establishes nothing: presence is honestly
    // unchecked, never a fabricated definitive field.
    let (entry, present, fail) =
        socket_entry(Some(&Err(std::io::Error::from_raw_os_error(5))), path);
    assert_eq!(entry["presence"], json!("unchecked"));
    assert_eq!(entry["status"], json!("not-applicable"));
    assert_eq!(entry["reason"], json!("io-error"));
    assert!(!present && !fail);

    // No runtime directory at all: unchecked + runtime-unavailable.
    let (entry, present, fail) = socket_entry(None, path);
    assert_eq!(entry["presence"], json!("unchecked"));
    assert_eq!(entry["status"], json!("not-applicable"));
    assert_eq!(entry["reason"], json!("runtime-unavailable"));
    assert!(!present && !fail);
}

#[test]
fn socket_entry_fails_when_the_derived_address_cannot_fit() {
    // The budget is path bytes, not Unicode scalars: `capacity + 1`
    // ASCII bytes fails while a scalar-shorter but byte-longer non-ASCII
    // path fails identically.
    let too_long_ascii = PathBuf::from("/".to_owned() + &"a".repeat(UNIX_SOCKET_PATH_CAPACITY));
    assert_eq!(
        too_long_ascii.as_os_str().as_encoded_bytes().len(),
        UNIX_SOCKET_PATH_CAPACITY + 1
    );
    for exists in [
        Some(Ok(true)),
        Some(Ok(false)),
        Some(Err(std::io::Error::from(
            std::io::ErrorKind::PermissionDenied,
        ))),
        Some(Err(std::io::Error::from_raw_os_error(5))),
        None,
    ] {
        let (entry, _present, fail) = socket_entry(exists.as_ref(), &too_long_ascii);
        assert_eq!(entry["status"], json!("fail"), "{exists:?}");
        assert_eq!(entry["reason"], json!("address-too-long"), "{exists:?}");
        assert_eq!(
            entry["addressBytes"],
            json!(UNIX_SOCKET_PATH_CAPACITY + 1),
            "{exists:?}"
        );
        assert_eq!(
            entry["addressCapacityBytes"],
            json!(UNIX_SOCKET_PATH_CAPACITY),
            "{exists:?}"
        );
        assert_eq!(entry["path"], json!(too_long_ascii), "{exists:?}");
        assert!(fail, "{exists:?}");
    }
    // Presence is still reported honestly under the fail verdict.
    let (entry, present, _) = socket_entry(Some(&Ok(true)), &too_long_ascii);
    assert_eq!(entry["presence"], json!("present"));
    assert!(present);
    let (entry, _, _) = socket_entry(Some(&Ok(false)), &too_long_ascii);
    assert_eq!(entry["presence"], json!("absent"));
    let (entry, _, _) = socket_entry(None, &too_long_ascii);
    assert_eq!(entry["presence"], json!("unchecked"));

    // 40 scalars but 121 bytes: byte length, not char count, is measured.
    let non_ascii = PathBuf::from("/".to_owned() + &"日".repeat(40));
    assert_eq!(non_ascii.as_os_str().as_encoded_bytes().len(), 1 + 40 * 3);
    let (entry, _, fail) = socket_entry(Some(&Ok(false)), &non_ascii);
    assert_eq!(entry["reason"], json!("address-too-long"));
    assert_eq!(entry["addressBytes"], json!(121));
    assert!(fail);

    // The exact boundary still fits — bytes equal to capacity pass.
    let boundary = PathBuf::from("/".to_owned() + &"a".repeat(UNIX_SOCKET_PATH_CAPACITY - 1));
    assert_eq!(
        boundary.as_os_str().as_encoded_bytes().len(),
        UNIX_SOCKET_PATH_CAPACITY
    );
    let (entry, _, fail) = socket_entry(Some(&Ok(false)), &boundary);
    assert_eq!(entry["status"], json!("not-applicable"));
    assert!(!fail);

    // A non-UTF-8 overlong path reports its exact bytes, never lossy text.
    let raw = std::ffi::OsStr::from_bytes(b"/\xff\xfe");
    let mut weird = PathBuf::from(raw);
    weird.push("a".repeat(UNIX_SOCKET_PATH_CAPACITY));
    let (entry, _, fail) = socket_entry(None, &weird);
    assert_eq!(entry["reason"], json!("address-too-long"));
    assert!(entry.get("path").is_none());
    assert_eq!(
        entry["pathBytesHex"].as_str().unwrap().len(),
        weird.as_os_str().as_encoded_bytes().len() * 2
    );
    assert!(fail);
}

#[test]
fn socket_checks_report_derived_budget_entries() {
    // `clientPending` and `host` are derived-address budget entries only:
    // presence is never probed, so every fitting path reports
    // `presence-not-inspected` — never a claim that the endpoint exists.
    let report = socket_checks(
        &Err(RendezvousError::System {
            operation: "test",
            path: BASE.into(),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        }),
        Path::new(SCOPE_DIR),
    );
    for name in ["clientPending", "host"] {
        let entry = &report.details[name];
        assert_eq!(entry["presence"], json!("unchecked"), "{name}: {entry}");
        assert_eq!(entry["status"], json!("not-applicable"), "{name}: {entry}");
        assert_eq!(
            entry["reason"],
            json!("presence-not-inspected"),
            "{name}: {entry}"
        );
        assert_eq!(
            entry["addressCapacityBytes"],
            json!(UNIX_SOCKET_PATH_CAPACITY),
            "{name}: {entry}"
        );
        assert!(
            entry["addressBytes"].as_u64().unwrap() > 0,
            "{name}: {entry}"
        );
        assert!(
            entry.get("path").is_some() || entry.get("pathBytesHex").is_some(),
            "{name}: {entry}"
        );
    }
    // A fitting unchecked budget never promotes the aggregate to pass.
    assert_eq!(report.status, "not-applicable");

    // An over-budget derived address fails on its own: only the budget
    // verdict changes, `presence` stays honestly unchecked.
    let long_dir = PathBuf::from("/".to_owned() + &"a".repeat(UNIX_SOCKET_PATH_CAPACITY));
    let report = socket_checks(&Err(RendezvousError::InvalidSiteScope), &long_dir);
    assert_eq!(report.status, "fail");
    for name in ["clientPending", "host"] {
        let entry = &report.details[name];
        assert_eq!(entry["presence"], json!("unchecked"), "{name}: {entry}");
        assert_eq!(entry["status"], json!("fail"), "{name}: {entry}");
        assert_eq!(
            entry["reason"],
            json!("address-too-long"),
            "{name}: {entry}"
        );
        assert_eq!(
            entry["addressCapacityBytes"],
            json!(UNIX_SOCKET_PATH_CAPACITY),
            "{name}: {entry}"
        );
        let bytes = entry["addressBytes"].as_u64().unwrap() as usize;
        assert!(bytes > UNIX_SOCKET_PATH_CAPACITY, "{name}: {entry}");
    }
}

#[test]
fn socket_checks_aggregate_real_states() {
    // No valid runtime directory: both entries unchecked.
    let report = socket_checks(
        &Err(RendezvousError::System {
            operation: "test",
            path: BASE.into(),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        }),
        Path::new(SCOPE_DIR),
    );
    assert_eq!(report.status, "not-applicable");
    for name in ["client", "worker"] {
        assert_eq!(report.details[name]["presence"], json!("unchecked"));
        assert_eq!(report.details[name]["reason"], json!("runtime-unavailable"));
    }

    // A real prepared scope without sockets: absent + not-applicable. The
    // base must stay short — the derived Host endpoint is the deepest
    // budget fact and a deep scratch would itself trip `address-too-long`.
    let base = {
        let directory = std::env::temp_dir().join(format!(
            "obsc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir(&directory).expect("create short socket base");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .expect("chmod short socket base");
        directory
    };
    let site = || {
        ModeCheckedLocalSiteRendezvous::validate_mode_checked(
            &base,
            SiteScope::parse("ob-test-sockets").unwrap(),
        )
        .expect("validated scope")
    };
    let scope_directory = ModeCheckedLocalSiteRendezvous::prepare_mode_checked(
        &base,
        SiteScope::parse("ob-test-sockets").unwrap(),
    )
    .expect("prepared scope")
    .runtime_directory()
    .to_owned();
    let report = socket_checks(&Ok(site()), &scope_directory);
    assert_eq!(report.status, "not-applicable");
    assert_eq!(report.details["client"]["presence"], json!("absent"));
    assert_eq!(report.details["worker"]["presence"], json!("absent"));

    // One compliant socket passes the check.
    let listener = UnixListener::bind(scope_directory.join("client.sock")).unwrap();
    fs::set_permissions(
        scope_directory.join("client.sock"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let report = socket_checks(&Ok(site()), &scope_directory);
    assert_eq!(report.status, "pass");
    assert_eq!(report.details["client"]["presence"], json!("present"));
    assert_eq!(report.details["client"]["status"], json!("pass"));
    assert_eq!(report.details["worker"]["presence"], json!("absent"));
    drop(listener);

    // An inspected invalid entry fails: a regular file is not a socket.
    fs::remove_file(scope_directory.join("client.sock")).unwrap();
    fs::write(scope_directory.join("client.sock"), b"not a socket").unwrap();
    let report = socket_checks(&Ok(site()), &scope_directory);
    assert_eq!(report.status, "fail");
    assert_eq!(report.details["client"]["presence"], json!("present"));
    assert_eq!(report.details["client"]["status"], json!("fail"));
    assert_eq!(report.details["client"]["reason"], json!("unsafe"));
    assert_eq!(report.details["worker"]["presence"], json!("absent"));
    fs::remove_dir_all(&base).ok();
}
