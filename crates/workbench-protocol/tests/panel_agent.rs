use orchbee_workbench_protocol::*;
#[test]
fn physical_observation_answers_only_matching_binding_and_contiguous_cursor() {
    let id = "11111111111111111111111111111111";
    let binding = serde_json::from_value(
        serde_json::json!({"presentationId":id,"generation":id,"hostEpoch":id,"hostTerminalId":id}),
    )
    .unwrap();
    let mut request = PanelAgentRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(id).unwrap(),
        site_id: SiteId::parse(id).unwrap(),
        action: PanelAgentAction::Observe {
            binding,
            after_sequence: Some(WorkbenchRevision::new(0)),
        },
    };
    let PanelAgentAction::Observe { binding, .. } = &request.action else {
        unreachable!()
    };
    let mut result = PanelAgentResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        site_id: request.site_id.clone(),
        outcome: PanelAgentOutcome::Observed {
            workbench_revision: WorkbenchRevision::new(1),
            report_revision: WorkbenchRevision::new(1),
            observation: Box::new(TerminalAgentObservation {
                binding: binding.clone(),
                sequence: WorkbenchRevision::new(1),
                changes: vec![TerminalAgentChange {
                    sequence: WorkbenchRevision::new(1),
                    agent: None,
                    state: PanelAgentState::Idle,
                }],
                view: PanelAgentView {
                    state: PanelAgentState::Idle,
                    ..Default::default()
                },
            }),
        },
    };
    assert!(result.answers(&request));
    if let PanelAgentAction::Observe { after_sequence, .. } = &mut request.action {
        *after_sequence = None;
    }
    assert!(!result.answers(&request));
    if let PanelAgentOutcome::Observed { observation, .. } = &mut result.outcome {
        observation.changes.clear();
    }
    assert!(result.answers(&request));
    if let PanelAgentAction::Observe { after_sequence, .. } = &mut request.action {
        *after_sequence = Some(WorkbenchRevision::new(0));
    }
    assert!(!result.answers(&request));
    if let PanelAgentAction::Observe { after_sequence, .. } = &mut request.action {
        *after_sequence = Some(WorkbenchRevision::new(1));
    }
    assert!(result.answers(&request));
    if let PanelAgentAction::Observe { after_sequence, .. } = &mut request.action {
        *after_sequence = Some(WorkbenchRevision::new(2));
    }
    assert!(!result.answers(&request));
}
#[test]
fn panel_agent_shared_corpus_and_carrier_bounds() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-panel-agent-conformance-v1.json"
    ))
    .unwrap();
    for (group, expected) in [("valid", true), ("invalid", false)] {
        for case in corpus[group].as_array().unwrap() {
            let bytes = serde_json::to_vec(&case["document"]).unwrap();
            let mut frame = vec![];
            if case["direction"] == "request" {
                let parsed = parse_workbench_request(&bytes);
                assert_eq!(parsed.is_ok(), expected, "{}", case["name"]);
                if let Ok(request) = parsed {
                    write_workbench_request(&mut frame, &request).unwrap();
                    assert_eq!(&frame[6..8], &35_u16.to_be_bytes());
                    assert_eq!(
                        read_workbench_request(&mut frame.as_slice()).unwrap(),
                        request
                    );
                    assert!(read_workbench_response(&mut frame.as_slice()).is_err());
                }
            } else {
                let parsed = parse_workbench_response(&bytes);
                assert_eq!(parsed.is_ok(), expected, "{}", case["name"]);
                if let Ok(response) = parsed {
                    write_workbench_response(&mut frame, &response).unwrap();
                    assert_eq!(&frame[6..8], &36_u16.to_be_bytes());
                    assert_eq!(
                        read_workbench_response(&mut frame.as_slice()).unwrap(),
                        response
                    );
                    assert!(read_workbench_request(&mut frame.as_slice()).is_err());
                }
            }
        }
    }
    for (kind, maximum) in [
        (35_u16, MAXIMUM_ENCODED_INTENT_BYTES),
        (36, MAXIMUM_ENCODED_PANEL_AGENT_RESULT_BYTES),
    ] {
        let mut header = b"OBWP\0\x01".to_vec();
        header.extend(kind.to_be_bytes());
        header.extend(u32::try_from(maximum + 1).unwrap().to_be_bytes());
        let mut cursor = std::io::Cursor::new(header);
        let result = if kind == 35 {
            read_workbench_request(&mut cursor).map(|_| ())
        } else {
            read_workbench_response(&mut cursor).map(|_| ())
        };
        assert!(matches!(result, Err(WorkbenchWireError::Contract(_))));
        assert_eq!(cursor.position(), 12);
    }
}

#[test]
fn presentation_reads_answer_only_exact_order_and_request_site_identity() {
    let ids: Vec<_> = (1..=2)
        .map(|n| TerminalPresentationId::parse(&format!("{n:032x}")).unwrap())
        .collect();
    let panel = WorkbenchPanelId::parse(&"3".repeat(32)).unwrap();
    let request = PanelAgentRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&"4".repeat(32)).unwrap(),
        site_id: SiteId::parse(&"5".repeat(32)).unwrap(),
        action: PanelAgentAction::ReadPresentations {
            presentation_ids: ids.clone(),
        },
    };
    let mut result = PanelAgentResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        site_id: request.site_id.clone(),
        outcome: PanelAgentOutcome::PresentationsRead {
            workbench_revision: WorkbenchRevision::new(9),
            report_revision: WorkbenchRevision::new(0),
            presentations: ids
                .iter()
                .map(|id| PresentationAgentRow {
                    presentation_id: id.clone(),
                    panel: PanelAgentRow {
                        panel_id: panel.clone(),
                        binding: None,
                        report: None,
                    },
                })
                .collect(),
        },
    };
    result.validate().unwrap();
    assert!(result.answers(&request));
    if let PanelAgentOutcome::PresentationsRead { presentations, .. } = &mut result.outcome {
        presentations.reverse();
    }
    assert!(!result.answers(&request));
    if let PanelAgentOutcome::PresentationsRead { presentations, .. } = &mut result.outcome {
        presentations.reverse();
    }
    result.site_id = SiteId::parse(&"6".repeat(32)).unwrap();
    assert!(!result.answers(&request));
    result.site_id = request.site_id.clone();
    result.request_id = WorkbenchRequestId::parse(&"7".repeat(32)).unwrap();
    assert!(!result.answers(&request));
}
