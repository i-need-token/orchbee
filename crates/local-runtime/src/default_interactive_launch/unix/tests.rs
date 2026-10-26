use std::{ffi::OsStr, os::unix::ffi::OsStrExt, path::Path};

use super::{
    FALLBACK_SHELL, MAXIMUM_SHELL_PATH_BYTES, admissible_shell_path, executable_is_usable,
    observe_usable_shell,
};
use crate::default_interactive_launch::resolve_default_interactive_launch;

#[test]
fn candidate_admission_is_absolute_utf8_nul_free_and_bounded() {
    let maximum = format!("/{}", "a".repeat(MAXIMUM_SHELL_PATH_BYTES - 1));
    let oversized = format!("/{maximum}");
    let non_utf8 = OsStr::from_bytes(b"/shell/\xff");

    for rejected in [
        OsStr::new(""),
        OsStr::new("relative/shell"),
        OsStr::from_bytes(b"/shell/with\0nul"),
        oversized.as_ref(),
        non_utf8,
    ] {
        assert_eq!(admissible_shell_path(rejected), None);
    }
    assert_eq!(
        admissible_shell_path(OsStr::new(&maximum)),
        Some(maximum.as_str())
    );
}

#[test]
fn invalid_candidates_never_reach_the_executable_probe() {
    for candidate in [
        OsStr::new("relative/inherited"),
        OsStr::from_bytes(b"/account/\xff"),
    ] {
        assert_eq!(
            observe_usable_shell(Some(candidate), |_path| {
                panic!("an inadmissible candidate reached the executable probe")
            }),
            None
        );
    }
}

#[test]
fn admissible_but_unusable_candidates_are_removed_before_selection() {
    assert_eq!(
        observe_usable_shell(Some(OsStr::new("/shell/inherited")), |path| {
            assert_eq!(path, Path::new("/shell/inherited"));
            false
        }),
        None
    );
}

#[test]
fn native_executable_probe_requires_an_executable_regular_file() {
    assert!(executable_is_usable(Path::new(FALLBACK_SHELL)));
    assert!(!executable_is_usable(Path::new("/bin")));
    assert!(!executable_is_usable(Path::new("/etc/passwd")));
    assert!(!executable_is_usable(Path::new(
        "/orchbee-test-path-that-must-not-exist/default-shell"
    )));
}

#[test]
fn native_resolution_returns_an_admissible_usable_launch() {
    let launch = resolve_default_interactive_launch()
        .expect("the supported POSIX target provides at least /bin/sh");

    assert_eq!(
        admissible_shell_path(OsStr::new(&launch.executable)),
        Some(launch.executable.as_str())
    );
    assert!(executable_is_usable(Path::new(&launch.executable)));
    assert!(launch.arguments.is_empty());
    assert_eq!(launch.environment.len(), 3);
}
