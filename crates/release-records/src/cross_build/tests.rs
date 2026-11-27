use super::CrossBuildName;

const NAME: CrossBuildName = CrossBuildName {
    current: "--complete-install",
    earlier: "--complete-install-v1",
};

// Releases up to 0.1.0-beta.24 answer only the earlier spelling and 0.1.0-beta.25 only the current
// one, so the boundary decides whether every update from those releases can complete.
#[test]
fn only_releases_up_to_beta_24_receive_the_earlier_spelling() {
    for version in [
        "0.1.0-beta.1",
        "0.1.0-beta.9",
        "0.1.0-beta.10",
        "0.1.0-beta.24",
    ] {
        assert_eq!(NAME.for_release(version), NAME.earlier, "{version}");
    }
    for version in [
        "0.1.0-beta.25",
        "0.1.0-beta.26",
        "0.1.0-beta.0",
        "0.1.0-beta.01",
        "0.1.0-beta.240",
        "0.1.0-beta.24.1",
        "0.1.0-beta.2a",
        "0.1.0-beta.+1",
        "0.1.0",
        "0.1.1-beta.3",
        "",
    ] {
        assert_eq!(NAME.for_release(version), NAME.current, "{version}");
    }
}

#[test]
fn a_receiver_accepts_both_spellings_and_nothing_else() {
    assert!(NAME.is("--complete-install"));
    assert!(NAME.is("--complete-install-v1"));
    for other in [
        "--complete-install-v2",
        "--complete-install=",
        "complete-install",
        "",
    ] {
        assert!(!NAME.is(other), "{other}");
    }
}
