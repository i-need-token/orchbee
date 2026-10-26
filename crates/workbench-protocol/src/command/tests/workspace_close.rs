use super::*;

fn id(value: u8) -> String {
    format!(
        "{value:02x}{value:02x}{value:02x}{value:02x}{value:02x}{value:02x}{value:02x}{value:02x}{value:02x}{value:02x}{value:02x}{value:02x}{value:02x}{value:02x}{value:02x}{value:02x}"
    )
}

fn workspace_close_read() -> WorkbenchWorkspaceCloseRead {
    WorkbenchWorkspaceCloseRead {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&id(0x11)).unwrap(),
        session_id: SessionId::parse(&id(0x22)).unwrap(),
        workspace_id: WorkbenchWorkspaceId::parse(&id(0x44)).unwrap(),
    }
}

fn member_status_worst() -> WorkbenchWorkspaceCloseMemberStatus {
    WorkbenchWorkspaceCloseMemberStatus::DisposedAwaitingPublication {
        reason: WorkbenchTabClosePublicationReason::TargetCleanupPending,
        resumable: false,
    }
}

fn member(
    index: usize,
    status: WorkbenchWorkspaceCloseMemberStatus,
) -> WorkbenchWorkspaceCloseMember {
    WorkbenchWorkspaceCloseMember {
        tab_id: WorkbenchTabId::parse(&format!("{index:032x}"))
            .unwrap_or_else(|_| WorkbenchTabId::parse(&id(0x55)).unwrap()),
        panel_id: WorkbenchPanelId::parse(&format!("{index:032x}"))
            .unwrap_or_else(|_| WorkbenchPanelId::parse(&id(0x66)).unwrap()),
        presentation_id: TerminalPresentationId::parse(&format!("{index:032x}"))
            .unwrap_or_else(|_| TerminalPresentationId::parse(&id(0x77)).unwrap()),
        status,
    }
}

#[test]
fn workspace_close_envelopes_preserve_exact_fields_through_json_and_obwp() {
    let read = WorkbenchRequest::WorkspaceCloseRead(workspace_close_read());
    let encoded = serialize_workbench_request(&read).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&encoded).unwrap(),
        serde_json::json!({
            "kind": "workbench.workspace-close.read",
            "protocolVersion": 1,
            "requestId": "11111111111111111111111111111111",
            "sessionId": "22222222222222222222222222222222",
            "workspaceId": "44444444444444444444444444444444",
        })
    );
    assert_eq!(parse_workbench_request(&encoded).unwrap(), read);
    let mut frame = Vec::new();
    crate::write_workbench_request(&mut frame, &read).unwrap();
    assert_eq!(&frame[6..8], &15_u16.to_be_bytes());
    assert_eq!(
        crate::read_workbench_request(&mut Cursor::new(frame)).unwrap(),
        read
    );

    let resume = WorkbenchRequest::WorkspaceCloseResume(WorkbenchWorkspaceCloseResume {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&id(0x12)).unwrap(),
        session_id: SessionId::parse(&id(0x22)).unwrap(),
        workspace_id: WorkbenchWorkspaceId::parse(&id(0x44)).unwrap(),
        close_request_id: WorkbenchRequestId::parse(&id(0x11)).unwrap(),
        expected_attempt_generation: WorkbenchTabCloseAttemptGeneration::new(3).unwrap(),
    });
    let encoded = serialize_workbench_request(&resume).unwrap();
    assert_eq!(parse_workbench_request(&encoded).unwrap(), resume);
    let mut frame = Vec::new();
    crate::write_workbench_request(&mut frame, &resume).unwrap();
    assert_eq!(&frame[6..8], &17_u16.to_be_bytes());
    assert_eq!(
        crate::read_workbench_request(&mut Cursor::new(frame)).unwrap(),
        resume
    );

    let read_result = WorkbenchResponse::WorkspaceCloseRead(WorkbenchWorkspaceCloseReadResult {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&id(0x12)).unwrap(),
        outcome: WorkbenchWorkspaceCloseReadOutcome::Available {
            close_request_id: WorkbenchRequestId::parse(&id(0x11)).unwrap(),
            workspace_id: WorkbenchWorkspaceId::parse(&id(0x44)).unwrap(),
            terminal_context_id: WorkbenchTerminalContextId::parse(&id(0x55)).unwrap(),
            attempt_generation: WorkbenchTabCloseAttemptGeneration::new(2).unwrap(),
            status: WorkbenchTabCloseStatus::Closing {
                phase: WorkbenchTabClosePhase::Queued,
            },
            counts: WorkbenchWorkspaceCloseCounts {
                closing: 1,
                ..WorkbenchWorkspaceCloseCounts::default()
            },
            members: vec![member(
                0x66,
                WorkbenchWorkspaceCloseMemberStatus::Closing {
                    phase: WorkbenchTabClosePhase::Disposing,
                },
            )],
        },
    });
    let encoded = serialize_workbench_response(&read_result).unwrap();
    assert_eq!(parse_workbench_response(&encoded).unwrap(), read_result);
    let mut frame = Vec::new();
    crate::write_workbench_response(&mut frame, &read_result).unwrap();
    assert_eq!(&frame[6..8], &16_u16.to_be_bytes());
    assert_eq!(
        crate::read_workbench_response(&mut Cursor::new(frame)).unwrap(),
        read_result
    );

    let resume_result =
        WorkbenchResponse::WorkspaceCloseResume(WorkbenchWorkspaceCloseResumeResult {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&id(0x12)).unwrap(),
            session_id: SessionId::parse(&id(0x22)).unwrap(),
            workspace_id: WorkbenchWorkspaceId::parse(&id(0x44)).unwrap(),
            close_request_id: WorkbenchRequestId::parse(&id(0x11)).unwrap(),
            outcome: WorkbenchWorkspaceCloseResumeOutcome::Resumed {
                attempt_generation: WorkbenchTabCloseAttemptGeneration::new(3).unwrap(),
            },
        });
    let encoded = serialize_workbench_response(&resume_result).unwrap();
    assert_eq!(parse_workbench_response(&encoded).unwrap(), resume_result);
    let mut frame = Vec::new();
    crate::write_workbench_response(&mut frame, &resume_result).unwrap();
    assert_eq!(&frame[6..8], &18_u16.to_be_bytes());
    assert_eq!(
        crate::read_workbench_response(&mut Cursor::new(frame)).unwrap(),
        resume_result
    );
}

/// The dedicated cap is the exact encoded length of the worst legal document:
/// the protocol's maximum captured membership at every member's longest legal
/// status spelling, maximum-width counts and attempt generation, and the
/// longest aggregate status.
#[test]
fn workspace_close_read_result_cap_is_the_exact_worst_legal_document() {
    let members: Vec<_> = (0..crate::validation::MAXIMUM_PANEL_NODES_PER_CONTEXT)
        .map(|index| member(index + 1, member_status_worst()))
        .collect();
    let result = WorkbenchResponse::WorkspaceCloseRead(WorkbenchWorkspaceCloseReadResult {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&id(0x12)).unwrap(),
        outcome: WorkbenchWorkspaceCloseReadOutcome::Available {
            close_request_id: WorkbenchRequestId::parse(&id(0x11)).unwrap(),
            workspace_id: WorkbenchWorkspaceId::parse(&id(0x44)).unwrap(),
            terminal_context_id: WorkbenchTerminalContextId::parse(&id(0x55)).unwrap(),
            attempt_generation: WorkbenchTabCloseAttemptGeneration::new(u64::MAX).unwrap(),
            status: WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                reason: WorkbenchTabClosePublicationReason::TargetCleanupPending,
                resumable: false,
            },
            counts: WorkbenchWorkspaceCloseCounts {
                closing: u32::MAX,
                cleanup_blocked: u32::MAX,
                outcome_unknown: u32::MAX,
                disposed_awaiting_publication: u32::MAX,
            },
            members,
        },
    });
    let encoded = serialize_workbench_response(&result).unwrap();
    assert_eq!(
        encoded.len(),
        MAXIMUM_ENCODED_WORKSPACE_CLOSE_READ_RESULT_BYTES
    );
    assert_eq!(parse_workbench_response(&encoded).unwrap(), result);
    let mut oversized = encoded;
    oversized.push(b' ');
    assert!(matches!(
        parse_workbench_response(&oversized),
        Err(ContractError::EncodedValueTooLarge { .. })
    ));
}

#[test]
fn workspace_close_read_result_rejects_membership_beyond_the_context_bound() {
    let result = WorkbenchResponse::WorkspaceCloseRead(WorkbenchWorkspaceCloseReadResult {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&id(0x12)).unwrap(),
        outcome: WorkbenchWorkspaceCloseReadOutcome::Available {
            close_request_id: WorkbenchRequestId::parse(&id(0x11)).unwrap(),
            workspace_id: WorkbenchWorkspaceId::parse(&id(0x44)).unwrap(),
            terminal_context_id: WorkbenchTerminalContextId::parse(&id(0x55)).unwrap(),
            attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
            status: WorkbenchTabCloseStatus::Closing {
                phase: WorkbenchTabClosePhase::Queued,
            },
            counts: WorkbenchWorkspaceCloseCounts::default(),
            members: (0..=crate::validation::MAXIMUM_PANEL_NODES_PER_CONTEXT)
                .map(|index| {
                    member(
                        index + 1,
                        WorkbenchWorkspaceCloseMemberStatus::Closing {
                            phase: WorkbenchTabClosePhase::Queued,
                        },
                    )
                })
                .collect(),
        },
    });
    assert!(serialize_workbench_response(&result).is_err());
}
