use super::*;
use orchbee_workbench_protocol::{PanelAgentDisposition, ProtocolVersion, WorkbenchRevision};
#[test]
fn panel_agent_receipts_survive_late_interrupt_and_validate_the_public_schema() {
    let mut answer = Answer {
        presentation_id: None,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
        session_id: SessionId::parse(&format!("{:032x}", 2)).unwrap(),
        panel_id: WorkbenchPanelId::parse(&format!("{:032x}", 3)).unwrap(),
        observation: Observation::Unknown("response-deadline"),
        interrupted: true,
        cleanup_failed: true,
    };
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/orchbee.panel.agent-report.json")).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    assert_eq!(answer.exit_code(), 8);
    assert!(validator.is_valid(&answer.document()));
    answer.observation = Observation::Server(PanelAgentResult {
        protocol_version: ProtocolVersion,
        request_id: answer.request_id.clone(),
        session_id: answer.session_id.clone(),
        outcome: PanelAgentOutcome::Reported {
            panel_id: answer.panel_id.clone(),
            binding: orchbee_workbench_protocol::PresentationBinding {
                worker_id: orchbee_workbench_protocol::WorkerId::parse(&format!("{:032x}", 4))
                    .unwrap(),
                terminal_id: orchbee_workbench_protocol::TerminalId::parse(&format!("{:032x}", 5))
                    .unwrap(),
            },
            workbench_revision: WorkbenchRevision::new(5),
            report_revision: WorkbenchRevision::new(9),
            disposition: PanelAgentDisposition::Applied,
            changed: true,
            view: Box::new(orchbee_workbench_protocol::PanelAgentView {
                completion: Default::default(),
                agent: None,
                state: orchbee_workbench_protocol::PanelAgentState::Unknown,
                authority_source: None,
                message: None,
            }),
        },
    });
    assert_eq!(answer.exit_code(), 1);
    answer.cleanup_failed = false;
    assert_eq!(answer.exit_code(), 0);
    assert!(validator.is_valid(&answer.document()));
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let emitted = answer.emit(OutputFormat::Text, locale);
        assert_eq!(emitted.status, 0);
        assert!(emitted.stderr.is_empty());
        assert!(!emitted.stdout.contains(&0x1b));
        assert!(
            String::from_utf8(emitted.stdout)
                .unwrap()
                .contains(answer.request_id.as_str())
        );
    }
    for disposition in [
        PanelAgentDisposition::Applied,
        PanelAgentDisposition::Ignored,
    ] {
        if let Observation::Server(reply) = &mut answer.observation
            && let PanelAgentOutcome::Reported {
                disposition: actual,
                ..
            } = &mut reply.outcome
        {
            *actual = disposition;
        }
        assert_eq!(answer.exit_code(), 0);
        assert!(validator.is_valid(&answer.document()));
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let text = answer.emit(OutputFormat::Text, locale);
            assert_eq!(text.status, 0);
            assert!(!text.stdout.contains(&0x1b));
        }
    }
    let mut invalid = answer.document();
    invalid["result"]["observation"]["reply"]["outcome"]["disposition"] =
        serde_json::json!("invented");
    assert!(!validator.is_valid(&invalid));
    for state in ["idle", "working", "blocked", "done", "error", "unknown"] {
        let mut current = answer.document();
        current["result"]["observation"]["reply"]["outcome"]["view"]["agent"] = "Fixture".into();
        current["result"]["observation"]["reply"]["outcome"]["view"]["state"] = state.into();
        validator.validate(&current).unwrap();
    }
}
