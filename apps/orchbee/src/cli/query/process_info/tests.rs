use super::*;
use crate::{cli::results::Projection, invocation::QueryCommand};
use orchbee_terminal_protocol::{
    TerminalId, TerminalProcess, TerminalProcessConsole, TerminalProcessInfo,
};

fn info() -> TerminalProcessInfo {
    TerminalProcessInfo {
        attachment_id: AttachmentId::from_bytes([1; 16]).unwrap(),
        terminal_id: TerminalId::from_bytes([2; 16]).unwrap(),
        root_pid: 41,
        processes: vec![TerminalProcess {
            pid: 41,
            parent_pid: None,
            creation_time: u64::MAX,
            console: TerminalProcessConsole::Attached,
            image: Some("C:\\Windows\\cmd.exe".into()),
            command_line: Some("cmd /C echo 中 😀\u{1b}]2;title\u{7}".into()),
            current_directory: None,
        }],
    }
}

#[test]
fn process_info_preserves_expected_host_failures_with_valid_failure_schema() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../schemas/orchbee.cli.failure.process-info.json"
    ))
    .unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    use crate::agent_runtime::startup::processes::{Error, HostFailure};
    for (code, expected) in [
        ("limit-exceeded", Some(HostFailure::LimitExceeded)),
        ("missing-resource", Some(HostFailure::MissingResource)),
        ("process-sample-unsupported", Some(HostFailure::Unsupported)),
        ("process-sample-unavailable", Some(HostFailure::Unavailable)),
        ("invalid-field", None),
    ] {
        let failure = classify_process(expected.map_or(Error::Protocol, Error::Host));
        if code == "invalid-field" {
            assert_eq!(failure.kind, FailureKind::ProtocolInvalid);
            continue;
        }
        let output = crate::cli::failure_output(
            failure,
            Some(crate::invocation::OutputFormat::Json),
            crate::localization::Locale::EnUs,
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validator.validate(&document).unwrap();
        assert_eq!(document["failure"]["details"]["reason"], code);
        assert!(!output.stderr.contains("untrusted"));
    }
}

/// Every identity result, grade, and reason the Server can report.
pub(crate) fn identities() -> Vec<orchbee_agent_observation_protocol::AgentProcessIdentity> {
    use orchbee_agent_observation_protocol::{
        AgentIdentityGrade, AgentIdentityUnknownReason, AgentIdentityUnsupportedReason,
        AgentProcessIdentity as Identity, AgentReportProvider,
    };
    let mut all = AgentIdentityGrade::ALL
        .map(|grade| Identity::Agent {
            provider: AgentReportProvider::Claude,
            grade,
        })
        .to_vec();
    all.push(Identity::None);
    all.extend(AgentIdentityUnknownReason::ALL.map(Identity::Unknown));
    all.extend(AgentIdentityUnsupportedReason::ALL.map(Identity::Unsupported));
    all
}

/// Localized identity values in [identities] order: en-US, then zh-CN.
pub(crate) const IDENTITY_TEXT: [(&str, &str); 13] = [
    ("claude-code Agent, confirmed", "claude-code 智能体，已确认"),
    (
        "claude-code Agent, recognized",
        "claude-code 智能体，已识别",
    ),
    ("No Agent", "没有智能体"),
    (
        "Unknown: the Agent is still settling",
        "未知：智能体尚未稳定",
    ),
    (
        "Unknown: process sampling is failing",
        "未知：进程采样持续失败",
    ),
    (
        "Unknown: the process sample reached its limit",
        "未知：进程采样达到上限",
    ),
    (
        "Unknown: a command line is unreadable",
        "未知：有命令行无法读取",
    ),
    (
        "Unknown: several Agent candidates",
        "未知：存在多个智能体候选",
    ),
    (
        "Unknown: arguments are not classified",
        "未知：参数无法分类",
    ),
    (
        "Unknown: the install source is unverified",
        "未知：安装来源未经验证",
    ),
    (
        "Unknown: the Agent is detached from the console",
        "未知：智能体已脱离控制台",
    ),
    ("Unsupported: this platform", "不支持：当前平台"),
    ("Unsupported: remote execution", "不支持：远程执行"),
];

#[test]
fn process_info_machine_contract_preserves_precision_nulls_and_escapes_text() {
    use crate::localization::Locale;
    let terminal = results::TerminalRow {
        terminal_id: format!("{:032x}", 10),
        worker_id: format!("{:032x}", 11),
        presentation_id: format!("{:032x}", 12),
        panel_id: format!("{:032x}", 13),
        tab_id: format!("{:032x}", 14),
        terminal_context_id: format!("{:032x}", 15),
    };
    let source = json!({"kind":"terminal-process-query","sessionId":format!("{:032x}",16),"snapshotRevision":"7","requestId":format!("{:032x}",17),"hostRequestId":format!("{:032x}",18),"hostEpoch":format!("{:032x}",19),"attachmentId":format!("{:032x}",20),"hostTerminalId":format!("{:032x}",21)});
    let schema: Value = serde_json::from_str(include_str!(
        "../../schemas/orchbee.panel.process-info.json"
    ))
    .unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    let identities = identities();
    assert_eq!(identities.len(), IDENTITY_TEXT.len());
    let project = |identity| {
        Projection::PanelProcessInfo(
            results::PanelProcessInfoDoc {
                terminal: terminal.clone(),
                info: info(),
            },
            identity,
        )
    };
    for (identity, (en, zh)) in identities.iter().copied().zip(IDENTITY_TEXT) {
        let projection = project(identity);
        let document = json!({"schema":"orchbee.panel.process-info","version":1,"source":source,"result":projection.result()});
        validator.validate(&document).unwrap();
        assert_eq!(document["result"]["identity"], json!(identity));
        assert_eq!(
            document["result"]["processes"][0]["creationTime"],
            u64::MAX.to_string()
        );
        assert!(document["result"]["processes"][0]["currentDirectory"].is_null());
        for (locale, label, value) in [
            (Locale::EnUs, "Identity", en),
            (Locale::ZhCn, "识别结果", zh),
        ] {
            let output = crate::cli::output::render_text(|out| {
                crate::cli::present::render(
                    out,
                    &projection,
                    &QueryCommand::PanelProcessInfo(crate::invocation::PanelQueryTarget::Current),
                    locale,
                )
            })
            .unwrap();
            let text = String::from_utf8(output).unwrap();
            assert!(!text.contains('\u{1b}'));
            assert!(!text.contains('\u{7}'));
            assert!(text.contains("中 😀"));
            assert!(
                text.lines()
                    .any(|line| line.starts_with(label) && line.ends_with(value)),
                "{text}"
            );
        }
    }
    let mut invalid = json!({"schema":"orchbee.panel.process-info","version":1,"source":source,"result":project(identities[0]).result()});
    for identity in [
        json!({"result":"unknown","reason":"other"}),
        json!({"result":"none","reason":"settling"}),
        json!({"result":"agent","providerId":"claude-code"}),
        json!({"kind":"unsupported"}),
    ] {
        invalid["result"]["identity"] = identity;
        assert!(!validator.is_valid(&invalid));
    }
}

#[test]
fn process_info_targets_only_the_primary_binding_without_child_fallback() {
    let id = |value: u8| format!("{value:032x}");
    let mut value = json!({"kind":"workbench.snapshot","protocolVersion":1,"sessionId":id(1),"revision":"7",
    "navigation":{"projects":[], "directories":[], "workspaces":[{"workspaceId":id(2),"terminalContextId":id(2),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"}]},
    "terminalContexts":[{"terminalContextId":id(2),"groups":[],"tabs":[{
        "tabId":id(3),"label":"tab","pinned":false,"groupId":null,
        "root":{"kind":"panel","panelId":id(4),
            "primaryPresentation":{"customLabel":null,"presentationId":id(5),"label":"primary","binding":{"terminalId":id(10),"workerId":id(11)}},
            "childPresentations":[{"customLabel":null,"presentationId":id(6),"label":"child","binding":{"terminalId":id(12),"workerId":id(11)}}]}
    }]}]});
    let parse = |value: &Value| {
        orchbee_workbench_protocol::parse_snapshot(&serde_json::to_vec(value).unwrap()).unwrap()
    };
    assert_eq!(
        primary_terminal(&parse(&value), &Target::Selector("@panel:0x4".into())).unwrap(),
        Target::Id(id(10))
    );
    value["terminalContexts"][0]["tabs"][0]["root"]["primaryPresentation"]["binding"] = Value::Null;
    assert_eq!(
        primary_terminal(&parse(&value), &Target::Id(id(4)))
            .unwrap_err()
            .kind,
        FailureKind::NotFound
    );
}

#[test]
fn physical_explanation_carries_every_identity_in_both_projections() {
    use crate::localization::Locale;
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../../../../schemas/workbench/v1/workbench-panel-agent-conformance-v1.json"
    ))
    .unwrap();
    let mut terminal = corpus["valid"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "explain-first-priority-tie")
        .unwrap()["document"]["outcome"]["explanation"]
        .clone();
    let schema: Value =
        serde_json::from_str(include_str!("../../schemas/orchbee.agent.explain.json")).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    let source = json!({"kind":"panel-agent-explain","sessionId":format!("{:032x}",1),"workbenchRevision":"9","reportRevision":"2","requestId":format!("{:032x}",2)});
    for (identity, (en, zh)) in identities().into_iter().zip(IDENTITY_TEXT) {
        terminal["identity"] = json!(identity);
        let projection =
            Projection::AgentTerminalExplanation(serde_json::from_value(terminal.clone()).unwrap());
        let document = json!({"schema":"orchbee.agent.explain","version":1,"source":source,"result":projection.result()});
        validator.validate(&document).unwrap();
        assert_eq!(document["result"]["terminal"]["identity"], json!(identity));
        for (locale, label, value) in [
            (Locale::EnUs, "Identity", en),
            (Locale::ZhCn, "识别结果", zh),
        ] {
            let output = crate::cli::output::render_text(|out| {
                crate::cli::present::render(
                    out,
                    &projection,
                    &QueryCommand::AgentExplain(crate::invocation::AgentTarget::Panel {
                        target: Target::Id(format!("{:032x}", 3)),
                    }),
                    locale,
                )
            })
            .unwrap();
            let text = String::from_utf8(output).unwrap();
            assert!(
                text.lines()
                    .any(|line| line.starts_with(label) && line.ends_with(value)),
                "{text}"
            );
        }
    }
}
