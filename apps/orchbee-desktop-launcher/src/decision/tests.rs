//! The window launcher's decision over the window pointer, its own arguments, the parent check and
//! which programs exist, and its step after each start.
use super::{Failure, Next, Plan, Start, Watched, after, plan};
use std::{
    cell::RefCell,
    ffi::OsString,
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
};

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const LAUNCHER: &str = r"C:\Store\desktop\orchbee-desktop.exe";
const PARENT: u32 = 4242;

fn pointer(active: &str, previous: Option<&str>) -> String {
    let previous = previous
        .map(|product| format!(r#","previous":{{"product":"{product}","manifest":"{DIGEST}"}}"#))
        .unwrap_or_default();
    format!(r#"{{"version":1,"active":{{"product":"{active}","manifest":"{DIGEST}"}}{previous}}}"#)
}

fn strings(arguments: &[&str]) -> Vec<OsString> {
    arguments.iter().map(OsString::from).collect()
}

fn start(version: &str, arguments: &[&str], announced: bool) -> Start {
    let folder = PathBuf::from(format!(r"C:\Store\desktop\versions\{version}"));
    Start {
        window: folder.join("orchbee-desktop.exe"),
        folder,
        arguments: strings(arguments),
        announced,
    }
}

/// Decides for `LAUNCHER` over `bytes` as its pointer, with `PARENT` as its confirmed parent and
/// the window programs of `present` versions on disk; records which pointer the launcher asked for.
fn decide(
    bytes: Option<&str>,
    arguments: Vec<OsString>,
    present: &[&str],
) -> Result<Plan, Failure> {
    let asked = RefCell::new(Vec::new());
    let decision = plan(
        Path::new(LAUNCHER),
        |path| {
            asked.borrow_mut().push(path.to_path_buf());
            bytes.map(|text| text.as_bytes().to_vec())
        },
        arguments,
        Some(PARENT),
        |path| {
            present
                .iter()
                .any(|version| path == start(version, &[], false).window)
        },
    );
    assert_eq!(
        asked.into_inner(),
        [PathBuf::from(r"C:\Store\desktop\active.json")]
    );
    decision
}

#[test]
fn the_active_version_starts_with_the_launchers_own_name_folder_and_arguments() {
    for (bytes, present, plan) in [
        (
            pointer("0.2.0", None),
            &["0.2.0"][..],
            Plan {
                wait_for: None,
                first: start("0.2.0", &["--flag", "a b"], false),
                fallback: None,
            },
        ),
        // A previous version whose program exists is the fallback, and the active start is
        // announced and watched; the fallback names the active version first and is not announced.
        (
            pointer("0.3.0-beta.2", Some("0.2.0")),
            &["0.3.0-beta.2", "0.2.0"][..],
            Plan {
                wait_for: None,
                first: start("0.3.0-beta.2", &["--flag", "a b"], true),
                fallback: Some(start(
                    "0.2.0",
                    &["--fallback-from-v1=0.3.0-beta.2", "--flag", "a b"],
                    false,
                )),
            },
        ),
        // The fallback stands in when the active program is missing; the start reports the rest.
        (
            pointer("0.3.0", Some("0.2.0")),
            &["0.2.0"][..],
            Plan {
                wait_for: None,
                first: start("0.3.0", &["--flag", "a b"], true),
                fallback: Some(start(
                    "0.2.0",
                    &["--fallback-from-v1=0.3.0", "--flag", "a b"],
                    false,
                )),
            },
        ),
        // A previous version without its program, or naming the active one, is no fallback.
        (
            pointer("0.3.0", Some("0.2.0")),
            &["0.3.0"][..],
            Plan {
                wait_for: None,
                first: start("0.3.0", &["--flag", "a b"], false),
                fallback: None,
            },
        ),
        (
            pointer("0.3.0", Some("0.3.0")),
            &["0.3.0"][..],
            Plan {
                wait_for: None,
                first: start("0.3.0", &["--flag", "a b"], false),
                fallback: None,
            },
        ),
    ] {
        assert_eq!(
            decide(Some(&bytes), strings(&["--flag", "a b"]), present),
            Ok(plan),
            "{bytes}"
        );
    }
}

#[test]
fn nothing_starts_without_an_active_version_or_any_program() {
    let pointer_path = PathBuf::from(r"C:\Store\desktop\active.json");
    for bytes in [
        None,
        Some(String::new()),
        Some(pointer("0.2.0", None)[..30].to_owned()),
        Some(pointer("0.2.0", None).replace(r#""version":1"#, r#""version":2"#)),
        Some(pointer(r"..\\..\\Windows", None)),
        Some(pointer("0.2.0+build.7", None)),
        Some(pointer("0.2.0", Some("0.1"))),
    ] {
        assert_eq!(
            decide(bytes.as_deref(), Vec::new(), &["0.2.0"]),
            Err(Failure::NoActiveVersion(pointer_path.clone())),
            "{bytes:?}"
        );
    }
    let missing = start("0.3.0", &[], false).window;
    for bytes in [pointer("0.3.0", None), pointer("0.3.0", Some("0.2.0"))] {
        assert_eq!(
            decide(Some(&bytes), Vec::new(), &[]),
            Err(Failure::ClientUnavailable(missing.clone())),
            "{bytes}"
        );
    }
}

#[test]
fn only_a_leading_relaunch_switch_is_the_launchers_and_it_waits_only_for_its_parent() {
    let parent = PARENT.to_string();
    let other = (PARENT + 1).to_string();
    let invalid = OsString::from_wide(&[0xD800]);
    let cases: [(Vec<OsString>, Option<u32>, Vec<OsString>); 12] = [
        (
            strings(&["--relaunch-after-v1", &parent, "--select-workspace-v1=w"]),
            Some(PARENT),
            strings(&["--select-workspace-v1=w"]),
        ),
        (
            strings(&["--relaunch-after-v1", &parent]),
            Some(PARENT),
            Vec::new(),
        ),
        // Another process, or a value that names none, is dropped with the switch and not waited for.
        (
            strings(&["--relaunch-after-v1", &other, "x"]),
            None,
            strings(&["x"]),
        ),
        (
            strings(&["--relaunch-after-v1", "0", "x"]),
            None,
            strings(&["x"]),
        ),
        (
            strings(&["--relaunch-after-v1", "04242", "x"]),
            None,
            strings(&["x"]),
        ),
        (
            strings(&["--relaunch-after-v1", "+4242", "x"]),
            None,
            strings(&["x"]),
        ),
        (
            strings(&["--relaunch-after-v1", "4294967296", "x"]),
            None,
            strings(&["x"]),
        ),
        (
            strings(&["--relaunch-after-v1", "", "x"]),
            None,
            strings(&["x"]),
        ),
        (
            vec![OsString::from("--relaunch-after-v1"), invalid.clone()],
            None,
            Vec::new(),
        ),
        (strings(&["--relaunch-after-v1"]), None, Vec::new()),
        // Not the exact switch in first place: an ordinary argument.
        (
            strings(&[&format!("--relaunch-after-v1={PARENT}"), "x"]),
            None,
            strings(&[&format!("--relaunch-after-v1={PARENT}"), "x"]),
        ),
        (
            vec![
                invalid.clone(),
                OsString::from("--relaunch-after-v1"),
                OsString::from(&parent),
            ],
            None,
            vec![
                invalid,
                OsString::from("--relaunch-after-v1"),
                OsString::from(&parent),
            ],
        ),
    ];
    for (arguments, wait_for, rest) in cases {
        let label = format!("{arguments:?}");
        assert_eq!(
            decide(Some(&pointer("0.2.0", None)), arguments, &["0.2.0"]),
            Ok(Plan {
                wait_for,
                first: Start {
                    arguments: rest,
                    ..start("0.2.0", &[], false)
                },
                fallback: None,
            }),
            "{label}"
        );
    }
    // A launcher whose parent is unknown waits for nobody.
    let unknown = plan(
        Path::new(LAUNCHER),
        |_| Some(pointer("0.2.0", None).into_bytes()),
        strings(&["--relaunch-after-v1", &parent]),
        None,
        |_| true,
    );
    assert_eq!(unknown.map(|plan| plan.wait_for), Ok(None));
}

#[test]
fn a_watched_failure_gives_way_to_the_previous_version_once() {
    for (watched, fallback, next) in [
        (Watched::NotStarted, true, Next::Fallback),
        (Watched::Ended(1), true, Next::Fallback),
        // Access violation, as a crashed window ends.
        (
            Watched::Ended(0xC000_0005_u32.cast_signed()),
            true,
            Next::Fallback,
        ),
        // A second instance hands its command line to the running window and ends with 0.
        (Watched::Ended(0), true, Next::Done),
        (Watched::Running, true, Next::Done),
        // Without a fallback, or after it, a start that stands or ran is the end.
        (Watched::NotStarted, false, Next::Unavailable),
        (Watched::Ended(1), false, Next::Done),
        (Watched::Ended(0), false, Next::Done),
        (Watched::Running, false, Next::Done),
    ] {
        assert_eq!(after(watched, fallback), next, "{watched:?} {fallback}");
    }
}
