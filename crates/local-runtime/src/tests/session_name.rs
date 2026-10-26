use super::*;
use crate::SessionName;

fn named(fixture: &Fixture, name: &str) -> (SessionName, Rendezvous) {
    let name = SessionName::parse(name).unwrap();
    let scope = name.scope_for_user(&orchbee_local_ipc::current_user_sid().unwrap());
    let session = fixture.open(&scope);
    (name, session)
}

fn activation(session: &Rendezvous) -> crate::ModeCheckedSessionLock {
    match session.try_acquire_activation_lock().unwrap() {
        LockAttempt::Acquired(lock) => lock,
        LockAttempt::Contended => panic!("unexpected activation owner"),
    }
}

#[test]
fn names_are_bounded_exact_and_windows_path_independent() {
    for invalid in [
        "",
        ".",
        "..",
        "a/b",
        "a\\b",
        "中文",
        "a b",
        "a:b",
        &"a".repeat(65),
    ] {
        assert!(SessionName::parse(invalid).is_err());
    }
    for valid in ["default", "work", "Work", "CON", "a.", "-", &"a".repeat(64)] {
        let name = SessionName::parse(valid).unwrap();
        assert_eq!(name.as_str(), valid);
        assert!(SessionScope::parse(&name.scope_for_user("sid")).is_ok());
    }
    let name = SessionName::parse("work").unwrap();
    assert_ne!(name.scope_for_user("sid-a"), name.scope_for_user("sid-b"));
    assert_ne!(
        name.scope_for_user("sid"),
        SessionName::parse("Work").unwrap().scope_for_user("sid")
    );
    assert_eq!(
        SessionName::parse("default").unwrap().scope_for_user("sid"),
        format!("orchbee-native-{:x}", Sha256::digest(b"sid"))[..47]
    );
}

#[test]
fn named_identity_requires_its_exact_activation_owner_and_admits_reads_after_registration() {
    let fixture = Fixture::new();
    let (name, session) = named(&fixture, "CON");
    let (_, other) = named(&fixture, "other");
    assert!(Rendezvous::from_runtime_directory(session.runtime_directory()).is_err());
    assert_eq!(
        fs::read_dir(session.runtime_directory()).unwrap().count(),
        0
    );
    let other_lock = activation(&other);
    assert_eq!(
        session
            .register_session_name(&other_lock, &name)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
    let lifetime = match session.try_acquire_session_lifetime_lock().unwrap() {
        LockAttempt::Acquired(lock) => lock,
        LockAttempt::Contended => panic!("unexpected lifetime owner"),
    };
    assert!(session.register_session_name(&lifetime, &name).is_err());
    let lock = activation(&session);
    session.register_session_name(&lock, &name).unwrap();
    let bytes = fs::read(session.runtime_directory().join("session-name.json")).unwrap();
    session.register_session_name(&lock, &name).unwrap();
    assert_eq!(
        fs::read(session.runtime_directory().join("session-name.json")).unwrap(),
        bytes
    );
    assert!(
        session
            .register_session_name(&lock, &SessionName::parse("other").unwrap())
            .is_err()
    );
    let admitted = Rendezvous::from_runtime_directory(session.runtime_directory()).unwrap();
    assert_eq!(admitted.client_socket_path(), session.client_socket_path());
    assert_ne!(admitted.client_socket_path(), other.client_socket_path());
}

#[test]
fn malformed_identity_is_never_replaced_or_accepted() {
    let fixture = Fixture::new();
    let (name, session) = named(&fixture, "work");
    let lock = activation(&session);
    let path = session.runtime_directory().join("session-name.json");
    for bytes in [
        b"".as_slice(),
        b"{",
        b"{\"version\":1,\"name\":\"other\"}",
        b"{\"version\":2,\"name\":\"work\"}",
        b"{\"version\":1,\"name\":\"work\",\"extra\":true}",
        b"{\"version\":1,\"name\":\"work\",\"name\":\"work\"}",
        &[b' '; 257],
    ] {
        fs::write(&path, bytes).unwrap();
        assert!(session.register_session_name(&lock, &name).is_err());
        assert!(Rendezvous::from_runtime_directory(session.runtime_directory()).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}
