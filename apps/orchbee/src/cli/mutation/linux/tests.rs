//! Table-driven witnesses for the mapping arms a scripted peer cannot
//! stage deterministically: pre-handoff refusals, the per-variant
//! unknown-reason tokens, and every terminal outcome arm.

use std::io;

use orchbee_workbench_protocol::{
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabId, WorkbenchTerminalContextId,
    WorkbenchTerminalGeometry,
};

use super::*;

fn io_error() -> io::Error {
    io::Error::new(io::ErrorKind::ConnectionReset, "scripted")
}

fn id32(value: u64) -> String {
    format!("{value:032x}")
}

fn request(value: u64) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&id32(value)).unwrap()
}

#[test]
fn not_submitted_maps_only_request_to_transport_loss() {
    let cases = [
        (
            NotSubmitted::ConnectionUnavailable,
            FailureKind::InternalFailure,
        ),
        (NotSubmitted::RequestEncoding, FailureKind::InternalFailure),
        (NotSubmitted::Request, FailureKind::TransportLoss),
    ];
    for (reason, kind) in cases {
        let failure = not_submitted_failure(reason);
        assert_eq!(failure.kind, kind);
        match kind {
            FailureKind::TransportLoss => {
                assert_eq!(failure.details.as_ref().unwrap()["stage"], json!("command"));
            }
            _ => assert!(failure.details.is_none()),
        }
    }
}

#[test]
fn unknown_failure_carries_stage_request_and_reason() {
    let request = request(0x77);
    let failure = unknown_failure(request.clone(), "response-loss");
    assert_eq!(failure.kind, FailureKind::OutcomeUnknown);
    let details = failure.details.unwrap();
    assert_eq!(details["stage"], json!("command"));
    assert_eq!(details["requestId"], json!(request.as_str()));
    assert_eq!(details["reason"], json!("response-loss"));
}

/// Every `reason()` arm on one full reason enum plus the close
/// families' extra verdict-mismatch variants: the tokens are the same
/// closed vocabulary the failure schema enumerates.
#[test]
fn unknown_reason_tokens_cover_every_classification_arm() {
    let cases: Vec<(WorkbenchTabMoveOutcomeUnknownReason, &str)> = vec![
        (
            WorkbenchTabMoveOutcomeUnknownReason::Write(io_error()),
            "write-failure",
        ),
        (
            WorkbenchTabMoveOutcomeUnknownReason::Response(WorkbenchWireError::Io(io_error())),
            "response-loss",
        ),
        (
            WorkbenchTabMoveOutcomeUnknownReason::ResponseDeadline(io_error()),
            "response-loss",
        ),
        (
            WorkbenchTabMoveOutcomeUnknownReason::Response(WorkbenchWireError::InvalidHeader),
            "response-invalid",
        ),
        (
            WorkbenchTabMoveOutcomeUnknownReason::UnexpectedResponse,
            "response-invalid",
        ),
        (
            WorkbenchTabMoveOutcomeUnknownReason::CorrelationMismatch {
                expected: request(0x42),
                received: request(0x99),
            },
            "correlation-mismatch",
        ),
        (
            WorkbenchTabMoveOutcomeUnknownReason::ActionIncompatible,
            "action-incompatible",
        ),
        (
            WorkbenchTabMoveOutcomeUnknownReason::ExpectedRevisionOverflow {
                expected: WorkbenchRevision::new(u64::MAX),
            },
            "validation-failed",
        ),
        (
            WorkbenchTabMoveOutcomeUnknownReason::RevisionMismatch {
                expected: WorkbenchRevision::new(1),
                received: WorkbenchRevision::new(2),
            },
            "validation-failed",
        ),
        (
            WorkbenchTabMoveOutcomeUnknownReason::TerminalContextMismatch {
                expected: WorkbenchTerminalContextId::parse(&id32(1)).unwrap(),
                received: WorkbenchTerminalContextId::parse(&id32(2)).unwrap(),
            },
            "validation-failed",
        ),
        (
            WorkbenchTabMoveOutcomeUnknownReason::TabMismatch {
                expected: WorkbenchTabId::parse(&id32(3)).unwrap(),
                received: WorkbenchTabId::parse(&id32(4)).unwrap(),
            },
            "validation-failed",
        ),
    ];
    for (reason, expected) in cases {
        assert_eq!(reason.reason(), expected, "{reason}");
    }

    // The close families' extra verdict-mismatch variants all land in
    // the same catch-all bucket.
    let close_cases: Vec<(WorkbenchTabCloseFailure, &str)> = vec![
        (WorkbenchTabCloseFailure::Write(io_error()), "write-failure"),
        (
            WorkbenchTabCloseFailure::SiteMismatch {
                expected: SiteId::parse(&id32(5)).unwrap(),
                received: SiteId::parse(&id32(6)).unwrap(),
            },
            "validation-failed",
        ),
        (
            WorkbenchTabCloseFailure::OperationMismatch {
                expected: request(0x11),
                received: request(0x22),
            },
            "validation-failed",
        ),
        (
            WorkbenchTabCloseFailure::AcceptedRevisionMismatch {
                admitted: WorkbenchRevision::new(4),
                received: WorkbenchRevision::new(5),
            },
            "validation-failed",
        ),
        (
            WorkbenchTabCloseFailure::AttemptGenerationMismatch {
                expected: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                received: WorkbenchTabCloseAttemptGeneration::new(2).unwrap(),
            },
            "validation-failed",
        ),
    ];
    for (reason, expected) in close_cases {
        assert_eq!(reason.reason(), expected, "{reason}");
    }
}

#[test]
fn terminal_fact_covers_every_outcome_arm() {
    let geometry = WorkbenchTerminalGeometry::new(80, 24, 9, 18).unwrap();
    let cases = vec![
        (
            WorkbenchTerminalOutcome::Started { geometry },
            ("started", None, true),
        ),
        (
            WorkbenchTerminalOutcome::Unavailable {
                reason: WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
            },
            ("unavailable", Some("terminal-runtime-unavailable"), false),
        ),
        (
            WorkbenchTerminalOutcome::Unavailable {
                reason: WorkbenchTerminalUnavailableReason::ServerBusy,
            },
            ("unavailable", Some("server-busy"), false),
        ),
        (
            WorkbenchTerminalOutcome::Failed {
                reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
            },
            ("failed", Some("terminal-create-failed"), false),
        ),
        (
            WorkbenchTerminalOutcome::OutcomeUnknown {},
            ("unknown", None, false),
        ),
    ];
    for (outcome, (expected_outcome, expected_reason, has_geometry)) in cases {
        let fact = terminal_fact(&outcome);
        assert_eq!(fact.outcome, expected_outcome, "{outcome:?}");
        assert_eq!(fact.reason, expected_reason, "{outcome:?}");
        assert_eq!(fact.geometry.is_some(), has_geometry, "{outcome:?}");
    }
}

/// `panel move --target-panel` resolves inside the moved panel's own
/// tab — a selector tail that only exists on another tab's panel is a
/// miss, never a cross-tab fallback.
#[test]
fn panel_move_scopes_target_panel_to_the_source_tab() {
    use orchbee_workbench_protocol::parse_snapshot;

    fn tid(high: u64, tail: u64) -> String {
        format!("{high:016x}{tail:016x}")
    }
    let leaf = |panel: &str, pres: &str| {
        json!({
            "kind": "panel",
            "panelId": panel,
            "primaryPresentation": {"customLabel":null,
                "presentationId": pres,
                "label": "p",
                "binding": null,
            },
            "childPresentations": [],
        })
    };
    let document = json!({
        "kind": "workbench.snapshot",
        "protocolVersion": 1,
        "revision": "9",
        "siteId": tid(0, 0xaa00),
        "navigation": {"projects":[{"projectId":tid(0, 0xbb), "label":"p"}], "directories":[], "workspaces":[{"workspaceId":tid(0, 0x01),"terminalContextId":tid(0, 0x01),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"},{"workspaceId":tid(0, 0xdd), "label":"w", "primaryDirectoryId":null, "terminalContextId":tid(0, 0x60), "projectId":tid(0, 0xbb)}]},
        "terminalContexts": [
            {
                "terminalContextId": tid(0, 0x01),
                "groups": [],
                "tabs": [{
                    "tabId": tid(0, 0xee),
                    "label": "t",
                    "pinned": false,
                    "groupId": null,
                    // Panel identity `ff` lives here — outside the
                    // source tab's scope.
                    "root": leaf(&tid(0, 0xff), &tid(0xa, 0x01)),
                }],
            },
            {
                "terminalContextId": tid(0, 0x60),
                "groups": [],
                "tabs": [{
                    "tabId": tid(0, 0xe1),
                    "label": "t3",
                    "pinned": false,
                    "groupId": null,
                    "root": {
                        "kind": "split",
                        "splitId": tid(9, 0x92),
                        "orientation": "horizontal",
                        "children": [
                            {"slotId": tid(8, 0x92), "node": leaf(&tid(0, 0xf2), &tid(0xa, 0x02))},
                            {"slotId": tid(8, 0x93), "node": leaf(&tid(1, 0x1f2), &tid(0xa, 0x03))},
                        ],
                    },
                }],
            },
        ],
    });
    let snapshot = parse_snapshot(&serde_json::to_vec(&document).unwrap()).unwrap();
    let move_command = |into: &str| MutationCommand::PanelMove {
        target: Target::Selector("@panel:0xf2".into()),
        destination: PanelMoveDestination::Tab {
            tab: Target::Id(tid(0, 0xe1)),
            panel: Some(Target::Selector(into.into())),
            direction: WorkbenchPanelSplitDirection::Left,
        },
    };

    // Exact in-scope identity: the action carries the resolved full id.
    let resolved = action(&move_command("@panel:0x100000000000001f2"), &snapshot)
        .expect("in-scope selector resolves");
    let WorkbenchAction::PanelMove {
        panel_id,
        target_panel_id,
        ..
    } = resolved
    else {
        panic!("panel move action");
    };
    assert_eq!(panel_id.as_str(), tid(0, 0xf2));
    assert_eq!(target_panel_id.as_str(), tid(1, 0x1f2));

    // `ff` exists globally (the initial_workspace tab) but not inside the
    // source tab — no global fallback.
    let failure = action(&move_command("@panel:0xff"), &snapshot)
        .expect_err("out-of-scope identity must miss");
    assert_eq!(failure.kind, FailureKind::NotFound);
}
