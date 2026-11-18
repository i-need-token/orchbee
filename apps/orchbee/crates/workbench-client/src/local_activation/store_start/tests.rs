//! The store start rule over explicit facts. No real run enumerates these inputs, and a wrong
//! branch silently brings back the setup program's in-use refusal or crosses environments.
use super::{Decision, Own, decide};
use orchbee_local_runtime::{ActiveRelease, CohortArtifact, EnvironmentKind};
use std::{
    io,
    path::{Path, PathBuf},
};

const OWN_VERSION: &str = "0.1.0-beta.3";

#[cfg(windows)]
fn root() -> PathBuf {
    PathBuf::from(r"C:\Users\person\AppData\Local\Programs\store")
}

#[cfg(target_os = "macos")]
fn root() -> PathBuf {
    PathBuf::from("/Users/person/.local/share/store")
}

#[cfg(windows)]
fn elsewhere() -> PathBuf {
    PathBuf::from(r"D:\elsewhere")
}

#[cfg(target_os = "macos")]
fn elsewhere() -> PathBuf {
    PathBuf::from("/Volumes/elsewhere")
}

/// An outside Client's folder, such as the one setup keeps for the window.
#[cfg(windows)]
fn outside() -> PathBuf {
    root().join("desktop").join("resources").join("cohort")
}

/// An outside Client's folder, such as the window application's.
#[cfg(target_os = "macos")]
fn outside() -> PathBuf {
    PathBuf::from("/Applications/Orchbee.app/Contents/Resources/cohort")
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
    decide_installing(kind, own_directory, false, active)
}

fn decide_installing(
    kind: EnvironmentKind,
    own_directory: &Path,
    installs: bool,
    active: impl FnOnce(&Path) -> io::Result<Option<ActiveRelease>>,
) -> Decision {
    decide(
        kind,
        own_directory,
        OWN_VERSION,
        installs,
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
            root().join("versions").join("0.1.0-beta.3"),
            elsewhere().join("Versions").join("0.1.0"),
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

/// A store without an active version installs with the PATH step, and a store whose active version
/// is lower than this Client upgrades without repeating the profile step; an equal or higher store,
/// an unreadable pointer, and any build or folder that never hands a start on keep their decision.
#[test]
fn an_empty_or_lower_store_is_installed_into_when_allowed() {
    for kind in [EnvironmentKind::Release, EnvironmentKind::Test] {
        assert_eq!(
            decide_installing(kind, &outside(), true, |_| Ok(None)),
            Decision::Install { no_path: false }
        );
        assert_eq!(
            decide_installing(kind, &outside(), true, active("0.1.0-beta.2")),
            Decision::Install { no_path: true }
        );
        assert_eq!(
            decide_installing(kind, &outside(), true, |_| Err(
                io::ErrorKind::InvalidData.into()
            )),
            Decision::Own(Own::PointerUnreadable)
        );
        assert!(matches!(
            decide_installing(kind, &outside(), true, active(OWN_VERSION)),
            Decision::Store(_)
        ));
        assert!(matches!(
            decide_installing(kind, &outside(), true, active("0.1.0-beta.4")),
            Decision::Store(_)
        ));
        assert_eq!(
            decide_installing(
                kind,
                &root().join("versions").join(OWN_VERSION),
                true,
                |_| Ok(None)
            ),
            Decision::Own(Own::StoreClient)
        );
    }
    assert_eq!(
        decide_installing(EnvironmentKind::Development, &outside(), true, |_| Ok(None)),
        Decision::Own(Own::Development)
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
