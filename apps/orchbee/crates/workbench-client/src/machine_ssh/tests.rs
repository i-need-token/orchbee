use super::*;
#[test]
fn machine_probe_refusals_and_transient_network_errors_have_distinct_retry_policy() {
    assert_eq!(
        probe_exit_kind(Some(255), b"Permission denied (publickey)."),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        probe_exit_kind(Some(255), b"Host key verification failed."),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(probe_exit_kind(Some(125), b""), io::ErrorKind::NotFound);
    assert_eq!(
        probe_exit_kind(Some(255), b"Connection timed out"),
        io::ErrorKind::ConnectionRefused
    );
}

fn status(executable: &str) -> Value {
    serde_json::json!({"schema":"orchbee.status.client","version":1,"result":{
        "version":"0.0.0-remote","platform":"windows","architecture":"x86_64",
        "protocols":{"admission":1,"terminal":255},
        "cohort":{"state":"available","identity":"a".repeat(64)},
        "executable":{"path":executable}}})
}
#[test]
fn admission_requires_the_same_environment_kind_and_never_compares_versions() {
    const DEV: &str = "C:/o/ob-dev-0123456789abcdef.exe";
    const TEST: &str = r"D:\o\OB-TEST-fedcba9876543210.EXE";
    const RELEASE: &str = "C:/Program Files/Orchbee/orchbee.exe";
    let release_kind: [&[&str]; 2] = [&[RELEASE], &[DEV, TEST]];
    let development_kind: [&[&str]; 2] = [&[DEV, TEST], &[RELEASE]];
    for (release, [admitted, refused]) in [(true, release_kind), (false, development_kind)] {
        for &path in admitted {
            let client = client_of_kind(&status(path), release).unwrap();
            assert_eq!(
                (client.executable.as_str(), client.version.as_str()),
                (path, "0.0.0-remote")
            );
        }
        for &path in refused {
            assert_eq!(
                client_of_kind(&status(path), release).err(),
                Some(AdmissionError::OtherEnvironment)
            );
        }
        for path in [
            "C:/o/ob-dev-0123.exe",
            "C:/o/other.exe",
            "C:/o/orchbee",
            "relative/orchbee.exe",
        ] {
            assert_eq!(
                client_of_kind(&status(path), release).err(),
                Some(AdmissionError::Protocol)
            );
        }
    }
    let mut document = status(RELEASE);
    document["result"]["architecture"] = serde_json::json!("aarch64");
    assert!(client_of_kind(&document, true).is_ok());
    for (pointer, value) in [
        ("/result/architecture", serde_json::json!("x86")),
        ("/result/platform", serde_json::json!("linux")),
        ("/result/cohort", serde_json::json!({"state":"unavailable"})),
    ] {
        let mut document = status(RELEASE);
        *document.pointer_mut(pointer).unwrap() = value;
        assert_eq!(
            client_of_kind(&document, true).err(),
            Some(AdmissionError::Unsupported)
        );
    }
    let refusal = io::Error::from(AdmissionError::OtherEnvironment.refusal().unwrap());
    assert_eq!(refusal.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(Refusal::of(&refusal), Some(Refusal::OtherEnvironment));
}
