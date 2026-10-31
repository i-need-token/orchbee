use super::super::tests::move_membership::ready_ui;
use super::super::*;
use crate::terminal_input::{PaneAdmission, PointerSelectionIntent};
use crate::workbench_client::input::PointerSelectionCell;
use crate::workbench_client::state;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, MessageRole, PayloadEncoding,
    SelectionChanged, SelectionChangedState, SelectionControl, SelectionPoint,
    encode_selection_control, encode_transport_envelope,
};

fn response(ui: &WorkbenchClientUi, control: SelectionControl) -> Vec<u8> {
    let encoded = encode_selection_control(&control).unwrap();
    encode_transport_envelope(&Envelope {
        correlation_id: Some(CorrelationId::from_bytes([9; 16]).unwrap()),
        host_epoch: Some(*ui.projections[0].value.epoch()),
        kind: encoded.kind,
        optional_message: false,
        payload: &encoded.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Response,
    })
    .unwrap()
}

fn command(
    ui: &mut WorkbenchClientUi,
    mailbox: &Arc<CommandMailbox>,
) -> (PaneRef, super::super::super::terminal_selection::Command) {
    ui.admit_selections(mailbox);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Selection(tagged) = lease.record() else {
        panic!("selection command");
    };
    let value = (tagged.key().clone(), *tagged.value());
    mailbox.finish_lease(lease).unwrap();
    value
}

#[test]
fn accepted_release_and_complete_host_export_reach_the_copy_queue_once() {
    use orchbee_terminal_protocol::{
        SelectionExportChunk, SelectionExportControl, SelectionExportFinished,
        SelectionExportFormat, SelectionExportStarted, SelectionExportStatus,
        encode_selection_export_chunk, encode_selection_export_control,
    };
    use std::num::NonZeroU64;
    let (mut ui, mailbox, _, _) = ready_ui();
    let key = super::super::tests::viewport::install_projection(&mut ui, Some((9, false, 24)), 0);
    assert_eq!(
        ui.pointer_selection(
            &key,
            PointerSelectionIntent::Start {
                click_count: 2,
                cell: PointerSelectionCell { column: 1, row: 0 },
            }
        ),
        PaneAdmission::Admitted
    );
    assert_eq!(
        ui.pointer_selection(
            &key,
            PointerSelectionIntent::End {
                cell: PointerSelectionCell { column: 3, row: 0 },
            }
        ),
        PaneAdmission::Admitted
    );
    let attachment = *ui.projections[0].value.attachment();
    for _ in 0..3 {
        let (pane, command) = command(&mut ui, &mailbox);
        let record = response(
            &ui,
            SelectionControl::ChangedResponse(SelectionChanged {
                attachment_id: attachment,
                state: SelectionChangedState::Active {
                    start: SelectionPoint { column: 1, row: 9 },
                    end: SelectionPoint { column: 3, row: 9 },
                    rectangle: false,
                    preview: None,
                },
            }),
        );
        assert!(ui.apply_selection_outcome(
            &mailbox,
            pane,
            command,
            orchbee_client_server_protocol::TerminalViewerOutcome::Record(record)
        ));
    }
    let (pane, export) = command(&mut ui, &mailbox);
    let viewer = orchbee_client_server_protocol::ViewerId::parse(&attachment.to_string()).unwrap();
    let TerminalViewerAction::SelectionExport {
        export_sequence: sequence,
        ..
    } = export.viewer_action(viewer)
    else {
        panic!("export after accepted End");
    };
    let text = "中文e\u{301}\nsecond";
    let started = encode_selection_export_control(&SelectionExportControl::StartedResponse(
        SelectionExportStarted {
            attachment_id: attachment,
            export_sequence: sequence,
            byte_length: text.len() as u32,
            format: SelectionExportFormat::Plain,
        },
    ))
    .unwrap();
    let record = encode_transport_envelope(&Envelope {
        correlation_id: Some(CorrelationId::from_bytes([10; 16]).unwrap()),
        host_epoch: Some(*ui.projections[0].value.epoch()),
        kind: started.kind,
        optional_message: false,
        payload: &started.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Response,
    })
    .unwrap();
    ui.apply_selection_outcome(
        &mailbox,
        pane,
        export,
        orchbee_client_server_protocol::TerminalViewerOutcome::Record(record),
    );
    assert!(!ui.effects.has_pending());
    let chunk = encode_selection_export_chunk(&SelectionExportChunk {
        attachment_id: attachment,
        export_sequence: sequence,
        chunk_sequence: 0,
        bytes: text.as_bytes().to_vec(),
        final_chunk: true,
    })
    .unwrap();
    let epoch = *ui.projections[0].value.epoch();
    let event = |kind, payload: &[u8], encoding| {
        encode_transport_envelope(&Envelope {
            correlation_id: None,
            host_epoch: Some(epoch),
            kind,
            optional_message: false,
            payload,
            payload_encoding: encoding,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Event,
        })
        .unwrap()
    };
    ui.apply_selection_event(
        0,
        &event(chunk.kind, &chunk.payload, PayloadEncoding::RawBytes),
    )
    .unwrap();
    let (pane, ack) = command(&mut ui, &mailbox);
    let finished = encode_selection_export_control(&SelectionExportControl::FinishedEvent(
        SelectionExportFinished {
            attachment_id: attachment,
            export_sequence: sequence,
            byte_length: text.len() as u32,
            status: SelectionExportStatus::Completed,
        },
    ))
    .unwrap();
    ui.apply_selection_event(
        0,
        &event(
            finished.kind,
            &finished.payload,
            PayloadEncoding::ControlJson,
        ),
    )
    .unwrap();
    ui.admit_selections(&mailbox);
    assert!(!ui.effects.has_pending());
    ui.apply_selection_outcome(
        &mailbox,
        pane,
        ack,
        orchbee_client_server_protocol::TerminalViewerOutcome::FeedbackAccepted {},
    );
    assert!(ui.effects.has_pending());
    // Inspect the actual admitted effect without changing the shared OS clipboard.
    let job = ui.effects.dispatch().unwrap();
    assert_eq!(job.text(), Some(text));
    assert!(job.begin());
    job.settle(super::super::super::terminal_effects::Outcome::Delivered);
    assert!(ui.settle_effect_delivery());
    ui.admit_selections(&mailbox);
    assert!(ui.effects.dispatch().is_none());
    assert!(ui.retirement_failure.is_none());
    assert_eq!(sequence, NonZeroU64::MIN);
}

#[test]
fn selection_start_on_a_ready_read_only_pane_enters_the_real_command_mailbox() {
    let (mut ui, mailbox, _, _) = ready_ui();
    let key = super::super::tests::viewport::install_projection(&mut ui, Some((9, false, 24)), 0);
    assert_eq!(
        ui.pointer_selection(
            &key,
            PointerSelectionIntent::Start {
                click_count: 1,
                cell: PointerSelectionCell { column: 1, row: 2 },
            }
        ),
        PaneAdmission::Admitted
    );
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Selection(tagged) = lease.record() else {
        panic!("selection command")
    };
    assert_eq!(tagged.key(), &key);
    let projection = ui
        .projections
        .iter()
        .find(|projection| projection.key == key)
        .unwrap();
    let attachment = *projection.value.attachment();
    let epoch = *projection.value.epoch();
    let command = *tagged.value();
    let response = encode_selection_control(&SelectionControl::ChangedResponse(SelectionChanged {
        attachment_id: attachment,
        state: SelectionChangedState::None,
    }))
    .unwrap();
    let record = encode_transport_envelope(&Envelope {
        correlation_id: Some(CorrelationId::from_bytes([9; 16]).unwrap()),
        host_epoch: Some(epoch),
        kind: response.kind,
        optional_message: false,
        payload: &response.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Response,
    })
    .unwrap();
    mailbox
        .publish_notice_before(
            WorkerEvent::SelectionOutcome {
                key: key.clone(),
                command,
                outcome: orchbee_client_server_protocol::TerminalViewerOutcome::Record(record),
            },
            ui.clock.now() + state::OPERATION_TOTAL,
            &ui.clock,
        )
        .unwrap();
    ui.tick();
    assert!(ui.retirement_failure.is_none());
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn selection_poll_keeps_the_frame_and_uses_the_normal_no_feedback_settlement() {
    let (mut ui, mailbox, clock, _) = ready_ui();
    let key = super::super::tests::viewport::install_projection(&mut ui, Some((9, false, 24)), 0);
    let projection = &ui.projections[0];
    let changed = SelectionChanged {
        attachment_id: *projection.value.attachment(),
        state: SelectionChangedState::Active {
            start: SelectionPoint { column: 1, row: 9 },
            end: SelectionPoint { column: 3, row: 9 },
            rectangle: false,
            preview: None,
        },
    };
    let encoded =
        encode_selection_control(&SelectionControl::ChangedEvent(changed.clone())).unwrap();
    let record = encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(*projection.value.epoch()),
        kind: encoded.kind,
        optional_message: false,
        payload: &encoded.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
    })
    .unwrap();
    mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(key.clone(), ())))
        .unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    let poll = lease.poll_key().unwrap();
    let cutoff = clock.now() + state::OPERATION_TOTAL;
    mailbox
        .publish_poll_result_before(poll.clone(), Some(record), false, cutoff, &clock)
        .unwrap();
    ui.tick();
    ui.after_draw();
    assert!(ui.retirement_failure.is_none());
    assert_eq!(
        ui.projections[0].status,
        ClientProjectionStatus::FrameAvailable
    );
    assert!(ui.projections[0].value.frame().is_some());
    assert_eq!(
        ui.projections[0].selection.as_ref().unwrap().selection(),
        &changed.state
    );
    assert!(matches!(
        mailbox.take_causal_before(&poll, cutoff, &clock).unwrap(),
        Some(CausalSettlement::NoFeedback)
    ));
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn pane_effect_poll_keeps_the_frame_and_settles_without_frame_feedback() {
    use orchbee_terminal_protocol::{
        TerminalBell, TerminalClipboardWriteRequest, TerminalEffectEvent,
        encode_terminal_effect_event,
    };
    use std::num::NonZeroU64;

    for clipboard in [true, false] {
        let (mut ui, mailbox, clock, _) = ready_ui();
        let key =
            super::super::tests::viewport::install_projection(&mut ui, Some((9, false, 24)), 0);
        let projection = &ui.projections[0];
        let frame = format!("{:?}", projection.value.frame().unwrap());
        let attachment_id = *projection.value.attachment();
        let effect = if clipboard {
            TerminalEffectEvent::ClipboardWriteRequest(TerminalClipboardWriteRequest {
                attachment_id,
                sequence: NonZeroU64::MIN,
                text: "中文e\u{301}\nnext".into(),
            })
        } else {
            TerminalEffectEvent::Bell(TerminalBell {
                attachment_id,
                sequence: NonZeroU64::MIN,
            })
        };
        let encoded = encode_terminal_effect_event(&effect).unwrap();
        let record = encode_transport_envelope(&Envelope {
            correlation_id: None,
            host_epoch: Some(*projection.value.epoch()),
            kind: encoded.kind,
            optional_message: false,
            payload: &encoded.payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Event,
        })
        .unwrap();
        mailbox
            .try_enqueue(QueuedCommand::Poll(Tagged::new(key, ())))
            .unwrap();
        let lease = mailbox.try_recv().unwrap().unwrap();
        let poll = lease.poll_key().unwrap();
        let cutoff = clock.now() + state::OPERATION_TOTAL;
        mailbox
            .publish_poll_result_before(poll.clone(), Some(record), false, cutoff, &clock)
            .unwrap();
        ui.tick();
        assert!(
            ui.retirement_failure.is_none(),
            "a valid pane effect must not retire the carrier"
        );
        assert_eq!(
            format!("{:?}", ui.projections[0].value.frame().unwrap()),
            frame
        );
        assert_eq!(
            ui.projections[0].status,
            ClientProjectionStatus::FrameAvailable
        );
        assert!(
            ui.effects.has_pending(),
            "the effect must reach the owned FIFO"
        );

        // Inspect admission without touching the real clipboard or outer terminal.
        // Removing this job before after_draw also catches an accidental second copy.
        let job = ui.effects.dispatch().unwrap();
        ui.effects.close();
        drop(job);
        ui.after_draw();
        assert!(matches!(
            mailbox.take_causal_before(&poll, cutoff, &clock).unwrap(),
            Some(CausalSettlement::NoFeedback)
        ));
        assert!(!ui.effects.has_pending());
        mailbox.finish_lease(lease).unwrap();
    }
}
