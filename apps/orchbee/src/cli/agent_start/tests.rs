use super::*;
use crate::agent_start::ReservationState;
use orchbee_agent_observation_protocol::{
    AgentName, AgentProviderId, AgentSessionId, ObapRequestId,
};
use orchbee_client_server_protocol::{
    SendCleanupStatus, SendProgress, SendRejectedReason, SendUnavailableReason, TerminalSendOutcome,
};
use orchbee_workbench_client::test_support::agent_runtime::startup::send::ReceiptTestAccess;
use orchbee_workbench_client::test_support::agent_start::DeliveryDisposition;
use orchbee_workbench_client::test_support::agent_start::ReportFixture;
use serde_json::Value;

fn panel_a() -> WorkbenchPanelId {
    WorkbenchPanelId::parse(&"a".repeat(32)).unwrap()
}

/// A start typed at an existing Panel's prompt.
fn here() -> Placed {
    Placed {
        place: AgentStartPlace::Here,
        panel: Some(panel_a()),
        created: false,
    }
}

fn report(receipt: &Value, state: ReservationState, verdict: Verdict) -> Report {
    Report {
        name: Some(AgentName::parse("alpha").unwrap()),
        provider: AgentProviderId::parse("codex").unwrap(),
        reservation: Some(ObapRequestId::parse(&"1".repeat(32)).unwrap()),
        last_observed: None,
        reservation_state: state,
        submission: Some(receipt.clone()),
        submission_status: Some(0),
        elapsed_ms: 1,
        verdict,
    }
}

#[test]
fn every_non_success_send_receipt_keeps_its_cli_status_and_exact_envelope() {
    use super::super::send::SendVerdict;
    use crate::agent_runtime::startup::{binding::Binding, send::Receipt as DomainReceipt};
    use crate::agent_start::{
        Failure as DomainFailure, Report as DomainReport, StartFailure, Submission,
        Verdict as DomainVerdict,
    };
    use orchbee_client_server_protocol::{
        HostEpoch, HostTerminalId, RequestId, SendTarget, TerminalSendResult,
    };
    use orchbee_workbench_client::test_support::agent_runtime::startup::send::Context;

    let cases = [
        (
            TerminalSendOutcome::Rejected {
                reason: SendRejectedReason::HostInputBackpressure,
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::Confirmed,
            },
            3,
            SendVerdict::Rejected,
            DeliveryDisposition::NotApplied,
            FailureKind::Rejected,
        ),
        (
            TerminalSendOutcome::Unavailable {
                reason: SendUnavailableReason::WorkerUnavailable,
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::NotNeeded,
            },
            4,
            SendVerdict::Unavailable,
            DeliveryDisposition::NotApplied,
            FailureKind::ServiceUnavailable,
        ),
        (
            TerminalSendOutcome::Unavailable {
                reason: SendUnavailableReason::HostProtocolFault,
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::NotNeeded,
            },
            5,
            SendVerdict::Unavailable,
            DeliveryDisposition::NotApplied,
            FailureKind::TransportLoss,
        ),
        (
            TerminalSendOutcome::Accepted {
                progress: SendProgress::complete(2, 10),
                cleanup: SendCleanupStatus::Unconfirmed,
            },
            5,
            SendVerdict::AcceptedCleanupUnconfirmed,
            DeliveryDisposition::Applied,
            FailureKind::TransportLoss,
        ),
        (
            TerminalSendOutcome::OutcomeUnknown {
                progress: SendProgress::none(),
                cleanup: SendCleanupStatus::Unconfirmed,
            },
            8,
            SendVerdict::OutcomeUnknown,
            DeliveryDisposition::Unknown,
            FailureKind::OutcomeUnknown,
        ),
        (
            TerminalSendOutcome::Rejected {
                reason: SendRejectedReason::HostInputBackpressure,
                progress: SendProgress::stopped(1, 4),
                cleanup: SendCleanupStatus::Confirmed,
            },
            9,
            SendVerdict::PartialAccepted,
            DeliveryDisposition::PartiallyApplied,
            FailureKind::Rejected,
        ),
        (
            TerminalSendOutcome::Unavailable {
                reason: SendUnavailableReason::InputBudgetExhausted,
                progress: SendProgress::stopped(1, 4),
                cleanup: SendCleanupStatus::Confirmed,
            },
            9,
            SendVerdict::PartialAccepted,
            DeliveryDisposition::PartiallyApplied,
            FailureKind::Rejected,
        ),
    ];
    let schema = jsonschema::validator_for(
        &serde_json::from_str(include_str!(
            "../schemas/orchbee.cli.failure.agent-start.json"
        ))
        .unwrap(),
    )
    .unwrap();
    for (outcome, status, expected_verdict, delivery, failure_kind) in cases {
        let receipt = super::super::send::test_receipt(outcome.clone());
        assert_eq!(receipt.exit_status(), status);
        assert_eq!(receipt.verdict(), expected_verdict);
        let document = receipt.document();
        let request = RequestId::parse(&"1".repeat(32)).unwrap();
        let target = SendTarget::new(
            std::num::NonZeroU64::MIN,
            HostEpoch::parse(&"2".repeat(32)).unwrap(),
            HostTerminalId::parse(&"3".repeat(32)).unwrap(),
        );
        let receipt = DomainReceipt {
            context: Context {
                binding: Binding::parse(&"4".repeat(32), &"5".repeat(32), &"6".repeat(32)).unwrap(),
                revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
                request: request.clone(),
            },
            result: TerminalSendResult::new(request, Some(target), outcome).unwrap(),
            records: 2,
            bytes: 10,
        };
        assert_eq!(receipt.verdict(), expected_verdict);
        let failure =
            DomainFailure::Start(StartFailure::InputNotAccepted(receipt.refusal().unwrap()));
        let report = Report::from(DomainReport::from(ReportFixture {
            name: Some(AgentName::parse("alpha").unwrap()),
            provider: AgentProviderId::parse("codex").unwrap(),
            reservation: Some(ObapRequestId::parse(&"1".repeat(32)).unwrap()),
            last_observed: None,
            reservation_state: ReservationState::Released,
            submission: Some(Submission::Sent(Box::new(receipt))),
            delivery,
            elapsed_ms: 1,
            verdict: DomainVerdict::Failed {
                failure,
                read_cause: None,
            },
        }));
        assert_eq!(report.failure_kind(), Some(failure_kind));
        let answer = project(report, &here(), OutputFormat::Json, Locale::EnUs, None);
        assert_eq!(answer.status, status);
        let envelope: Value = serde_json::from_slice(&answer.stdout).unwrap();
        assert!(schema.is_valid(&envelope), "{envelope}");
        assert_eq!(envelope["failure"]["details"]["submission"], document);
        assert!(
            envelope["failure"]["details"].get("delivery").is_none(),
            "delivery is private state, not a new CLI field"
        );
    }
}

#[test]
fn startup_outputs_preserve_delivery_and_reservation_facts_under_both_locales() {
    let receipt = super::super::send::test_receipt(TerminalSendOutcome::Accepted {
        progress: SendProgress::complete(2, 10),
        cleanup: SendCleanupStatus::Confirmed,
    })
    .document();
    let success_schema = jsonschema::validator_for(
        &serde_json::from_str(include_str!("../schemas/orchbee.agent.start.json")).unwrap(),
    )
    .unwrap();
    let failure_schema = jsonschema::validator_for(
        &serde_json::from_str(include_str!(
            "../schemas/orchbee.cli.failure.agent-start.json"
        ))
        .unwrap(),
    )
    .unwrap();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (kind, reason, state) in [
            (
                FailureKind::Timeout,
                "start-deadline",
                ReservationState::Pending,
            ),
            (
                FailureKind::Cancelled,
                "cancelled",
                ReservationState::Released,
            ),
            (
                FailureKind::OutcomeUnknown,
                "reply-not-confirmed",
                ReservationState::Unknown,
            ),
        ] {
            let answer = project(
                report(
                    &receipt,
                    state,
                    Verdict::Failed {
                        failure: super::failure(kind, reason),
                        reason: reason.to_owned(),
                        cause: None,
                    },
                ),
                &here(),
                OutputFormat::Json,
                locale,
                None,
            );
            let document: Value = serde_json::from_slice(&answer.stdout).unwrap();
            assert!(failure_schema.is_valid(&document), "{document}");
            assert_eq!(document["failure"]["details"]["submission"], receipt);
            assert_eq!(
                document["failure"]["details"]["reservationState"],
                state.as_str()
            );
            assert_eq!(answer.status, kind.exit_code());
            assert!(answer.stderr.contains(if locale == Locale::EnUs {
                "No process was terminated"
            } else {
                "没有终止进程"
            }));
        }
        let answer = project(
            report(
                &receipt,
                ReservationState::Completed,
                Verdict::Started {
                    session: AgentSessionId::parse("s1").unwrap(),
                },
            ),
            &here(),
            OutputFormat::Json,
            locale,
            None,
        );
        let document: Value = serde_json::from_slice(&answer.stdout).unwrap();
        assert!(success_schema.is_valid(&document), "{document}");
        assert_eq!(document["result"]["submission"], receipt);
        assert_eq!(document["result"]["confirmation"], "session");

        // An observed start has no Session to name and still satisfies the same contract.
        let answer = project(
            report(
                &receipt,
                ReservationState::Released,
                Verdict::Observed {
                    panel: orchbee_workbench_protocol::WorkbenchPanelId::parse(&"a".repeat(32))
                        .unwrap(),
                },
            ),
            &here(),
            OutputFormat::Json,
            locale,
            None,
        );
        assert_eq!(answer.status, 0);
        let document: Value = serde_json::from_slice(&answer.stdout).unwrap();
        assert!(success_schema.is_valid(&document), "{document}");
        assert_eq!(document["result"]["confirmation"], "observed");
        assert!(document["result"].get("sessionId").is_none());
        assert_eq!(document["result"]["submission"], receipt);

        // A start without a name reserves nothing and promises no ready Agent.
        let mut sent = report(
            &receipt,
            ReservationState::None,
            Verdict::Sent {
                panel: orchbee_workbench_protocol::WorkbenchPanelId::parse(&"a".repeat(32))
                    .unwrap(),
            },
        );
        sent.name = None;
        sent.reservation = None;
        let answer = project(sent, &here(), OutputFormat::Json, locale, None);
        assert_eq!(answer.status, 0);
        let document: Value = serde_json::from_slice(&answer.stdout).unwrap();
        assert!(success_schema.is_valid(&document), "{document}");
        assert_eq!(document["result"]["confirmation"], "sent");
        assert_eq!(document["result"]["interactiveReady"], false);
        assert_eq!(document["result"]["panelId"], "a".repeat(32));
    }
}

#[test]
fn finish_boundary_replacement_retains_only_the_workflow_read_cause() {
    let receipt = serde_json::json!({"kind":"fixture"});
    let read_cause = serde_json::json!({"kind":"transport-loss", "stage":"agent-read"});
    let mut retained = report(
        &receipt,
        ReservationState::Pending,
        Verdict::Failed {
            failure: super::failure(FailureKind::Timeout, "start-deadline"),
            reason: "start-deadline".into(),
            cause: Some(read_cause.clone()),
        },
    );
    retained.replace_failure(super::failure(FailureKind::Cancelled, "cancelled"));
    let Verdict::Failed { cause, .. } = retained.verdict else {
        panic!("replacement stays failed")
    };
    assert_eq!(cause, Some(read_cause));

    let mut upstream = report(
        &receipt,
        ReservationState::None,
        Verdict::Failed {
            failure: Failure::new(FailureKind::TransportLoss)
                .details(super::super::stage_details("admission")),
            reason: "startup-failed".into(),
            cause: Some(serde_json::json!({"kind":"transport-loss"})),
        },
    );
    upstream.replace_failure(super::failure(FailureKind::Cancelled, "cancelled"));
    let Verdict::Failed { cause, .. } = upstream.verdict else {
        panic!("replacement stays failed")
    };
    assert_eq!(cause, None);
}

fn command(place: AgentStartPlace, name: Option<&str>, args: &[&str]) -> AgentStartCommand {
    AgentStartCommand {
        name: name.map(|name| AgentName::parse(name).unwrap()),
        provider: "codex".into(),
        place,
        panel: None,
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        timeout_ms: 30000,
    }
}

fn schemas() -> (jsonschema::Validator, jsonschema::Validator) {
    let load =
        |text: &str| jsonschema::validator_for(&serde_json::from_str(text).unwrap()).unwrap();
    (
        load(include_str!("../schemas/orchbee.agent.start.json")),
        load(include_str!(
            "../schemas/orchbee.cli.failure.agent-start.json"
        )),
    )
}

#[test]
fn own_panel_here_is_refused_from_the_caller_context_alone() {
    let caller = |name: &str| match name {
        "ORCHBEE_TERMINAL_ID" => Some(std::ffi::OsString::from("1".repeat(32))),
        "ORCHBEE_WORKER_ID" => Some(std::ffi::OsString::from("2".repeat(32))),
        _ => None,
    };
    // `--here` aimed at the caller's own Panel is decided before any runtime is discovered.
    let Err(refused) = reference(&command(AgentStartPlace::Here, None, &[]), caller) else {
        panic!("--here without --panel names the caller's own Panel")
    };
    assert_eq!(refused.kind, FailureKind::InvalidUsage);
    assert_eq!(
        refused.details.as_ref().unwrap()["reason"],
        serde_json::json!("own-panel")
    );
    // An explicit Panel is compared with the caller's own once the Snapshot resolves both.
    let mut explicit = command(AgentStartPlace::Here, None, &[]);
    explicit.panel = Some(crate::invocation::Target::Id("a".repeat(32)));
    assert!(reference(&explicit, caller).unwrap().caller.is_some());
    // Every other place takes the caller's own Panel as its reference.
    for place in [
        AgentStartPlace::NewTab,
        AgentStartPlace::SplitRight,
        AgentStartPlace::SplitDown,
    ] {
        assert!(
            reference(&command(place, None, &[]), caller)
                .unwrap()
                .caller
                .is_none()
        );
    }
}

#[test]
fn only_an_exact_catalog_start_in_a_new_panel_runs_as_the_first_command() {
    for place in [
        AgentStartPlace::NewTab,
        AgentStartPlace::SplitRight,
        AgentStartPlace::SplitDown,
        AgentStartPlace::Here,
    ] {
        for (name, args) in [
            (None, &[][..]),
            (None, &["--model", "o3"][..]),
            (Some("alpha"), &[][..]),
        ] {
            let first = first_command(&command(place, name, args));
            let expected = place != AgentStartPlace::Here && name.is_none() && args.is_empty();
            assert_eq!(first.is_some(), expected, "{place:?} {name:?} {args:?}");
            if let Some(selector) = first {
                assert_eq!(selector.agent().id(), "codex");
            }
        }
    }
}

#[test]
fn new_panel_outcomes_name_their_place_under_both_locales() {
    let (success_schema, failure_schema) = schemas();
    let receipt = super::super::send::test_receipt(TerminalSendOutcome::Accepted {
        progress: SendProgress::complete(2, 10),
        cleanup: SendCleanupStatus::Confirmed,
    })
    .document();
    let new = |place| Placed {
        place,
        panel: Some(panel_a()),
        created: true,
    };
    let unnamed = |submission: Option<Value>, verdict| Report {
        name: None,
        provider: AgentProviderId::parse("codex").unwrap(),
        reservation: None,
        last_observed: None,
        reservation_state: ReservationState::None,
        submission,
        submission_status: None,
        elapsed_ms: 1,
        verdict,
    };
    let panel = "a".repeat(32);
    for locale in [Locale::EnUs, Locale::ZhCn] {
        // The new shell runs the catalog Agent first: nothing was typed.
        let sent = || Verdict::Sent { panel: panel_a() };
        let placed = new(AgentStartPlace::NewTab);
        let answer = project(
            unnamed(None, sent()),
            &placed,
            OutputFormat::Json,
            locale,
            None,
        );
        assert_eq!(answer.status, 0);
        let document: Value = serde_json::from_slice(&answer.stdout).unwrap();
        assert!(success_schema.is_valid(&document), "{document}");
        assert_eq!(document["result"]["confirmation"], "launched");
        assert_eq!(document["result"]["place"], "new-tab");
        assert_eq!(document["result"]["panelId"], panel);
        assert_eq!(document["result"]["submission"], Value::Null);
        assert_eq!(document["result"]["interactiveReady"], false);
        let text = project(
            unnamed(None, sent()),
            &placed,
            OutputFormat::Text,
            locale,
            None,
        );
        let text = String::from_utf8(text.stdout).unwrap();
        assert!(
            text.starts_with(&match locale {
                Locale::EnUs => format!("Started Codex in a new Tab (Panel {panel}).\n"),
                Locale::ZhCn => format!("已在新标签页（面板 {panel}）中启动 Codex。\n"),
            }),
            "{text}"
        );
        assert!(text.contains(&format!("orchbee agent wait --panel {panel} --until idle")));

        // Arguments were typed at the split's first prompt.
        let placed = new(AgentStartPlace::SplitDown);
        let answer = project(
            unnamed(Some(receipt.clone()), sent()),
            &placed,
            OutputFormat::Json,
            locale,
            None,
        );
        let document: Value = serde_json::from_slice(&answer.stdout).unwrap();
        assert!(success_schema.is_valid(&document), "{document}");
        assert_eq!(document["result"]["confirmation"], "sent");
        assert_eq!(document["result"]["place"], "split-down");

        // A later failure keeps the new Panel and says so.
        let mut failed = report(
            &receipt,
            ReservationState::Released,
            Verdict::Failed {
                failure: super::failure(FailureKind::Timeout, "start-deadline"),
                reason: "start-deadline".into(),
                cause: None,
            },
        );
        failed.submission_status = None;
        let answer = project(failed, &placed, OutputFormat::Json, locale, None);
        let document: Value = serde_json::from_slice(&answer.stdout).unwrap();
        assert!(failure_schema.is_valid(&document), "{document}");
        assert_eq!(document["failure"]["details"]["createdPanelId"], panel);
        assert_eq!(document["failure"]["details"]["place"], "split-down");
        assert!(answer.stderr.contains(&match locale {
            Locale::EnUs => format!("The new Panel stays open as Panel {panel}."),
            Locale::ZhCn => format!("新面板保留为面板 {panel}。"),
        }));

        // A refused split names the Server's reason and keeps the creation's exit.
        let rejected = Failure::new(FailureKind::Rejected).details(serde_json::Map::from_iter([
            ("stage".to_owned(), serde_json::json!("command")),
            ("reason".to_owned(), serde_json::json!("panel-unavailable")),
            ("revision".to_owned(), serde_json::json!("3")),
        ]));
        let refused = Report::place_failed(
            crate::agent_start::Request::new(
                None,
                AgentProviderId::parse("codex").unwrap(),
                vec![],
            ),
            Instant::now(),
            rejected,
            "panel.split",
        );
        let placed = Placed {
            place: AgentStartPlace::SplitRight,
            panel: None,
            created: false,
        };
        let answer = project(refused, &placed, OutputFormat::Json, locale, None);
        assert_eq!(answer.status, FailureKind::Rejected.exit_code());
        let document: Value = serde_json::from_slice(&answer.stdout).unwrap();
        assert!(failure_schema.is_valid(&document), "{document}");
        let details = &document["failure"]["details"];
        assert_eq!(details["reason"], "place-failed");
        assert_eq!(details["createdPanelId"], Value::Null);
        assert_eq!(details["cause"]["operation"], "panel.split");
        assert_eq!(details["cause"]["reason"], "panel-unavailable");
        assert!(
            answer.stderr.contains("panel-unavailable"),
            "{}",
            answer.stderr
        );

        // The own-Panel refusal is a usage failure that names the new-Tab flag.
        let own = Report::preflight(
            crate::agent_start::Request::new(
                None,
                AgentProviderId::parse("codex").unwrap(),
                vec![],
            ),
            Instant::now(),
            super::failure(FailureKind::InvalidUsage, "own-panel"),
        );
        let placed = Placed {
            place: AgentStartPlace::Here,
            panel: None,
            created: false,
        };
        let answer = project(own, &placed, OutputFormat::Json, locale, None);
        assert_eq!(answer.status, 2);
        let document: Value = serde_json::from_slice(&answer.stdout).unwrap();
        assert!(failure_schema.is_valid(&document), "{document}");
        assert!(answer.stderr.contains("--new-tab"), "{}", answer.stderr);
    }
}
