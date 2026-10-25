use super::*;
fn command(args: &[&str]) -> Result<Request, ParseFailure> {
    super::super::parse_request(args.iter().map(std::ffi::OsString::from))
}
#[test]
fn panel_relocation_destinations_defaults_and_ratio_follow_reference() {
    let Request::PanelRelocation {
        command: parsed, ..
    } = command(&[
        "panel",
        "move",
        "00000000000000000000000000000001",
        "--new-tab",
    ])
    .unwrap()
    else {
        panic!()
    };
    assert!(parsed.focus);
    assert!(matches!(
        parsed.destination,
        PanelRelocationDestination::NewTab {
            context: None,
            label: None
        }
    ));
    let Request::PanelRelocation {
        command: parsed, ..
    } = command(&[
        "panel",
        "move",
        "00000000000000000000000000000001",
        "--into-tab",
        "target",
        "--target-panel",
        "00000000000000000000000000000002",
        "--direction",
        "right",
        "--ratio",
        "2",
        "--no-focus",
    ])
    .unwrap()
    else {
        panic!()
    };
    assert!(!parsed.focus);
    assert_eq!(parsed.ratio(), ClientSplitRatio::new(0.9).ok());
    for args in [
        vec!["--new-tab", "--workspace", "other"],
        vec![
            "--new-workspace",
            "--label",
            "工作",
            "--tab-label",
            "shell",
            "--project",
            "project",
        ],
    ] {
        let mut words = vec!["panel", "move", "00000000000000000000000000000001"];
        words.extend(args);
        assert!(matches!(
            command(&words).unwrap(),
            Request::PanelRelocation { .. }
        ));
    }
}
#[test]
fn panel_relocation_rejects_conflicts_before_effects_and_keeps_adjacent() {
    for tail in [
        vec!["--new-tab", "--new-workspace"],
        vec!["--new-tab", "--ratio", "0.3"],
        vec!["--new-workspace", "--workspace", "other"],
        vec!["--new-tab", "--tab-label", "other"],
        vec!["--into-tab", "t", "--direction", "right", "--ratio", "NaN"],
        vec!["--new-tab", "--focus", "--no-focus"],
        vec!["--new-tab", "--standalone", "--workspace", "w"],
    ] {
        let mut words = vec!["panel", "move", "00000000000000000000000000000001"];
        words.extend(tail);
        assert!(command(&words).is_err(), "{words:?}");
    }
    assert!(matches!(
        command(&[
            "panel",
            "move",
            "00000000000000000000000000000001",
            "--target-panel",
            "00000000000000000000000000000002",
            "--direction",
            "left"
        ])
        .unwrap(),
        Request::Mutation { .. }
    ));
}
