use super::*;
use orchbee_capability_node_protocol::{
    LifecycleInventoryCursor, LifecycleInventoryPage, LifecycleTargetObservation,
    LifecycleTerminalPhase, TerminalControlAction, TerminalControlOutcome, TerminalControlResult,
    TerminalLifecycleAction, TerminalLifecycleOutcome, TerminalLifecycleResult,
    TerminalViewerResult, read_message, write_message,
};
use std::collections::BTreeMap;

/// More pages than any fixed allowance, within a popup's lifecycle step ceiling.
pub(in crate::local_terminal_service::viewer_connection::tests) const POPUP_INVENTORY_PAGES: u64 =
    12;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::local_terminal_service::viewer_connection::tests) enum Effect {
    Attach([u8; 16], usize),
    Capacity,
    Poll([u8; 16]),
    ProcessInfo([u8; 16]),
    GraphicsDecode([u8; 16]),
    Lifecycle([u8; 16]),
    DetachRefused([u8; 16]),
    Feedback([u8; 16]),
    Detach([u8; 16]),
    Acquire([u8; 16], [u8; 16]),
    Clear([u8; 16], [u8; 16]),
    Input([u8; 16], [u8; 16], u64),
    Resize([u8; 16], [u8; 16], u64),
    Paste([u8; 16], [u8; 16], u64),
    Release([u8; 16], [u8; 16]),
    Dispose(usize),
    PopupLaunch([u8; 16]),
    PopupForce(usize),
    PopupInventory(usize, u8),
    EditorCommand,
    EditorCapture(orchbee_capability_node_protocol::TerminalEditorSource),
    EditorStart,
}
pub(super) fn run(
    mut peer: UnixStream,
    effects: Arc<Mutex<Vec<Effect>>>,
    detach_refusals: Arc<std::sync::atomic::AtomicUsize>,
    popup_fault: Arc<std::sync::atomic::AtomicU8>,
    static_codex: bool,
) {
    let mut viewers: BTreeMap<[u8; 16], (usize, u64, bool)> = BTreeMap::new();
    loop {
        let message = match read_message(&mut peer) {
            Ok(message) => message,
            Err(_) => return,
        };
        let reply = match message {
            CapabilityNodeMessage::TerminalGraphicsDecodeRequest(request) => {
                assert!(viewers.contains_key(&request.attachment_id.as_bytes()));
                effects
                    .lock()
                    .unwrap()
                    .push(Effect::GraphicsDecode(request.attachment_id.as_bytes()));
                CapabilityNodeMessage::TerminalViewerResult(TerminalViewerResult {
                    request_id: request.request_id,
                    attachment_id: request.attachment_id,
                    outcome: WorkerOutcome::Rejected(ViewerRejection::HostRejected),
                })
            }
            CapabilityNodeMessage::TerminalViewerRequest(request) => {
                let id = request.attachment_id.as_bytes();
                let outcome = if request.request_id.as_bytes()[..8] == [250; 8] {
                    WorkerOutcome::Empty
                } else {
                    match request.action {
                        WorkerAction::Attach { terminal_id, .. } => {
                            if viewers.len() == 8 {
                                effects.lock().unwrap().push(Effect::Capacity);
                                WorkerOutcome::Rejected(ViewerRejection::CapacityExceeded)
                            } else {
                                let target = usize::from(terminal_id.as_bytes()[0] - 5);
                                assert!(viewers.insert(id, (target, 0, false)).is_none());
                                effects.lock().unwrap().push(Effect::Attach(id, target));
                                WorkerOutcome::Attached
                            }
                        }
                        WorkerAction::Poll => {
                            effects.lock().unwrap().push(Effect::Poll(id));
                            if let Some((_, sequence, flight)) = viewers.get_mut(&id) {
                                // The record slot stays occupied until its
                                // feedback: a poll during flight sees Empty.
                                if *flight {
                                    WorkerOutcome::Empty
                                } else {
                                    *sequence += 1;
                                    *flight = true;
                                    WorkerOutcome::Record(frame(id, *sequence, static_codex))
                                }
                            } else {
                                WorkerOutcome::Empty
                            }
                        }
                        WorkerAction::ReadProcessInfo => {
                            effects.lock().unwrap().push(Effect::ProcessInfo(id));
                            if static_codex {
                                let target = viewers.get(&id).expect("attached viewer").0;
                                WorkerOutcome::Record(process_record(request, target))
                            } else {
                                WorkerOutcome::Rejected(ViewerRejection::HostRejected)
                            }
                        },
                        WorkerAction::ReadLifecycle { target } => {
                            assert!(viewers.contains_key(&id));
                            effects.lock().unwrap().push(Effect::Lifecycle(id));
                            WorkerOutcome::LifecycleObserved { target, revision: 42, settled: true }
                        },
                        WorkerAction::NeedSnapshot { sequence, reason } if static_codex => {
                            effects.lock().unwrap().push(Effect::Feedback(id));
                            let (_, current, flight) =
                                viewers.get_mut(&id).expect("attached viewer");
                            static_snapshot_feedback(*current, flight, sequence, reason)
                        }
                        WorkerAction::FrameApplied { .. } | WorkerAction::NeedSnapshot { .. } => {
                            effects.lock().unwrap().push(Effect::Feedback(id));
                            if let Some((_, _, flight)) = viewers.get_mut(&id) {
                                *flight = false;
                            }
                            WorkerOutcome::FeedbackAccepted
                        }
                        WorkerAction::ReadRows {
                            detail,
                            start_row,
                            row_count,
                            revision,
                            generation,
                            request_sequence,
                            maximum_bytes,
                        } if static_codex => {
                            let (_, current, flight) = viewers.get(&id).expect("attached viewer");
                            if !*flight
                                || request_sequence.get() != *current
                                || revision != *current
                                || generation != NonZeroU64::MIN
                                || start_row != 123
                                || row_count != 2
                            {
                                WorkerOutcome::RowsRejected(
                                    orchbee_capability_node_protocol::ViewerRowsRejectedReason::StaleRevision,
                                )
                            } else {
                                row_window_record(
                                    request,
                                    detail,
                                    start_row,
                                    row_count,
                                    revision,
                                    generation,
                                    request_sequence,
                                    maximum_bytes,
                                )
                            }
                        }
                        WorkerAction::ReadRows { .. } | WorkerAction::ReadViewportRows { .. } => {
                            WorkerOutcome::RowsRejected(
                                orchbee_capability_node_protocol::ViewerRowsRejectedReason::MissingResource,
                            )
                        }
                        WorkerAction::SetProjectionDemand { .. }
                        | WorkerAction::SetHistoryDemand { .. }
                        | WorkerAction::RowsApplied { .. }
                        | WorkerAction::HistoryObserved { .. } => {
                            effects.lock().unwrap().push(Effect::Feedback(id));
                            WorkerOutcome::FeedbackAccepted
                        }
                        WorkerAction::SelectionGesture { .. }
                        | WorkerAction::SelectionCommand { .. }
                        | WorkerAction::SelectionExport { .. }
                        | WorkerAction::SelectionExportChunkApplied { .. }
                        | WorkerAction::SelectionExportCancel { .. } => {
                            WorkerOutcome::Rejected(ViewerRejection::HostRejected)
                        }
                        WorkerAction::Detach => {
                            use std::sync::atomic::Ordering;
                            if detach_refusals.fetch_update(Ordering::AcqRel,Ordering::Acquire,|n|n.checked_sub(1)).is_ok() {
                                effects.lock().unwrap().push(Effect::DetachRefused(id));
                                WorkerOutcome::Rejected(ViewerRejection::HostRejected)
                            } else {
                                effects.lock().unwrap().push(Effect::Detach(id));
                                viewers.remove(&id);
                                WorkerOutcome::Detached
                            }
                        }
                    }
                };
                CapabilityNodeMessage::TerminalViewerResult(TerminalViewerResult {
                    request_id: request.request_id,
                    attachment_id: request.attachment_id,
                    outcome,
                })
            }
            CapabilityNodeMessage::TerminalControlRequest(request) => {
                let v = request.viewer_id.as_bytes();
                let c = request.controller_id.as_bytes();
                let (effect, outcome) = match request.action {
                    TerminalControlAction::Clear => {
                        assert!(viewers.contains_key(&v));
                        assert_ne!(v, c);
                        (
                            Effect::Clear(v, c),
                            TerminalControlOutcome::Cleared {
                                generation: std::num::NonZeroU64::new(2).unwrap(),
                                revision: std::num::NonZeroU64::new(7).unwrap(),
                                cleanup_confirmed: true,
                            },
                        )
                    }
                    TerminalControlAction::Reset => panic!("unexpected reset in viewer fixture"),
                    TerminalControlAction::Takeover { .. } => {
                        panic!("unexpected takeover in viewer fixture")
                    }
                    TerminalControlAction::HostColors { .. } => {
                        panic!("unexpected host colors in viewer fixture")
                    }
                    TerminalControlAction::SemanticInput { .. } => {
                        panic!("unexpected semantic input in viewer fixture")
                    }
                    TerminalControlAction::Acquire => {
                        (Effect::Acquire(v, c), TerminalControlOutcome::Acquired)
                    }
                    TerminalControlAction::Input { sequence, .. } => (
                        Effect::Input(v, c, sequence.get()),
                        TerminalControlOutcome::InputAccepted,
                    ),
                    TerminalControlAction::Resize { sequence, geometry } => (
                        Effect::Resize(v, c, sequence.get()),
                        TerminalControlOutcome::Resized(geometry),
                    ),
                    TerminalControlAction::Paste { sequence, .. } => (
                        Effect::Paste(v, c, sequence.get()),
                        TerminalControlOutcome::Pasted {
                            accepted_bytes:
                                orchbee_capability_node_protocol::PasteAcceptedBytes::new(1)
                                    .expect("bounded"),
                        },
                    ),
                    TerminalControlAction::Release => {
                        (Effect::Release(v, c), TerminalControlOutcome::Released)
                    }
                };
                effects.lock().unwrap().push(effect);
                CapabilityNodeMessage::TerminalControlResult(TerminalControlResult {
                    request_id: request.request_id,
                    viewer_id: request.viewer_id,
                    controller_id: request.controller_id,
                    outcome,
                })
            }
            CapabilityNodeMessage::TerminalLifecycleRequest(request) => {
                let target = usize::from(request.target.terminal_id().as_bytes()[0] - 5);
                let outcome = match request.action {
                    TerminalLifecycleAction::Force if target >= 10 => {
                        effects.lock().unwrap().push(Effect::PopupForce(target));
                        if popup_fault.load(std::sync::atomic::Ordering::Acquire) == 2 {
                            return;
                        }
                        TerminalLifecycleOutcome::ForceAccepted
                    }
                    TerminalLifecycleAction::InventoryPage { cursor } => {
                        if target >= 10 {
                            effects
                                .lock()
                                .unwrap()
                                .push(Effect::PopupInventory(target, request.step_ordinal.get()));
                        }
                        // Fault 12 spreads a popup's inventory over `POPUP_INVENTORY_PAGES` pages of
                        // one entry, its target on the last.
                        let offset = cursor.map_or(0, |cursor| cursor.offset);
                        let pending = target >= 10
                            && popup_fault.load(std::sync::atomic::Ordering::Acquire) == 12
                            && offset + 1 < POPUP_INVENTORY_PAGES;
                        TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
                            inventory_generation: NonZeroU64::MIN,
                            next_cursor: pending.then_some(LifecycleInventoryCursor {
                                inventory_generation: NonZeroU64::MIN,
                                offset: offset + 1,
                            }),
                            returned_count: 1,
                            target: (!pending).then_some(LifecycleTargetObservation {
                                revision: 3,
                                phase: if popup_fault.load(std::sync::atomic::Ordering::Acquire)
                                    == 9
                                {
                                    LifecycleTerminalPhase::AwaitingStart
                                } else {
                                    LifecycleTerminalPhase::Exited {
                                        terminate_requested: false,
                                    }
                                },
                            }),
                        })
                    }
                    TerminalLifecycleAction::Dispose => {
                        viewers.retain(|_, (current, ..)| *current != target);
                        effects.lock().unwrap().push(Effect::Dispose(target));
                        TerminalLifecycleOutcome::Disposed
                    }
                    _ => panic!("settled target needs no signal"),
                };
                CapabilityNodeMessage::TerminalLifecycleResult(
                    TerminalLifecycleResult::new(request, outcome).unwrap(),
                )
            }
            CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) => {
                let state = if let Some(source) = launch.editor_source() {
                    assert_eq!(launch.mode(), TerminalLaunchMode::Deferred);
                    assert!(launch.plugin_environment().is_none());
                    assert_eq!(launch.launch().unwrap().executable(), "C:/editor.exe");
                    assert_eq!(
                        launch.launch().unwrap().arguments(),
                        ["--wait", "", "literal & % 字"]
                    );
                    assert!(viewers.contains_key(&source.attachment_id.as_bytes()));
                    assert_eq!(source.host_epoch.as_bytes(), [4; 16]);
                    assert_eq!(source.terminal_id.as_bytes(), [5; 16]);
                    assert_eq!((source.generation.get(), source.revision), (1, 1));
                    effects.lock().unwrap().push(Effect::EditorCapture(source));
                    HostLaunchState::AwaitingStart
                } else {
                    assert!(launch.plugin_environment().is_some());
                    assert!(launch.launch().unwrap().executable().ends_with("pane.exe"));
                    let TerminalLaunchMode::Immediate(geometry) = launch.mode() else {
                        panic!("popup immediate")
                    };
                    HostLaunchState::Started(geometry)
                };
                effects.lock().unwrap().push(Effect::PopupLaunch(
                    launch.authorization().terminal_id().as_bytes(),
                ));
                if popup_fault.load(std::sync::atomic::Ordering::Acquire) == 1 {
                    return;
                }
                let identity = HostTerminalIdentity::new(
                    HostCreationId::from_bytes([20; 16]).unwrap(),
                    orchbee_capability_node_protocol::HostEpoch::from_bytes([4; 16]).unwrap(),
                    HostTerminalId::from_bytes([15; 16]).unwrap(),
                );
                CapabilityNodeMessage::TerminalLaunchResult(
                    TerminalLaunchResult::new(
                        launch.authorization(),
                        if popup_fault.load(std::sync::atomic::Ordering::Acquire)==11 {
                            TerminalLaunchOutcome::Rejected(orchbee_capability_node_protocol::TerminalLaunchRejection::HostRejected {host_epoch:identity.host_epoch()})
                        } else { TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                            identity,
                            state,
                        )) },
                    )
                    .unwrap(),
                )
            }
            CapabilityNodeMessage::TerminalEditorCommandRequest(request) => {
                effects.lock().unwrap().push(Effect::EditorCommand);
                CapabilityNodeMessage::TerminalEditorCommandResult(
                    orchbee_capability_node_protocol::TerminalEditorCommandResult {
                        request,
                        outcome: if popup_fault.load(std::sync::atomic::Ordering::Acquire) == 8 {
                            orchbee_capability_node_protocol::TerminalEditorCommandOutcome::Refused(orchbee_capability_node_protocol::TerminalEditorCommandRefusal::InvalidCommand)
                        } else {
                            orchbee_capability_node_protocol::TerminalEditorCommandOutcome::Resolved {
                        source:orchbee_capability_node_protocol::TerminalEditorCommandSource::Editor,
                        selection:orchbee_capability_node_protocol::LaunchSelection::new("C:/editor.exe".into(),vec!["--wait".into(),"".into(),"literal & % 字".into()]).unwrap(),
                    }
                        },
                    },
                )
            }
            CapabilityNodeMessage::AuthorizedTerminalStart(request) => {
                effects.lock().unwrap().push(Effect::EditorStart);
                if popup_fault.load(std::sync::atomic::Ordering::Acquire) == 10 {
                    return;
                }
                let outcome = if popup_fault.load(std::sync::atomic::Ordering::Acquire) == 9 {
                    orchbee_capability_node_protocol::TerminalStartOutcome::Failed(orchbee_capability_node_protocol::TerminalStartFailure::HostReportedFailure {host_epoch:request.identity().host_epoch()})
                } else {
                    orchbee_capability_node_protocol::TerminalStartOutcome::HostAccepted(
                        orchbee_capability_node_protocol::HostStartAccepted::new(
                            request.identity(),
                            request.geometry(),
                            orchbee_capability_node_protocol::StartTransition::Started,
                        ),
                    )
                };
                CapabilityNodeMessage::TerminalStartResult(
                    orchbee_capability_node_protocol::TerminalStartResult::new(
                        request.authorization(),
                        request.identity(),
                        outcome,
                    )
                    .unwrap(),
                )
            }
            other => panic!("unexpected Worker message {other:?}"),
        };
        if write_message(&mut peer, &reply).is_err() {
            return;
        }
    }
}

pub(in crate::local_terminal_service::viewer_connection::tests) fn static_codex_process(
    target: usize,
    attachment: [u8; 16],
) -> orchbee_terminal_protocol::TerminalProcessInfo {
    use orchbee_terminal_protocol::{
        AttachmentId, TerminalId, TerminalProcess, TerminalProcessConsole, TerminalProcessInfo,
    };
    let target = u8::try_from(target).expect("fixture target fits in one byte");
    let pid = 4_100 + u32::from(target);
    TerminalProcessInfo {
        attachment_id: AttachmentId::from_bytes(attachment).unwrap(),
        terminal_id: TerminalId::from_bytes([5 + target; 16]).unwrap(),
        root_pid: pid,
        processes: vec![TerminalProcess {
            pid,
            parent_pid: None,
            creation_time: 9_000 + u64::from(target),
            console: TerminalProcessConsole::Attached,
            image: Some("C:/fixtures/codex.exe".into()),
            command_line: Some("codex.exe".into()),
            current_directory: None,
        }],
        launch_kind: None,
    }
}

fn process_record(request: WorkerRequest, target: usize) -> Vec<u8> {
    use orchbee_terminal_protocol::{
        CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, HostEpoch, PayloadEncoding,
        ProcessInfoControl, encode_process_info_control, encode_transport_envelope,
    };
    let encoded = encode_process_info_control(&ProcessInfoControl::ProcessInfo(
        static_codex_process(target, request.attachment_id.as_bytes()),
    ))
    .unwrap();
    encode_transport_envelope(&Envelope {
        correlation_id: Some(CorrelationId::from_bytes(request.request_id.as_bytes()).unwrap()),
        host_epoch: Some(HostEpoch::from_bytes([4; 16]).unwrap()),
        kind: encoded.kind,
        optional_message: false,
        payload: &encoded.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: encoded.role,
    })
    .unwrap()
}

fn static_snapshot_feedback(
    current: u64,
    flight: &mut bool,
    sequence: NonZeroU64,
    reason: orchbee_capability_node_protocol::ViewerSnapshotReason,
) -> WorkerOutcome {
    if *flight
        && sequence.get() == current
        && reason == orchbee_capability_node_protocol::ViewerSnapshotReason::StoreReset
    {
        *flight = false;
        WorkerOutcome::FeedbackAccepted
    } else {
        WorkerOutcome::Rejected(ViewerRejection::FeedbackMismatch)
    }
}

#[allow(clippy::too_many_arguments)]
fn row_window_record(
    request: WorkerRequest,
    detail: orchbee_capability_node_protocol::ViewerRowDetail,
    start_row: u64,
    row_count: u16,
    revision: u64,
    generation: NonZeroU64,
    request_sequence: NonZeroU64,
    maximum_bytes: u32,
) -> WorkerOutcome {
    use orchbee_terminal_protocol::{
        CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, HostEpoch, MessageRole, PayloadEncoding,
        encode_transport_envelope,
    };
    if detail != orchbee_capability_node_protocol::ViewerRowDetail::Text {
        return WorkerOutcome::RowsRejected(
            orchbee_capability_node_protocol::ViewerRowsRejectedReason::InvalidField,
        );
    }
    let Some(body_bytes) = usize::from(row_count).checked_mul(12) else {
        return WorkerOutcome::RowsRejected(
            orchbee_capability_node_protocol::ViewerRowsRejectedReason::LimitExceeded,
        );
    };
    let Some(bytes) = 112_usize.checked_add(body_bytes) else {
        return WorkerOutcome::RowsRejected(
            orchbee_capability_node_protocol::ViewerRowsRejectedReason::LimitExceeded,
        );
    };
    if bytes > maximum_bytes as usize {
        return WorkerOutcome::RowsRejected(
            orchbee_capability_node_protocol::ViewerRowsRejectedReason::LimitExceeded,
        );
    }
    let Some(total_rows) = start_row.checked_add(u64::from(row_count)) else {
        return WorkerOutcome::RowsRejected(
            orchbee_capability_node_protocol::ViewerRowsRejectedReason::InvalidField,
        );
    };
    let mut payload = vec![0_u8; bytes];
    payload[..4].copy_from_slice(b"OBRW");
    payload[4] = 1;
    payload[6] = 2;
    payload[8..12].copy_from_slice(&(bytes as u32).to_le_bytes());
    payload[12..14].copy_from_slice(&112_u16.to_le_bytes());
    payload[14..16].copy_from_slice(&6_u16.to_le_bytes());
    payload[16..32].fill(4);
    payload[32..48].copy_from_slice(&request.attachment_id.as_bytes());
    payload[48..56].copy_from_slice(&request_sequence.get().to_le_bytes());
    payload[56..64].copy_from_slice(&revision.to_le_bytes());
    payload[64..72].copy_from_slice(&generation.get().to_le_bytes());
    payload[72..80].copy_from_slice(&total_rows.to_le_bytes());
    payload[80..88].copy_from_slice(&start_row.to_le_bytes());
    payload[88..90].copy_from_slice(&row_count.to_le_bytes());
    payload[96..100].copy_from_slice(&(body_bytes as u32).to_le_bytes());
    payload[100] = 0;
    WorkerOutcome::Record(
        encode_transport_envelope(&Envelope {
            correlation_id: Some(CorrelationId::from_bytes(request.request_id.as_bytes()).unwrap()),
            host_epoch: Some(HostEpoch::from_bytes([4; 16]).unwrap()),
            kind: 46,
            optional_message: false,
            payload: &payload,
            payload_encoding: PayloadEncoding::RowWindow,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Response,
        })
        .unwrap(),
    )
}

fn frame(id: [u8; 16], sequence: u64, static_codex: bool) -> Vec<u8> {
    use orchbee_terminal_protocol::{
        CURRENT_PROTOCOL_VERSION, Envelope, HostEpoch, MessageRole, PayloadEncoding,
        encode_transport_envelope,
    };
    let mut payload = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/terminal/v1/fixtures/terminal-frame.bin"
    ))
    .to_vec();
    payload[6] = 1;
    payload[16..32].fill(4);
    payload[32..48].copy_from_slice(&id);
    payload[48..56].copy_from_slice(&sequence.to_le_bytes());
    payload[56..64].fill(255);
    payload[64..72].copy_from_slice(&sequence.to_le_bytes());
    payload[72..80].copy_from_slice(&1_u64.to_le_bytes());
    if static_codex {
        let old = "Orchbee 🐝".as_bytes();
        let new = "⠋ working!".as_bytes();
        assert_eq!(old.len(), new.len());
        let offset = payload
            .windows(old.len())
            .position(|window| window == old)
            .expect("canonical title");
        payload[offset..offset + old.len()].copy_from_slice(new);
    }
    encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(HostEpoch::from_bytes([4; 16]).unwrap()),
        kind: 41,
        optional_message: false,
        payload: &payload,
        payload_encoding: PayloadEncoding::TerminalFrame,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
    })
    .unwrap()
}

#[test]
fn static_codex_snapshot_feedback_requires_the_current_in_flight_store_reset() {
    use orchbee_capability_node_protocol::ViewerSnapshotReason;
    let mut flight = true;
    assert_eq!(
        static_snapshot_feedback(
            7,
            &mut flight,
            NonZeroU64::new(8).unwrap(),
            ViewerSnapshotReason::StoreReset,
        ),
        WorkerOutcome::Rejected(ViewerRejection::FeedbackMismatch)
    );
    assert!(flight, "a wrong sequence cannot release the record slot");
    assert_eq!(
        static_snapshot_feedback(
            7,
            &mut flight,
            NonZeroU64::new(7).unwrap(),
            ViewerSnapshotReason::SequenceGap,
        ),
        WorkerOutcome::Rejected(ViewerRejection::FeedbackMismatch)
    );
    assert!(flight, "a wrong reason cannot release the record slot");
    assert_eq!(
        static_snapshot_feedback(
            7,
            &mut flight,
            NonZeroU64::new(7).unwrap(),
            ViewerSnapshotReason::StoreReset,
        ),
        WorkerOutcome::FeedbackAccepted
    );
    assert!(!flight);
    assert_eq!(
        static_snapshot_feedback(
            7,
            &mut flight,
            NonZeroU64::new(7).unwrap(),
            ViewerSnapshotReason::StoreReset,
        ),
        WorkerOutcome::Rejected(ViewerRejection::FeedbackMismatch),
        "a stale duplicate cannot acknowledge an already released slot"
    );
}
