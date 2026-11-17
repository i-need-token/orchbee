//! A refused projection record stays with its pane: it resyncs the pane, and past the bound or on
//! an unreadable record stops that pane alone, without retiring the Session.
use super::super::test_support::{controlling_ui, install_projection};
use super::super::*;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, Envelope, MessageRole, PayloadEncoding, encode_transport_envelope,
};

const SNAPSHOT: &[u8] =
    include_bytes!("../../../../../../../../schemas/terminal/v1/fixtures/ghostty-surface.bin");

/// A snapshot whose route is readable but names another attachment, so the pane refuses it.
fn foreign_frame(sequence: u64) -> Vec<u8> {
    let mut snapshot = SNAPSHOT.to_vec();
    snapshot[48..56].copy_from_slice(&sequence.to_le_bytes());
    envelope(41, PayloadEncoding::TerminalFrame, &snapshot)
}

fn envelope(kind: u16, payload_encoding: PayloadEncoding, payload: &[u8]) -> Vec<u8> {
    let host_epoch = orchbee_terminal_protocol::inspect_terminal_frame_route(SNAPSHOT)
        .unwrap()
        .host_epoch;
    encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
        kind,
        payload_encoding,
        host_epoch: Some(host_epoch),
        correlation_id: None,
        optional_message: false,
        payload,
    })
    .unwrap()
}

/// Delivers one record through the ordinary Poll path and returns the poll's causal settlement.
fn deliver(
    ui: &mut WorkbenchClientUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    key: &PaneRef,
    record: Vec<u8>,
) -> Option<CausalSettlement> {
    mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(key.clone(), ())))
        .expect("Poll admission");
    let lease = mailbox.try_recv().unwrap().expect("queued Poll");
    let poll_key = lease.poll_key().expect("Poll key").clone();
    let cutoff = clock.now() + crate::workbench_state::OPERATION_TOTAL;
    mailbox
        .publish_poll_result_before(poll_key.clone(), Some(record), false, cutoff, clock)
        .unwrap();
    ui.tick();
    ui.after_draw();
    let settlement = mailbox
        .take_causal_before(&poll_key, cutoff, clock)
        .unwrap();
    mailbox.finish_lease(lease).unwrap();
    settlement
}

fn status(ui: &WorkbenchClientUi) -> ClientProjectionStatus {
    ui.projections[0].status
}

#[test]
fn refused_frames_resync_their_pane_and_stop_it_alone_past_the_bound() {
    let (mut ui, mailbox, clock, _) = controlling_ui();
    let key = install_projection(&mut ui, Some((0, false, 24)), 0);
    for sequence in 1..=3 {
        let settlement = deliver(&mut ui, &mailbox, &clock, &key, foreign_frame(sequence));
        assert!(
            matches!(
                settlement,
                Some(CausalSettlement::Feedback(TerminalViewerAction::NeedSnapshot {
                    sequence: answered,
                    ..
                })) if answered.get() == sequence
            ),
            "refusal {sequence} asks for a snapshot of that flight"
        );
        assert_eq!(status(&ui), ClientProjectionStatus::WaitingForFullFrame);
        assert!(ui.projections[0].value.frame().is_none());
        assert!(ui.retirement_failure.is_none());
    }
    let settlement = deliver(&mut ui, &mailbox, &clock, &key, foreign_frame(4));
    assert!(matches!(settlement, Some(CausalSettlement::NoFeedback)));
    assert_eq!(status(&ui), ClientProjectionStatus::Stopped);
    assert!(ui.pane_stopped(&key));
    assert!(ui.retirement_failure.is_none());
    assert_eq!(
        ui.status(),
        crate::localization::TerminalStatus::Unavailable
    );
}

#[test]
fn an_unreadable_record_stops_its_pane_and_a_refused_effect_is_dropped_alone() {
    let (mut ui, mailbox, clock, _) = controlling_ui();
    let key = install_projection(&mut ui, Some((0, false, 24)), 0);
    let effect = envelope(55, PayloadEncoding::ControlJson, b"{}");
    let settlement = deliver(&mut ui, &mailbox, &clock, &key, effect);
    assert!(matches!(settlement, Some(CausalSettlement::NoFeedback)));
    assert_eq!(status(&ui), ClientProjectionStatus::FrameAvailable);
    assert!(ui.projections[0].value.frame().is_some());

    let settlement = deliver(&mut ui, &mailbox, &clock, &key, vec![0; 3]);
    assert!(matches!(settlement, Some(CausalSettlement::NoFeedback)));
    assert_eq!(status(&ui), ClientProjectionStatus::Stopped);
    assert!(ui.projections[0].value.frame().is_none());
    assert!(ui.retirement_failure.is_none());
}
