use super::{Build, UpdateControl, control, joins};
use orchbee_connection_handshake::{Generation, GenerationRange};
use orchbee_local_runtime::EnvironmentKind;

const RELEASE: Build = Build {
    live: true,
    kind: EnvironmentKind::Release,
    store: true,
};

fn ready(version: &str) -> UpdateControl {
    UpdateControl::Ready {
        version: version.to_owned(),
    }
}

/// Ready exactly when the store names a version newer than this Client's own, by SemVer
/// precedence, in a build that can offer updates.
#[test]
fn the_control_is_ready_only_for_a_newer_active_version() {
    assert_eq!(control(RELEASE, "0.2.0", Some("0.3.0")), ready("0.3.0"));
    assert_eq!(control(RELEASE, "0.2.0", Some("0.10.0")), ready("0.10.0"));
    assert_eq!(
        control(RELEASE, "0.2.0-beta.4", Some("0.2.0")),
        ready("0.2.0")
    );
    for active in [Some("0.2.0"), Some("0.1.9"), Some("0.2.0-beta.9"), None] {
        assert_eq!(
            control(RELEASE, "0.2.0", active),
            UpdateControl::Absent,
            "{active:?}"
        );
    }
}

/// With the automatic-update switch off, in a development build and where no update writes the store the
/// control never shows, whatever the store says; a test build shows it as a release build does.
#[test]
fn the_control_is_absent_where_the_build_offers_no_update() {
    for build in [
        Build {
            live: false,
            ..RELEASE
        },
        Build {
            kind: EnvironmentKind::Development,
            ..RELEASE
        },
        Build {
            store: false,
            ..RELEASE
        },
    ] {
        assert_eq!(
            control(build, "0.2.0", Some("0.3.0")),
            UpdateControl::Absent,
            "{build:?}"
        );
    }
    let test = Build {
        kind: EnvironmentKind::Test,
        ..RELEASE
    };
    assert_eq!(control(test, "0.2.0", Some("0.3.0")), ready("0.3.0"));
}

fn generation(value: u16) -> Generation {
    Generation::new(value).unwrap()
}

fn range(lowest: u16, highest: u16) -> GenerationRange {
    GenerationRange::new(generation(lowest), generation(highest)).unwrap()
}

/// The new Client joins only a Session that carries the marker and whose negotiated generation
/// the active manifest's `client` component lists.
#[test]
fn the_join_check_needs_the_marker_and_a_listed_generation() {
    assert!(joins(true, Some(generation(2)), Some(range(1, 3))));
    assert!(joins(true, Some(generation(1)), Some(range(1, 1))));
    // A Session before the first automatic-update release.
    assert!(!joins(false, Some(generation(2)), Some(range(1, 3))));
    // The new Client dropped the negotiated generation, or does not reach it yet.
    assert!(!joins(true, Some(generation(1)), Some(range(2, 3))));
    assert!(!joins(true, Some(generation(4)), Some(range(2, 3))));
    // An unreadable manifest, or no promoted connection, never joins.
    assert!(!joins(true, Some(generation(2)), None));
    assert!(!joins(true, None, Some(range(1, 3))));
}
