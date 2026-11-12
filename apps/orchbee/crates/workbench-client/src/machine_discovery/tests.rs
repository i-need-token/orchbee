use super::*;
use crate::client_context::ClientContextLocation;
use crate::local_client::cancellation::OwnedCancellation;
use crate::local_client::test_support::multiplex_peer::{admit, read_open, respond};
use crate::{local_client::test_support::server::FakeServer, machines::Draft};
use orchbee_workbench_protocol::{
    MultiplexMessage, Navigation, ProtocolVersion, StreamFrameKind, StreamMode, TerminalContext,
    WorkbenchRequest, WorkbenchResponse, WorkbenchSnapshotOutcome, WorkbenchSnapshotResult,
    WorkbenchTerminalContextId, read_multiplex_message, read_workbench_request,
    write_multiplex_message,
};
use orchbee_workbench_protocol::{Snapshot, WorkbenchProjectId};
use std::{num::NonZeroU64, thread, time::Duration};

fn profile(n: u8) -> Profile {
    Profile::new(
        ProfileId::parse(&format!("{n:032x}")).unwrap(),
        Draft::parse("remote", "example.invalid", "default").unwrap(),
    )
}
fn snapshot() -> Snapshot {
    let context = WorkbenchTerminalContextId::parse(&format!("{:032x}", 2)).unwrap();
    Snapshot {
        navigation: {
            let fixture_context_id = context.clone();
            let fixture_workspace_id = orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                fixture_context_id.as_str(),
            )
            .unwrap();
            Navigation {
                project_order: Vec::new(),
                directories: Vec::new(),
                workspaces: std::iter::once(orchbee_workbench_protocol::Workspace {
                    terminal_directory: None,
                    automatic_label: false,
                    workspace_id: fixture_workspace_id.clone(),
                    terminal_context_id: fixture_context_id.clone(),
                    project_id: None,
                    primary_directory_id: None,
                    label: "Workspace".into(),
                })
                .chain(Vec::new())
                .collect(),
                projects: Vec::new(),
                workspace_order: Vec::new(),
            }
        },
        protocol_version: ProtocolVersion,
        revision: WorkbenchRevision::new(17),
        session_id: SessionId::parse(&format!("{:032x}", 3)).unwrap(),
        terminal_contexts: vec![TerminalContext {
            groups: Vec::new(),
            tabs: Vec::new(),
            terminal_context_id: context,
        }],
    }
}
fn blocked_worker(_: Profile, generation: u64) -> io::Result<Worker> {
    Worker::spawn(generation, |cancel| {
        while !cancel.is_cancelled()? {
            thread::sleep(Duration::from_millis(1));
        }
        Err(io::ErrorKind::Interrupted.into())
    })
}
#[test]
fn startup_budget_and_profile_generations_survive_rename_replacement_and_readd() {
    let now = Instant::now();
    let mut d = Discovery::default();
    let mut profiles: Vec<_> = (1..=10).map(profile).collect();
    d.reconcile(&profiles, now);
    d.start_due(now, blocked_worker);
    assert_eq!(d.entries.iter().filter(|e| e.worker.is_some()).count(), 4);
    assert_eq!(
        d.observations(now)
            .iter()
            .filter(|(_, o)| matches!(o, Observation::Waiting))
            .count(),
        6
    );
    let generation = d.entries[0].generation;
    profiles[0].draft.label = "renamed".into();
    d.reconcile(&profiles, now);
    assert_eq!(d.entries[0].generation, generation);
    profiles[0].draft.connection = crate::machines::Connection::ssh("replacement.invalid").unwrap();
    d.reconcile(&profiles, now);
    assert_ne!(d.entries[0].generation, generation);
    d.entries[0].accept(Update {
        generation,
        event: Event::Failed {
            permanent: true,
            refusal: None,
            at: now,
        },
    });
    assert_eq!(d.entries[0].observation(now), Observation::Waiting);
    d.start_due(now, |_, _| {
        panic!("retiring connection must consume the startup budget")
    });
    let replacement = d.entries[0].generation;
    profiles[0].enabled = false;
    d.reconcile(&profiles, now);
    assert!(
        !d.observations(now)
            .iter()
            .any(|(id, _)| id == &profiles[0].id)
    );
    profiles[0].enabled = true;
    d.reconcile(&profiles, now);
    assert_ne!(d.entries[0].generation, replacement);
    d.reconcile(&[], now); // Invalid catalog follows this same suspension path.
    assert!(d.observations(now).is_empty());
    let deadline = Instant::now() + Duration::from_secs(2);
    while !d.retired.iter().all(Worker::finished) {
        assert!(
            Instant::now() < deadline,
            "retired observers were not cancelled"
        );
        thread::sleep(Duration::from_millis(1));
    }
    drop(d); // All workers observe cancellation before the first join.
}
#[test]
fn online_observers_release_startup_budget() {
    let now = Instant::now();
    let mut d = Discovery::default();
    d.reconcile(&(1..=8).map(profile).collect::<Vec<_>>(), now);
    d.start_due(now, blocked_worker);
    for e in &mut d.entries[..4] {
        e.policy.healthy(now);
    }
    d.start_due(now, blocked_worker);
    assert_eq!(d.entries.iter().filter(|e| e.worker.is_some()).count(), 8);
}
#[test]
fn repeated_failures_cannot_starve_later_profiles_when_the_ui_ticks_slowly() {
    let now = Instant::now();
    let mut d = Discovery::default();
    d.reconcile(&(1..=8).map(profile).collect::<Vec<_>>(), now);
    d.start_due(now, blocked_worker);
    for e in &mut d.entries[..4] {
        drop(e.worker.take());
        e.policy.fail(false, now);
    }
    d.start_due(now + Duration::from_secs(1), blocked_worker);
    assert!(d.entries[..4].iter().all(|e| e.worker.is_none()));
    assert!(d.entries[4..].iter().all(|e| e.worker.is_some()));
}
fn next_update(worker: &Worker) -> Update {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(update) = worker.take() {
            return update;
        }
        assert!(Instant::now() < deadline, "observer update");
        thread::yield_now();
    }
}
fn drain_close(stream: &mut orchbee_local_ipc::LocalStream) -> io::Result<()> {
    loop {
        match read_multiplex_message(stream) {
            Ok(MultiplexMessage::StreamClose { .. } | MultiplexMessage::LaneClose { .. }) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            result => panic!("only cancellation or carrier close expected: {result:?}"),
        }
    }
}
fn observer(endpoint: std::path::PathBuf, generation: u64) -> io::Result<Worker> {
    Worker::spawn(generation, move |_| {
        LocalClient::connect_before(
            &endpoint,
            &crate::local_client::test_support::handshake::offer(),
            Instant::now() + Duration::from_secs(2),
        )
        .map_err(io::Error::other)
    })
}
#[test]
fn terminal_subscription_refusals_require_attention_without_automatic_retry() -> io::Result<()> {
    for refusal in 0..4 {
        let server = FakeServer::start(move |mut stream| {
            admit(&mut stream, 1)?;
            let open = read_open(&mut stream)?;
            assert_eq!(open.mode, StreamMode::State);
            if refusal >= 2 {
                write_multiplex_message(
                    &mut stream,
                    &MultiplexMessage::Disconnect {
                        reason: if refusal == 2 {
                            orchbee_workbench_protocol::MultiplexDisconnectReason::Capacity
                        } else {
                            orchbee_workbench_protocol::MultiplexDisconnectReason::SlowConsumer
                        },
                    },
                )?;
                return drain_close(&mut stream);
            }
            let WorkbenchRequest::SnapshotRead(read) =
                read_workbench_request(&mut io::Cursor::new(&open.request))
                    .map_err(io::Error::other)?
            else {
                panic!("observer sent a mutation");
            };
            let request_id = if refusal == 1 {
                orchbee_workbench_protocol::WorkbenchRequestId::parse(
                    "11111111111111111111111111111111",
                )
                .unwrap()
            } else {
                read.request_id
            };
            respond(
                &mut stream,
                &open,
                &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                    protocol_version: ProtocolVersion,
                    request_id,
                    outcome: WorkbenchSnapshotOutcome::Denied {},
                }),
                1,
            )?;
            drain_close(&mut stream)
        })?;
        let worker = observer(server.endpoint().to_path_buf(), 1)?;
        let update = next_update(&worker);
        let mut d = Discovery::default();
        d.reconcile(&[profile(1)], Instant::now());
        d.entries[0].accept(update);
        assert_eq!(d.observations(Instant::now())[0].1, Observation::Attention);
        d.start_due(Instant::now() + Duration::from_secs(3600), |_, _| {
            panic!("permanent refusal retried")
        });
        drop(worker);
        server.finish()?;
    }
    Ok(())
}
#[test]
fn a_remote_refusal_survives_the_observer_and_clears_on_retry() -> io::Result<()> {
    let worker = Worker::spawn(1, |_| Err(Refusal::SessionTooOld.into()))?;
    let update = next_update(&worker);
    let remote = profile(1);
    let mut d = Discovery::default();
    d.reconcile(std::slice::from_ref(&remote), Instant::now());
    d.entries[0].accept(update);
    assert_eq!(d.observations(Instant::now())[0].1, Observation::Attention);
    assert_eq!(d.refusal(&remote), Some(Refusal::SessionTooOld));
    d.retry(&remote.id, Instant::now());
    assert_eq!(d.refusal(&remote), None);
    Ok(())
}
#[test]
fn discovered_origin_derives_a_logical_lane_on_the_single_admitted_carrier() -> io::Result<()> {
    let server = FakeServer::start(|mut stream| {
        admit(&mut stream, 1)?;
        for mode in [StreamMode::State, StreamMode::Request] {
            let open = read_open(&mut stream)?;
            assert_eq!(open.mode, mode);
            let WorkbenchRequest::SnapshotRead(read) =
                read_workbench_request(&mut io::Cursor::new(&open.request))
                    .map_err(io::Error::other)?
            else {
                panic!("unexpected mutation");
            };
            respond(
                &mut stream,
                &open,
                &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                    protocol_version: ProtocolVersion,
                    request_id: read.request_id,
                    outcome: WorkbenchSnapshotOutcome::Available {
                        snapshot: snapshot(),
                    },
                }),
                1,
            )?;
        }
        drain_close(&mut stream)
    })?;
    let worker = observer(server.endpoint().to_path_buf(), 1)?;
    let Event::Healthy { ready, .. } = next_update(&worker).event else {
        panic!("discovery failed");
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut selected = ready
        .connection
        .reconnect(deadline, OwnedCancellation::new().unwrap().borrow())?;
    let result = selected
        .submit_workbench_snapshot_read_before(
            orchbee_workbench_protocol::SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: orchbee_workbench_protocol::WorkbenchRequestId::parse(
                    "22222222222222222222222222222222",
                )
                .unwrap(),
            },
            deadline,
        )
        .map_err(io::Error::other)?;
    assert!(matches!(
        result.outcome,
        WorkbenchSnapshotOutcome::Available { .. }
    ));
    drop(selected);
    drop(ready);
    drop(worker);
    server.finish()
}
#[test]
fn native_observer_withdraws_on_gap_keeps_its_worker_and_cancels_without_snapshot_polling()
-> io::Result<()> {
    let (gap, send_gap) = std::sync::mpsc::sync_channel(1);
    let (fresh, send_fresh) = std::sync::mpsc::sync_channel(1);
    let (fail, send_failure) = std::sync::mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut stream| {
        admit(&mut stream, 1)?;
        let open = read_open(&mut stream)?;
        assert_eq!(open.mode, StreamMode::State);
        let WorkbenchRequest::SnapshotRead(read) =
            read_workbench_request(&mut io::Cursor::new(&open.request))
                .map_err(io::Error::other)?
        else {
            panic!("observer sent a mutation");
        };
        let result = WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: read.request_id,
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: snapshot(),
            },
        });
        respond(&mut stream, &open, &result, 1)?;
        send_gap.recv_timeout(Duration::from_secs(2)).unwrap();
        write_multiplex_message(
            &mut stream,
            &MultiplexMessage::Gap {
                stream_id: open.id,
                through: NonZeroU64::new(1).unwrap(),
            },
        )?;
        send_fresh.recv_timeout(Duration::from_secs(2)).unwrap();
        let mut record = Vec::new();
        orchbee_workbench_protocol::write_workbench_response(&mut record, &result)
            .map_err(io::Error::other)?;
        write_multiplex_message(
            &mut stream,
            &MultiplexMessage::StreamFrame {
                stream_id: open.id,
                sequence: NonZeroU64::new(2).unwrap(),
                kind: StreamFrameKind::Snapshot,
                record,
            },
        )?;
        send_failure.recv_timeout(Duration::from_secs(2)).unwrap();
        write_multiplex_message(
            &mut stream,
            &MultiplexMessage::StreamClose {
                stream_id: open.id,
                reason: orchbee_workbench_protocol::StreamCloseReason::Unsupported,
            },
        )?;
        drain_close(&mut stream)
    })?;
    let worker = observer(server.endpoint().to_path_buf(), 7)?;
    let update = next_update(&worker);
    let mut d = Discovery::default();
    let p = profile(1);
    d.reconcile(std::slice::from_ref(&p), Instant::now());
    d.entries[0].generation = 7;
    d.entries[0].worker = Some(worker);
    d.entries[0].accept(update);
    let observed = Instant::now();
    assert!(matches!(
        d.observations(observed)[0].1,
        Observation::Online(_)
    ));
    assert!(d.connection(&p, observed).is_some());
    assert!(
        d.connection(&p, observed + Duration::from_secs(8))
            .is_none(),
        "projection cannot outlive carrier progress deadline"
    );
    let mut replacement = p.clone();
    replacement.draft.connection = crate::machines::Connection::ssh("changed.invalid").unwrap();
    assert!(d.connection(&replacement, observed).is_none());
    gap.send(()).unwrap();
    let reset = next_update(d.entries[0].worker.as_ref().unwrap());
    assert!(matches!(reset.event, Event::Reset));
    d.entries[0].accept(reset);
    d.tick(Instant::now());
    assert_eq!(
        d.observations(Instant::now())[0].1,
        Observation::Reconnecting
    );
    assert!(d.connection(&p, Instant::now()).is_none());
    assert!(
        !d.entries[0].worker.as_ref().unwrap().finished(),
        "gap must not cancel the carrier owner"
    );
    fresh.send(()).unwrap();
    let update = next_update(d.entries[0].worker.as_ref().unwrap());
    d.entries[0].accept(update);
    assert!(matches!(
        d.observations(Instant::now())[0].1,
        Observation::Online(_)
    ));
    let cached_source = d.connection(&p, Instant::now()).unwrap();
    fail.send(()).unwrap();
    let failure = next_update(d.entries[0].worker.as_ref().unwrap());
    assert!(matches!(failure.event, Event::Failed { .. }));
    d.entries[0].accept(failure);
    d.tick(Instant::now());
    assert!(d.entries[0].ready.is_none());
    assert!(
        !d.entries[0].worker.as_ref().unwrap().cancelled(),
        "failure publication must retain the background guard until cached consumers retire"
    );
    drop(cached_source);
    d.finish_retirement();
    assert!(d.entries[0].worker.as_ref().unwrap().cancelled());
    let cancelled = Instant::now();
    drop(d);
    assert!(cancelled.elapsed() < Duration::from_secs(2));
    server.finish()
}
#[test]
fn display_labels_strip_complete_terminal_sequences_and_hidden_formats() {
    for value in [
        "\u{1b}[31mremote\u{1b}[0m",
        "\u{1b}]0;hidden title\u{7}remote",
        "\u{9d}0;hidden title\u{9c}remote",
        "\u{1b}Phidden payload\u{1b}\\remote",
        "\u{061c}\u{feff}re\u{2028}mo\u{2029}te\u{206a}",
    ] {
        assert_eq!(display_label(value), "remote", "{value:?}");
    }
    assert_eq!(display_label("remote\u{1b}]unfinished"), "remote");
}

#[test]
fn display_labels_retain_complete_graphemes_within_the_scalar_limit() {
    for grapheme in ["e\u{301}", "🇨🇳", "👩\u{200d}💻"] {
        let prefix = "a".repeat(79);
        assert_eq!(display_label(&format!("{prefix}{grapheme}z")), prefix);
        let prefix = "a".repeat(80 - grapheme.chars().count());
        let expected = format!("{prefix}{grapheme}");
        assert_eq!(display_label(&format!("{expected}z")), expected);
    }
}

#[test]
fn overview_is_bounded_and_remote_labels_cannot_control_the_host_terminal() {
    let mut s = snapshot();
    for n in 1..=20 {
        s.navigation
            .projects
            .push(orchbee_workbench_protocol::Project {
                label: format!("\u{1b}\u{7}\u{202e}{}", "界".repeat(100)),
                project_id: orchbee_workbench_protocol::WorkbenchProjectId::parse(&format!(
                    "{n:032x}"
                ))
                .unwrap(),
            });
    }
    let overview = Overview::from_snapshot(&s).unwrap();
    assert_eq!(overview.projects, 20);
    assert_eq!(overview.labels.len(), 8);
    assert!(overview.labels.iter().all(|s| s == &"界".repeat(80)));
}

#[test]
fn discovered_contexts_preserve_global_order_and_full_identity_beyond_label_preview_limit() {
    let mut s = snapshot();
    let workspaces: Vec<_> = (20..40)
        .map(|n| orchbee_workbench_protocol::Workspace {
            terminal_directory: None,
            automatic_label: false,
            project_id: (n % 2 == 0)
                .then(|| WorkbenchProjectId::parse(&format!("{:032x}", 10)).unwrap()),
            label: format!("\u{202e}{}", "界".repeat(100)),
            primary_directory_id: None,
            terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{:032x}", n + 100))
                .unwrap(),
            workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(&format!(
                "{n:032x}"
            ))
            .unwrap(),
        })
        .collect();
    s.navigation.workspace_order = workspaces
        .iter()
        .rev()
        .map(|w| w.workspace_id.clone())
        .collect();
    s.navigation.workspaces = workspaces;
    s.navigation
        .projects
        .push(orchbee_workbench_protocol::Project {
            project_id: orchbee_workbench_protocol::WorkbenchProjectId::parse(&format!(
                "{:032x}",
                10
            ))
            .unwrap(),
            label: "project".into(),
        });
    let overview = Overview::from_snapshot(&s).unwrap();
    assert_eq!(overview.contexts.len(), 20);
    assert_eq!(overview.workspaces, 20);
    assert!(overview.contexts.iter().all(|context| matches!(
        context.target.location,
        ClientContextLocation::Workspace { .. }
    )));
    for (context, expected) in overview.contexts.iter().zip(&s.navigation.workspace_order) {
        assert_eq!(context.target.session_id, s.session_id);
        let ClientContextLocation::Workspace { workspace_id, .. } = &context.target.location else {
            panic!("workspace row");
        };
        let expected_workspace = s.navigation.workspace(expected).unwrap();
        assert_eq!(context.project_id, expected_workspace.project_id);
        assert_eq!(
            context.project.is_some(),
            expected_workspace.project_id.is_some()
        );
        assert_eq!(workspace_id, expected);
        assert_eq!(context.workspace, Some("界".repeat(80)));
    }
}
