use super::*;
use crate::SessionName;

#[test]
fn names_are_bounded_portable_and_compared_without_letter_case() {
    for invalid in [
        "",
        ".",
        "..",
        ".work",
        "work.",
        "a/b",
        "a\\b",
        "中文",
        "a b",
        "a:b",
        "con",
        "CON",
        "Nul.txt",
        "com1",
        "LPT9.log",
        &"a".repeat(65),
    ] {
        assert!(SessionName::parse(invalid).is_err(), "{invalid}");
        // A Windows runtime folder takes its Session's name, so the scope grammar is the same.
        assert!(SessionScope::parse(invalid).is_err(), "{invalid}");
    }
    for valid in [
        "work",
        "Work",
        "a.b",
        "-",
        "_x",
        "con1",
        "com10",
        "console",
        &"a".repeat(64),
    ] {
        let name = SessionName::parse(valid).unwrap();
        assert_eq!(name.as_str(), valid);
        assert!(SessionScope::parse(valid).is_ok());
    }
    assert_eq!(
        SessionName::parse("Work").unwrap(),
        SessionName::parse("wORK").unwrap()
    );
    assert_ne!(
        SessionName::parse("work").unwrap(),
        SessionName::parse("works").unwrap()
    );
    let default = SessionName::parse("DEFAULT").unwrap();
    assert_eq!(default.as_str(), "default");
    assert!(default.is_default());
    assert_eq!(default, SessionName::default_session());
}

#[test]
fn runtime_folders_differing_only_in_letter_case_share_their_endpoints() {
    let fixture = Fixture::new();
    let upper = fixture.open("Work");
    let lower = Rendezvous::validate_mode_checked(&fixture.0, SessionScope::parse("work").unwrap())
        .unwrap();
    assert_eq!(upper.client_socket_path(), lower.client_socket_path());
    assert_eq!(
        Rendezvous::host_endpoint_id(upper.runtime_directory()),
        Rendezvous::host_endpoint_id(lower.runtime_directory())
    );
    assert_ne!(
        upper.client_socket_path(),
        fixture.open("other").client_socket_path()
    );
}
