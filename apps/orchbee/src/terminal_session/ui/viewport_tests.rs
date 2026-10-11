//! UI-level coverage for the retained-history viewport seam: entry
//! availability gates with the honest `HistoryUnavailable` notice and zero
//! wire commands, the per-incarnation read-sequence watermark carried by
//! `KeyedProjection` across episodes, and the recover-before-input and
//! history-key consumption contracts — driven on real `NativeUi` fixtures
//! without a session worker.

use super::move_membership_tests::ready_ui;
use super::publication_hint_tests::ready_split_ui;
use super::*;
#[cfg(windows)]
mod chrome;
mod scrollbar;
use crate::client_view::{ClientKey, ClientView, KeyCode, KeyEvent, Modifiers};
use crate::local_client::test_server::adjustable_operation_clock;
use crate::terminal_input::{InputDispatcher, TerminalInputPort, TerminalInputUnavailable};
use crate::terminal_session::mailbox::EventDeadline;
use orchbee_client_server_protocol::{ControllerId, ViewportRowOrigin};
use orchbee_terminal_protocol::{
    AttachmentId, HistoryAnchorStatus, HistoryChanged, HostEpoch, TerminalFrameRoute,
    TerminalInteraction, TerminalPixelGeometry, TerminalTextCell, TerminalTextCursor,
    TerminalTextFrame, TerminalTextRow, TerminalTextStyle,
};
use std::{io, num::NonZeroU64, sync::atomic::AtomicU64, time::Instant};

fn attachment() -> AttachmentId {
    AttachmentId::parse(&format!("{:032x}", 7_001)).unwrap()
}

fn epoch() -> HostEpoch {
    HostEpoch::parse(&format!("{:032x}", 7_002)).unwrap()
}

/// A minimal live frame whose gate-relevant fields the caller controls,
/// routed to the projection it will be installed on.
pub(super) fn frame(
    projection: &TerminalProjection,
    scrollback_rows: u64,
    alternate_screen: bool,
    rows: u16,
) -> TerminalTextFrame {
    TerminalTextFrame {
        route: TerminalFrameRoute {
            attachment_id: *projection.attachment(),
            host_epoch: *projection.epoch(),
            sequence: NonZeroU64::new(1).unwrap(),
            generation: NonZeroU64::new(1).unwrap(),
            revision: 1,
            base_revision: None,
        },
        geometry: TerminalPixelGeometry {
            cell_pixel_height: 16,
            cell_pixel_width: 8,
            columns: 80,
            rows: 24,
        },
        cursor: TerminalTextCursor {
            column: 0,
            row: 0,
            visible: true,
        },
        alternate_screen,
        interaction: TerminalInteraction::default(),
        foreground: 0,
        background: 0,
        theme_revision: 1,
        scrollback_rows,
        progress: None,
        graphics_present: None,
        command_completion: None,
        title: None,
        current_directory: None,
        rows: (0..rows)
            .map(|index| TerminalTextRow {
                prompt_class: 0,
                index,
                flags: 0,
                cells: vec![TerminalTextCell {
                    text: "x".into(),
                    style: TerminalTextStyle {
                        foreground: 0,
                        background: 0,
                        underline_color: 0,
                        flags: 0,
                        underline: 0,
                    },
                    width: 0,
                }],
            })
            .collect(),
    }
}

/// Installs a `KeyedProjection` for the exact focus incarnation — with the
/// frame spec `(scrollback_rows, alternate_screen, rows)` and read
/// watermark the test needs — bypassing the event path for fixtures that
/// exercise gates rather than the owner seam.
pub(super) fn install_projection(
    ui: &mut NativeUi,
    frame_spec: Option<(u64, bool, u16)>,
    read_watermark: u64,
) -> PaneRef {
    let key = ui
        .state
        .as_ref()
        .unwrap()
        .focus_pane_ref()
        .expect("focus pane");
    let mut value = TerminalProjection::new(attachment(), epoch()).with_host_terminal(
        orchbee_client_server_protocol::HostTerminalId::parse(&format!("{:032x}", 7_003)).unwrap(),
    );
    if let Some((scrollback_rows, alternate_screen, rows)) = frame_spec {
        let frame = frame(&value, scrollback_rows, alternate_screen, rows);
        value.install_frame_for_test(frame);
    }
    ui.projections.push(KeyedProjection {
        effect_stream: super::super::terminal_effects::Stream::new(
            *value.attachment(),
            *value.epoch(),
        ),
        key: key.clone(),
        value,
        status: NativeProjectionStatus::FrameAvailable,
        viewport: None,
        tail: None,
        tail_attempted: None,
        history_sequence: 0,
        read_watermark,
        selection: None,
    });
    key
}

/// Installs the live frame `enter_history`'s gates require on a projection
/// the event path already installed.
fn install_frame(ui: &mut NativeUi, key: &PaneRef) {
    let projection = projection_at(ui, key);
    let frame = frame(&projection.value, 9, false, 24);
    projection.value.install_frame_for_test(frame);
}

/// Publishes one worker event through the real intake the way a session
/// worker would deliver it.
fn publish(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    event: WorkerEvent,
) {
    let wait = mailbox
        .publish_event_before(
            event,
            EventDeadline::Forward(clock.now() + crate::terminal_session::state::OPERATION_TOTAL),
            clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
}

/// The real `Attached` arm installs the incarnation's projection — the
/// owner seam that mints a fresh `read_watermark` domain per attachment.
fn attach_projection(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    key: &PaneRef,
) {
    publish(
        ui,
        mailbox,
        clock,
        WorkerEvent::Attached {
            key: key.clone(),
            projection: TerminalProjection::new(attachment(), epoch()),
        },
    );
}

/// Releases every queued command — fixture bookkeeping work (the post-Ready
/// focus poll, reconcile markers) that is not part of the viewport wire
/// the strict `recv_viewports` drain expects.
fn drain(mailbox: &CommandMailbox) {
    while let Some(lease) = mailbox.try_recv().unwrap() {
        mailbox.finish_lease(lease).unwrap();
    }
}

/// The first read sequence staged for `key`'s core after `facts`, drained
/// off the wire queue.
fn staged_read_sequence(
    ui: &mut NativeUi,
    mailbox: &Arc<CommandMailbox>,
    key: &PaneRef,
    sequence: u64,
    generation: u64,
) -> u64 {
    let projection = projection_at(ui, key);
    let changed = facts(&projection.value, sequence, generation);
    projection.viewport.as_mut().unwrap().facts(changed);
    ui.admit_viewport_steps(mailbox, key);
    recv_viewports(mailbox)
        .iter()
        .find_map(|step| match step {
            ViewportStep::Read {
                request_sequence, ..
            } => Some(request_sequence.get()),
            _ => None,
        })
        .expect("a read staged")
}

fn facts(projection: &TerminalProjection, sequence: u64, generation: u64) -> HistoryChanged {
    HistoryChanged {
        attachment_id: *projection.attachment(),
        sequence: NonZeroU64::new(sequence).unwrap(),
        revision: sequence,
        generation: NonZeroU64::new(generation).unwrap(),
        anchor_status: HistoryAnchorStatus::Valid,
        total_rows: 400,
    }
}

/// A row window answering `sequence` under `generation` at the pane's live
/// 80x24 geometry — identity rows, the shape the host's OBRW payload
/// decodes to before `window()` gates it. The route must carry the exact
/// attachment identity: `window()` treats a foreign attachment as a
/// protocol fault, not a stale read.
fn window(
    projection: &TerminalProjection,
    sequence: u64,
    generation: u64,
    start_row: u64,
) -> crate::terminal_session::terminal_viewport::ViewportWindow {
    crate::terminal_session::terminal_viewport::ViewportWindow {
        graphics_present: false,
        route: orchbee_terminal_protocol::TerminalRowWindowRoute {
            attachment_id: *projection.attachment(),
            host_epoch: *projection.epoch(),
            sequence: NonZeroU64::new(sequence).unwrap(),
            revision: sequence,
            generation: NonZeroU64::new(generation).unwrap(),
            total_rows: 400,
            start_row,
            columns: 80,
            row_count: 24,
            detail: orchbee_terminal_protocol::RowWindowDetail::Render,
        },
        foreground: 0xffffff,
        background: 0,
        rows: (0..24)
            .map(|index| orchbee_terminal_protocol::TerminalTextRow {
                prompt_class: 0,
                index,
                flags: 0,
                cells: vec![orchbee_terminal_protocol::TerminalTextCell {
                    text: "x".into(),
                    width: 0,
                    style: orchbee_terminal_protocol::TerminalTextStyle {
                        foreground: 0xffffff,
                        background: 0,
                        underline_color: 0x1000000,
                        flags: 0,
                        underline: 0,
                    },
                }],
            })
            .collect(),
    }
}

/// Drives `key`'s core through one entry cycle: facts, the entry read,
/// its window, and the accepted acknowledgement — the committed
/// `Viewing` state that navigation then reads from.
pub(super) fn settle_viewing(
    ui: &mut NativeUi,
    mailbox: &Arc<CommandMailbox>,
    key: &PaneRef,
    generation: u64,
) {
    let changed = {
        let projection = projection_at(ui, key);
        facts(&projection.value, 1, generation)
    };
    projection_at(ui, key)
        .viewport
        .as_mut()
        .unwrap()
        .facts(changed);
    ui.admit_viewport_steps(mailbox, key);
    let sequence = recv_viewports(mailbox)
        .iter()
        .find_map(|step| match step {
            ViewportStep::Read {
                request_sequence, ..
            } => Some(*request_sequence),
            _ => None,
        })
        .expect("entry read staged");
    let window = {
        let projection = projection_at(ui, key);
        window(&projection.value, sequence.get(), generation, 100)
    };
    projection_at(ui, key)
        .viewport
        .as_mut()
        .unwrap()
        .window(window, Instant::now());
    ui.admit_viewport_steps(mailbox, key);
    assert!(
        recv_viewports(mailbox)
            .iter()
            .all(|step| matches!(step, ViewportStep::Ack { .. })),
        "the installed window stages exactly its Ack"
    );
    projection_at(ui, key)
        .viewport
        .as_mut()
        .unwrap()
        .ack_outcome(sequence, true);
}

fn projection_at<'a>(ui: &'a mut NativeUi, key: &PaneRef) -> &'a mut KeyedProjection {
    ui.projections
        .iter_mut()
        .find(|projection| projection.key == *key)
        .expect("installed projection")
}

fn recv_viewports(mailbox: &CommandMailbox) -> Vec<ViewportStep> {
    let mut steps = Vec::new();
    while let Some(lease) = mailbox.try_recv().unwrap() {
        match lease.record() {
            QueuedCommand::Viewport(tagged) => steps.push(*tagged.value()),
            other => panic!("only Viewport commands expected, got {other:?}"),
        }
        mailbox.finish_lease(lease).unwrap();
    }
    steps
}

/// A wire-order witness for commands the mailbox actually carried.
#[derive(Debug, Eq, PartialEq)]
enum WireCmd {
    Enter(PaneRef),
    Recover(PaneRef),
    Input(PaneRef, Vec<u8>),
    Read(PaneRef, u64),
}

fn recv_wire(mailbox: &CommandMailbox) -> Vec<WireCmd> {
    let mut commands = Vec::new();
    while let Some(lease) = mailbox.try_recv().unwrap() {
        match lease.record() {
            QueuedCommand::Viewport(tagged) => commands.push(match tagged.value() {
                ViewportStep::Enter => WireCmd::Enter(tagged.key().clone()),
                ViewportStep::Recover => WireCmd::Recover(tagged.key().clone()),
                ViewportStep::Read {
                    request_sequence, ..
                } => WireCmd::Read(tagged.key().clone(), request_sequence.get()),
                ViewportStep::Ack { .. } => {
                    panic!("no Ack expected in this scenario")
                }
            }),
            QueuedCommand::Input(tagged) => commands.push(WireCmd::Input(
                tagged.key().clone(),
                tagged.value().as_bytes().to_vec(),
            )),
            other => panic!("unexpected command: {other:?}"),
        }
        mailbox.finish_lease(lease).unwrap();
    }
    commands
}

// Every entry refusal — no frame, zero scrollback, alternate screen, zero
// readable rows — publishes the honest unavailable notice and enqueues
// exactly nothing on the wire.
#[test]
fn entry_gates_notice_and_zero_wire() {
    for denied in [
        None,
        Some((0, false, 24)),
        Some((9, true, 24)),
        Some((9, false, 0)),
    ] {
        let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
        install_projection(&mut ui, denied, 0);
        assert_eq!(ui.enter_history(), SessionActionDisposition::KeepNavigator);
        assert_eq!(
            ui.view().notice(),
            Some(NativeSessionNotice::HistoryUnavailable),
            "the refusal is an honest localized notice"
        );
        assert!(
            mailbox.try_recv().unwrap().is_none(),
            "a refused entry never reaches the wire"
        );
        assert!(ui.projections.iter().all(|p| p.viewport.is_none()));
    }
}

// An available pane admits exactly one `Enter` for the exact focus
// incarnation and installs the core only then.
#[test]
fn entry_admits_one_enter_then_installs() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let key = install_projection(&mut ui, Some((9, false, 24)), 0);
    ui.enter_history();
    let lease = mailbox.try_recv().unwrap().expect("Enter queued");
    let QueuedCommand::Viewport(tagged) = lease.record() else {
        panic!("one Viewport step");
    };
    assert_eq!(tagged.key(), &key);
    assert_eq!(tagged.value(), &ViewportStep::Enter);
    assert!(
        ui.projections
            .iter()
            .any(|projection| projection.key == key && projection.viewport.is_some())
    );
    assert!(mailbox.try_recv().unwrap().is_none());
}

// The projection's read watermark survives episodes, exercised through the
// real owner seam: the fixture's `Attached` event installed the
// incarnation's projection, `Enter` → facts → read → window → accepted
// `RowsApplied` commits Viewing, `Esc` rides Recover out, and the next frame
// ends the episode. A re-entered history session resumes strictly above the
// sequence the attachment's host watermark remembers — issued, admitted, or
// cancelled alike.
#[test]
fn read_watermark_carries_across_reentry() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_split_ui();
    drain(&mailbox);
    let key = ui
        .state
        .as_ref()
        .unwrap()
        .focus_pane_ref()
        .expect("focus pane");
    assert_eq!(
        projection_at(&mut ui, &key).read_watermark,
        0,
        "the Attached install mints the sequence domain at zero"
    );
    install_frame(&mut ui, &key);

    // Episode one: Enter → facts → read seq 1 → window → accepted Ack.
    ui.enter_history();
    assert_eq!(recv_viewports(&mailbox), [ViewportStep::Enter]);
    settle_viewing(&mut ui, &mailbox, &key, 1);
    assert_eq!(projection_at(&mut ui, &key).read_watermark, 1);

    // `Esc` rides Recover out; recovery applied plus the next frame ends
    // the episode — Live is the absent core again.
    let esc = KeyEvent::press(KeyCode::Esc, Modifiers::NONE);
    assert!(ui.history_key(&esc));
    assert_eq!(recv_viewports(&mailbox), [ViewportStep::Recover]);
    let effect = {
        let viewport = projection_at(&mut ui, &key).viewport.as_mut().unwrap();
        viewport.recovered();
        viewport.frame_applied()
    };
    ui.settle_viewport_effect(&mailbox, &key, effect);
    assert!(ui.projections.iter().all(|p| p.viewport.is_none()));

    // Episode two on the same attachment: the first read is strictly above
    // the watermark, never a reused 1.
    ui.enter_history();
    assert_eq!(recv_viewports(&mailbox), [ViewportStep::Enter]);
    assert_eq!(staged_read_sequence(&mut ui, &mailbox, &key, 2, 2), 2);
    assert_eq!(projection_at(&mut ui, &key).read_watermark, 2);
}

#[test]
fn removed_projection_drops_late_history_facts_and_viewport_outcomes() {
    use orchbee_terminal_protocol::{
        CURRENT_PROTOCOL_VERSION, Envelope, HistoryControl, PayloadEncoding,
        encode_history_control, encode_transport_envelope,
    };

    let (mut ui, mailbox, clock, _elapsed) = ready_split_ui();
    drain(&mailbox);
    let key = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
    install_frame(&mut ui, &key);
    ui.enter_history();
    assert_eq!(recv_viewports(&mailbox), [ViewportStep::Enter]);
    settle_viewing(&mut ui, &mailbox, &key, 1);

    let mut changed = facts(&projection_at(&mut ui, &key).value, 2, 1);
    changed.anchor_status = HistoryAnchorStatus::Evicted;
    let encoded = encode_history_control(&HistoryControl::HistoryChanged(changed)).unwrap();
    let record = encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: encoded.role,
        kind: encoded.kind,
        payload_encoding: PayloadEncoding::ControlJson,
        host_epoch: Some(epoch()),
        correlation_id: None,
        optional_message: false,
        payload: &encoded.payload,
    })
    .unwrap();

    // The poll belongs to the old projection before removal; its reply arrives afterwards.
    mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(key.clone(), ())))
        .unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    let poll_key = lease.poll_key().unwrap();
    ui.clear_pane_projection(&key);
    let cutoff = clock.now() + crate::terminal_session::state::OPERATION_TOTAL;
    assert_eq!(
        mailbox
            .publish_poll_result_before(poll_key.clone(), Some(record), false, cutoff, &clock)
            .unwrap(),
        MutationResult::Applied
    );
    ui.tick();
    assert!(!ui.apply_viewport_outcome(
        &mailbox,
        key.clone(),
        ViewportOutcome::Ack {
            request_sequence: NonZeroU64::new(1).unwrap(),
            accepted: false,
        },
    ));
    ui.admit_viewport_steps(&mailbox, &key);
    assert!(recv_viewports(&mailbox).is_empty());
    assert!(
        ui.projections
            .iter()
            .all(|projection| projection.key != key)
    );
    assert!(ui.retirement_failure.is_none());

    ui.after_draw();
    assert_eq!(
        mailbox
            .take_causal_before(&poll_key, cutoff, &clock)
            .unwrap(),
        Some(CausalSettlement::NoFeedback),
        "a removed incarnation's facts produce no HistoryObserved feedback"
    );
    mailbox.finish_lease(lease).unwrap();
}

// A genuinely new attachment restarts the domain — the host's
// `last_row_request_sequence` is attachment-scoped, so a different
// attachment identity legitimately sequences from 1 even after the
// incumbent pane's watermark advanced. Driven through the real seams: the
// peer leaf joins membership via `observe_panes`, attaches via the same
// `WorkerEvent::Attached` intake, and its freshly installed
// `KeyedProjection` owns `read_watermark == 0`.
#[test]
fn fresh_incarnation_restarts_the_sequence_domain() {
    let (mut ui, mailbox, clock, _elapsed) = ready_split_ui();
    drain(&mailbox);
    let focus_panel = ui.state.as_ref().unwrap().focus_panel_id().unwrap().clone();
    let key = ui
        .state
        .as_ref()
        .unwrap()
        .focus_pane_ref()
        .expect("focus pane");

    // The incumbent attachment already spent sequence 1.
    install_frame(&mut ui, &key);
    ui.enter_history();
    assert_eq!(recv_viewports(&mailbox), [ViewportStep::Enter]);
    assert_eq!(staged_read_sequence(&mut ui, &mailbox, &key, 1, 1), 1);
    assert_eq!(projection_at(&mut ui, &key).read_watermark, 1);

    // The peer leaf joins the observed membership and its viewer attaches
    // through the same event intake — a genuinely new attachment.
    let peer_panel = WorkbenchPanelId::parse(&format!("{:032x}", 3_010)).unwrap();
    let geometry = Geometry::new(80, 24, 8, 16).unwrap();
    ui.observe_panes(&[
        (focus_panel, 0, geometry),
        (peer_panel.clone(), 1, geometry),
    ]);
    drain(&mailbox);
    let peer = ui
        .state
        .as_ref()
        .unwrap()
        .pane_by_panel(&peer_panel)
        .expect("peer pane record")
        .pane_ref()
        .clone();
    attach_projection(&mut ui, &mailbox, &clock, &peer);
    assert_eq!(
        projection_at(&mut ui, &peer).read_watermark,
        0,
        "the new attachment's projection restarts the domain at zero"
    );
    drain(&mailbox);

    // `enter_history` is focus-gated, so the peer's core is minted directly
    // with the same `read_watermark + 1` handoff the entry path performs —
    // its first read is 1 again, not a continuation of the focus pane's.
    projection_at(&mut ui, &peer).viewport = Some(TerminalViewport::entering(
        attachment(),
        epoch(),
        80,
        24,
        u32::from(24u16),
        1,
        clock.now(),
    ));
    assert_eq!(
        staged_read_sequence(&mut ui, &mailbox, &peer, 1, 1),
        1,
        "a fresh attachment legitimately restarts the sequence domain"
    );
    assert_eq!(projection_at(&mut ui, &peer).read_watermark, 1);
}

// A worker-published `ViewportOutcome` notice survives the real
// publish → take → apply seam: `publish_notice_before` deposits it with no
// wait key, `ui.tick`'s `take_event_before` must hand it to
// `apply_viewport_outcome` instead of faulting the mailbox closed, and the
// applied Enter is state-neutral — it leaves the Entering core in place so
// the next facts still stage the entry Read. Regression for the take-side
// whitelist that used to accept only pane detach/failure notices and
// protocol-closed on every legitimate lookback result.
#[test]
fn viewport_outcome_notice_survives_publish_take_apply() {
    let (mut ui, mailbox, clock, _elapsed) = ready_split_ui();
    drain(&mailbox);
    let key = ui
        .state
        .as_ref()
        .unwrap()
        .focus_pane_ref()
        .expect("focus pane");
    install_frame(&mut ui, &key);
    ui.enter_history();
    assert_eq!(recv_viewports(&mailbox), [ViewportStep::Enter]);

    mailbox
        .publish_notice_before(
            WorkerEvent::ViewportOutcome {
                key: key.clone(),
                outcome: ViewportOutcome::Enter { applied: true },
            },
            clock.now() + crate::terminal_session::state::OPERATION_TOTAL,
            &clock,
        )
        .expect("the fire-and-forget outcome publishes");
    ui.tick();
    assert!(
        ui.retirement_failure.is_none() && ui.state.is_some(),
        "a legitimate outcome notice must not close the protocol"
    );
    assert!(
        projection_at(&mut ui, &key).viewport.is_some(),
        "the applied Enter kept the Entering core"
    );
    drain(&mailbox);

    // The applied outcome changed nothing itself; the surviving Entering
    // core's read demand still stages the entry read once facts arrive.
    assert_eq!(staged_read_sequence(&mut ui, &mailbox, &key, 1, 1), 1);
}

/// A ready fixture under a `Controlling` mode — `forward` requires it.
pub(super) fn controlling_ui() -> (
    NativeUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Arc<AtomicU64>,
) {
    let elapsed = Arc::new(AtomicU64::new(0));
    let clock = adjustable_operation_clock(Arc::clone(&elapsed));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = NativeUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache:
                    crate::terminal_session::workbench_projection::project_native_standalone_tabs(
                        &super::membership_tests::snapshot(2, 1),
                    )
                    .unwrap(),
            },
            EventDeadline::Startup(clock.now() + crate::terminal_session::state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    let state = ui.state.as_mut().unwrap();
    let key = state.desired().unwrap().clone();
    let geometry = Geometry::new(80, 24, 8, 16).unwrap();
    let mode = ControllerMode::Controlling(ControllerId::parse(&format!("{:032x}", 8)).unwrap());
    assert!(state.install_projection_t(&key));
    assert!(state.record_setup_configuration_t(&key, mode.clone(), Some(geometry)));
    assert!(state.stage_drawn_frame_t(&key, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted_t(&key, FrameIdentity::new(1)));
    assert!(state.publish_ready_t(&key, FrameIdentity::new(1), mode, Some(geometry)));
    (ui, mailbox, clock, elapsed)
}

// A typed byte on a non-Live pane lands on the wire strictly after the
// Recover that must precede it — and a second byte while Recovering
// mints no second Recover.
#[test]
fn typed_byte_lands_once_behind_one_recover() {
    let (mut ui, mailbox, _clock, _elapsed) = controlling_ui();
    let key = install_projection(&mut ui, Some((9, false, 24)), 0);
    ui.enter_history();
    assert_eq!(recv_viewports(&mailbox), [ViewportStep::Enter]);

    assert!(ui.forward(b"x"));
    assert_eq!(
        recv_wire(&mailbox),
        [
            WireCmd::Recover(key.clone()),
            WireCmd::Input(key.clone(), b"x".to_vec()),
        ],
        "Recover first, the byte exactly once"
    );
    // Recovering already holds its admission: another byte forwards alone.
    assert!(ui.forward(b"y"));
    assert_eq!(
        recv_wire(&mailbox),
        [WireCmd::Input(key.clone(), b"y".to_vec()),],
    );
}

/// Delegates `TerminalInputPort` onto `NativeUi` the way the production
/// `TerminalSession` impl does, including the history hooks.
struct NativePort<'a>(&'a mut NativeUi);

impl TerminalInputPort for NativePort<'_> {
    fn pointer_probe(&mut self) -> crate::terminal_input::PointerProbe {
        self.0.pointer_probe()
    }

    fn scrollbar_scroll(
        &mut self,
        key: &PaneRef,
        source: crate::terminal_scrollbar::ScrollbarSnapshot,
        offset: u32,
    ) -> bool {
        self.0.scrollbar_scroll(key, source, offset)
    }
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.0
            .forward(input)
            .then_some(())
            .ok_or(TerminalInputUnavailable)
    }

    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        Some(self.0.apply_client_action(action))
    }

    fn history_key(&mut self, event: &crate::client_view::KeyEvent) -> bool {
        self.0.history_key(event)
    }

    fn history_wheel(&mut self, key: &crate::terminal_session::state::PaneRef, up: bool) -> bool {
        self.0.history_wheel(key, up)
    }

    fn capture_confirmation_key(&mut self, key: ClientKey) -> bool {
        self.0.capture_confirmation_key(key)
    }

    fn rename_input_context(&mut self) -> Option<NativeRenameInputContext> {
        self.0.rename_input_context()
    }

    fn edit_rename_draft(
        &mut self,
        token: NativeRenameDraftToken,
        edit: NativeRenameEdit<'_>,
    ) -> bool {
        self.0.edit_rename_draft(token, edit)
    }

    fn cancel_rename_draft(&mut self, token: NativeRenameDraftToken) {
        self.0.cancel_rename_draft(token);
    }

    fn submit_rename_draft(
        &mut self,
        token: NativeRenameDraftToken,
    ) -> Option<SessionActionDisposition> {
        Some(self.0.submit_rename_draft(token))
    }

    fn setup_input_context(&mut self) -> Option<NativeSetupInputContext> {
        self.0.setup_input_context()
    }

    fn edit_setup_draft(&mut self, token: NativeSetupToken, edit: NativeSetupEdit<'_>) -> bool {
        self.0.edit_setup_draft(token, edit)
    }

    fn cancel_setup_draft(&mut self, token: NativeSetupToken) {
        self.0.cancel_setup_draft(token);
    }

    fn submit_setup_draft(&mut self, token: NativeSetupToken) -> Option<SessionActionDisposition> {
        Some(self.0.submit_setup_draft(token))
    }
}

fn dispatch(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    ui: &mut NativeUi,
    bytes: &[u8],
) {
    let mut port = NativePort(ui);
    dispatcher
        .dispatch(
            view,
            bytes,
            &mut port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
}

// The dispatcher's history guard: a navigation key on a non-Live pane is
// consumed locally — never an Input — while an ordinary byte rides the
// recover-first contract. `Ctrl+B PageUp` keeps its prefix meaning and is
// not a history key.
#[test]
fn dispatcher_consumes_history_keys_and_orders_recovery() {
    let (mut ui, mailbox, _clock, _elapsed) = controlling_ui();
    let key = install_projection(&mut ui, Some((9, false, 24)), 0);
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();

    // Entering before any facts: PageUp coalesces locally, nothing wires.
    ui.enter_history();
    assert_eq!(recv_viewports(&mailbox), [ViewportStep::Enter]);
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\x1b[5~");
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "PageUp on a non-Live pane is consumed, never an Input"
    );

    // The entry cycle settles into Viewing; PageUp now stages a
    // navigation read — a Viewport step, still never an Input.
    settle_viewing(&mut ui, &mailbox, &key, 1);
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\x1b[5~");
    let steps = recv_viewports(&mailbox);
    assert!(
        !steps.is_empty()
            && steps
                .iter()
                .all(|step| matches!(step, ViewportStep::Read { .. })),
        "history keys mint Viewport reads only: {steps:?}"
    );

    // An ordinary byte rides recover-before-input.
    dispatch(&mut dispatcher, &mut view, &mut ui, b"a");
    let commands = recv_wire(&mailbox);
    assert!(
        matches!(
            commands.as_slice(),
            [WireCmd::Recover(_), WireCmd::Input(_, _)]
        ),
        "Recover before the forwarded byte: {commands:?}"
    );
}

#[test]
fn wheel_entry_starts_three_rows_back_and_coalesces_before_facts() {
    for (next, rows_before) in [(None, 3), (Some(true), 6), (Some(false), 0)] {
        let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
        let key = install_projection(&mut ui, Some((9, false, 24)), 0);
        assert!(ui.history_wheel(&key, true));
        assert_eq!(recv_viewports(&mailbox), [ViewportStep::Enter]);
        if let Some(up) = next {
            assert!(ui.history_wheel(&key, up));
        }
        assert!(
            recv_viewports(&mailbox).is_empty(),
            "wait for real history facts"
        );
        let projection = projection_at(&mut ui, &key);
        let changed = facts(&projection.value, 1, 1);
        projection.viewport.as_mut().unwrap().facts(changed);
        ui.admit_viewport_steps(&mailbox, &key);
        let steps = recv_viewports(&mailbox);
        if rows_before == 0 {
            assert_eq!(steps, [ViewportStep::Recover]);
        } else {
            assert!(
                matches!(&steps[..], [ViewportStep::Read {
                origin: ViewportRowOrigin::Tail { rows_before: actual }, ..
            }] if *actual == rows_before),
                "{steps:?}"
            );
        }
    }
}

#[cfg(windows)]
#[test]
#[cfg(windows)]
fn client_viewport_reads_installed_live_history_and_pending_without_commands() {
    use orchbee_workbench_protocol::*;
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let snapshot = super::membership_tests::snapshot(2, 1);
    let key = install_projection(&mut ui, Some((9, false, 24)), 0);
    let panel = ui
        .control_observation(false)
        .unwrap()
        .1
        .selection
        .unwrap()
        .panel_id
        .unwrap();
    let mut command = ClientViewControl {
        operation_id: WorkbenchRequestId::parse(&format!("{:032x}", 901)).unwrap(),
        client_id: WorkbenchClientId::parse(&format!("{:032x}", 902)).unwrap(),
        expected_view_revision: ClientViewSequence::new(1).unwrap(),
        minimum_workbench_revision: snapshot.revision,
        intent: ClientViewIntent::Viewports {
            panel_ids: vec![panel.clone()],
        },
    };
    let live = ui.read_client_viewports(&command, &snapshot).unwrap();
    assert!(
        matches!(&live.panels[0].observation, ClientViewportObservation::Observed { mode: ClientViewportMode::Live, scroll, .. } if scroll.offset_from_bottom == 0 && scroll.max_offset_from_bottom == 9 && scroll.viewport_rows == 24)
    );
    assert!(mailbox.try_recv().unwrap().is_none());
    assert!(ui.history_wheel(&key, true));
    assert_eq!(recv_viewports(&mailbox), [ViewportStep::Enter]);
    let pending = ui.read_client_viewports(&command, &snapshot).unwrap();
    assert!(matches!(
        pending.panels[0].observation,
        ClientViewportObservation::Pending {
            phase: ClientViewportPending::Entering
        }
    ));
    assert!(mailbox.try_recv().unwrap().is_none());
    settle_viewing(&mut ui, &mailbox, &key, 1);
    let before = ui.control_observation(false).unwrap();
    assert_eq!(before.1.phase, ClientViewPhase::Busy);
    let history = ui.read_client_viewports(&command, &snapshot).unwrap();
    assert!(
        matches!(&history.panels[0].observation, ClientViewportObservation::Observed { mode: ClientViewportMode::History, scroll, .. } if scroll.offset_from_bottom == 276 && scroll.max_offset_from_bottom == 376 && scroll.viewport_rows == 24)
    );
    assert_eq!(
        ui.read_client_viewports(&command, &snapshot).unwrap(),
        history
    );
    assert_eq!(ui.control_observation(false).unwrap(), before);
    assert!(mailbox.try_recv().unwrap().is_none());
    // A new primary binding may not inherit this incarnation's old viewport.
    let mut rebound = snapshot.clone();
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut rebound.terminal_contexts[0].tabs[0].root
    else {
        panic!()
    };
    primary_presentation.binding.as_mut().unwrap().terminal_id =
        orchbee_workbench_protocol::TerminalId::parse(&format!("{:032x}", 999)).unwrap();
    assert!(matches!(
        ui.read_client_viewports(&command, &rebound).unwrap().panels[0].observation,
        ClientViewportObservation::Unavailable {
            reason: ClientViewportUnavailable::NotAttached
        }
    ));
    projection_at(&mut ui, &key)
        .viewport
        .as_mut()
        .unwrap()
        .typed();
    ui.admit_viewport_steps(&mailbox, &key);
    assert_eq!(recv_viewports(&mailbox), [ViewportStep::Recover]);
    assert!(matches!(
        ui.read_client_viewports(&command, &snapshot)
            .unwrap()
            .panels[0]
            .observation,
        ClientViewportObservation::Pending {
            phase: ClientViewportPending::Recovering
        }
    ));
    command.intent = ClientViewIntent::Viewports {
        panel_ids: vec![WorkbenchPanelId::parse(&format!("{:032x}", 999)).unwrap()],
    };
    assert_eq!(
        ui.read_client_viewports(&command, &snapshot),
        Err(ClientViewControlReason::TargetUnavailable)
    );
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn wheel_entry_empty_down_and_stale_focus_have_no_effect_or_notice() {
    for (frame, up) in [
        (None, true),
        (Some((0, false, 24)), true),
        (Some((9, true, 24)), true),
        (Some((9, false, 0)), true),
        (Some((9, false, 24)), false),
    ] {
        let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
        let key = install_projection(&mut ui, frame, 0);
        assert!(!ui.history_wheel(&key, up));
        assert!(recv_viewports(&mailbox).is_empty());
        assert!(ui.projections.iter().all(|p| p.viewport.is_none()));
        assert_ne!(
            ui.view().notice(),
            Some(NativeSessionNotice::HistoryUnavailable)
        );
    }
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let key = install_projection(&mut ui, Some((9, false, 24)), 0);
    let stale = PaneRef::new(key.pane().clone(), 42);
    assert!(!ui.history_wheel(&stale, true));
    assert!(recv_viewports(&mailbox).is_empty());
}
