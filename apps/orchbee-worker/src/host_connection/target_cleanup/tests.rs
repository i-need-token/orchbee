use super::*;
use orchbee_capability_node_protocol::{
    ControlRejection, HostCreationId, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerRequest, ViewerRejection, WorkerGeneration, WorkerId,
};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_terminal_protocol::{
    AttachmentId, Envelope, HistoryAnchorStatus, HistoryChanged, SelectionChanged,
    SelectionChangedState, SelectionControl, SelectionExportChunk, SelectionExportControl,
    SelectionExportFinished, SelectionExportStatus, encode_history_control,
    encode_selection_control, encode_selection_export_chunk, encode_selection_export_control,
    encode_transport_envelope,
};
use std::{cell::RefCell, num::NonZeroU64, rc::Rc};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn host() -> TestResult<(HostBrokerConnection, UnixStream)> {
    let (stream, peer) = UnixStream::pair()?;
    Ok((
        HostBrokerConnection {
            host_epoch: HostEpoch::from_bytes([4; 16])?,
            negotiated_limits: super::super::NegotiatedHostLimits {
                frame_bytes: 1_048_576,
                input_bytes: 4096,
                row_window_bytes: 4096,
            },
            stream,
            viewers: RefCell::new(super::super::viewers::HostViewers::default()),
            controllers: RefCell::new(super::super::controllers::HostControllers::default()),
            retired_targets: RefCell::new(RetiredTargets::default()),
            titles: RefCell::new(super::super::titles::Titles::default()),
            host_colors: RefCell::new(crate::host_connection::HostColors::default()),
            drain: Rc::new(RefCell::new(crate::agent_drain::AgentDrain::new())),
        },
        peer,
    ))
}
fn identity(id: u8) -> TestResult<HostTerminalIdentity> {
    Ok(HostTerminalIdentity::new(
        HostCreationId::from_bytes([id; 16])?,
        orchbee_capability_node_protocol::HostEpoch::from_bytes([4; 16])?,
        HostTerminalId::from_bytes([id; 16])?,
    ))
}
fn viewer(id: u8, terminal: u8) -> TestResult<TerminalViewerRequest> {
    Ok(TerminalViewerRequest {
        request_id: OperationId::from_bytes([id + 20; 16])?,
        attachment_id: OperationId::from_bytes([id; 16])?,
        worker_id: WorkerId::from_bytes([3; 16])?,
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action: TerminalViewerAction::Attach {
            host_epoch: identity(terminal)?.host_epoch(),
            terminal_id: identity(terminal)?.terminal_id(),
        },
    })
}
fn frame(id: u8, epoch: HostEpoch) -> TestResult<Vec<u8>> {
    let mut payload = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/terminal/v1/fixtures/terminal-frame.bin"
    ))
    .to_vec();
    payload[6] = 1;
    payload[56..64].fill(255);
    payload[16..32].copy_from_slice(epoch.as_bytes());
    payload[32..48].fill(id);
    payload[48..56].copy_from_slice(&1_u64.to_le_bytes());
    payload[64..72].copy_from_slice(&5_u64.to_le_bytes());
    payload[72..80].copy_from_slice(&1_u64.to_le_bytes());
    Ok(encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(epoch),
        kind: 41,
        optional_message: false,
        payload: &payload,
        payload_encoding: PayloadEncoding::TerminalFrame,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
    })?)
}

#[test]
fn actual_zero_one_twenty_three_twenty_four_slots_are_freed_only_on_commit() -> TestResult {
    for count in [0, 1, 23, 24] {
        let (host, _peer) = host()?;
        for id in 1..=count {
            host.viewers
                .borrow_mut()
                .admit(&viewer(id, 5)?)
                .map_err(|e| format!("{e:?}"))?;
            host.receive_event(frame(id, host.host_epoch())?)?;
        }
        if count == 24 {
            assert_eq!(
                host.viewers.borrow_mut().admit(&viewer(25, 5)?),
                Err(ViewerRejection::CapacityExceeded)
            );
        }
        let prepared = host.prepare_target_cleanup(identity(5)?)?;
        let capacity = host.retired_targets.borrow().targets.capacity();
        for id in 1..=count {
            assert!(host.viewers.borrow().contains(viewer(id, 5)?.attachment_id));
        }
        assert!(!host.disposed_terminal(identity(5)?.terminal_id()));
        host.commit_target_cleanup(prepared);
        assert_eq!(host.retired_targets.borrow().targets.capacity(), capacity);
        for id in 1..=count {
            let request = viewer(id, 5)?;
            assert!(!host.viewers.borrow().contains(request.attachment_id));
            assert_eq!(
                host.viewers.borrow_mut().poll(request.attachment_id),
                TerminalViewerOutcome::Rejected(ViewerRejection::AttachmentUnavailable)
            );
            host.receive_event(frame(id, host.host_epoch())?)?;
        }
        for id in 10..34 {
            host.viewers
                .borrow_mut()
                .admit(&viewer(id, 6)?)
                .map_err(|e| format!("{e:?}"))?;
        }
        assert_eq!(
            host.viewers.borrow_mut().admit(&viewer(35, 6)?),
            Err(ViewerRejection::CapacityExceeded)
        );
    }
    Ok(())
}

#[test]
fn prepared_cleanup_captures_all_viewers_and_commit_releases_their_records() -> TestResult {
    let (host, _peer) = host()?;
    let mut retained = 0;
    for id in 1..=24_u8 {
        host.viewers
            .borrow_mut()
            .admit(&viewer(id, 5)?)
            .map_err(|e| format!("{e:?}"))?;
        let record = frame(id, host.host_epoch())?;
        retained += record.len();
        host.receive_event(record)?;
    }
    assert_eq!(host.viewers.borrow().retained_record_bytes(), retained);
    // The tombstone store is a separately bounded owner allocation.
    assert!(
        size_of::<RemovedTarget>()
            * crate::terminal_authorization::MAXIMUM_RETAINED_TERMINAL_BINDINGS
            <= 1024 * 1024
    );
    let prepared = host.prepare_target_cleanup(identity(5)?)?;
    assert_eq!(prepared.target.viewers.iter().flatten().count(), 24);
    host.commit_target_cleanup(prepared);
    for id in 1..=24_u8 {
        let request = viewer(id, 5)?;
        assert!(host.disposed_viewer(request.attachment_id));
        assert_eq!(
            host.viewers.borrow_mut().poll(request.attachment_id),
            TerminalViewerOutcome::Rejected(ViewerRejection::AttachmentUnavailable)
        );
    }
    assert_eq!(host.viewers.borrow().retained_record_bytes(), 0);
    Ok(())
}

#[test]
fn target_cleanup_preserves_sibling_flight_and_seven_other_controllers() -> TestResult {
    let (host, _peer) = host()?;
    for id in 1..=8 {
        let request = viewer(id, id)?;
        host.viewers
            .borrow_mut()
            .admit(&request)
            .map_err(|e| format!("{e:?}"))?;
        host.receive_event(frame(id, host.host_epoch())?)?;
        host.controllers
            .borrow_mut()
            .reserve(
                request.attachment_id,
                OperationId::from_bytes([100 + id; 16])?,
                identity(id)?.terminal_id(),
            )
            .map_err(|e| format!("{e:?}"))?;
    }
    assert_eq!(
        host.controllers.borrow_mut().reserve(
            OperationId::from_bytes([9; 16])?,
            OperationId::from_bytes([109; 16])?,
            identity(9)?.terminal_id()
        ),
        Err(ControlRejection::ControllerBusy)
    );
    // One target has one controller even though it may have eight viewers.
    assert_eq!(
        host.controllers.borrow_mut().reserve(
            OperationId::from_bytes([19; 16])?,
            OperationId::from_bytes([119; 16])?,
            identity(1)?.terminal_id()
        ),
        Err(ControlRejection::ControllerBusy)
    );
    host.commit_target_cleanup(host.prepare_target_cleanup(identity(1)?)?);
    assert!(
        !host
            .controllers
            .borrow()
            .contains(OperationId::from_bytes([101; 16])?)
    );
    for id in 2..=8 {
        assert!(
            host.controllers
                .borrow()
                .contains(OperationId::from_bytes([100 + id; 16])?)
        );
        assert!(matches!(
            host.viewers
                .borrow_mut()
                .poll(viewer(id, id)?.attachment_id),
            TerminalViewerOutcome::Record(_)
        ));
    }
    host.controllers
        .borrow_mut()
        .reserve(
            OperationId::from_bytes([9; 16])?,
            OperationId::from_bytes([109; 16])?,
            identity(9)?.terminal_id(),
        )
        .map_err(|e| format!("{e:?}"))?;
    // An exact old ID removal does not erase a distinct later reservation.
    host.controllers
        .borrow_mut()
        .remove(OperationId::from_bytes([101; 16])?);
    assert!(
        host.controllers
            .borrow()
            .contains(OperationId::from_bytes([109; 16])?)
    );
    Ok(())
}

#[test]
fn prepared_but_unacknowledged_cleanup_does_not_retire_any_owner() -> TestResult {
    let (host, _peer) = host()?;
    let request = viewer(1, 5)?;
    host.viewers
        .borrow_mut()
        .admit(&request)
        .map_err(|e| format!("{e:?}"))?;
    let _prepared = host.prepare_target_cleanup(identity(5)?)?;
    assert!(!host.disposed_viewer(request.attachment_id));
    host.receive_event(frame(1, host.host_epoch())?)?;
    assert!(matches!(
        host.viewers.borrow_mut().poll(request.attachment_id),
        TerminalViewerOutcome::Record(_)
    ));
    // Exact ordinary lease cleanup remains possible without any Dispose fact.
    host.viewers.borrow_mut().remove(request.attachment_id);
    assert!(!host.disposed_terminal(identity(5)?.terminal_id()));
    Ok(())
}

#[test]
fn late_retired_frames_require_exact_epoch_and_id_and_old_detach_is_local() -> TestResult {
    let (host, mut peer) = host()?;
    let request = viewer(1, 5)?;
    host.viewers
        .borrow_mut()
        .admit(&request)
        .map_err(|e| format!("{e:?}"))?;
    host.commit_target_cleanup(host.prepare_target_cleanup(identity(5)?)?);
    host.receive_event(frame(1, host.host_epoch())?)?;
    assert!(host.receive_event(frame(99, host.host_epoch())?).is_err());
    assert!(
        host.receive_event(frame(1, HostEpoch::from_bytes([99; 16])?)?)
            .is_err()
    );
    let (server, _server_peer) = UnixStream::pair()?;
    let detached = crate::terminal_viewer_operation::execute(
        TerminalViewerRequest {
            action: TerminalViewerAction::Detach,
            ..request
        },
        orchbee_local_worker_protocol::WorkerId::from_bytes([3; 16]),
        NonZeroU64::MIN,
        &host,
        &server,
    )?;
    assert_eq!(detached.outcome, TerminalViewerOutcome::Detached);
    let attached = crate::terminal_viewer_operation::execute(
        viewer(2, 5)?,
        orchbee_local_worker_protocol::WorkerId::from_bytes([3; 16]),
        NonZeroU64::MIN,
        &host,
        &server,
    )?;
    assert_eq!(
        attached.outcome,
        TerminalViewerOutcome::Rejected(ViewerRejection::AttachmentUnavailable)
    );
    peer.set_nonblocking(true)?;
    let mut byte = [0];
    assert_eq!(
        std::io::Read::read(&mut peer, &mut byte)
            .expect_err("zero Host requests")
            .kind(),
        io::ErrorKind::WouldBlock
    );
    Ok(())
}

#[test]
fn retired_viewer_history_events_are_swallowed_with_exact_identity() -> TestResult {
    fn history_event(id: u8, epoch: HostEpoch) -> TestResult<Vec<u8>> {
        let control = encode_history_control(&HistoryControl::HistoryChanged(HistoryChanged {
            anchor_status: HistoryAnchorStatus::Valid,
            attachment_id: AttachmentId::from_bytes([id; 16])?,
            generation: NonZeroU64::MIN,
            revision: 5,
            sequence: NonZeroU64::MIN,
            total_rows: 100,
        }))?;
        Ok(encode_transport_envelope(&Envelope {
            correlation_id: None,
            host_epoch: Some(epoch),
            kind: control.kind,
            optional_message: false,
            payload: &control.payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Event,
        })?)
    }
    let (host, _peer) = host()?;
    let request = viewer(1, 5)?;
    host.viewers
        .borrow_mut()
        .admit(&request)
        .map_err(|e| format!("{e:?}"))?;
    // Before retirement the same event is accepted as a live-viewer record.
    assert!(
        host.receive_event(history_event(1, host.host_epoch())?)
            .is_ok()
    );
    host.commit_target_cleanup(host.prepare_target_cleanup(identity(5)?)?);
    // A late HistoryChanged for the retired viewer is consumed quietly.
    host.receive_event(history_event(1, host.host_epoch())?)?;
    // Another attachment's event and a foreign epoch still fail the broker.
    assert!(
        host.receive_event(history_event(2, host.host_epoch())?)
            .is_err()
    );
    assert!(
        host.receive_event(history_event(1, HostEpoch::from_bytes([9; 16])?)?)
            .is_err()
    );
    Ok(())
}

#[test]
fn retired_viewer_selection_events_are_swallowed_with_exact_identity() -> TestResult {
    fn selection_events(id: u8, epoch: HostEpoch) -> TestResult<[Vec<u8>; 3]> {
        let changed =
            encode_selection_control(&SelectionControl::ChangedEvent(SelectionChanged {
                attachment_id: AttachmentId::from_bytes([id; 16])?,
                state: SelectionChangedState::None,
            }))?;
        let chunk = encode_selection_export_chunk(&SelectionExportChunk {
            attachment_id: AttachmentId::from_bytes([id; 16])?,
            bytes: vec![0x00, 0xff, 0x80, 0x61],
            chunk_sequence: 0,
            export_sequence: NonZeroU64::MIN,
            final_chunk: true,
        })?;
        let finished = encode_selection_export_control(&SelectionExportControl::FinishedEvent(
            SelectionExportFinished {
                attachment_id: AttachmentId::from_bytes([id; 16])?,
                byte_length: 4,
                export_sequence: NonZeroU64::MIN,
                status: SelectionExportStatus::Completed,
            },
        ))?;
        Ok([
            encode_transport_envelope(&Envelope {
                correlation_id: None,
                host_epoch: Some(epoch),
                kind: changed.kind,
                optional_message: false,
                payload: &changed.payload,
                payload_encoding: PayloadEncoding::ControlJson,
                protocol: CURRENT_PROTOCOL_VERSION,
                role: MessageRole::Event,
            })?,
            encode_transport_envelope(&Envelope {
                correlation_id: None,
                host_epoch: Some(epoch),
                kind: chunk.kind,
                optional_message: false,
                payload: &chunk.payload,
                payload_encoding: PayloadEncoding::RawBytes,
                protocol: CURRENT_PROTOCOL_VERSION,
                role: MessageRole::Event,
            })?,
            encode_transport_envelope(&Envelope {
                correlation_id: None,
                host_epoch: Some(epoch),
                kind: finished.kind,
                optional_message: false,
                payload: &finished.payload,
                payload_encoding: PayloadEncoding::ControlJson,
                protocol: CURRENT_PROTOCOL_VERSION,
                role: MessageRole::Event,
            })?,
        ])
    }

    let (host, _peer) = host()?;
    let request = viewer(1, 5)?;
    host.viewers
        .borrow_mut()
        .admit(&request)
        .map_err(|error| format!("{error:?}"))?;
    host.commit_target_cleanup(host.prepare_target_cleanup(identity(5)?)?);

    for event in selection_events(1, host.host_epoch())? {
        host.receive_event(event)?;
    }
    for event in selection_events(2, host.host_epoch())? {
        assert!(host.receive_event(event).is_err());
    }
    assert_eq!(host.viewers.borrow().retained_record_bytes(), 0);
    Ok(())
}

#[test]
fn retired_controller_events_keep_exact_terminal_identity_and_existing_preack_rules() -> TestResult
{
    use orchbee_terminal_protocol::{AttachmentId, ResizeLeaseChanged, encode_attachment_control};
    let (host, _peer) = host()?;
    let viewer = viewer(1, 5)?;
    let controller = OperationId::from_bytes([101; 16])?;
    host.viewers
        .borrow_mut()
        .admit(&viewer)
        .map_err(|e| format!("{e:?}"))?;
    host.controllers
        .borrow_mut()
        .reserve(viewer.attachment_id, controller, identity(5)?.terminal_id())
        .map_err(|e| format!("{e:?}"))?;
    let event = |terminal: u8, granted: bool| -> TestResult<Vec<u8>> {
        let control = encode_attachment_control(&AttachmentControl::ResizeLeaseChanged(
            ResizeLeaseChanged {
                attachment_id: AttachmentId::from_bytes(controller.as_bytes())?,
                resize_lease: granted,
                terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes([terminal; 16])?,
            },
        ))?;
        Ok(encode_transport_envelope(&Envelope {
            correlation_id: None,
            host_epoch: Some(host.host_epoch()),
            kind: control.kind,
            optional_message: false,
            payload: &control.payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Event,
        })?)
    };
    let prepared = host.prepare_target_cleanup(identity(5)?)?;
    assert!(
        host.receive_event(event(5, false)?).is_err(),
        "pre-ACK policy is unchanged"
    );
    host.commit_target_cleanup(prepared);
    host.receive_event(event(5, false)?)?;
    host.receive_event(event(5, true)?)?;
    assert!(host.receive_event(event(99, false)?).is_err());
    let (server, _server_peer) = UnixStream::pair()?;
    let feedback = crate::terminal_viewer_operation::execute(
        TerminalViewerRequest {
            action: TerminalViewerAction::FrameApplied {
                sequence: NonZeroU64::MIN,
                generation: NonZeroU64::MIN,
                revision: 5,
            },
            ..viewer
        },
        orchbee_local_worker_protocol::WorkerId::from_bytes([3; 16]),
        NonZeroU64::MIN,
        &host,
        &server,
    )?;
    assert_eq!(
        feedback.outcome,
        TerminalViewerOutcome::Rejected(ViewerRejection::AttachmentUnavailable)
    );
    Ok(())
}

#[test]
fn retired_target_storage_reuses_prepared_cell_and_stops_at_binding_capacity() -> TestResult {
    let (host, _peer) = host()?;
    for id in 1_u16..=1024 {
        let mut bytes = [1; 16];
        bytes[..2].copy_from_slice(&id.to_le_bytes());
        let target = HostTerminalIdentity::new(
            HostCreationId::from_bytes(bytes)?,
            orchbee_capability_node_protocol::HostEpoch::from_bytes([4; 16])?,
            HostTerminalId::from_bytes(bytes)?,
        );
        let first = host.prepare_target_cleanup(target)?;
        let second = host.prepare_target_cleanup(target)?;
        assert_eq!(first.index, second.index);
        assert_eq!(host.retired_targets.borrow().targets.len(), usize::from(id));
    }
    let overflow = HostTerminalIdentity::new(
        HostCreationId::from_bytes([99; 16])?,
        orchbee_capability_node_protocol::HostEpoch::from_bytes([4; 16])?,
        HostTerminalId::from_bytes([99; 16])?,
    );
    assert!(host.prepare_target_cleanup(overflow).is_err());
    assert_eq!(host.retired_targets.borrow().targets.len(), 1024);
    assert!(!host.disposed_terminal(overflow.terminal_id()));
    Ok(())
}
