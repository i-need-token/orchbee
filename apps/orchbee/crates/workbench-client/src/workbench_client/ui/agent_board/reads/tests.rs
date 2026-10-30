use std::sync::{Arc, atomic::AtomicU64};

use orchbee_agent_observation_protocol::{AgentGetOutcome, AgentSessionRejectionReason};

use super::*;
use crate::local_client::test_support::server::adjustable_operation_clock;
use crate::workbench_client::agent_board::AgentBoardReadAnswer;

fn reads_with(episode: AgentBoardEpisodeToken, outcomes: Vec<LaneOutcome>) -> BoardReads {
    let mut reads = BoardReads::new(adjustable_operation_clock(Arc::new(AtomicU64::new(0))));
    reads.live = Some((episode, AgentReadLane::for_test(outcomes)));
    reads
}

#[test]
fn an_ended_lane_returns_its_queued_answers_before_its_end() {
    let episode = AgentBoardEpisodeToken::for_test(1);
    let mut reads = reads_with(
        episode,
        vec![LaneOutcome::Board(AgentBoardReadAnswer::Get {
            episode,
            serial: 5,
            result: Ok(AgentGetOutcome::Rejected {
                reason: AgentSessionRejectionReason::NotFound,
            }),
        })],
    );
    assert!(matches!(
        reads.next_event(),
        Some(AgentBoardReadEvent::Answer(AgentBoardReadAnswer::Get {
            serial: 5,
            ..
        }))
    ));
    assert!(matches!(
        reads.next_event(),
        Some(AgentBoardReadEvent::LaneEnded)
    ));
    assert!(reads.live.is_none(), "the ended lane is released");
    assert!(reads.next_event().is_none());
}

#[test]
fn a_lane_retires_when_its_episode_ends_and_never_serves_another() {
    let first = AgentBoardEpisodeToken::for_test(1);
    let mut reads = reads_with(first, Vec::new());
    reads.follow(Some(first));
    assert!(reads.live.is_some(), "the open episode keeps its lane");

    reads.follow(None);
    assert!(reads.live.is_none());
    assert_eq!(reads.retired.len(), 1, "the retired thread stays owned");

    let mut reads = reads_with(first, Vec::new());
    let second = AgentBoardEpisodeToken::for_test(3);
    assert_eq!(
        reads.issue(AgentBoardReadRequest::List {
            episode: second,
            deadline: Instant::now(),
        }),
        Err(AgentBoardReadRefusal::LaneUnavailable),
        "without an origin no lane starts for the new episode"
    );
    assert!(
        reads.live.is_none(),
        "the previous episode's lane is not reused"
    );
}
