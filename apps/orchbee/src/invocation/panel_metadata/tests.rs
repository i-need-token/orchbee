use super::*;
fn parse_args(args: &[&str]) -> Result<Request, ParseFailure> {
    super::super::parse_request(args.iter().map(std::ffi::OsString::from))
}
#[test]
fn agent_guard_is_normalized_losslessly_or_rejected_before_admission() {
    let report = |agent: &str| {
        parse_args(&[
            "panel",
            "report-metadata",
            "@panel:abc",
            "--source=hook",
            "--title=x",
            "--agent",
            agent,
        ])
    };
    for (raw, expected) in [
        (" CLAUDE-Code.exe ", "claude"),
        ("自定义 Agent", "自定义 Agent"),
    ] {
        let Request::PanelMetadata { command, .. } = report(raw).unwrap() else {
            panic!("report")
        };
        assert_eq!(command.patch.agent.unwrap().as_str(), expected);
    }
    let boundary = "界".repeat(80);
    let Request::PanelMetadata { command, .. } = report(&boundary).unwrap() else {
        panic!("report")
    };
    assert_eq!(command.patch.agent.unwrap().as_str(), boundary);
    for raw in [
        "".to_owned(),
        " \t".to_owned(),
        "foo\tbar".to_owned(),
        "界".repeat(81),
    ] {
        assert!(report(&raw).is_err(), "{raw:?}");
    }
}

#[test]
fn whole_report_normalization_and_conflicts_precede_effects() {
    let Request::PanelMetadata { command, .. } = parse_args(&[
        "panel",
        "report-metadata",
        "@panel:abc",
        "--source= hook:agent ",
        "--title=  标题\t ",
        "--display-agent= Agent ",
        "--state-label= WORKING =忙碌",
        "--state-label=working=更新",
        "--token=x=y",
        "--clear-token=x",
        "--seq=18446744073709551615",
        "--ttl-ms=86400000",
    ])
    .unwrap() else {
        panic!("report")
    };
    assert_eq!(command.patch.title.unwrap().as_str(), "标题");
    assert_eq!(
        command.patch.state_labels[&MetadataState::Working].as_str(),
        "更新"
    );
    assert!(
        command
            .patch
            .tokens
            .unwrap()
            .as_map()
            .values()
            .all(Option::is_none)
    );
    for tail in [
        vec![],
        vec!["--agent=codex"],
        vec!["--title= "],
        vec!["--state-label=working="],
        vec!["--state-label=nope=text"],
        vec!["--title=x", "--clear-title"],
        vec!["--title=", "--clear-title"],
        vec!["--display-agent= ", "--clear-display-agent"],
        vec!["--display-agent=x", "--clear-display-agent"],
        vec!["--state-label=done=x", "--clear-state-labels"],
        vec!["--clear-title", "--clear-title"],
        vec!["--title=x", "--seq=-1"],
        vec!["--clear-title", "--ttl-ms=0"],
        vec!["--title=x", "--focus"],
        vec!["--title=x", "--client=00000000000000000000000000000001"],
        vec!["--title=x", "--applies-to-source=bad source"],
    ] {
        let args = [
            vec!["panel", "report-metadata", "@panel:abc", "--source=hook"],
            tail,
        ]
        .concat();
        assert!(parse_args(&args).is_err(), "{args:?}");
    }
    for flag in [
        "--clear-title",
        "--clear-display-agent",
        "--clear-state-labels",
    ] {
        assert!(
            parse_args(&[
                "panel",
                "report-metadata",
                "@panel:abc",
                "--source=hook",
                flag
            ])
            .is_ok()
        );
    }
    for args in [
        vec!["panel", "report-metadata", "@panel:abc", "--title=x"],
        vec!["panel", "get", "@panel:abc", "--title=x"],
        vec![
            "workspace",
            "report-metadata",
            "main",
            "--source=hook",
            "--token=x=y",
            "--title=x",
        ],
        vec!["--title=x", "--help"],
    ] {
        assert!(parse_args(&args).is_err(), "{args:?}");
    }
}
