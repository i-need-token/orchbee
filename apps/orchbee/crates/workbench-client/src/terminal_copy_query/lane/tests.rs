use std::{
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicU64},
        mpsc,
    },
};

use orchbee_local_ipc::LocalStream;

use super::{Driver, Lane, Pending};
use crate::background_thread::BackgroundThread;
use crate::terminal_content::{ContentSource, PaintedRows, SnapshotMember, TerminalScreen};
use crate::terminal_copy::{CopySearchDirection, TerminalTextPoint};
use crate::terminal_copy_mode::{EpisodeId, QueryId};
use crate::terminal_copy_query::{Action, Admission, Failure, Outcome, QueryIdentity, Request};
use crate::workbench_state::{AttachmentKey, PaneKey, PaneRef};

#[test]
fn oversized_query_is_refused_before_a_lane_or_connection_exists() {
    let clock = crate::local_client::test_support::server::adjustable_operation_clock(Arc::new(
        AtomicU64::new(0),
    ));
    let mut driver = Driver::new(clock);
    assert_eq!(
        driver.admit(Request {
            identity: identity(1, 1),
            source: source(),
            action: Action::Search {
                cursor: TerminalTextPoint { row: 0, col: 0 },
                query: "x".repeat(4097).into_boxed_str(),
                case: crate::terminal_copy::CopySearchCase::Smart,
                direction: CopySearchDirection::Forward,
                previous: None,
            },
        }),
        Admission::LimitExceeded
    );
    assert!(driver.lane.is_none());
    assert!(driver.inflight.is_none());
}

#[test]
fn busy_driver_never_queues_or_replaces_the_exact_inflight_identity() {
    let clock = crate::local_client::test_support::server::adjustable_operation_clock(Arc::new(
        AtomicU64::new(0),
    ));
    let mut driver = Driver::new(clock);
    let retained = Pending {
        identity: identity(1, 1),
        source: source(),
    };
    driver.inflight = Some(retained.clone());
    assert_eq!(
        driver.admit(Request {
            identity: identity(1, 2),
            source: source(),
            action: Action::Motion {
                cursor: TerminalTextPoint { row: 0, col: 0 },
                motion: crate::terminal_copy::CopyMotion::NextWordStart,
            },
        }),
        Admission::Busy
    );
    assert_eq!(
        driver.inflight.as_ref().unwrap().identity,
        retained.identity
    );
    assert!(driver.lane.is_none());
}

#[test]
fn episode_cancel_clears_the_exact_flight_but_retains_the_session_origin() {
    let clock = crate::local_client::test_support::server::adjustable_operation_clock(Arc::new(
        AtomicU64::new(0),
    ));
    let mut driver = Driver::new(clock);
    driver.set_origin(crate::local_client::test_support::server::disconnected_origin());
    driver.inflight = Some(Pending {
        identity: identity(4, 5),
        source: source(),
    });
    driver.cancel();
    assert!(driver.inflight.is_none());
    assert!(driver.lane.is_none());
    assert!(driver.origin.is_some());
    driver.close();
    assert!(driver.origin.is_none());
}

#[test]
fn worker_loss_is_reported_against_the_retained_identity_and_complete_source() {
    let clock = crate::local_client::test_support::server::adjustable_operation_clock(Arc::new(
        AtomicU64::new(0),
    ));
    let mut driver = Driver::new(clock);
    let pending = Pending {
        identity: identity(2, 3),
        source: source(),
    };
    driver.inflight = Some(pending.clone());
    driver.lane = Some(finished_lane(None));
    let outcome = driver.take_outcome().unwrap();
    assert_eq!(outcome.identity, pending.identity);
    assert_eq!(outcome.source, pending.source);
    assert_eq!(outcome.result, Err(Failure::Unexpected));
    assert!(driver.inflight.is_none());
    assert!(driver.lane.is_none());
}

#[test]
fn mismatched_worker_source_returns_unexpected_for_the_retained_request() {
    let clock = crate::local_client::test_support::server::adjustable_operation_clock(Arc::new(
        AtomicU64::new(0),
    ));
    let mut driver = Driver::new(clock);
    let pending = Pending {
        identity: identity(6, 7),
        source: source(),
    };
    let mut foreign = source();
    foreign.rows.revision += 1;
    driver.inflight = Some(pending.clone());
    driver.lane = Some(finished_lane(Some(Outcome {
        identity: pending.identity,
        source: foreign,
        result: Err(Failure::TransportUnavailable),
    })));
    let outcome = driver.take_outcome().unwrap();
    assert_eq!(outcome.identity, pending.identity);
    assert_eq!(outcome.source, pending.source);
    assert_eq!(outcome.result, Err(Failure::Unexpected));
}

fn finished_lane(outcome: Option<Outcome>) -> Lane {
    let (sender, commands) = mpsc::sync_channel(1);
    drop(commands);
    let (output, outcomes) = mpsc::sync_channel(1);
    if let Some(outcome) = outcome {
        output.send(outcome).unwrap();
    }
    drop(output);
    let (reader, wake) = LocalStream::pair().unwrap();
    drop(reader);
    Lane {
        sender: Some(sender),
        outcomes,
        wake,
        stopping: Arc::new(AtomicU8::new(0)),
        task: BackgroundThread::ended(),
    }
}

fn identity(episode: u64, query: u64) -> QueryIdentity {
    QueryIdentity {
        episode: EpisodeId::new(NonZeroU64::new(episode).unwrap()),
        query: QueryId::new(NonZeroU64::new(query).unwrap()),
    }
}

fn source() -> ContentSource {
    let id = |value: u128| format!("{value:032x}");
    let tab = orchbee_workbench_protocol::WorkbenchTabId::parse(&id(1)).unwrap();
    let panel = orchbee_workbench_protocol::WorkbenchPanelId::parse(&id(2)).unwrap();
    ContentSource {
        pane: PaneRef::new(
            PaneKey::new(AttachmentKey::new(1, tab.clone()), panel.clone()),
            0,
        ),
        member: SnapshotMember {
            session: orchbee_workbench_protocol::SessionId::parse(&id(3)).unwrap(),
            context: orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(&id(4)).unwrap(),
            tab,
            panel,
            presentation: orchbee_workbench_protocol::TerminalPresentationId::parse(&id(5))
                .unwrap(),
            worker: orchbee_workbench_protocol::WorkerId::parse(&id(6)).unwrap(),
            terminal: orchbee_workbench_protocol::TerminalId::parse(&id(7)).unwrap(),
        },
        host_terminal: orchbee_client_server_protocol::HostTerminalId::parse(&id(8)).unwrap(),
        screen: TerminalScreen::Primary,
        rows: PaintedRows {
            attachment: orchbee_terminal_protocol::AttachmentId::from_bytes([9; 16]).unwrap(),
            host_epoch: orchbee_terminal_protocol::HostEpoch::from_bytes([10; 16]).unwrap(),
            sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 1,
            columns: 80,
            start_column: 0,
            total_rows: 24,
            start_row: 0,
            row_count: 24,
            history: false,
        },
    }
}
