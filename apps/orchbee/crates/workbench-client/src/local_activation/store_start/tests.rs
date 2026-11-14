//! The store start rule over explicit facts. No real run enumerates these inputs, and a wrong
//! branch silently brings back the setup program's in-use refusal or crosses environments.
use super::{Decision, Own, decide};
use orchbee_local_runtime::{ActiveRelease, CohortArtifact, EnvironmentKind};
use std::{
    io,
    path::{Path, PathBuf},
};

const OWN_VERSION: &str = "0.1.0-beta.3";

fn root() -> PathBuf {
    PathBuf::from(r"C:\Users\person\AppData\Local\Programs\store")
}

/// An outside Client's folder, such as the one setup keeps for the window.
fn outside() -> PathBuf {
    root().join(r"desktop\resources\cohort")
}

fn active(version: &str) -> impl FnOnce(&Path) -> io::Result<Option<ActiveRelease>> {
    move |root| {
        Ok(Some(ActiveRelease {
            version: version.to_owned(),
            directory: root.join("versions").join(version),
        }))
    }
}

fn decide_with(
    kind: EnvironmentKind,
    own_directory: &Path,
    active: impl FnOnce(&Path) -> io::Result<Option<ActiveRelease>>,
) -> Decision {
    decide(
        kind,
        own_directory,
        OWN_VERSION,
        || Ok(root()),
        active,
        |_| true,
    )
}

#[test]
fn a_development_build_never_hands_a_start_to_the_store() {
    assert_eq!(
        decide_with(EnvironmentKind::Development, &outside(), active("9.0.0")),
        Decision::Own(Own::Development)
    );
}

#[test]
fn a_client_inside_a_store_never_hands_a_start_on() {
    for kind in [EnvironmentKind::Release, EnvironmentKind::Test] {
        for own in [
            root().join(r"versions\0.1.0-beta.3"),
            PathBuf::from(r"D:\elsewhere\Versions\0.1.0"),
        ] {
            assert_eq!(
                decide_with(kind, &own, active("9.0.0")),
                Decision::Own(Own::StoreClient),
                "{}",
                own.display()
            );
        }
    }
}

#[test]
fn without_a_pointer_the_client_starts_its_own_cohort() {
    assert_eq!(
        decide_with(EnvironmentKind::Test, &outside(), |_| Ok(None)),
        Decision::Own(Own::NoPointer)
    );
}

#[test]
fn a_store_lower_than_the_client_is_not_used() {
    for lower in ["0.1.0-beta.2", "0.0.9"] {
        assert_eq!(
            decide_with(EnvironmentKind::Release, &outside(), active(lower)),
            Decision::Own(Own::StoreLower),
            "{lower}"
        );
    }
}

#[test]
fn an_equal_or_higher_store_starts_with_its_own_client() {
    for version in [OWN_VERSION, "0.1.0-beta.4", "0.2.0"] {
        for kind in [EnvironmentKind::Release, EnvironmentKind::Test] {
            let expected = root()
                .join("versions")
                .join(version)
                .join(CohortArtifact::Client.file_name());
            assert_eq!(
                decide_with(kind, &outside(), active(version)),
                Decision::Store(expected),
                "{version}"
            );
        }
    }
}
