use super::*;
use std::ffi::OsString;
fn parse(args: &[&str]) -> Result<Request, ParseFailure> {
    super::super::parse_request(args.iter().map(OsString::from))
}
fn report(args: &[&str]) -> PanelAgentReport {
    let Request::PanelAgent { command, .. } = parse(args).expect("report command") else {
        panic!("report");
    };
    command.report
}
#[test]
fn panel_agent_input_preserves_labels_messages_references_and_full_sequences() {
    let value = report(&[
        "panel",
        "report-agent",
        "@panel:0xabc",
        "--source=  custom source  ",
        "--agent=  C:/bin/CLAUDE.EXE  ",
        "--state=blocked",
        "--message=  问题\n\u{1b}[31m  ",
        "--seq=18446744073709551615",
        "--agent-session-id= opaque ",
        "--agent-session-path=relative",
    ]);
    assert_eq!(
        value,
        PanelAgentReport::State {
            source: "custom source".into(),
            agent: "claude".into(),
            state: PanelAgentState::Blocked,
            message: Some("  问题\n\u{1b}[31m  ".into()),
            sequence: Some(AgentReportSequence::new(u64::MAX)),
            agent_session_id: Some(" opaque ".into()),
            agent_session_path: Some("relative".into())
        }
    );
    let custom = " A custom\tAgent ";
    assert!(
        matches!(report(&["panel", "release-agent", "@panel:0xabc", "--source", "source",
        "--agent", custom, "--seq=+0007"]), PanelAgentReport::Release { agent, sequence: Some(seq), .. }
        if agent == custom.trim() && seq.get() == 7)
    );
    assert!(
        matches!(report(&["panel", "report-agent-session", "@panel:0xabc", "--source=custom:pi",
        "--agent=pi", "--session-start-source= NEW ", "--agent-session-path=C:/session"]),
        PanelAgentReport::Session { session_start_source: Some(start), agent_session_path: Some(path), .. }
        if start == " NEW " && path == "C:/session")
    );
    assert!(matches!(
        report(&["panel", "clear-agent-authority", "@panel:0xabc"]),
        PanelAgentReport::Clear {
            source: None,
            sequence: None
        }
    ));
    assert!(
        matches!(report(&["panel", "clear-agent-authority", "@panel:0xabc", "--source="]),
        PanelAgentReport::Clear { source: Some(source), .. } if source.is_empty())
    );
}
#[test]
fn panel_agent_rejects_invalid_and_out_of_scope_flags_before_effect() {
    let base = [
        "panel",
        "report-agent",
        "@panel:0xabc",
        "--source=hook",
        "--agent=codex",
        "--state=working",
    ];
    for extra in [
        "--title=x",
        "--token=x=y",
        "--ttl-ms=1",
        "--focus",
        "--session-start-source=startup",
        "--agent=pi",
        "--seq=-1",
        "--seq=18446744073709551616",
        "--seq= 1",
        "--seq=",
        "--seq=1e2",
    ] {
        assert!(
            parse(&[base.as_slice(), &[extra]].concat()).is_err(),
            "{extra}"
        );
    }
    for args in [
        vec![
            "panel",
            "report-agent",
            "@panel:0xabc",
            "--source=hook",
            "--agent=x",
        ],
        vec![
            "panel",
            "report-agent",
            "@panel:0xabc",
            "--source=hook",
            "--state=working",
        ],
        vec![
            "panel",
            "report-agent",
            "@panel:0xabc",
            "--agent=x",
            "--state=working",
        ],
        vec![
            "panel",
            "release-agent",
            "@panel:0xabc",
            "--source= ",
            "--agent=x",
        ],
        vec![
            "panel",
            "release-agent",
            "@panel:0xabc",
            "--source=hook",
            "--agent= ",
        ],
        vec![
            "panel",
            "release-agent",
            "@panel:0xabc",
            "--source=hook",
            "--agent=x",
            "--message=x",
        ],
        vec![
            "panel",
            "clear-agent-authority",
            "@panel:0xabc",
            "--agent=x",
        ],
        vec![
            "panel",
            "report-agent-session",
            "@panel:0xabc",
            "--source=hook",
            "--agent=x",
            "--state=idle",
        ],
        vec![
            "panel",
            "report-agent-session",
            "@panel:0xabc",
            "--source=hook",
            "--agent=x",
            "--message=x",
        ],
        vec!["panel", "get", "@panel:0xabc", "--state=working"],
        vec![
            "panel",
            "report-metadata",
            "@panel:0xabc",
            "--source=hook",
            "--title=x",
            "--message=x",
        ],
        vec![
            "workspace",
            "report-metadata",
            "main",
            "--source=hook",
            "--token=x=y",
            "--agent-session-id=x",
        ],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
    for state in ["successful", "Working", " idle", "", "blocked "] {
        assert!(
            parse(&[
                "panel",
                "report-agent",
                "@panel:0xabc",
                "--source=hook",
                "--agent=x",
                &format!("--state={state}")
            ])
            .is_err()
        );
    }
}
#[test]
fn panel_agent_bounds_the_complete_wire_request_before_runtime_discovery() {
    // Neither identities nor nonempty messages inherit display metadata's 80-character limit.
    let long = "界".repeat(1000);
    assert!(
        parse(&[
            "panel",
            "report-agent",
            "@panel:0xabc",
            "--source=hook",
            "--agent",
            &long,
            "--state=unknown",
            "--message",
            &long
        ])
        .is_ok()
    );
    let huge = "\n".repeat(9000); // JSON escaping counts toward the request bound.
    let error = parse(&[
        "panel",
        "report-agent",
        "@panel:0xabc",
        "--source=hook",
        "--agent=x",
        "--state=unknown",
        "--message",
        &huge,
    ])
    .unwrap_err();
    assert!(matches!(
        error.usage,
        Some(UsageReason::InvalidValue {
            expected: ExpectedValue::ReportSize,
            ..
        })
    ));
}
