//! The macOS window launcher serves only a relaunch its parent asked for, and replaces only an
//! application outside the store; a hostile or malformed request must never name a folder for the
//! renames and the removal that follow.
use super::{Relaunch, relaunch};
use std::{ffi::OsString, path::PathBuf};

const DESKTOP: &str = "/Users/a/.local/share/orchbee/desktop";
const LAUNCHER: &str = "/Users/a/.local/share/orchbee/desktop/versions/0.3.0/Orchbee.app/Contents/Resources/cohort/orchbee-desktop-launcher";
const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const PARENT: u32 = 4242;

fn strings(arguments: &[&str]) -> Vec<OsString> {
    arguments.iter().map(OsString::from).collect()
}

fn asked(application: &str, rest: &[&str]) -> Vec<OsString> {
    let mut arguments = strings(&[
        "--relaunch-after",
        &PARENT.to_string(),
        &format!("--replace-application={application}"),
    ]);
    arguments.extend(strings(rest));
    arguments
}

#[test]
fn a_relaunch_names_the_application_the_program_and_the_place_switches() {
    let served = relaunch(
        LAUNCHER.as_ref(),
        asked("/Applications/Orchbee.app", &["--select-workspace=w"]),
        PARENT,
    )
    .expect("served");
    assert_eq!(served.wait_for, PARENT);
    assert_eq!(
        served.application,
        PathBuf::from("/Applications/Orchbee.app")
    );
    assert_eq!(served.program, PathBuf::from("Contents/MacOS/Orchbee"));
    assert_eq!(served.arguments, strings(&["--select-workspace=w"]));
    assert_eq!(served.pointer(), PathBuf::from(DESKTOP).join("active.json"));
    assert_eq!(
        served.fallback_arguments(Some("0.3.0")),
        strings(&["--fallback-from=0.3.0", "--select-workspace=w"])
    );
    assert_eq!(
        served.fallback_arguments(None),
        strings(&["--select-workspace=w"])
    );
}

#[test]
fn nothing_is_served_for_another_parent_a_misplaced_launcher_or_an_unsafe_application() {
    let other = (PARENT + 1).to_string();
    let refused: [(&str, Vec<OsString>); 12] = [
        (
            LAUNCHER,
            strings(&[
                "--relaunch-after",
                &other,
                "--replace-application=/Applications/Orchbee.app",
            ]),
        ),
        (
            LAUNCHER,
            strings(&["--replace-application=/Applications/Orchbee.app"]),
        ),
        // The application switch only right after the relaunch pair.
        (
            LAUNCHER,
            strings(&[
                "--relaunch-after",
                &PARENT.to_string(),
                "x",
                "--replace-application=/Applications/Orchbee.app",
            ]),
        ),
        (LAUNCHER, asked("Applications/Orchbee.app", &[])),
        (LAUNCHER, asked("/Applications/Orchbee", &[])),
        (LAUNCHER, asked("/Applications/../Orchbee.app", &[])),
        (LAUNCHER, asked("/Applications/./Orchbee.app", &[])),
        (LAUNCHER, asked("", &[])),
        // The store's own copies are never replaced, in any letter case.
        (
            LAUNCHER,
            asked(&format!("{DESKTOP}/versions/0.2.0/Orchbee.app"), &[]),
        ),
        (
            LAUNCHER,
            asked(
                &format!("{}/versions/0.2.0/Orchbee.app", DESKTOP.to_uppercase()),
                &[],
            ),
        ),
        // A launcher outside a window version's bundle in the store serves nothing.
        (
            "/Applications/Orchbee.app/Contents/Resources/cohort/orchbee-desktop-launcher",
            asked("/Applications/Orchbee.app", &[]),
        ),
        (
            "/Users/a/.local/share/orchbee/desktop/versions/latest/Orchbee.app/Contents/Resources/cohort/orchbee-desktop-launcher",
            asked("/Applications/Orchbee.app", &[]),
        ),
    ];
    for (launcher, arguments) in refused {
        let label = format!("{launcher} {arguments:?}");
        assert_eq!(
            relaunch(launcher.as_ref(), arguments, PARENT),
            None,
            "{label}"
        );
    }
}

#[test]
fn the_source_is_the_active_versions_bundle_when_its_program_exists() {
    let served: Relaunch = relaunch(
        LAUNCHER.as_ref(),
        asked("/Users/a/Applications/Orchbee.app", &[]),
        PARENT,
    )
    .expect("served");
    let pointer =
        format!(r#"{{"version":1,"active":{{"product":"0.4.0","manifest":"{DIGEST}"}}}}"#);
    let bundle = PathBuf::from(DESKTOP).join("versions/0.4.0/Orchbee.app");
    let program = bundle.join("Contents/MacOS/Orchbee");
    let found = served
        .source(Some(pointer.clone().into_bytes()), |path| path == program)
        .expect("source");
    assert_eq!((found.version.as_str(), found.bundle), ("0.4.0", bundle));
    assert_eq!(served.source(Some(pointer.into_bytes()), |_| false), None);
    assert_eq!(served.source(None, |_| true), None);
    assert_eq!(served.source(Some(b"{}".to_vec()), |_| true), None);
}
