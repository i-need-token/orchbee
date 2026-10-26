//! Program effects use the exact attached stream, including before the first frame.
use super::super::*;
use super::super::{tests::move_membership::ready_ui, tests::publication_hint::starting_split_ui};
use crate::workbench_client::{
    mailbox::EventDeadline,
    terminal_effects::{Kind, Outcome},
};
use orchbee_client_server_protocol::ControllerId;
use orchbee_terminal_protocol::{
    AttachmentId, CURRENT_PROTOCOL_VERSION, Envelope, HostEpoch, MessageRole, PayloadEncoding,
    TerminalBell, TerminalClipboardWriteRequest, TerminalEffectEvent, encode_terminal_effect_event,
    encode_transport_envelope,
};
use std::{io, num::NonZeroU64};

fn record(ui: &NativeUi, clipboard: bool) -> Vec<u8> {
    let projection = &ui.projections[0].value;
    let attachment_id = *projection.attachment();
    let event = if clipboard {
        TerminalEffectEvent::ClipboardWriteRequest(TerminalClipboardWriteRequest {
            attachment_id,
            sequence: NonZeroU64::MIN,
            text: "中文e\u{301}\t\r\nnext".into(),
        })
    } else {
        TerminalEffectEvent::Bell(TerminalBell {
            attachment_id,
            sequence: NonZeroU64::MIN,
        })
    };
    let encoded = encode_terminal_effect_event(&event).unwrap();
    encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(*projection.epoch()),
        kind: encoded.kind,
        optional_message: false,
        payload: &encoded.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
    })
    .unwrap()
}

#[test]
fn program_effect_before_first_frame_survives_starting_and_uses_no_feedback() {
    for clipboard in [true, false] {
        let (mut ui, mailbox, clock, _) = starting_split_ui();
        let state = ui.state.as_ref().unwrap();
        let key = state.pane_ref_for(state.desired().unwrap()).unwrap();
        let geometry = Geometry::new(80, 24, 8, 16).unwrap();
        let events = [
            WorkerEvent::Attached {
                key: key.clone(),
                projection: TerminalProjection::new(
                    AttachmentId::parse(&format!("{:032x}", 4_001)).unwrap(),
                    HostEpoch::parse(&format!("{:032x}", 4_002)).unwrap(),
                ),
            },
            WorkerEvent::SetupConfigured {
                key: key.clone(),
                mode: ControllerMode::Controlling(
                    ControllerId::parse(&format!("{:032x}", 8)).unwrap(),
                ),
                applied_geometry: Some(geometry),
            },
        ];
        for event in events {
            let wait = mailbox
                .publish_event_before(
                    event,
                    EventDeadline::Startup(
                        clock.now() + super::super::super::state::OPERATION_TOTAL,
                    ),
                    &clock,
                )
                .unwrap();
            ui.tick();
            assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
        }
        assert_eq!(
            ui.state.as_ref().unwrap().phase_kind(),
            ClientPhaseKind::Starting
        );
        assert!(ui.projections[0].value.frame().is_none());
        let poll = mailbox.allocate_startup_poll_key(&key).unwrap();
        let cutoff = clock.now() + super::super::super::state::OPERATION_TOTAL;
        mailbox
            .publish_poll_result_before(
                poll.clone(),
                Some(record(&ui, clipboard)),
                false,
                cutoff,
                &clock,
            )
            .unwrap();
        ui.tick();
        ui.reconcile_effects();
        assert!(ui.effects.has_pending());
        if clipboard {
            // Inspect the owned payload without writing a shared system resource.
            let job = ui.effects.dispatch().unwrap();
            assert_eq!(job.text(), Some("中文e\u{301}\t\r\nnext"));
            ui.effects.close();
            drop(job);
        }
        ui.after_draw();
        let mut bells = 0;
        ui.deliver_terminal_effect(&mut || {
            bells += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(bells, usize::from(!clipboard));
        assert!(matches!(
            mailbox.take_causal_before(&poll, cutoff, &clock).unwrap(),
            Some(CausalSettlement::NoFeedback)
        ));
        assert_eq!(
            ui.state.as_ref().unwrap().phase_kind(),
            ClientPhaseKind::Starting
        );
        assert!(ui.projections[0].value.frame().is_none());
        assert!(ui.retirement_failure.is_none());
    }
}

#[test]
fn bell_delivery_is_once_and_projection_retirement_cancels_before_dispatch() {
    for cancel_before in [false, true] {
        for fail_write in [false, true] {
            let (mut ui, _, _, _) = ready_ui();
            super::super::tests::viewport::install_projection(&mut ui, Some((9, false, 24)), 0);
            assert!(ui.apply_program_effect(0, &record(&ui, false)).unwrap());
            ui.after_draw();
            let mut attempts = 0;
            if cancel_before {
                ui.clear_projections();
            }
            let mut emit = || {
                attempts += 1;
                if fail_write {
                    Err(io::ErrorKind::BrokenPipe.into())
                } else {
                    Ok(())
                }
            };
            assert_eq!(
                ui.deliver_terminal_effect(&mut emit).is_err(),
                fail_write && !cancel_before
            );
            ui.deliver_terminal_effect(&mut emit).unwrap();
            assert_eq!(attempts, usize::from(!cancel_before));
            assert_eq!(
                ui.effects.take_outcome(),
                Some((
                    Kind::Bell,
                    if cancel_before {
                        Outcome::Cancelled
                    } else if fail_write {
                        Outcome::Unknown
                    } else {
                        Outcome::Delivered
                    }
                ))
            );
            assert!(!ui.effects.has_pending());
            ui.after_draw();
            ui.deliver_terminal_effect(&mut || panic!("a settled effect must never replay"))
                .unwrap();
        }
    }
}
