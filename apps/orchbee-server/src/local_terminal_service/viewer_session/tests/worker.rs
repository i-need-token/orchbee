use super::*;
use orchbee_capability_node_protocol::{
    LifecycleInventoryPage, LifecycleTargetObservation, LifecycleTerminalPhase,
    TerminalControlAction, TerminalControlOutcome, TerminalControlResult, TerminalLifecycleAction,
    TerminalLifecycleOutcome, TerminalLifecycleResult, TerminalViewerResult, read_message,
    write_message,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::local_terminal_service::viewer_session::tests) enum Effect {
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
                                    WorkerOutcome::Record(frame(id, *sequence))
                                }
                            } else {
                                WorkerOutcome::Empty
                            }
                        }
                        WorkerAction::ReadProcessInfo => {effects.lock().unwrap().push(Effect::ProcessInfo(id));WorkerOutcome::Rejected(ViewerRejection::HostRejected)},
                        WorkerAction::ReadLifecycle { target } => {
                            assert!(viewers.contains_key(&id));
                            effects.lock().unwrap().push(Effect::Lifecycle(id));
                            WorkerOutcome::LifecycleObserved { target, revision: 42, settled: true }
                        },
                        WorkerAction::FrameApplied { .. } | WorkerAction::NeedSnapshot { .. } => {
                            effects.lock().unwrap().push(Effect::Feedback(id));
                            if let Some((_, _, flight)) = viewers.get_mut(&id) {
                                *flight = false;
                            }
                            WorkerOutcome::FeedbackAccepted
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
                    TerminalControlAction::Takeover { .. } => {
                        panic!("unexpected takeover in viewer fixture")
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
                    TerminalLifecycleAction::InventoryPage { .. } => {
                        if target >= 10 {
                            effects
                                .lock()
                                .unwrap()
                                .push(Effect::PopupInventory(target, request.step_ordinal.get()));
                        }
                        TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
                            inventory_generation: NonZeroU64::MIN,
                            next_cursor: None,
                            returned_count: 1,
                            target: Some(LifecycleTargetObservation {
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
fn frame(id: [u8; 16], sequence: u64) -> Vec<u8> {
    use orchbee_terminal_protocol::{
        CURRENT_PROTOCOL_VERSION, Envelope, HostEpoch, MessageRole, PayloadEncoding,
        encode_transport_envelope,
    };
    let mut payload = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin"
    ))
    .to_vec();
    payload[6] = 1;
    payload[16..32].fill(4);
    payload[32..48].copy_from_slice(&id);
    payload[48..56].copy_from_slice(&sequence.to_le_bytes());
    payload[56..64].fill(255);
    payload[64..72].copy_from_slice(&sequence.to_le_bytes());
    payload[72..80].copy_from_slice(&1_u64.to_le_bytes());
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
