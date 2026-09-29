use super::*;
use crate::local_client::test_server::FakeServer;
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use std::{io, num::NonZeroU64};

fn id(n: u64) -> String {
    format!("{n:032x}")
}
pub(crate) fn fixture() -> Snapshot {
    Snapshot {
        protocol_version: ProtocolVersion,
        site_id: SiteId::parse(&id(1)).unwrap(),
        revision: WorkbenchRevision::new(1),
        navigation: Navigation {
            projects: Vec::new(),
            workspace_order: Vec::new(),
            standalone_terminal_context_id: WorkbenchTerminalContextId::parse(&id(2)).unwrap(),
        },
        terminal_contexts: vec![TerminalContext {
            terminal_context_id: WorkbenchTerminalContextId::parse(&id(2)).unwrap(),
            groups: Vec::new(),
            tabs: [10, 20]
                .into_iter()
                .map(|n| Tab {
                    automatic_label: false,
                    group_id: None,
                    label: "tab".into(),
                    pinned: false,
                    tab_id: WorkbenchTabId::parse(&id(n)).unwrap(),
                    root: PanelNode::Panel {
                        right_click: Default::default(),
                        panel_id: WorkbenchPanelId::parse(&id(n + 1)).unwrap(),
                        primary_presentation: presentation(n + 2),
                        child_presentations: vec![presentation(n + 3)],
                    },
                })
                .collect(),
        }],
    }
}
fn presentation(n: u64) -> Presentation {
    Presentation {
        custom_label: None,
        label: "terminal\u{202e}label".into(),
        presentation_id: TerminalPresentationId::parse(&id(n)).unwrap(),
        binding: Some(PresentationBinding {
            worker_id: WorkerId::parse(&id(99)).unwrap(),
            terminal_id: TerminalId::parse(&id(n + 100)).unwrap(),
        }),
    }
}
pub(crate) fn physical(n: u64) -> AgentLaunchBinding {
    use orchbee_agent_observation_protocol::*;
    AgentLaunchBinding {
        presentation_id: AgentPresentationId::parse(&id(n)).unwrap(),
        generation: AgentAttachmentGeneration::parse(&id(1)).unwrap(),
        host_epoch: AgentHostEpoch::parse(&id(2)).unwrap(),
        host_terminal_id: AgentHostTerminalId::parse(&id(n + 100)).unwrap(),
    }
}
pub(crate) fn target() -> Target {
    Target {
        context: NativeContextTarget {
            site_id: SiteId::parse(&id(1)).unwrap(),
            location: NativeContextLocation::Standalone(
                WorkbenchTerminalContextId::parse(&id(2)).unwrap(),
            ),
        },
        tab: WorkbenchTabId::parse(&id(20)).unwrap(),
        panel: WorkbenchPanelId::parse(&id(21)).unwrap(),
        member: TerminalPresentationId::parse(&id(23)).unwrap(),
        terminal: presentation(23).binding.unwrap(),
        physical: physical(23),
        agent: "codex".into(),
    }
}
fn state(member: &TerminalPresentationId) -> TerminalAgentStateRow {
    let n = u64::from_str_radix(member.as_str(), 16).unwrap();
    TerminalAgentStateRow {
        presentation_id: member.clone(),
        snapshot: Some(TerminalAgentStateSnapshot {
            binding: physical(n),
            sequence: WorkbenchRevision::new(8),
            agent: (n == 12 || n == 23).then(|| "codex".into()),
            state: PanelAgentState::Working,
        }),
    }
}
pub(crate) fn accept(stream: &mut orchbee_local_ipc::LocalStream) -> io::Result<()> {
    assert!(matches!(
        read_message(stream).map_err(io::Error::other)?,
        LocalClientMessage::ClientHello(_)
    ));
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::new(1).unwrap(),
        },
    )
    .map_err(io::Error::other)
}
fn peer(bad: u8) -> io::Result<FakeServer> {
    FakeServer::start(move |mut stream| {
        accept(&mut stream)?;
        let WorkbenchRequest::SnapshotRead(request) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("snapshot only")
        };
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: fixture(),
                },
            }),
        )
        .map_err(io::Error::other)?;
        for round in 0..=1 {
            let WorkbenchRequest::PanelAgent(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("read-only states")
            };
            let PanelAgentAction::States { presentation_ids } = &request.action else {
                panic!("no mutation")
            };
            assert_eq!(presentation_ids.len(), if round == 0 { 4 } else { 0 });
            let mut rows: Vec<_> = presentation_ids.iter().map(state).collect();
            if bad == 2 && round == 0 {
                rows.pop();
            }
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::PanelAgent(PanelAgentResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    site_id: request.site_id,
                    outcome: PanelAgentOutcome::States {
                        workbench_revision: WorkbenchRevision::new(1),
                        report_revision: WorkbenchRevision::new(if bad == 1 && round == 1 {
                            3
                        } else {
                            2
                        }),
                        presentations: rows,
                    },
                }),
            )
            .map_err(io::Error::other)?;
            if bad == 2 {
                break;
            }
        }
        Ok(())
    })
}
#[test]
fn agent_checkpoint_joins_exact_primary_and_child_without_terminal_effects() -> io::Result<()> {
    let peer = peer(0)?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut client = LocalClient::connect_before(peer.endpoint(), "test-cohort", deadline)
        .map_err(io::Error::other)?;
    let rows = read(&mut client, deadline, &AtomicU8::new(0)).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].target, target());
    assert_eq!(rows[0].target.member.as_str(), id(12));
    assert!(!rows[0].label.contains('\u{202e}'));
    drop(client);
    peer.finish()
}
#[test]
fn agent_checkpoint_rejects_partial_membership_and_spliced_report_revisions() -> io::Result<()> {
    for bad in [1, 2] {
        let peer = peer(bad)?;
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut client = LocalClient::connect_before(peer.endpoint(), "test-cohort", deadline)
            .map_err(io::Error::other)?;
        assert!(read(&mut client, deadline, &AtomicU8::new(0)).is_err());
        drop(client);
        peer.finish()?;
    }
    Ok(())
}
#[test]
fn agent_placements_respect_membership_and_row_capacity() {
    let mut snapshot = fixture();
    assert_eq!(placements(&snapshot).unwrap().len(), 4);
    snapshot.terminal_contexts.clear();
    assert!(placements(&snapshot).is_err());
    let snapshot = fixture();
    let mut rows = Vec::new();
    for _ in 0..MAXIMUM_ROWS / 2 {
        collect(
            &snapshot.terminal_contexts[0].tabs[0].root,
            &target().context,
            &target().tab,
            "",
            &mut rows,
        )
        .unwrap();
    }
    assert_eq!(rows.len(), MAXIMUM_ROWS);
    assert!(
        collect(
            &snapshot.terminal_contexts[0].tabs[0].root,
            &target().context,
            &target().tab,
            "",
            &mut rows
        )
        .is_err()
    );
}

fn source_connection() -> io::Result<Connection> {
    // The scheduler seam never reconnects; this admission supplies a real owned origin.
    let peer = FakeServer::start(|mut stream| accept(&mut stream))?;
    let client = LocalClient::connect_before(
        peer.endpoint(),
        "test-cohort",
        Instant::now() + Duration::from_secs(3),
    )
    .map_err(io::Error::other)?;
    let source = Connection::from_origin(client.origin(), client.operation_clock());
    drop(client);
    peer.finish()?;
    Ok(source)
}
fn sources(connection: &Connection, count: usize) -> Vec<Source> {
    (0..count)
        .map(|n| Source {
            choice: Choice::Remote(ProfileId::parse(&id(n as u64 + 1)).unwrap()),
            connection: connection.clone(),
        })
        .collect()
}
fn completed(at: Instant) -> Job {
    let task = thread::spawn(move || Completed {
        at,
        rows: Ok(Arc::from([])),
    });
    let cutoff = Instant::now() + Duration::from_secs(3);
    while !task.is_finished() {
        assert!(Instant::now() < cutoff);
        thread::yield_now();
    }
    Job {
        cancel: Arc::new(AtomicU8::new(0)),
        task: Some(task),
    }
}
#[test]
fn agent_collector_bounds_workers_and_cancels_removed_sources() -> io::Result<()> {
    let connection = source_connection()?;
    let now = Instant::now();
    let mut collector = Collector::default();
    let mut flags = Vec::new();
    collector.tick_with_start(sources(&connection, 10), now, |_, deadline| {
        assert_eq!(deadline, now + Duration::from_secs(5));
        let cancel = Arc::new(AtomicU8::new(0));
        flags.push(cancel.clone());
        let worker = cancel.clone();
        let task = thread::spawn(move || {
            while worker.load(Ordering::Acquire) == 0 && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(1));
            }
            Completed {
                at: Instant::now(),
                rows: Ok(Arc::from([])),
            }
        });
        Ok(Job {
            cancel,
            task: Some(task),
        })
    });
    assert_eq!(flags.len(), 4);
    collector.tick_with_start(sources(&connection, 10), now, |_, _| {
        panic!("no fifth worker")
    });
    assert!(
        collector
            .tick_with_start(Vec::new(), now, |_, _| panic!("removed source"))
            .is_empty()
    );
    assert!(flags.iter().all(|f| f.load(Ordering::Acquire) == 1));
    drop(collector); // Joins every cancelled writer before fixture disposal.
    Ok(())
}
#[test]
fn agent_collector_rotates_past_four_sources_without_waiting_for_first_machines_to_stale()
-> io::Result<()> {
    let connection = source_connection()?;
    let now = Instant::now();
    let mut collector = Collector::default();
    let mut latest = Vec::new();
    for turn in 0..5 {
        let at = now + Duration::from_millis(turn);
        latest = collector.tick_with_start(sources(&connection, 10), at, |_, _| Ok(completed(at)));
    }
    assert_eq!(latest.len(), 10);
    assert!(
        latest
            .iter()
            .all(|(_, o)| matches!(o, Observation::Ready(rows) if rows.is_empty()))
    );
    Ok(())
}
#[test]
fn agent_collector_ages_from_worker_completion_and_discards_previous_generation() -> io::Result<()>
{
    let connection = source_connection()?;
    let now = Instant::now();
    let mut collector = Collector::default();
    collector.tick_with_start(sources(&connection, 1), now, |_, _| Ok(completed(now)));
    let stale = collector.tick_with_start(
        sources(&connection, 1),
        now + FRESH_FOR + Duration::from_secs(1),
        |_, _| panic!("cooldown starts at consumption"),
    );
    assert_eq!(stale[0].1, Observation::Unavailable);
    collector.clear();
    let next = collector.tick_with_start(sources(&connection, 1), now, |_, _| Ok(completed(now)));
    assert_eq!(
        next[0].1,
        Observation::Loading,
        "cleared checkpoint must not resurface on profile replacement"
    );
    Ok(())
}

#[test]
fn agent_checkpoint_cancel_interrupts_real_inflight_read_without_followup() -> io::Result<()> {
    let (arrived, reading) = std::sync::mpsc::sync_channel(1);
    let peer = FakeServer::start(move |mut stream| {
        accept(&mut stream)?;
        assert!(matches!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::SnapshotRead(_)
        ));
        arrived.send(()).map_err(io::Error::other)?;
        // Cancellation closes the owned lane; no States, Attach or other followup.
        assert!(read_workbench_request(&mut stream).is_err());
        Ok(())
    })?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut client = LocalClient::connect_before(peer.endpoint(), "test-cohort", deadline)
        .map_err(io::Error::other)?;
    let cancel = Arc::new(AtomicU8::new(0));
    let worker = cancel.clone();
    let task = thread::spawn(move || read(&mut client, deadline, &worker));
    reading
        .recv_timeout(Duration::from_secs(2))
        .map_err(io::Error::other)?;
    cancel.store(1, Ordering::Release);
    assert!(task.join().unwrap().is_err());
    peer.finish()
}
