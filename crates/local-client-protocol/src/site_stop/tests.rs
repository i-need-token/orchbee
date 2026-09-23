use super::*;
use crate::{LocalClientMessage, read_message, write_message};

fn round_trip(message: LocalClientMessage) {
    let mut bytes = vec![];
    write_message(&mut bytes, &message).unwrap();
    assert_eq!(read_message(&mut bytes.as_slice()).unwrap(), message);
    for end in 0..bytes.len() {
        assert!(read_message(&mut &bytes[..end]).is_err());
    }
    bytes.push(0);
    let size = (bytes.len() - 8) as u16;
    bytes[6..8].copy_from_slice(&size.to_be_bytes());
    assert!(read_message(&mut bytes.as_slice()).is_err());
}

#[test]
fn stop_control_round_trips_all_closed_states_and_rejects_ambiguous_frames() {
    let id = StopId::new([1; 16]).unwrap();
    let hello = ClientHello::new("cohort").unwrap();
    for action in [StopAction::Start, StopAction::Status, StopAction::Retry] {
        round_trip(LocalClientMessage::SiteStopRequest(SiteStopRequest {
            hello: hello.clone(),
            id,
            action,
        }));
    }
    for status in [
        SiteStopStatus::NotRecorded,
        SiteStopStatus::Unmanaged,
        SiteStopStatus::Busy {
            active_id: StopId::new([2; 16]).unwrap(),
        },
    ] {
        round_trip(LocalClientMessage::SiteStopResult(SiteStopResult {
            id,
            status,
        }));
    }
    for p in 1..=11 {
        let phase = phase(p).unwrap();
        for attempt in [StopAttempt::new(1).unwrap(), StopAttempt::new(64).unwrap()] {
            round_trip(LocalClientMessage::SiteStopResult(SiteStopResult {
                id,
                status: SiteStopStatus::Pending { attempt, phase },
            }));
            for f in 1..=7 {
                round_trip(LocalClientMessage::SiteStopResult(SiteStopResult {
                    id,
                    status: SiteStopStatus::Failed {
                        attempt,
                        phase,
                        failure: failure(f).unwrap(),
                        disposed_terminals: u32::MAX,
                    },
                }));
            }
        }
    }
    assert!(StopId::new([0; 16]).is_err());
    assert!(StopAttempt::new(0).is_err());
    assert!(StopAttempt::new(65).is_err());
    let result = SiteStopResult {
        id,
        status: SiteStopStatus::Failed {
            attempt: StopAttempt::new(1).unwrap(),
            phase: StopPhase::Dispose,
            failure: StopFailure::OutcomeUnknown,
            disposed_terminals: 9,
        },
    };
    for index in [17, 18, 19, 20] {
        let mut bytes = encode_result(&result);
        bytes[index] = 255;
        assert!(decode_result(&bytes).is_err());
    }
}
