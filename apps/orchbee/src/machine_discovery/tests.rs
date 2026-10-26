use super::*;
use crate::{local_client::tests::server::FakeServer, machines::Draft};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_workbench_protocol::{
    Navigation, ProtocolVersion, TerminalContext, WorkbenchRequest, WorkbenchResponse,
    WorkbenchSnapshotOutcome, WorkbenchSnapshotResult, WorkbenchTerminalContextId,
    read_workbench_request, write_workbench_response,
};
use std::{io::Read, num::NonZeroU64, sync::atomic::Ordering, thread};

fn profile(n: u8) -> Profile {
    Profile {
        id: ProfileId::parse(&format!("{n:032x}")).unwrap(),
        draft: Draft::parse("remote", "example.invalid", "default").unwrap(),
        enabled: true,
    }
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
        while cancel.load(Ordering::Acquire) == 0 {
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
    profiles[0].draft.target = "replacement.invalid".into();
    d.reconcile(&profiles, now);
    assert_ne!(d.entries[0].generation, generation);
    d.entries[0].accept(Update {
        generation,
        event: Event::Failed {
            permanent: true,
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
#[test]
fn denied_or_uncorrelated_native_snapshot_requires_attention_without_automatic_retry()
-> io::Result<()> {
    for wrong_identity in [false, true] {
        let server = FakeServer::start(move |mut stream| {
            assert!(matches!(
                read_message(&mut stream).map_err(io::Error::other)?,
                LocalClientMessage::ClientHello(_)
            ));
            write_message(
                &mut stream,
                &LocalClientMessage::ClientAccepted {
                    generation: NonZeroU64::new(1).unwrap(),
                },
            )
            .map_err(io::Error::other)?;
            let WorkbenchRequest::SnapshotRead(read) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("observer sent a mutation");
            };
            let request_id = if wrong_identity {
                orchbee_workbench_protocol::WorkbenchRequestId::parse(
                    "11111111111111111111111111111111",
                )
                .unwrap()
            } else {
                read.request_id
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                    protocol_version: ProtocolVersion,
                    request_id,
                    outcome: WorkbenchSnapshotOutcome::Denied {},
                }),
            )
            .map_err(io::Error::other)?;
            assert_eq!(stream.read(&mut [0])?, 0);
            Ok(())
        })?;
        let endpoint = server.endpoint().to_path_buf();
        let worker = Worker::spawn(1, move |_| {
            LocalClient::connect_before(
                &endpoint,
                "test-cohort",
                Instant::now() + Duration::from_secs(2),
            )
            .map_err(io::Error::other)
        })?;
        let deadline = Instant::now() + Duration::from_secs(3);
        let update = loop {
            if let Some(update) = worker.take() {
                break update;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        };
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
fn discovered_origin_opens_a_separate_admitted_lane() -> io::Result<()> {
    let server = FakeServer::start_connections(2, |_, mut stream| {
        assert!(matches!(
            read_message(&mut stream).map_err(io::Error::other)?,
            LocalClientMessage::ClientHello(_)
        ));
        write_message(
            &mut stream,
            &LocalClientMessage::ClientAccepted {
                generation: NonZeroU64::new(1).unwrap(),
            },
        )
        .map_err(io::Error::other)?;
        let WorkbenchRequest::SnapshotRead(read) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("unexpected mutation");
        };
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: read.request_id,
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot(),
                },
            }),
        )
        .map_err(io::Error::other)
    })?;
    let endpoint = server.endpoint().to_path_buf();
    let worker = Worker::spawn(1, move |_| {
        LocalClient::connect_before(
            &endpoint,
            "test-cohort",
            Instant::now() + Duration::from_secs(2),
        )
        .map_err(io::Error::other)
    })?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let update = loop {
        if let Some(update) = worker.take() {
            break update;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    };
    let Event::Healthy { ready, .. } = update.event else {
        panic!("discovery failed");
    };
    let mut selected = ready.connection.reconnect(deadline, &AtomicU8::new(0))?;
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
    drop(worker);
    drop(ready);
    server.finish()
}
#[test]
fn native_observer_only_reads_snapshots_and_cancels_inflight_read_without_replay() -> io::Result<()>
{
    let (sent, received) = std::sync::mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut stream| {
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        assert!(matches!(
            read_message(&mut stream).map_err(io::Error::other)?,
            LocalClientMessage::ClientHello(_)
        ));
        write_message(
            &mut stream,
            &LocalClientMessage::ClientAccepted {
                generation: NonZeroU64::new(1).unwrap(),
            },
        )
        .map_err(io::Error::other)?;
        let WorkbenchRequest::SnapshotRead(read) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("observer sent a mutation");
        };
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: read.request_id,
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot(),
                },
            }),
        )
        .map_err(io::Error::other)?;
        assert!(matches!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::SnapshotRead(_)
        ));
        sent.send(()).map_err(io::Error::other)?;
        let mut byte = [0];
        assert_eq!(
            stream.read(&mut byte)?,
            0,
            "cancellation must close the read lane, without control or replay"
        );
        Ok(())
    })?;
    let endpoint = server.endpoint().to_path_buf();
    let worker = Worker::spawn(7, move |_| {
        LocalClient::connect_before(
            &endpoint,
            "test-cohort",
            Instant::now() + Duration::from_secs(2),
        )
        .map_err(io::Error::other)
    })?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let update = loop {
        if let Some(update) = worker.take() {
            break update;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    };
    let mut d = Discovery::default();
    let p = profile(1);
    d.reconcile(std::slice::from_ref(&p), Instant::now());
    d.entries[0].generation = 7;
    d.entries[0].accept(update);
    let observed = d.entries[0]
        .ready
        .as_ref()
        .expect("successful native snapshot")
        .observed;
    assert!(matches!(
        d.observations(observed)[0].1,
        Observation::Online(_)
    ));
    assert!(d.connection(&p, observed).is_some());
    assert!(
        d.connection(&p, observed + Duration::from_secs(11))
            .is_none()
    );
    assert_eq!(
        d.observations(observed + Duration::from_secs(11))[0].1,
        Observation::Stale
    );
    let mut replacement = p;
    replacement.draft.target = "changed.invalid".into();
    assert!(d.connection(&replacement, observed).is_none());
    received
        .recv_timeout(Duration::from_secs(4))
        .map_err(io::Error::other)?;
    let cancelled = Instant::now();
    drop(worker);
    assert!(cancelled.elapsed() < Duration::from_secs(2));
    drop(d);
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
