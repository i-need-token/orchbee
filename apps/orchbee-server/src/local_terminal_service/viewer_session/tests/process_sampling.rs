use super::fixture::{Effect, Fixture, static_codex_process};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
#[test]
fn explanation_process_refusal_releases_viewer_and_uncertain_detach_keeps_ownership() {
    let fixture = Fixture::new();
    let target = fixture
        .service
        .coordinator
        .lock()
        .unwrap()
        .process_observation_targets()
        .unwrap()
        .remove(0);
    let stop = std::sync::atomic::AtomicBool::new(true);
    let detector = crate::agent_screen::ScreenDetector::bundled().unwrap();
    let provider = orchbee_agent_observation_protocol::AgentReportProvider::Codex;
    let (captured, pending) = fixture
        .service
        .sample_agent_explanation(&target, &stop, &detector, provider);
    assert!(captured.is_none() && pending.is_none());
    assert!(fixture.barrier().unwrap().is_empty());
    stop.store(false, Ordering::Release);
    let (captured, pending) = fixture
        .service
        .sample_agent_explanation(&target, &stop, &detector, provider);
    assert!(
        captured.is_none() && pending.is_none(),
        "failed process read cannot produce a guessed diagnostic"
    );
    let effects = fixture.barrier().unwrap();
    let [
        Effect::Attach(a, _),
        Effect::ProcessInfo(b),
        Effect::Detach(c),
    ] = effects.as_slice()
    else {
        panic!("{effects:?}")
    };
    assert_eq!(a, b);
    assert_eq!(b, c);
    fixture.effects.lock().unwrap().clear();
    fixture.detach_refusals.store(1, Ordering::Release);
    let (captured, pending) = fixture
        .service
        .sample_agent_explanation(&target, &stop, &detector, provider);
    assert!(captured.is_none());
    let pending = pending.expect("uncertain detach transfers the exact cleanup obligation");
    assert!(pending.settle(&fixture.service));
    let effects = fixture.barrier().unwrap();
    let [
        Effect::Attach(a, _),
        Effect::ProcessInfo(b),
        Effect::DetachRefused(c),
        Effect::Detach(d),
    ] = effects.as_slice()
    else {
        panic!("{effects:?}")
    };
    assert_eq!(a, b);
    assert_eq!(b, c);
    assert_eq!(c, d);
}
#[test]
fn periodic_turns_observe_lifecycle_and_content_before_process_refusal_and_release_before_next_attach()
 {
    let fixture = Fixture::new();
    let mut cursor = None;
    fixture.service.process_observer_turn_for_test(&mut cursor);
    let effects = fixture.barrier().unwrap();
    assert_eq!(effects.len(), 56);
    for events in effects.chunks_exact(7) {
        let [
            Effect::Attach(a, _),
            Effect::Lifecycle(l),
            Effect::Poll(p1),
            Effect::Feedback(f),
            Effect::Poll(p2),
            Effect::ProcessInfo(b),
            Effect::Detach(c),
        ] = events
        else {
            panic!("unexpected effects: {events:?}")
        };
        assert_eq!(a, b);
        assert_eq!(b, c);
        assert!([l, p1, f, p2].iter().all(|id| *id == a));
    }
    assert!(!fixture.service.process_observer_pending_for_test());
    fixture.service.process_observer_turn_for_test(&mut cursor);
    let effects = fixture.barrier().unwrap();
    let targets: std::collections::BTreeSet<_> = effects
        .iter()
        .filter_map(|e| {
            if let Effect::Attach(_, target) = e {
                Some(target)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(targets.len(), 10);
}

#[test]
fn static_codex_refreshes_the_retained_frame_after_process_before() {
    use orchbee_workbench_protocol::{
        PanelAgentAction, PanelAgentOutcome, PanelAgentRequest, PanelAgentState, ProtocolVersion,
        WorkbenchRequestId,
    };
    let fixture = Fixture::with_static_codex();
    let target = fixture
        .service
        .coordinator
        .lock()
        .unwrap()
        .process_observation_targets()
        .unwrap()
        .remove(0);
    let seed = static_codex_process(0, [1; 16]);
    let origin = Instant::now() - Duration::from_secs(3);
    assert!(
        fixture
            .service
            .coordinator
            .lock()
            .unwrap()
            .observe_host_process(target.target, &seed, origin, origin,)
    );
    let stop = std::sync::atomic::AtomicBool::new(false);
    assert!(
        fixture
            .service
            .sample_process_target(
                &target,
                &stop,
                &crate::agent_screen::ScreenDetector::bundled().unwrap(),
            )
            .is_none(),
        "the exact Detach must settle before the sampler returns",
    );
    let effects = fixture.barrier().unwrap();
    let [
        Effect::Attach(attachment, 0),
        Effect::Lifecycle(lifecycle),
        Effect::Poll(first),
        Effect::Feedback(first_reset),
        Effect::Poll(retained),
        Effect::ProcessInfo(before),
        Effect::Feedback(second_reset),
        Effect::Poll(captured),
        Effect::ProcessInfo(after),
        Effect::Detach(detach),
    ] = effects.as_slice()
    else {
        panic!(
            "the retained frame must receive an exact second snapshot reset before one capture poll; current trace: {effects:?}"
        )
    };
    assert!(
        [
            lifecycle,
            first,
            first_reset,
            retained,
            before,
            second_reset,
            captured,
            after,
            detach,
        ]
        .into_iter()
        .all(|id| id == attachment)
    );

    let snapshot = super::fixture::read_snapshot(&fixture.service);
    let pane = orchbee_server::semantic_panes_for_binding(&snapshot, &target.binding).remove(0);
    let result = fixture.service.execute_panel_agent(PanelAgentRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 77_777)).unwrap(),
        site_id: snapshot.site_id,
        action: PanelAgentAction::Read {
            panel_ids: vec![pane.panel_id],
        },
    });
    let mut panels = match result.outcome {
        PanelAgentOutcome::Read { panels, .. } => panels,
        other => panic!("agent read failed: {other:?}"),
    };
    let view = panels.remove(0).report.expect("bound Agent view");
    assert_eq!(view.agent.as_deref(), Some("codex"));
    assert_eq!(view.state, PanelAgentState::Working);
}

#[test]
fn semantic_events_keep_real_settlement_independent_of_process_refusal_and_reject_old_versions() {
    use orchbee_workbench_protocol::{
        SemanticEventFilter, SemanticEventKind as K, SemanticEventMatch, SemanticEventScope,
    };
    let fixture = Fixture::new();
    let journal = fixture.service.event_journal().unwrap();
    let cursor = journal.head().unwrap();
    let mut round = None;
    fixture.service.process_observer_turn_for_test(&mut round);
    let filters = [K::PaneExited, K::PaneOutputChanged].map(|event| SemanticEventFilter::Event {
        event,
        scope: SemanticEventScope::All {},
        condition: SemanticEventMatch::Any {},
    });
    let page = journal.read(&cursor, &filters, 32).unwrap();
    assert_eq!(page.events.len(), 8);
    assert!(page.events.iter().all(|event| event.event == K::PaneExited));
    let mut coordinator = fixture.service.coordinator.lock().unwrap();
    let target = coordinator.process_observation_targets().unwrap().remove(0);
    let baseline = journal.head().unwrap();
    let started = Instant::now();
    assert!(coordinator.observe_terminal_projection(
        target.target,
        Some((u64::MAX, 7)),
        Some((42, true)),
        started,
        started
    ));
    let changed = journal.read(&baseline, &filters, 32).unwrap();
    assert_eq!(changed.events.len(), 1);
    assert_eq!(changed.events[0].event, K::PaneOutputChanged);
    let started = Instant::now();
    assert!(!coordinator.observe_terminal_projection(
        target.target,
        Some((1, 9)),
        Some((41, false)),
        started,
        started
    ));
    assert!(
        journal
            .read(&changed.cursor, &filters, 32)
            .unwrap()
            .events
            .is_empty()
    );
}

#[test]
fn semantic_events_pending_physical_baseline_is_busy_and_real_source_refusal_never_becomes_timeout()
{
    use crate::workbench_service::{WorkbenchExecution, WorkbenchPolicy, WorkbenchService};
    use orchbee_workbench_protocol::{
        ProtocolVersion, SemanticEventFilter, SemanticEventSource, SemanticEventsAction,
        SemanticEventsFailure, SemanticEventsOutcome, SemanticEventsRequest, SemanticOutputMatch,
        SemanticOutputSource, WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse,
    };
    let fixture = Fixture::new();
    let snapshot = super::fixture::read_snapshot(&fixture.service);
    let target = fixture
        .service
        .coordinator
        .lock()
        .unwrap()
        .process_observation_targets()
        .unwrap()
        .remove(0);
    let pane = orchbee_server::semantic_panes_for_binding(&snapshot, &target.binding).remove(0);
    let service = WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: false,
            associate_directory: false,
            create_terminal: false,
            manage_terminal_lifecycle: false,
        },
        std::sync::Arc::clone(&fixture.service),
    );
    let session = service.client_session(std::num::NonZeroU64::MIN);
    let id = WorkbenchRequestId::parse(&"a".repeat(32)).unwrap();
    let exchange = |action| {
        let request = SemanticEventsRequest {
            protocol_version: ProtocolVersion,
            request_id: id.clone(),
            site_id: snapshot.site_id.clone(),
            action,
        };
        let WorkbenchExecution::Response(WorkbenchResponse::Events(result)) =
            session.execute(WorkbenchRequest::Events(request.clone()))
        else {
            panic!("event reply")
        };
        assert!(result.answers(&request));
        result.outcome
    };
    assert!(matches!(
        exchange(SemanticEventsAction::Subscribe {
            subscription_id: id.clone(),
            filters: vec![SemanticEventFilter::OutputMatched {
                target: SemanticEventSource {
                    panel_id: pane.panel_id,
                    presentation_id: Some(pane.presentation_id)
                },
                source: SemanticOutputSource::Recent,
                lines: 80,
                matcher: SemanticOutputMatch::Substring {
                    value: "match".into()
                },
                strip_ansi: true,
                client_id: None,
            }],
            after: None,
        }),
        SemanticEventsOutcome::Opened { .. }
    ));
    assert!(matches!(
        exchange(SemanticEventsAction::Wait {
            subscription_id: id.clone(),
            timeout_milliseconds: 0
        }),
        SemanticEventsOutcome::Failed {
            reason: SemanticEventsFailure::Busy
        }
    ));
    assert!(matches!(
        exchange(SemanticEventsAction::Wait {
            subscription_id: id.clone(),
            timeout_milliseconds: 1000
        }),
        SemanticEventsOutcome::Failed {
            reason: SemanticEventsFailure::SourceUnavailable
        }
    ));
    assert!(matches!(
        exchange(SemanticEventsAction::Close {
            subscription_id: id.clone()
        }),
        SemanticEventsOutcome::Closed { .. }
    ));
    let effects = fixture.barrier().unwrap();
    assert_eq!(
        effects
            .iter()
            .filter(|effect| matches!(effect, Effect::Attach(..)))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|effect| matches!(effect, Effect::Detach(..)))
            .count(),
        1
    );
}

#[test]
fn semantic_events_output_refusal_is_not_a_false_timeout_and_uncertain_cleanup_blocks_replacement()
{
    use crate::local_terminal_service::semantic_events::OutputRead;
    use orchbee_workbench_protocol::{SemanticEventsFailure as F, SemanticOutputSource};
    let fixture = Fixture::new();
    let target = fixture
        .service
        .coordinator
        .lock()
        .unwrap()
        .process_observation_targets()
        .unwrap()
        .remove(0);
    let snapshot = super::fixture::read_snapshot(&fixture.service);
    let pane = orchbee_server::semantic_panes_for_binding(&snapshot, &target.binding).remove(0);
    let stop = std::sync::atomic::AtomicBool::new(false);
    let read = OutputRead {
        source: SemanticOutputSource::Recent,
        lines: 80,
        strip_ansi: true,
        viewport: None,
    };
    assert!(matches!(
        fixture.service.capture_event_output(
            &target,
            pane.clone(),
            snapshot.revision,
            &read,
            &stop
        ),
        Err(F::SourceUnavailable)
    ));
    fixture.detach_refusals.store(1, Ordering::Release);
    assert!(matches!(
        fixture.service.capture_event_output(
            &target,
            pane.clone(),
            snapshot.revision,
            &read,
            &stop
        ),
        Err(F::PhysicalUnknown)
    ));
    let effects = fixture.barrier().unwrap();
    assert_eq!(
        effects
            .iter()
            .filter(|effect| matches!(effect, Effect::Attach(..)))
            .count(),
        2
    );
    assert!(matches!(
        fixture
            .service
            .capture_event_output(&target, pane, snapshot.revision, &read, &stop),
        Err(F::SourceUnavailable)
    ));
    let effects = fixture.barrier().unwrap();
    let refused = effects
        .iter()
        .position(|effect| matches!(effect, Effect::DetachRefused(..)))
        .unwrap();
    assert!(matches!(effects[refused + 1], Effect::Detach(..)));
    assert!(matches!(effects[refused + 2], Effect::Attach(..)));
}
#[test]
fn uncertain_cleanup_blocks_new_attachments_and_transfers_to_matching_worker_shutdown() {
    let fixture = Fixture::new();
    fixture.detach_refusals.store(2, Ordering::Release);
    let mut cursor = None;
    fixture.service.process_observer_turn_for_test(&mut cursor);
    assert!(fixture.service.process_observer_pending_for_test());
    fixture.service.process_observer_turn_for_test(&mut cursor);
    let effects = fixture.barrier().unwrap();
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Attach(..)))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::DetachRefused(..)))
            .count(),
        2
    );
    fixture.service.process_observer_turn_for_test(&mut cursor);
    assert!(!fixture.service.process_observer_pending_for_test());
    // A separate uncertainty survives sampler stop; exact retired Worker proof owns its release.
    fixture.detach_refusals.store(1, Ordering::Release);
    fixture.service.process_observer_turn_for_test(&mut cursor);
    let old = fixture
        .service
        .workers
        .lock()
        .unwrap()
        .request_handle()
        .unwrap();
    fixture
        .service
        .stop_process_observer_before(Instant::now())
        .unwrap();
    assert!(fixture.service.process_observer_pending_for_test());
    fixture.replace_worker();
    let current = fixture
        .service
        .workers
        .lock()
        .unwrap()
        .request_handle()
        .unwrap();
    assert!(
        fixture
            .service
            .finish_process_observer_after_worker_exit(&current)
            .is_err()
    );
    assert!(fixture.service.process_observer_pending_for_test());
    fixture
        .service
        .finish_process_observer_after_worker_exit(&old)
        .unwrap();
    assert!(!fixture.service.process_observer_pending_for_test());
}
#[test]
fn discovered_target_cannot_attach_a_new_worker_generation_or_run_after_cancel() {
    let fixture = Fixture::new();
    let target = fixture
        .service
        .coordinator
        .lock()
        .unwrap()
        .process_observation_targets()
        .unwrap()
        .remove(0);
    let stop = std::sync::atomic::AtomicBool::new(true);
    assert!(
        fixture
            .service
            .sample_process_target(
                &target,
                &stop,
                &crate::agent_screen::ScreenDetector::bundled().unwrap()
            )
            .is_none()
    );
    assert!(fixture.barrier().unwrap().is_empty());
    stop.store(false, Ordering::Release);
    fixture.replace_worker();
    assert!(
        fixture
            .service
            .sample_process_target(
                &target,
                &stop,
                &crate::agent_screen::ScreenDetector::bundled().unwrap()
            )
            .is_none()
    );
    assert!(fixture.barrier().unwrap().is_empty());
}

#[test]
fn checkpoint_viewer_cleanup_is_owned_after_sampler_join_and_prevents_another_capture() {
    let fixture = Fixture::new();
    assert!(fixture.service.settle_checkpoint_viewer(None).is_err());
    fixture
        .service
        .stop_process_observer_before(Instant::now())
        .unwrap();
    let (_, targets) = fixture
        .service
        .coordinator
        .lock()
        .unwrap()
        .checkpoint_observation_targets()
        .unwrap();
    fixture.detach_refusals.store(2, Ordering::Release);
    let (_, pending) = fixture.service.sample_checkpoint_directory(&targets[0]);
    assert!(pending.is_some());
    assert!(fixture.service.settle_checkpoint_viewer(pending).is_err());
    assert!(fixture.service.settle_checkpoint_viewer(None).is_err());
    assert!(fixture.service.process_observer_pending_for_test());
    let effects = fixture.barrier().unwrap();
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Attach(..)))
            .count(),
        1
    );
    fixture.service.settle_checkpoint_viewer(None).unwrap();
    assert!(!fixture.service.process_observer_pending_for_test());
    let (_, pending) = fixture.service.sample_checkpoint_directory(&targets[1]);
    assert!(pending.is_none());
    fixture.service.settle_checkpoint_viewer(pending).unwrap();
    let effects = fixture.barrier().unwrap();
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Attach(..)))
            .count(),
        2
    );
}
