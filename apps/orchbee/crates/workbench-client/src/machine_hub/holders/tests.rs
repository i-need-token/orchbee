//! The holder decision of the incompatible-hub check: which processes named like the hub's program
//! may be clients attached to it, and what a Session read that cannot finish means.

use super::{Scan, decide};
use crate::machine_hub::SessionRead;

const STORE: &str = r"C:\Users\person\AppData\Local\Programs\Orchbee";
const HUB: u32 = 100;
const CALLER: u32 = 200;
const LAUNCHER: u32 = 199;
const SHELL: u32 = 150;

fn program(version: &str) -> String {
    format!(r"{STORE}\versions\{version}\orchbee.exe")
}

/// An installed release: the hub of version 0.1.0, and this client of version 0.2.0 started by the
/// store's launcher from a shell, with `others` named like the hub's program besides them.
fn release(others: &[(u32, Option<String>)]) -> Scan {
    let mut named = vec![
        (HUB, Some(program("0.1.0"))),
        (CALLER, Some(program("0.2.0"))),
        (LAUNCHER, Some(format!(r"{STORE}\bin\orchbee.exe"))),
    ];
    named.extend_from_slice(others);
    Scan {
        hub: HUB,
        hub_image: program("0.1.0"),
        caller: CALLER,
        caller_image: Some(program("0.2.0")),
        ancestors: vec![LAUNCHER, SHELL],
        named,
    }
}

#[test]
fn the_launcher_ancestors_and_this_version_never_hold() {
    let scan = release(&[
        // Another window of this version, and its own launcher, which is no ancestor of this one.
        (300, Some(program("0.2.0"))),
        (299, Some(format!(r"{STORE}\BIN\ORCHBEE.EXE"))),
        // An ancestor named like the hub's program, whatever it runs.
        (SHELL, Some(program("0.1.0"))),
    ]);
    assert!(!decide(SessionRead::NoneLive, Some(&scan)));
}

#[test]
fn a_run_of_another_version_holds() {
    for version in ["0.1.0", "0.1.5"] {
        let scan = release(&[(300, Some(program(version)))]);
        assert!(decide(SessionRead::NoneLive, Some(&scan)), "{version}");
    }
}

#[test]
fn a_process_that_cannot_be_classified_holds() {
    assert!(decide(
        SessionRead::NoneLive,
        Some(&release(&[(300, None)]))
    ));
    let mut unknown_self = release(&[(300, Some(program("0.2.0")))]);
    unknown_self.caller_image = None;
    assert!(decide(SessionRead::NoneLive, Some(&unknown_self)));
    assert!(decide(SessionRead::NoneLive, None));
}

#[test]
fn a_live_session_or_a_read_that_cannot_finish_holds() {
    let scan = release(&[]);
    assert!(!decide(SessionRead::NoneLive, Some(&scan)));
    assert!(decide(SessionRead::Live, Some(&scan)));
    assert!(decide(SessionRead::Unknown, Some(&scan)));
}

#[test]
fn outside_a_release_store_no_process_is_a_launcher() {
    let development = |image: &str| Scan {
        hub: HUB,
        hub_image: r"C:\checkout\cohort\ob-dev-client.exe".to_owned(),
        caller: CALLER,
        caller_image: Some(r"D:\other\cohort\ob-dev-client.exe".to_owned()),
        ancestors: Vec::new(),
        named: vec![(300, Some(image.to_owned()))],
    };
    assert!(decide(
        SessionRead::NoneLive,
        Some(&development(r"C:\checkout\bin\ob-dev-client.exe"))
    ));
    assert!(!decide(
        SessionRead::NoneLive,
        Some(&development(r"D:\other\cohort\ob-dev-client.exe"))
    ));
}
