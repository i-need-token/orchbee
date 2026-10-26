use super::{RendezvousError, SessionScope};

#[test]
fn session_scope_accepts_one_canonical_bounded_component() -> Result<(), RendezvousError> {
    for valid in [
        "orchbee-release",
        "orchbee-local",
        "orchbee-dev-a1b2c3",
        "a",
        &"a".repeat(63),
    ] {
        assert_eq!(SessionScope::parse(valid)?.as_str(), valid);
    }

    for invalid in [
        "",
        "1orchbee",
        "Orchbee",
        "orchbee_dev",
        "orchbee--dev",
        "orchbee-",
        ".",
        "..",
        "orchbee/dev",
        &format!("a{}", "0".repeat(63)),
    ] {
        assert!(matches!(
            SessionScope::parse(invalid),
            Err(RendezvousError::InvalidSessionScope)
        ));
    }
    Ok(())
}
