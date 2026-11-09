//! The published-title read end to end: a real registered carrier, the production read pump and a
//! scripted Worker peer feed the registry, and the read answers each Presentation with its
//! Workbench binding and projected title. Ingesting and publishing titles leave the Workbench
//! revision, the commit count, the report revision and the semantic log unchanged (ORC-413).
use super::super::super::agent_drain_read::tests::{
    admit_generation, host_identity, open_member, register, registry, spawn_owner,
};
use super::*;
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, TerminalTitleChangePage, TerminalTitleChangesOutcome,
    TerminalTitleChangesResult, TerminalTitleObservation, TerminalTitleRow, TerminalTitlesChanged,
    read_message, write_message,
};
use orchbee_server::SessionChangeSignal;
use orchbee_workbench_protocol::{
    PanelAgentAction, SemanticEventCursor, WorkbenchRequestId, WorkbenchRevision,
};
use std::{
    num::NonZeroU64,
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

fn request(service: &LocalTerminalService, action: PanelAgentAction) -> PanelAgentRequest {
    let session_id = service
        .coordinator
        .lock()
        .unwrap()
        .close_authority()
        .unwrap()
        .snapshot()
        .session_id
        .clone();
    PanelAgentRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 7)).unwrap(),
        session_id,
        action,
    }
}

fn titles(
    service: &LocalTerminalService,
    ids: &[TerminalPresentationId],
) -> Vec<PresentationTitleRow> {
    let result = service.execute_panel_agent(request(
        service,
        PanelAgentAction::PresentationTitles {
            presentation_ids: ids.to_vec(),
        },
    ));
    let PanelAgentOutcome::PresentationTitles { presentations } = result.outcome else {
        panic!("presentation titles expected, got {:?}", result.outcome)
    };
    presentations
}

/// Every Session-wide counter title ingestion and publication must leave unchanged.
fn counters(
    service: &LocalTerminalService,
    presentation: &TerminalPresentationId,
    signal: &SessionChangeSignal,
) -> (
    WorkbenchRevision,
    WorkbenchRevision,
    SemanticEventCursor,
    (u64, u64),
) {
    let result = service.execute_panel_agent(request(
        service,
        PanelAgentAction::ReadPresentations {
            presentation_ids: vec![presentation.clone()],
        },
    ));
    let PanelAgentOutcome::PresentationsRead {
        workbench_revision,
        report_revision,
        ..
    } = result.outcome
    else {
        panic!("a presentation read expected, got {:?}", result.outcome)
    };
    let journal = service.event_journal().unwrap().head().unwrap();
    (
        workbench_revision,
        report_revision,
        journal,
        (signal.revision(), signal.commit_revision()),
    )
}

fn page(title: &str, next: u64) -> TerminalTitleChangesOutcome {
    TerminalTitleChangesOutcome::Page(TerminalTitleChangePage {
        rows: vec![TerminalTitleRow {
            identity: host_identity(),
            observation: TerminalTitleObservation::Observed {
                revision: NonZeroU64::new(next).unwrap(),
                title: Some(title.to_owned()),
            },
        }],
        next,
        complete: true,
    })
}

fn wait_for(
    service: &LocalTerminalService,
    ids: &[TerminalPresentationId],
    expected: &PresentationTitle,
) -> Vec<PresentationTitleRow> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let rows = titles(service, ids);
        if &rows[0].title == expected {
            return rows;
        }
        assert!(Instant::now() < deadline, "title never became {expected:?}");
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn background_titles_reach_the_read_without_advancing_the_session() {
    let workers = registry();
    let generation = admit_generation(&workers);
    let owner = register(&workers, generation);
    let service = LocalTerminalService::workspace_fixture(workers);
    let presentation = TerminalPresentationId::parse(&open_member(&service, generation)).unwrap();
    let unknown = TerminalPresentationId::parse(&format!("{:032x}", 5)).unwrap();
    let ids = [presentation.clone(), unknown];
    let binding = service
        .coordinator
        .lock()
        .unwrap()
        .agent_registration_snapshot()
        .unwrap()[0]
        .binding()
        .clone();
    let signal = Arc::new(SessionChangeSignal::default());
    service.watch_titles(&signal);
    service.event_journal().unwrap().watch(&signal);

    // Before any read, the bound Presentation carries its binding and no title yet.
    let before = titles(&service, &ids);
    assert_eq!(before[0].binding.as_ref(), Some(&binding));
    assert_eq!(before[0].title, PresentationTitle::Unavailable {});
    assert_eq!(before[1].binding, None);
    assert_eq!(before[1].title, PresentationTitle::Unavailable {});
    let _settled = counters(&service, &presentation, &signal);
    let baseline = counters(&service, &presentation, &signal);

    let (mut peer, owner_thread) = spawn_owner(owner);
    let (close, closed) = mpsc::channel::<()>();
    let worker = thread::spawn(move || {
        for (after, answer) in [
            (0, page("\u{280b} cargo build", 1)),
            (1, page("\u{1b}vim", 2)),
        ] {
            let CapabilityNodeMessage::TerminalTitleChangesRequest(request) =
                read_message(&mut peer).unwrap()
            else {
                panic!("a title change read expected")
            };
            assert_eq!(request.after, after);
            write_message(
                &mut peer,
                &CapabilityNodeMessage::TerminalTitleChangesResult(TerminalTitleChangesResult {
                    request,
                    outcome: answer,
                }),
            )
            .unwrap();
            if after == 0 {
                write_message(
                    &mut peer,
                    &CapabilityNodeMessage::TerminalTitlesChanged(TerminalTitlesChanged {
                        through: NonZeroU64::new(2).unwrap(),
                    }),
                )
                .unwrap();
            }
        }
        closed.recv().unwrap();
    });
    service.start_title_reads().unwrap();

    let first = wait_for(
        &service,
        &ids,
        &PresentationTitle::Observed {
            title: Some("cargo build".to_owned()),
        },
    );
    assert_eq!(first[0].binding.as_ref(), Some(&binding));
    assert_eq!(first[1].title, PresentationTitle::Unavailable {});
    wait_for(
        &service,
        &ids,
        &PresentationTitle::Observed {
            title: Some("vim".to_owned()),
        },
    );
    assert!(signal.titles_revision() >= 2);
    assert_eq!(counters(&service, &presentation, &signal), baseline);

    // A Worker that goes away takes its titles with it; the binding stays.
    close.send(()).unwrap();
    worker.join().unwrap();
    let _exit = owner_thread.join().unwrap();
    let after = wait_for(&service, &ids, &PresentationTitle::Unavailable {});
    assert_eq!(after[0].binding.as_ref(), Some(&binding));
    assert_eq!(counters(&service, &presentation, &signal), baseline);
    service
        .stop_title_reads_before(Instant::now() + Duration::from_secs(5))
        .unwrap();
}
