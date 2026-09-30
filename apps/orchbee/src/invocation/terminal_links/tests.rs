use super::*;
use crate::invocation::machine_routing;
fn args(words: &[&str]) -> Result<Request, ParseFailure> {
    crate::invocation::parse_request(words.iter().map(std::ffi::OsString::from))
}
const ID: &str = "11111111111111111111111111111111";
#[test]
fn link_activate_parser_shares_fences_and_remote_effect_admission() {
    let Request::TerminalLink { command, format } = args(&[
        "terminal",
        "link-activate",
        ID,
        "--cursor=0:2",
        "--offset-from-bottom=1",
        "--content-revision=0",
        "--generation=1",
        "--format=json",
    ])
    .unwrap() else {
        panic!()
    };
    assert!(command.activate);
    assert_eq!(
        (command.offset, command.revision, command.generation),
        (1, Some(0), Some(1))
    );
    assert_eq!(
        machine_routing::admit(&Request::TerminalLink { command, format }),
        Some((OutputFormat::Json, None))
    );
    assert!(matches!(
        args(&[
            "--machine",
            "remote",
            "terminal",
            "link-activate",
            ID,
            "--cursor",
            "0:0"
        ]),
        Ok(Request::MachineRoute(_))
    ));
}
#[test]
fn link_activate_rejects_malformed_points_and_foreign_or_duplicate_flags_before_effects() {
    for extra in [
        vec![],
        vec!["--cursor", "65536:0"],
        vec!["--cursor", "0:0", "--context", "{}"],
        vec!["--cursor", "0:0", "--client", ID],
        vec!["--cursor=0:0", "--cursor=0:0"],
        vec!["--cursor", "0:0", "--generation", "0"],
    ] {
        let mut argv = vec!["terminal", "link-activate", ID];
        argv.extend(extra);
        assert!(args(&argv).is_err(), "{argv:?}");
    }
}
#[test]
fn link_resolve_parser_preserves_viewport_coordinates_full_u64_fences_and_machine_route() {
    let Request::TerminalLink { command, format } = args(&[
        "terminal",
        "link-resolve",
        ID,
        "--cursor",
        "65535:65535",
        "--offset-from-bottom",
        "18446744073709551615",
        "--content-revision",
        "18446744073709551615",
        "--generation",
        "18446744073709551615",
        "--format",
        "json",
    ])
    .unwrap() else {
        panic!("link read")
    };
    assert_eq!(
        command.cursor,
        TerminalTextPoint {
            row: 65535,
            col: 65535
        }
    );
    assert_eq!(
        (command.offset, command.revision, command.generation),
        (u64::MAX, Some(u64::MAX), Some(u64::MAX))
    );
    assert_eq!(format, OutputFormat::Json);
    assert_eq!(
        machine_routing::admit(&Request::TerminalLink { command, format }),
        Some((format, None))
    );
    let Request::TerminalLink { command, .. } =
        args(&["terminal", "link-resolve", ID, "--cursor", "0:0"]).unwrap()
    else {
        panic!("link defaults")
    };
    assert_eq!(
        (command.offset, command.revision, command.generation),
        (0, None, None)
    );
}
#[test]
fn link_resolve_parser_refuses_missing_malformed_duplicate_and_foreign_arguments() {
    for cursor in ["65536:0", "4294967296:0", "0:65536", "-1:0", "0"] {
        let failure = args(&["terminal", "link-resolve", ID, "--cursor", cursor]).unwrap_err();
        assert!(
            matches!(
                failure.usage,
                Some(crate::invocation::UsageReason::InvalidValue {
                    expected: ExpectedValue::TerminalViewportPoint,
                    ..
                })
            ),
            "{cursor}"
        );
    }
    for extra in [
        vec![],
        vec!["--cursor", "65536:0"],
        vec!["--cursor", "0:65536"],
        vec!["--cursor", "-1:0"],
        vec!["--cursor", "0"],
        vec![
            "--cursor",
            "0:0",
            "--offset-from-bottom",
            "18446744073709551616",
        ],
        vec!["--cursor", "0:0", "--offset-from-bottom", "-1"],
        vec!["--cursor", "0:0", "--generation", "0"],
        vec!["--cursor", "0:0", "--motion", "line-end"],
        vec!["--cursor", "0:0", "--cursor", "1:1"],
        vec![
            "--cursor",
            "0:0",
            "--offset-from-bottom",
            "1",
            "--offset-from-bottom=1",
        ],
    ] {
        let mut argv = vec!["terminal", "link-resolve", ID];
        argv.extend(extra);
        assert!(args(&argv).is_err(), "{argv:?}");
    }
    assert!(args(&["terminal", "read", ID, "--offset-from-bottom", "0"]).is_err());
    assert!(args(&["terminal", "link-resolve", "not-id", "--cursor", "0:0"]).is_err());
}
