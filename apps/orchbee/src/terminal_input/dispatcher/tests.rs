use super::*;
use crate::terminal_input::TerminalInputUnavailable;
use std::time::Duration;
use std::{cell::RefCell, io, rc::Rc};

#[test]
fn appearance_query_is_issued_once_without_waiting_for_a_reply() {
    let now = Instant::now();
    let mut dispatcher = InputDispatcher::default();
    let mut requests = Vec::new();
    dispatcher
        .begin_host_appearance(now, &mut |request| {
            requests.push(request);
            Ok::<(), ()>(())
        })
        .unwrap();
    dispatcher
        .begin_host_appearance(now + Duration::from_secs(1), &mut |request| {
            requests.push(request);
            Ok::<(), ()>(())
        })
        .unwrap();

    assert_eq!(
        requests,
        vec![crate::terminal_input::ModeRequest::AppearanceQuery(
            crate::terminal_host_appearance::Query
        )]
    );
}

#[cfg(windows)]
mod action_menu;
#[cfg(windows)]
mod configured;
#[cfg(windows)]
mod copy;
#[cfg(windows)]
mod divider;
#[cfg(windows)]
mod navigator;
#[cfg(windows)]
mod settings;
#[cfg(windows)]
mod sidebar_width;
mod workspace_close;
mod workspace_setup;
#[cfg(windows)]
mod worktree_launch;

#[test]
fn confirmation_enter_accepts_legacy_cr_alias_and_named_unmodified_key_only() {
    for (code, mods, kind, expected) in [
        (
            KeyCode::Enter,
            Modifiers::NONE,
            KeyKind::Press,
            ClientKey::Enter,
        ),
        (
            KeyCode::Char('m'),
            Modifiers::CTRL,
            KeyKind::Press,
            ClientKey::Enter,
        ),
        (
            KeyCode::Enter,
            Modifiers::NONE,
            KeyKind::Repeat,
            ClientKey::Enter,
        ),
        (
            KeyCode::Enter,
            Modifiers::NONE,
            KeyKind::Release,
            ClientKey::Other,
        ),
        (
            KeyCode::Enter,
            Modifiers::CTRL,
            KeyKind::Press,
            ClientKey::Other,
        ),
        (
            KeyCode::Enter,
            Modifiers::NONE.with_shift(),
            KeyKind::Press,
            ClientKey::Other,
        ),
        (
            KeyCode::Enter,
            Modifiers::NONE.with_alt(),
            KeyKind::Press,
            ClientKey::Other,
        ),
        (
            KeyCode::Enter,
            Modifiers::NONE.with_super_(),
            KeyKind::Press,
            ClientKey::Other,
        ),
        (
            KeyCode::Char('m'),
            Modifiers::CTRL.with_shift(),
            KeyKind::Press,
            ClientKey::Other,
        ),
        (
            KeyCode::Char('y'),
            Modifiers::NONE,
            KeyKind::Press,
            ClientKey::Plain('y'),
        ),
    ] {
        let item = DecodedItem {
            kind: DecodedKind::Key(crate::client_view::KeyEvent { code, mods, kind }),
            raw: Box::default(),
            owned: false,
        };
        assert_eq!(client_key(&item), expected, "{code:?} {mods:?} {kind:?}");
    }
    let paste = DecodedItem {
        kind: DecodedKind::Paste { text: "\r".into() },
        raw: Box::default(),
        owned: false,
    };
    assert_eq!(client_key(&paste), ClientKey::Other);
}

#[cfg(windows)]
#[test]
fn physical_reply_queue_is_bounded_preserves_prefix_and_excludes_literal_paste() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(80);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02");
    assert!(view.prefix_active());
    let reply = b"\x1b_Gi=1,p=1;OK\x1b\\";
    dispatch_all(&mut dispatcher, &mut view, &mut port, &reply.repeat(129));
    assert!(view.prefix_active());
    assert!(forwarded_bytes(&calls).is_empty());
    let (replies, overflow) = dispatcher.take_graphics_replies();
    assert!(overflow);
    assert_eq!(replies.len(), 128);
    assert!(replies.iter().all(|reply| matches!(
        reply,
        crate::terminal_graphics::Reply::Kitty {
            image_id: 1,
            placement_id: Some(1),
            success: true
        }
    )));
    assert_eq!(dispatcher.take_graphics_replies(), (Vec::new(), false));
    let mut literal = b"\x1b[200~".to_vec();
    literal.extend_from_slice(reply);
    literal.extend_from_slice(b"\x1b[201~");
    let mut view = ClientView::new(80);
    dispatch_all(&mut dispatcher, &mut view, &mut port, &literal);
    assert_eq!(dispatcher.take_graphics_replies(), (Vec::new(), false));
    assert_eq!(forwarded_bytes(&calls), literal);
}

#[derive(Debug, Eq, PartialEq)]
enum Call {
    Forward(Vec<u8>),
    Focus(bool),
    Mode(ModeRequest),
    Action(ClientAction),
    Confirmation(crate::client_view::ClientKey),
    BoardSelect(orchbee_agent_observation_protocol::AgentSessionId),
}

struct UnavailablePort(Rc<RefCell<Vec<Call>>>);

#[test]
fn context_prefix_across_reads_consumes_only_the_navigation_keys() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let packets: [&[u8]; 3] = [b"[]\x02", b"]\x02", b"[literal"];
    for packet in packets {
        assert_eq!(
            dispatcher
                .dispatch(
                    &mut view,
                    packet,
                    &mut port,
                    Instant::now(),
                    |_, _| Ok::<bool, io::Error>(false),
                    &mut |_| Ok(()),
                )
                .unwrap(),
            DispatchOutcome::Continue,
        );
    }
    assert_eq!(
        *calls.borrow(),
        [
            Call::Forward(b"[]".to_vec()),
            Call::Action(ClientAction::SelectNextContext),
            Call::Action(ClientAction::SelectPreviousContext),
            Call::Forward(b"literal".to_vec()),
        ],
    );
    assert!(!view.prefix_active());
}

impl TerminalInputPort for UnavailablePort {
    fn observe_focus(&mut self, focused: bool) {
        self.0.borrow_mut().push(Call::Focus(focused));
    }

    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.0.borrow_mut().push(Call::Forward(bytes.to_vec()));
        Ok(())
    }

    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        self.0.borrow_mut().push(Call::Action(action));
        None
    }
}

struct BoardOwnershipPort {
    active: bool,
}

impl TerminalInputPort for BoardOwnershipPort {
    fn native_capture_claims(&self) -> crate::tui_composition::NativeCaptureClaims {
        crate::tui_composition::NativeCaptureClaims::default().with_agent_board(self.active)
    }

    fn native_capture_context(
        &self,
        owner: crate::tui_composition::NativeCaptureOwner,
    ) -> Option<crate::terminal_session::NativeCaptureContext> {
        (self.active && owner == crate::tui_composition::NativeCaptureOwner::AgentBoard).then(
            || {
                crate::terminal_session::NativeCaptureContext::AgentBoard(
                    crate::terminal_session::AgentBoardInputContext::for_test(
                        crate::terminal_session::AgentBoardEpisodeToken::for_test(1),
                        crate::tui_presentation::PresentationPhase::Admitted,
                        true,
                    ),
                )
            },
        )
    }

    fn forward(&mut self, _bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        Ok(())
    }
}

#[test]
fn board_ownership_edges_remain_observable_to_the_redraw_decision() {
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(80);
    let mut port = BoardOwnershipPort { active: false };
    let now = Instant::now();

    assert!(!dispatcher.reconcile(&mut view, &mut port, now));
    port.active = true;
    assert!(dispatcher.reconcile(&mut view, &mut port, now));
    assert!(!dispatcher.reconcile(&mut view, &mut port, now));
}

#[test]
fn button_free_motion_redraws_only_for_changed_projection_and_never_starves_escape() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(80);
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher.set_pointer_motion_observability(crate::tui_pointer::MotionObservability::AnyMotion);
    let target =
        crate::tui_pointer::InteractionTarget::Fixture(crate::tui_pointer::HitTarget::Help {
            page: 1,
            action: crate::tui_pointer::HelpControl::Close,
        });
    let area = ratatui::layout::Rect::new(10, 5, 8, 2);
    let spec = crate::tui_pointer::InteractionSpec::enabled(
        crate::tui_pointer::PrimaryContract::ActivateOnRelease,
    );
    let mut hits = crate::tui_pointer::HitMapBuilder::with_pointer(dispatcher.pointer_projection());
    hits.record_interaction(
        crate::tui_pointer::HitLayer::Foreground,
        area,
        area,
        target,
        spec,
    );
    dispatcher.publish_hit_map(hits.finish());

    let start = Instant::now();
    dispatcher
        .dispatch(
            &mut view,
            b"\x1b",
            &mut port,
            start,
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(calls.borrow().is_empty());

    let hover = b"\x1b[<35;13;6M";
    dispatcher
        .dispatch(
            &mut view,
            hover,
            &mut port,
            start + std::time::Duration::from_millis(60),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(*calls.borrow(), [Call::Forward(b"\x1b".to_vec())]);
    assert!(
        dispatcher.dispatch_requested_redraw(),
        "the expired key and first hover are both observable"
    );

    dispatcher
        .dispatch(
            &mut view,
            hover,
            &mut port,
            start + std::time::Duration::from_millis(61),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(!dispatcher.dispatch_requested_redraw());

    let outside = b"\x1b[<35;19;7M";
    dispatcher
        .dispatch(
            &mut view,
            outside,
            &mut port,
            start + std::time::Duration::from_millis(62),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(dispatcher.dispatch_requested_redraw());
    dispatcher
        .dispatch(
            &mut view,
            outside,
            &mut port,
            start + std::time::Duration::from_millis(63),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(!dispatcher.dispatch_requested_redraw());

    let mut mixed = outside.to_vec();
    mixed.push(b'x');
    dispatcher
        .dispatch(
            &mut view,
            &mixed,
            &mut port,
            start + std::time::Duration::from_millis(64),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(dispatcher.dispatch_requested_redraw());
    assert_eq!(
        *calls.borrow(),
        [
            Call::Forward(b"\x1b".to_vec()),
            Call::Forward(b"x".to_vec())
        ]
    );
}

#[test]
fn cancelled_generic_press_fences_only_its_exact_late_release_packet() {
    use crate::tui_pointer::{
        HelpControl, HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget,
        PrimaryContract,
    };
    use ratatui::layout::Rect;

    let publish = |dispatcher: &mut InputDispatcher| {
        let area = Rect::new(10, 5, 8, 2);
        let mut hits = HitMapBuilder::with_pointer(dispatcher.pointer_projection());
        hits.record_interaction(
            HitLayer::Foreground,
            area,
            area,
            InteractionTarget::Fixture(HitTarget::Help {
                page: 1,
                action: HelpControl::Close,
            }),
            InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
        );
        dispatcher.publish_hit_map(hits.finish());
    };

    for cancellation in 0..4 {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut port = UnavailablePort(Rc::clone(&calls));
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(80);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        publish(&mut dispatcher);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;13;6M");
        match cancellation {
            0 => dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[O"),
            1 => dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[O\x1b[I"),
            2 => dispatcher.set_pointer_capture(&mut view, &mut port, false),
            _ => dispatcher.invalidate_hit_map(&mut port),
        }
        calls.borrow_mut().clear();
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            b"\x1b[<0;13;6mtail\x1b[200~paste\x1b[201~\x1b[",
        );
        assert!(
            forwarded_bytes(&calls).is_empty(),
            "cancellation {cancellation} leaked the release packet"
        );
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"A");
        assert!(
            forwarded_bytes(&calls).is_empty(),
            "cancellation {cancellation} leaked an owned fragment"
        );
        if cancellation == 0 {
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[I");
        }
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"fresh");
        assert_eq!(forwarded_bytes(&calls), b"fresh");
    }

    // A new physical press retires the cleanup debt. The later orphaned
    // release does not manufacture ownership for its packet tail.
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(80);
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    publish(&mut dispatcher);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;13;6M\x1b[O\x1b[I\x1b[<0;1;1M",
    );
    calls.borrow_mut().clear();
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;1;1morphan");
    assert_eq!(forwarded_bytes(&calls), b"orphan");
}

#[test]
fn failed_mode_write_precedes_and_prevents_native_draft_request_and_packet_tail() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let result = dispatcher.dispatch(
        &mut view,
        b"before\x02,after",
        &mut port,
        Instant::now(),
        |_, _| Ok::<bool, io::Error>(false),
        &mut |request| {
            calls.borrow_mut().push(Call::Mode(request.clone()));
            if matches!(request, ModeRequest::BracketedPaste(true)) {
                Err(io::Error::other("partial enable write"))
            } else {
                Ok(())
            }
        },
    );
    assert!(result.is_err());
    assert_eq!(
        *calls.borrow(),
        [
            Call::Mode(ModeRequest::BracketedPaste(false)),
            Call::Forward(b"before".to_vec()),
            Call::Mode(ModeRequest::BracketedPaste(true))
        ]
    );
}

#[test]
fn prefix_then_arrow_sequence_is_one_unbound_event_without_tail_leak() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    assert_eq!(
        dispatcher
            .dispatch(
                &mut view,
                b"\x02\x1b[D",
                &mut port,
                Instant::now(),
                |_, _| Ok::<bool, io::Error>(false),
                &mut |_| Ok(()),
            )
            .unwrap(),
        DispatchOutcome::Continue,
    );
    assert_eq!(*calls.borrow(), [Call::Action(ClientAction::UnknownPrefix)]);
    assert_eq!(
        view.notice(),
        Some(crate::client_view::Notice::UnknownPrefix)
    );
    assert!(!view.prefix_active());
}

#[test]
fn prefix_then_csi_split_across_reads_is_still_one_event() {
    let patterns: [&[&[u8]]; 3] = [
        &[b"\x02\x1b", b"[D"],
        &[b"\x02", b"\x1b", b"[", b"D"],
        &[b"\x02\x1b[", b"D"],
    ];
    for packets in patterns {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut port = UnavailablePort(Rc::clone(&calls));
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        for &packet in packets {
            dispatcher
                .dispatch(
                    &mut view,
                    packet,
                    &mut port,
                    Instant::now(),
                    |_, _| Ok::<bool, io::Error>(false),
                    &mut |_| Ok(()),
                )
                .unwrap();
        }
        assert_eq!(
            *calls.borrow(),
            [Call::Action(ClientAction::UnknownPrefix)],
            "split pattern {packets:?}",
        );
        assert!(!view.prefix_active());
    }
}

#[test]
fn prefix_then_chinese_scalar_is_one_unbound_event_without_continuation_leak() {
    let patterns: [&[&[u8]]; 4] = [
        &[b"\x02\xe4\xb8\xad"],
        &[b"\x02\xe4", b"\xb8\xad"],
        &[b"\x02", b"\xe4", b"\xb8", b"\xad"],
        &[b"\x02\xe4\xb8", b"\xad"],
    ];
    for packets in patterns {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut port = UnavailablePort(Rc::clone(&calls));
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        for &packet in packets {
            dispatcher
                .dispatch(
                    &mut view,
                    packet,
                    &mut port,
                    Instant::now(),
                    |_, _| Ok::<bool, io::Error>(false),
                    &mut |_| Ok(()),
                )
                .unwrap();
        }
        assert_eq!(
            *calls.borrow(),
            [Call::Action(ClientAction::UnknownPrefix)],
            "split pattern {packets:?}",
        );
        assert!(!view.prefix_active());
    }
}

#[test]
fn prefix_then_emoji_split_across_reads_is_one_unbound_event() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let packets: [&[u8]; 4] = [b"\x02\xf0", b"\x9f", b"\x98", b"\x80"];
    for packet in packets {
        dispatcher
            .dispatch(
                &mut view,
                packet,
                &mut port,
                Instant::now(),
                |_, _| Ok::<bool, io::Error>(false),
                &mut |_| Ok(()),
            )
            .unwrap();
    }
    assert_eq!(*calls.borrow(), [Call::Action(ClientAction::UnknownPrefix)]);
    assert!(!view.prefix_active());
}

#[test]
fn lone_escape_is_forwarded_only_after_the_bounded_wait() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let start = Instant::now();
    dispatcher
        .dispatch(
            &mut view,
            b"\x1b",
            &mut port,
            start,
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(calls.borrow().is_empty());
    dispatcher
        .synchronize(
            &mut view,
            &mut port,
            start + std::time::Duration::from_millis(60),
            &mut |_| Ok::<(), io::Error>(()),
        )
        .unwrap();
    assert_eq!(*calls.borrow(), [Call::Forward(b"\x1b".to_vec())]);
}

#[test]
fn alt_modified_key_does_not_match_a_plain_prefix_binding() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    dispatcher
        .dispatch(
            &mut view,
            b"\x02\x1ba",
            &mut port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(*calls.borrow(), [Call::Action(ClientAction::UnknownPrefix)]);
    assert!(!view.prefix_active());
}

#[test]
fn paste_payload_never_enters_prefix_matching_and_forwards_whole() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    dispatcher
        .dispatch(
            &mut view,
            b"\x02\x1b[200~x\x1b[201~",
            &mut port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(
        *calls.borrow(),
        [Call::Forward(b"\x1b[200~x\x1b[201~".to_vec())]
    );
    assert!(
        !view.prefix_active(),
        "a terminal paste retires the interrupted prefix"
    );
}

#[test]
fn paste_between_ordinary_bytes_forwards_as_one_atomic_run() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    dispatcher
        .dispatch(
            &mut view,
            b"ab\x1b[200~xy\x1b[201~cd",
            &mut port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(
        *calls.borrow(),
        [Call::Forward(b"ab\x1b[200~xy\x1b[201~cd".to_vec())]
    );
}

fn forwarded(calls: &Rc<RefCell<Vec<Call>>>) -> Vec<u8> {
    calls
        .borrow()
        .iter()
        .flat_map(|call| match call {
            Call::Forward(bytes) => bytes.clone(),
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn a_utf8_continuation_9b_is_not_a_c1_paste_terminator() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let packets: [&[u8]; 3] = [b"\x1b[200~ab\xc2\x9b201~", b"\x02q", b"\x1b[201~"];
    for packet in packets {
        assert_eq!(
            dispatcher
                .dispatch(
                    &mut view,
                    packet,
                    &mut port,
                    Instant::now(),
                    |_, _| Ok::<bool, io::Error>(false),
                    &mut |_| Ok(()),
                )
                .unwrap(),
            DispatchOutcome::Continue,
        );
    }
    // The `\x9b` inside a UTF-8 scalar is a continuation byte, not a C1
    // terminator: the run stays paste until `ESC[201~` in its own notation,
    // so the `\x02q` payload never reaches the prefix layer.
    assert_eq!(
        forwarded(&calls),
        b"\x1b[200~ab\xc2\x9b201~\x02q\x1b[201~".to_vec()
    );
}

#[test]
fn a_csi_tail_after_the_byte_cap_stays_owned_until_the_final_byte() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let mut first = b"\x1b[".to_vec();
    first.extend(std::iter::repeat_n(b'0', 62));
    let packets: [&[u8]; 3] = [&first, b"\x02q", b"A"];
    for packet in packets {
        assert_eq!(
            dispatcher
                .dispatch(
                    &mut view,
                    packet,
                    &mut port,
                    Instant::now(),
                    |_, _| Ok::<bool, io::Error>(false),
                    &mut |_| Ok(()),
                )
                .unwrap(),
            DispatchOutcome::Continue,
        );
    }
    let mut expected = first;
    expected.extend_from_slice(b"\x02qA");
    assert_eq!(forwarded(&calls), expected);
}

#[test]
fn an_ss3_tail_after_the_byte_cap_stays_owned_until_the_final_byte() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let packets: [&[u8]; 3] = [b"\x1bO1234567", b"\x02q", b"A"];
    for packet in packets {
        assert_eq!(
            dispatcher
                .dispatch(
                    &mut view,
                    packet,
                    &mut port,
                    Instant::now(),
                    |_, _| Ok::<bool, io::Error>(false),
                    &mut |_| Ok(()),
                )
                .unwrap(),
            DispatchOutcome::Continue,
        );
    }
    assert_eq!(forwarded(&calls), b"\x1bO1234567\x02qA".to_vec());
}

#[test]
fn an_esc_inside_a_csi_aborts_the_sequence_instead_of_leaving_residue() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    for packet in [&b"\x02"[..], b"\x1b[3\x1b[31m"] {
        dispatcher
            .dispatch(
                &mut view,
                packet,
                &mut port,
                Instant::now(),
                |_, _| Ok::<bool, io::Error>(false),
                &mut |_| Ok(()),
            )
            .unwrap();
    }
    // The aborted first sequence consumes the armed prefix; the complete
    // second sequence is one non-decision item whose raw forwards whole —
    // never `3`,`1`,`m` as residue key events.
    assert_eq!(
        *calls.borrow(),
        [
            Call::Action(ClientAction::UnknownPrefix),
            Call::Forward(b"\x1b[31m".to_vec()),
        ]
    );
    assert!(!view.prefix_active());
}

#[test]
fn a_truncated_csi_releases_its_bytes_after_the_bounded_wait_then_drains() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let start = Instant::now();
    dispatcher
        .dispatch(
            &mut view,
            b"\x1b[12",
            &mut port,
            start,
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(calls.borrow().is_empty());
    dispatcher
        .synchronize(
            &mut view,
            &mut port,
            start + std::time::Duration::from_millis(60),
            &mut |_| Ok::<(), io::Error>(()),
        )
        .unwrap();
    // The truncated run is emitted once the bounded wait expires.
    assert_eq!(*calls.borrow(), [Call::Forward(b"\x1b[12".to_vec())]);
    // A late tail of the same frame drains without reaching bindings.
    calls.borrow_mut().clear();
    dispatcher
        .dispatch(
            &mut view,
            b"\x02qA",
            &mut port,
            start + std::time::Duration::from_millis(70),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(*calls.borrow(), [Call::Forward(b"\x02qA".to_vec())]);
    assert!(!view.prefix_active());
}

#[test]
fn paste_rejection_at_the_raw_cap_drains_without_forwarding() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let mut first = b"\x1b[200~".to_vec();
    first.extend(std::iter::repeat_n(b'a', 4 * 1024 * 1024));
    dispatcher
        .dispatch(
            &mut view,
            &first,
            &mut port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    dispatcher
        .dispatch(
            &mut view,
            b"\x02q\x1b[201~z",
            &mut port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    // Rejection owns the entire run; only post-terminator input reaches the pane.
    let delivered = forwarded(&calls);
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered, b"z");
    assert!(!view.prefix_active());
}

#[test]
fn paste_rejection_preserves_the_armed_prefix_and_never_forwards_invalid_text() {
    for payload in [vec![0xff, 0x02, b'q'], vec![b'x'; 256 * 1024 + 1]] {
        for prefix in [false, true] {
            let calls = Rc::new(RefCell::new(Vec::new()));
            let mut port = UnavailablePort(Rc::clone(&calls));
            let mut dispatcher = InputDispatcher::default();
            let mut view = ClientView::new(100);
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"before");
            if prefix {
                dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02");
            }
            let mut paste = b"\x1b[200~".to_vec();
            paste.extend_from_slice(&payload);
            paste.extend_from_slice(b"\x1b[201~");
            dispatch_all(&mut dispatcher, &mut view, &mut port, &paste);
            assert_eq!(forwarded(&calls), b"before");
            assert!(
                calls
                    .borrow()
                    .iter()
                    .all(|call| !matches!(call, Call::Action(_)))
            );
            assert_eq!(view.prefix_active(), prefix);
            dispatch_all(
                &mut dispatcher,
                &mut view,
                &mut port,
                if prefix { b"\x02" } else { b"after" },
            );
            assert_eq!(
                forwarded(&calls),
                if prefix {
                    b"before\x02".as_slice()
                } else {
                    b"beforeafter"
                }
            );
            assert!(!view.prefix_active());
        }
    }
}

#[test]
fn paste_rejection_after_timeout_keeps_payload_and_late_terminator_out_of_bindings() {
    for prefix in [false, true] {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut port = UnavailablePort(Rc::clone(&calls));
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        let now = Instant::now();
        let mut first = b"before".to_vec();
        if prefix {
            first.push(2);
        }
        first.extend_from_slice(b"\x1b[200~partial\x02q\x1b[20");
        for (bytes, elapsed) in [
            (first.as_slice(), 0),
            (b"".as_slice(), 60),
            (b"1~".as_slice(), 61),
        ] {
            assert_eq!(
                dispatcher
                    .dispatch(
                        &mut view,
                        bytes,
                        &mut port,
                        now + std::time::Duration::from_millis(elapsed),
                        |_, _| Ok::<bool, io::Error>(false),
                        &mut |_| Ok(())
                    )
                    .unwrap(),
                DispatchOutcome::Continue
            );
            assert_eq!(forwarded(&calls), b"before");
            assert_eq!(view.prefix_active(), prefix);
            assert!(
                calls
                    .borrow()
                    .iter()
                    .all(|call| !matches!(call, Call::Action(_)))
            );
        }
        dispatcher
            .dispatch(
                &mut view,
                if prefix { b"\x02" } else { b"after" },
                &mut port,
                now + std::time::Duration::from_millis(62),
                |_, _| Ok::<bool, io::Error>(false),
                &mut |_| Ok(()),
            )
            .unwrap();
        assert_eq!(
            forwarded(&calls),
            if prefix {
                b"before\x02".as_slice()
            } else {
                b"beforeafter"
            }
        );
        assert!(!view.prefix_active());
    }
}

#[test]
fn unavailable_entry_restores_mode_and_preserves_ordinary_batch_order() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    assert_eq!(
        dispatcher
            .dispatch(
                &mut view,
                b"before\x02,after",
                &mut port,
                Instant::now(),
                |_, _| Ok::<bool, io::Error>(false),
                &mut |request| {
                    calls.borrow_mut().push(Call::Mode(request.clone()));
                    Ok(())
                },
            )
            .unwrap(),
        DispatchOutcome::Continue
    );
    assert_eq!(
        *calls.borrow(),
        [
            Call::Mode(ModeRequest::BracketedPaste(false)),
            Call::Forward(b"before".to_vec()),
            Call::Mode(ModeRequest::BracketedPaste(true)),
            Call::Action(ClientAction::RequestRenameTab),
            Call::Mode(ModeRequest::BracketedPaste(false)),
            Call::Forward(b"after".to_vec())
        ]
    );
}

#[test]
fn prefixed_m_toggles_pointer_capture_through_the_mode_lease() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    // The shell settles the armed state after its mode write; from here
    // the binding releases and re-arms through the same lease.
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    for expected in [false, true] {
        let expected_request = if expected {
            PointerCaptureRequest::ButtonOnly
        } else {
            PointerCaptureRequest::Release
        };
        dispatcher
            .dispatch(
                &mut view,
                b"\x02m",
                &mut port,
                Instant::now(),
                |_, _| Ok::<bool, io::Error>(false),
                &mut |request| {
                    calls.borrow_mut().push(Call::Mode(request.clone()));
                    Ok(())
                },
            )
            .unwrap();
        // The prefix arm itself takes the paste lease; the assertion is
        // that exactly one pointer request of the expected direction ran.
        assert_eq!(
            calls
                .borrow()
                .iter()
                .filter(|call| { matches!(call, Call::Mode(ModeRequest::PointerCapture(_))) })
                .count(),
            1
        );
        assert!(
            calls
                .borrow()
                .contains(&Call::Mode(ModeRequest::PointerCapture(expected_request)))
        );
        assert_eq!(dispatcher.pointer_capture_enabled(), expected);
        calls.borrow_mut().clear();
    }
}

#[test]
fn exact_reset_reply_upgrades_global_tracking_once_after_the_query() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let now = Instant::now();
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let mut modes = Vec::new();
    dispatcher
        .begin_pointer_admission(now, &mut |request| {
            modes.push(request);
            Ok::<(), io::Error>(())
        })
        .unwrap();
    assert_eq!(
        modes
            .iter()
            .filter(|request| matches!(request, ModeRequest::PointerAdmissionQuery(_)))
            .count(),
        1
    );

    for reply in [
        b"\x1b[?1003;2$y".as_slice(),
        b"\x9b?1003;2$y",
        b"\x1b[?1;2c",
    ] {
        dispatcher
            .dispatch(
                &mut view,
                reply,
                &mut port,
                now,
                |_, _| Ok::<bool, io::Error>(false),
                &mut |request| {
                    modes.push(request);
                    Ok(())
                },
            )
            .unwrap();
    }
    assert_eq!(dispatcher.pointer_capture, PointerCaptureRequest::AnyMotion);
    assert_eq!(
        modes
            .iter()
            .filter(|request| {
                matches!(
                    request,
                    ModeRequest::PointerCapture(PointerCaptureRequest::AnyMotion)
                )
            })
            .count(),
        1
    );
}

#[test]
fn unsupported_contradictory_and_sentinel_replies_leave_button_only_active() {
    for reply in [
        b"\x1b[?1003;0$y".as_slice(),
        b"\x1b[?1003;1$y",
        b"\x1b[?1003;3$y",
        b"\x1b[?1003;4$y",
        b"\x1b[?1;2c",
    ] {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut port = UnavailablePort(Rc::clone(&calls));
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        let now = Instant::now();
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatcher
            .begin_pointer_admission(now, &mut |_| Ok::<(), io::Error>(()))
            .unwrap();
        let mut any_motion = 0;
        dispatcher
            .dispatch(
                &mut view,
                reply,
                &mut port,
                now,
                |_, _| Ok::<bool, io::Error>(false),
                &mut |request| {
                    if request == ModeRequest::PointerCapture(PointerCaptureRequest::AnyMotion) {
                        any_motion += 1;
                    }
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(any_motion, 0, "{reply:?}");
        assert_eq!(
            dispatcher.pointer_capture,
            PointerCaptureRequest::ButtonOnly
        );
    }

    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let now = Instant::now();
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher
        .begin_pointer_admission(now, &mut |_| Ok::<(), io::Error>(()))
        .unwrap();
    dispatcher
        .synchronize(
            &mut view,
            &mut port,
            now + Duration::from_millis(301),
            &mut |_| Ok::<(), io::Error>(()),
        )
        .unwrap();
    assert_eq!(
        dispatcher.pointer_capture,
        PointerCaptureRequest::ButtonOnly
    );
}

#[test]
fn admission_settled_while_released_is_reused_without_a_second_query() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let now = Instant::now();
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher
        .begin_pointer_admission(now, &mut |_| Ok::<(), io::Error>(()))
        .unwrap();
    dispatcher.set_pointer_capture(&mut view, &mut port, false);
    dispatcher
        .dispatch(
            &mut view,
            b"\x1b[?1003;2$y",
            &mut port,
            now,
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(dispatcher.pointer_capture, PointerCaptureRequest::Release);

    let mut modes = Vec::new();
    dispatcher
        .dispatch(
            &mut view,
            b"\x02m",
            &mut port,
            now,
            |_, _| Ok::<bool, io::Error>(false),
            &mut |request| {
                modes.push(request);
                Ok(())
            },
        )
        .unwrap();
    assert!(modes.contains(&ModeRequest::PointerCapture(
        PointerCaptureRequest::AnyMotion
    )));
    assert!(
        !modes
            .iter()
            .any(|request| matches!(request, ModeRequest::PointerAdmissionQuery(_)))
    );
    assert_eq!(dispatcher.pointer_capture, PointerCaptureRequest::AnyMotion);
}

#[test]
fn failed_query_or_tracking_transition_is_never_retried_or_published() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let now = Instant::now();
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let mut queries = 0;
    assert!(
        dispatcher
            .begin_pointer_admission(now, &mut |_| {
                queries += 1;
                Err::<(), _>(io::Error::other("query failed"))
            })
            .is_err()
    );
    dispatcher
        .begin_pointer_admission(now, &mut |_| {
            queries += 1;
            Ok::<(), io::Error>(())
        })
        .unwrap();
    assert_eq!(queries, 1);

    let mut dispatcher = InputDispatcher::default();
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher
        .begin_pointer_admission(now, &mut |_| Ok::<(), io::Error>(()))
        .unwrap();
    let mut transitions = 0;
    assert!(
        dispatcher
            .dispatch(
                &mut view,
                b"\x1b[?1003;2$y",
                &mut port,
                now,
                |_, _| Ok::<bool, io::Error>(false),
                &mut |request| {
                    if request == ModeRequest::PointerCapture(PointerCaptureRequest::AnyMotion) {
                        transitions += 1;
                        return Err(io::Error::other("tracking failed"));
                    }
                    Ok(())
                },
            )
            .is_err()
    );
    assert_eq!(
        dispatcher.pointer_capture,
        PointerCaptureRequest::ButtonOnly
    );
    assert!(dispatcher.pointer_transition_failed);
    dispatcher
        .synchronize(&mut view, &mut port, now, &mut |request| {
            if request == ModeRequest::PointerCapture(PointerCaptureRequest::AnyMotion) {
                transitions += 1;
            }
            Ok::<(), io::Error>(())
        })
        .unwrap();
    assert_eq!(transitions, 1);
}

#[test]
fn workspace_shortcuts_acquire_the_modal_lease_and_leave_exit_bindings_intact() {
    for (key, action) in [
        (b'a', ClientAction::RequestWorkspaceCreate),
        (b'e', ClientAction::RequestWorkspaceDirectory),
        (b'r', ClientAction::RequestWorkspaceRename),
        (b'R', ClientAction::RequestProjectRename),
    ] {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut port = UnavailablePort(Rc::clone(&calls));
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        dispatcher
            .dispatch(
                &mut view,
                &[2, key],
                &mut port,
                Instant::now(),
                |_, _| Ok::<bool, io::Error>(false),
                &mut |request| {
                    calls.borrow_mut().push(Call::Mode(request.clone()));
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(
            *calls.borrow(),
            [
                Call::Mode(ModeRequest::BracketedPaste(false)),
                Call::Mode(ModeRequest::BracketedPaste(true)),
                Call::Action(action),
                Call::Mode(ModeRequest::BracketedPaste(false))
            ]
        );
        for exit in *b"qd" {
            assert_eq!(
                dispatcher
                    .dispatch(
                        &mut view,
                        &[2, exit],
                        &mut port,
                        Instant::now(),
                        |_, _| Ok::<bool, io::Error>(false),
                        &mut |_| Ok(())
                    )
                    .unwrap(),
                DispatchOutcome::Exit
            );
        }
    }
}

fn forwarded_bytes(calls: &Rc<RefCell<Vec<Call>>>) -> Vec<u8> {
    calls
        .borrow()
        .iter()
        .filter_map(|call| match call {
            Call::Forward(bytes) => Some(bytes.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn dispatch_all(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    port: &mut impl TerminalInputPort,
    bytes: &[u8],
) {
    dispatcher
        .dispatch(
            view,
            bytes,
            port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
}

#[test]
fn malformed_mouse_reports_are_consumed_and_never_forwarded_to_the_pane() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    // Malformed SGR shape, malformed X10 body, then an ordinary key: the
    // mouse-shaped bytes are pointer protocol — dropped whole, only the
    // real key reaches the pane.
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"k\x1b[<0;0;1M\x1b[M\x20\x20\x29z",
    );
    assert_eq!(forwarded_bytes(&calls), b"kz".to_vec());
}

#[test]
fn aborted_and_overlong_mouse_runs_never_forward_their_bytes() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    // An ESC-aborted SGR partial: `\x1b[<0;` drops, the abort's arrow key
    // is client input — an unbound CSI, forwarded as one non-decision.
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;\x1b[Ak");
    assert_eq!(forwarded_bytes(&calls), b"\x1b[Ak".to_vec());
    // An overlong SGR parameter plus its release-final twin drop whole.
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;111111;1M\x1b[<999999;1;1mz",
    );
    assert_eq!(forwarded_bytes(&calls), b"\x1b[Akz".to_vec());
}

#[test]
fn an_x10_abort_and_timeout_drop_without_leaking_and_the_decoder_recovers() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    // ESC aborts a two-byte-in X10 body; the reprocessed `ESC[D` is client
    // input — forwarded — but the partial frame's bytes never are.
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[M\x20\x1b[Dk");
    assert_eq!(forwarded_bytes(&calls), b"\x1b[Dk".to_vec());
}

#[test]
fn unknown_csi_and_paste_keep_their_passthrough_alongside_dropped_mouse() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    // Unknown non-mouse CSI still passes through in order; a bracketed
    // paste forwards whole; only the malformed mouse bytes vanish.
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"k\x1b[?25h\x1b[<0;0;1M\x1b[200~hi\x1b[201~z",
    );
    assert_eq!(
        forwarded_bytes(&calls),
        b"k\x1b[?25h\x1b[200~hi\x1b[201~z".to_vec()
    );
}

#[test]
fn prefixed_z_issues_the_client_local_zoom_action() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    dispatcher
        .dispatch(
            &mut view,
            b"z",
            &mut port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(*calls.borrow(), [Call::Action(ClientAction::ToggleZoom)]);
    assert!(!view.prefix_active());
}

struct LayoutChangedPort(Rc<RefCell<Vec<Call>>>);

impl TerminalInputPort for LayoutChangedPort {
    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.0.borrow_mut().push(Call::Forward(bytes.to_vec()));
        Ok(())
    }

    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        self.0.borrow_mut().push(Call::Action(action));
        Some(SessionActionDisposition::LayoutChanged)
    }
}

#[test]
fn a_layout_changed_disposition_reobserves_layout_and_stales_the_pointer_map() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = LayoutChangedPort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let layout_passes = Rc::new(RefCell::new(0usize));
    let count = Rc::clone(&layout_passes);
    dispatcher
        .dispatch(
            &mut view,
            b"z",
            &mut port,
            Instant::now(),
            |_, _| {
                *count.borrow_mut() += 1;
                Ok::<bool, io::Error>(false)
            },
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(*calls.borrow(), [Call::Action(ClientAction::ToggleZoom)]);
    // The transition itself did not flag a layout change, so the layout pass
    // is attributed to the returned disposition alone.
    assert_eq!(*layout_passes.borrow(), 1);
}

/// A port whose Board capture intent the test flips directly, plus a record
/// of the exact-episode row selections the pointer path produces.
struct BoardPort {
    calls: Rc<RefCell<Vec<Call>>>,
    owned: Rc<std::cell::Cell<bool>>,
    #[cfg(windows)]
    action_menu: crate::action_menu::ActionMenu,
}

impl BoardPort {
    fn new(calls: Rc<RefCell<Vec<Call>>>, owned: Rc<std::cell::Cell<bool>>) -> Self {
        Self {
            calls,
            owned,
            #[cfg(windows)]
            action_menu: crate::action_menu::ActionMenu::default(),
        }
    }
}

impl TerminalInputPort for BoardPort {
    fn native_capture_claims(&self) -> crate::tui_composition::NativeCaptureClaims {
        let claims = crate::tui_composition::NativeCaptureClaims::default()
            .with_agent_board(self.owned.get());
        #[cfg(windows)]
        let claims = claims.with_action_menu(self.action_menu.input_context().is_some());
        claims
    }

    fn native_capture_context(
        &self,
        owner: crate::tui_composition::NativeCaptureOwner,
    ) -> Option<crate::terminal_session::NativeCaptureContext> {
        #[cfg(windows)]
        if owner == crate::tui_composition::NativeCaptureOwner::ActionMenu {
            return self
                .action_menu
                .input_context()
                .map(crate::terminal_session::NativeCaptureContext::ActionMenu);
        }
        (self.owned.get() && owner == crate::tui_composition::NativeCaptureOwner::AgentBoard).then(
            || {
                crate::terminal_session::NativeCaptureContext::AgentBoard(
                    crate::terminal_session::AgentBoardInputContext::for_test(
                        crate::terminal_session::AgentBoardEpisodeToken::for_test(1),
                        crate::tui_presentation::PresentationPhase::Admitted,
                        true,
                    ),
                )
            },
        )
    }

    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.calls.borrow_mut().push(Call::Forward(bytes.to_vec()));
        Ok(())
    }

    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        if action == ClientAction::ToggleAgentBoard {
            self.owned.set(!self.owned.get());
        } else {
            self.owned.set(false);
            #[cfg(windows)]
            self.action_menu.cancel();
        }
        self.calls.borrow_mut().push(Call::Action(action));
        Some(SessionActionDisposition::KeepNavigator)
    }

    #[cfg(windows)]
    fn open_action_menu(
        &mut self,
        target: crate::action_menu::MenuTarget,
        anchor: Option<(u16, u16)>,
        _pointer_released: bool,
    ) -> bool {
        self.action_menu.open_for(
            target,
            "Actions".to_owned(),
            vec![crate::action_menu::MenuAction::Help],
            anchor,
        )
    }

    #[cfg(windows)]
    fn cancel_action_menu(&mut self) {
        self.action_menu.cancel();
    }

    fn capture_agent_board_key_for(
        &mut self,
        episode: crate::terminal_session::AgentBoardEpisodeToken,
        key: crate::client_view::KeyEvent,
    ) -> bool {
        if !self.owned.get()
            || episode != crate::terminal_session::AgentBoardEpisodeToken::for_test(1)
        {
            return false;
        }
        if crate::client_view::Binding::new(
            crate::client_view::KeyCode::Esc,
            crate::client_view::Modifiers::NONE,
        )
        .matches(key)
        {
            self.owned.set(false);
        }
        true
    }

    fn agent_board_select(
        &mut self,
        target: &crate::terminal_session::AgentBoardRowTarget,
    ) -> bool {
        let expected = crate::terminal_session::AgentBoardRowTarget::for_test(
            crate::terminal_session::AgentBoardEpisodeToken::for_test(1),
            target.session().clone(),
        );
        if !self.owned.get() || target != &expected {
            return false;
        }
        self.calls
            .borrow_mut()
            .push(Call::BoardSelect(target.session().clone()));
        true
    }
}

struct ConfirmationBoardPort {
    calls: Rc<RefCell<Vec<Call>>>,
}

impl TerminalInputPort for ConfirmationBoardPort {
    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.calls.borrow_mut().push(Call::Forward(bytes.to_vec()));
        Ok(())
    }

    fn native_capture_claims(&self) -> crate::tui_composition::NativeCaptureClaims {
        crate::tui_composition::NativeCaptureClaims::default()
            .with_panel_close(true)
            .with_agent_board(true)
    }

    fn native_capture_context(
        &self,
        owner: crate::tui_composition::NativeCaptureOwner,
    ) -> Option<crate::terminal_session::NativeCaptureContext> {
        match owner {
            crate::tui_composition::NativeCaptureOwner::PanelClose => {
                Some(crate::terminal_session::NativeCaptureContext::PanelClose(
                    crate::terminal_session::ConfirmationCaptureContext {
                        token: crate::terminal_session::NativePanelCloseConfirmationToken::for_test(
                            2,
                        ),
                        phase: crate::tui_presentation::PresentationPhase::Admitted,
                        readiness: crate::terminal_session::CaptureReadiness::Interactive,
                    },
                ))
            }
            crate::tui_composition::NativeCaptureOwner::AgentBoard => {
                Some(crate::terminal_session::NativeCaptureContext::AgentBoard(
                    crate::terminal_session::AgentBoardInputContext::for_test(
                        crate::terminal_session::AgentBoardEpisodeToken::for_test(1),
                        crate::tui_presentation::PresentationPhase::Admitted,
                        true,
                    ),
                ))
            }
            _ => None,
        }
    }

    fn capture_panel_confirmation_key_for(
        &mut self,
        _token: crate::terminal_session::NativePanelCloseConfirmationToken,
        key: crate::client_view::ClientKey,
    ) -> bool {
        self.calls.borrow_mut().push(Call::Confirmation(key));
        true
    }

    fn capture_agent_board_key_for(
        &mut self,
        _episode: crate::terminal_session::AgentBoardEpisodeToken,
        _key: crate::client_view::KeyEvent,
    ) -> bool {
        true
    }

    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        self.calls.borrow_mut().push(Call::Action(action));
        Some(SessionActionDisposition::KeepNavigator)
    }
}

#[test]
fn a_board_close_edge_invalidates_the_published_hit_map() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let owned = Rc::new(std::cell::Cell::new(true));
    let mut port = BoardPort::new(Rc::clone(&calls), Rc::clone(&owned));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let session = orchbee_agent_observation_protocol::AgentSessionId::parse("s-1").unwrap();
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    // The open edge settles first so the map published below is the
    // current authority, matching the painted board.
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    let mut hits = crate::tui_pointer::HitMapBuilder::new();
    hits.mark_overlay_active();
    hits.record_interaction(
        crate::tui_pointer::HitLayer::Overlay,
        ratatui::layout::Rect::new(0, 0, 10, 1),
        ratatui::layout::Rect::new(0, 0, 10, 1),
        crate::tui_pointer::InteractionTarget::AgentBoardRow(
            crate::terminal_session::AgentBoardRowTarget::for_test(
                crate::terminal_session::AgentBoardEpisodeToken::for_test(1),
                session.clone(),
            ),
        ),
        crate::tui_pointer::InteractionSpec::enabled(
            crate::tui_pointer::PrimaryContract::SelectOnPress,
        ),
    );
    dispatcher.publish_hit_map(hits.finish());
    // Baseline: the press resolves the painted row.
    dispatcher
        .dispatch(
            &mut view,
            b"[<0;1;1M",
            &mut port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(*calls.borrow(), [Call::BoardSelect(session.clone())]);
    // The close edge turns the recorded regions into lies about a
    // surface that is no longer painted: the next synchronize drops the
    // whole map and the same press resolves nothing.
    owned.set(false);
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    calls.borrow_mut().clear();
    dispatcher
        .dispatch(
            &mut view,
            b"[<0;1;1M",
            &mut port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(
        calls.borrow().is_empty(),
        "the stale row must not resolve after the close edge"
    );
}

#[cfg(windows)]
#[test]
fn the_board_open_edge_also_invalidates_so_a_click_cannot_miss_the_overlay() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let owned = Rc::new(std::cell::Cell::new(false));
    let mut port = BoardPort::new(Rc::clone(&calls), Rc::clone(&owned));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    // A foreground region the last frame recorded under where the board
    // is about to paint — resolving by identity, it still answers while
    // the map is current.
    view.machines.available = true;
    view.machines.observe_agent_sidebar(true);
    let mut hits = crate::tui_pointer::HitMapBuilder::new();
    let sidebar_source = view.machines.sidebar_source();
    hits.record_interaction(
        crate::tui_pointer::HitLayer::Foreground,
        ratatui::layout::Rect::new(0, 0, 10, 1),
        ratatui::layout::Rect::new(0, 0, 10, 1),
        crate::tui_pointer::InteractionTarget::SidebarFixed(view.machines.drawn_sidebar_fixed(
            &sidebar_source,
            crate::machine_picker::SidebarFixedAction::Settings,
        )),
        crate::tui_pointer::InteractionSpec::enabled(
            crate::tui_pointer::PrimaryContract::ActivateOnRelease,
        ),
    );
    dispatcher.publish_hit_map(hits.finish());
    dispatcher
        .dispatch(
            &mut view,
            b"[<0;1;1M[<0;1;1m",
            &mut port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(*calls.borrow(), [Call::Action(ClientAction::OpenSettings)]);
    // Opening the board stales that map: the region it describes now
    // sits under a capturing overlay, so the same press must drop
    // instead of reaching through.
    owned.set(true);
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    calls.borrow_mut().clear();
    dispatcher
        .dispatch(
            &mut view,
            b"[<0;1;1M[<0;1;1m",
            &mut port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(
        calls.borrow().is_empty(),
        "the stale foreground region must not resolve past the open board"
    );
}

#[test]
fn an_open_board_admits_prefix_toggles_and_keeps_paste_captured() {
    for packet in [
        b"\x02\x1b[200~payload\x1b[201~".as_slice(),
        b"\x02\x1b[200~\xffpayload\x1b[201~".as_slice(),
    ] {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let owned = Rc::new(std::cell::Cell::new(true));
        let mut port = BoardPort::new(Rc::clone(&calls), Rc::clone(&owned));
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        dispatcher
            .dispatch(
                &mut view,
                packet,
                &mut port,
                Instant::now(),
                |_, _| Ok::<bool, io::Error>(false),
                &mut |_| Ok(()),
            )
            .unwrap();
        assert!(
            calls.borrow().is_empty(),
            "the board captures paste while the configured prefix stays armed"
        );
        assert!(view.prefix_active());
        assert!(view.notice().is_none());
        // The later `g` completes the prefix toggle through ClientView and
        // closes the same board episode.
        dispatcher
            .dispatch(
                &mut view,
                b"g",
                &mut port,
                Instant::now(),
                |_, _| Ok::<bool, io::Error>(false),
                &mut |_| Ok(()),
            )
            .unwrap();
        assert_eq!(
            *calls.borrow(),
            [Call::Action(ClientAction::ToggleAgentBoard)]
        );
        assert!(!owned.get());
        assert!(!view.prefix_active());
        assert!(forwarded_bytes(&calls).is_empty());
    }
}

#[test]
fn board_prefix_routes_switch_to_navigator_help_and_sidebar() {
    for (suffix, expected_action, expected_overlay) in [
        (
            b'w',
            ClientAction::OpenWorkbenchNavigator,
            Some(crate::client_view::Overlay::WorkbenchNavigator),
        ),
        (
            b'?',
            ClientAction::OpenShortcutHelp,
            Some(crate::client_view::Overlay::ShortcutHelp),
        ),
    ] {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let owned = Rc::new(std::cell::Cell::new(true));
        let mut port = BoardPort::new(Rc::clone(&calls), Rc::clone(&owned));
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);

        dispatch_all(&mut dispatcher, &mut view, &mut port, &[0x02, suffix]);

        assert!(!owned.get(), "switching away closes the native Board");
        assert_eq!(view.overlay(), expected_overlay);
        assert_eq!(
            calls.borrow().as_slice(),
            [Call::Action(expected_action.clone())]
        );

        if suffix == b'w' {
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02w");
            assert_eq!(view.overlay(), None, "repeating Navigator closes it");
            assert_eq!(
                calls.borrow().as_slice(),
                [
                    Call::Action(ClientAction::OpenWorkbenchNavigator),
                    Call::Action(ClientAction::CloseWorkbenchNavigator),
                ]
            );
        } else {
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02?");
            assert_eq!(view.overlay(), None, "repeating Help toggle closes it");
            assert_eq!(
                calls.borrow().as_slice(),
                [
                    Call::Action(ClientAction::OpenShortcutHelp),
                    Call::Action(ClientAction::CloseShortcutHelp),
                ]
            );
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02?");
            assert_eq!(
                view.overlay(),
                Some(crate::client_view::Overlay::ShortcutHelp),
                "the same binding opens Help again after the local close"
            );
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[27u");
            assert_eq!(view.overlay(), None, "Escape closes Help");
            assert_eq!(
                calls.borrow().as_slice(),
                [
                    Call::Action(ClientAction::OpenShortcutHelp),
                    Call::Action(ClientAction::CloseShortcutHelp),
                    Call::Action(ClientAction::OpenShortcutHelp),
                    Call::Action(ClientAction::CloseShortcutHelp),
                ]
            );
            assert!(forwarded_bytes(&calls).is_empty());
        }
    }

    let calls = Rc::new(RefCell::new(Vec::new()));
    let owned = Rc::new(std::cell::Cell::new(true));
    let mut port = BoardPort::new(Rc::clone(&calls), Rc::clone(&owned));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    view.machines.available = true;
    let hide_initial_sidebar = view.decide_action(ClientAction::ToggleBusinessSidebar);
    view.commit(
        hide_initial_sidebar,
        crate::client_view::SessionActionDisposition::KeepNavigator,
    );
    assert_eq!(
        view.sidebar_surface(),
        crate::client_view::SidebarSurface::Hidden,
        "the board binding opens the initially hidden wide sidebar"
    );
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02b");
    assert!(!owned.get());
    assert!(matches!(
        view.sidebar_surface(),
        crate::client_view::SidebarSurface::Docked(_)
    ));
    assert_eq!(
        calls.borrow().as_slice(),
        [Call::Action(ClientAction::ToggleBusinessSidebar)]
    );
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02b");
    assert_eq!(
        view.sidebar_surface(),
        crate::client_view::SidebarSurface::Hidden
    );
    assert_eq!(
        calls.borrow().len(),
        2,
        "repeating sidebar toggle closes it"
    );
}

#[test]
fn agent_board_consumes_literal_prefix_and_global_actions_use_the_action_owner() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let owned = Rc::new(std::cell::Cell::new(true));
    let mut port = BoardPort::new(Rc::clone(&calls), Rc::clone(&owned));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x02");
    assert!(
        owned.get(),
        "a literal prefix keeps the Board capture active"
    );
    assert!(!view.prefix_active());
    assert!(calls.borrow().is_empty());
    assert!(forwarded_bytes(&calls).is_empty());

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1b[27u");
    assert!(owned.get(), "Escape cancels only the armed prefix");
    assert!(!view.prefix_active());
    assert!(calls.borrow().is_empty());

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02v");
    assert!(owned.get(), "an unbound suffix does not replace the Board");
    assert_eq!(
        view.notice(),
        Some(crate::client_view::Notice::UnknownPrefix)
    );
    assert!(calls.borrow().is_empty());

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02m");
    assert!(
        !owned.get(),
        "a global prefix action leaves the Board episode"
    );
    assert_eq!(
        calls.borrow().as_slice(),
        [Call::Action(ClientAction::TogglePointerCapture)]
    );
    assert!(forwarded_bytes(&calls).is_empty());
}

#[cfg(windows)]
#[test]
fn board_prefix_focus_sidebar_releases_native_capture_before_focusing_sidebar() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let owned = Rc::new(std::cell::Cell::new(true));
    let mut port = BoardPort::new(Rc::clone(&calls), Rc::clone(&owned));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    view.machines.available = true;

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02");
    assert!(view.prefix_active());
    assert!(
        owned.get(),
        "the Board retains capture while its prefix is armed"
    );
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"B");

    assert!(!owned.get(), "the native Board owner releases capture");
    assert!(view.machines.sidebar_navigation_active());
    assert!(matches!(
        view.sidebar_surface(),
        crate::client_view::SidebarSurface::Docked(_)
    ));
    let actions = calls
        .borrow()
        .iter()
        .filter_map(|call| match call {
            Call::Action(action) => Some(action.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(actions, [ClientAction::FocusBusinessSidebar]);
    assert!(forwarded_bytes(&calls).is_empty());
}

#[cfg(windows)]
#[test]
fn board_prefix_opens_action_menu_after_native_capture_release() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let owned = Rc::new(std::cell::Cell::new(true));
    let mut port = BoardPort::new(Rc::clone(&calls), Rc::clone(&owned));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02");
    assert!(view.prefix_active());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1bOQ");

    assert!(!owned.get(), "the native Board owner releases capture");
    assert!(
        port.action_menu.active(),
        "the requested menu is actually open"
    );
    assert_eq!(
        port.action_menu.view().unwrap().source,
        &crate::action_menu::MenuTarget::Global
    );
    assert!(
        port.native_capture_claims()
            .contains(crate::tui_composition::NativeCaptureOwner::ActionMenu)
    );
    assert!(
        !port
            .native_capture_claims()
            .contains(crate::tui_composition::NativeCaptureOwner::AgentBoard)
    );
    let actions = calls
        .borrow()
        .iter()
        .filter_map(|call| match call {
            Call::Action(action) => Some(action.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(actions, [ClientAction::OpenActionMenu]);
    assert!(forwarded_bytes(&calls).is_empty());
}

#[test]
fn agent_board_prefix_toggle_opens_and_closes_through_native_owner() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let owned = Rc::new(std::cell::Cell::new(false));
    let mut port = BoardPort::new(Rc::clone(&calls), Rc::clone(&owned));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02g");
    assert!(owned.get());
    assert_eq!(
        calls.borrow().as_slice(),
        [Call::Action(ClientAction::ToggleAgentBoard)]
    );
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02g");
    assert!(!owned.get());
    assert_eq!(
        calls.borrow().as_slice(),
        [
            Call::Action(ClientAction::ToggleAgentBoard),
            Call::Action(ClientAction::ToggleAgentBoard),
        ]
    );
    assert!(forwarded_bytes(&calls).is_empty());
}

#[test]
fn confirmation_capture_keeps_first_refusal_over_the_agent_board_prefix_route() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = ConfirmationBoardPort {
        calls: Rc::clone(&calls),
    };
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02g");

    assert_eq!(
        *calls.borrow(),
        [Call::Confirmation(crate::client_view::ClientKey::ControlB)]
    );
    assert!(!view.prefix_active());
    assert!(forwarded_bytes(&calls).is_empty());
}

#[test]
fn opening_board_acquires_paste_before_installing_capture_and_fences_the_packet_tail() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let owned = Rc::new(std::cell::Cell::new(false));
    let mut port = BoardPort::new(Rc::clone(&calls), Rc::clone(&owned));
    let mut dispatcher = InputDispatcher::default();
    dispatcher
        .dispatch(
            &mut ClientView::new(100),
            b"\x02gtail",
            &mut port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |request| {
                if request == ModeRequest::BracketedPaste(true) {
                    calls.borrow_mut().push(Call::Mode(request));
                }
                Ok(())
            },
        )
        .unwrap();
    assert!(owned.get());
    assert_eq!(
        *calls.borrow(),
        [
            Call::Mode(ModeRequest::BracketedPaste(true)),
            Call::Action(ClientAction::ToggleAgentBoard),
        ]
    );
}

#[test]
fn board_continuation_accepts_two_moves_but_not_stale_activation_in_one_packet() {
    struct Port {
        selected: usize,
        exact_activation: bool,
        activations: usize,
    }

    impl TerminalInputPort for Port {
        fn native_capture_claims(&self) -> crate::tui_composition::NativeCaptureClaims {
            crate::tui_composition::NativeCaptureClaims::default().with_agent_board(true)
        }

        fn native_capture_context(
            &self,
            owner: crate::tui_composition::NativeCaptureOwner,
        ) -> Option<crate::terminal_session::NativeCaptureContext> {
            (owner == crate::tui_composition::NativeCaptureOwner::AgentBoard).then(|| {
                crate::terminal_session::NativeCaptureContext::AgentBoard(
                    crate::terminal_session::AgentBoardInputContext::for_test(
                        crate::terminal_session::AgentBoardEpisodeToken::for_test(7),
                        crate::tui_presentation::PresentationPhase::Admitted,
                        true,
                    ),
                )
            })
        }

        fn capture_agent_board_key_for(
            &mut self,
            episode: crate::terminal_session::AgentBoardEpisodeToken,
            key: crate::client_view::KeyEvent,
        ) -> bool {
            if episode != crate::terminal_session::AgentBoardEpisodeToken::for_test(7) {
                return false;
            }
            match key.normalized() {
                (crate::client_view::KeyCode::Down, crate::client_view::Modifiers::NONE) => {
                    self.selected += 1;
                    self.exact_activation = false;
                }
                (crate::client_view::KeyCode::Enter, crate::client_view::Modifiers::NONE)
                    if self.exact_activation =>
                {
                    self.activations += 1;
                }
                _ => {}
            }
            true
        }

        fn forward(&mut self, _bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
            Ok(())
        }
    }

    let mut port = Port {
        selected: 0,
        exact_activation: true,
        activations: 0,
    };
    let mut dispatcher = InputDispatcher::default();
    dispatcher
        .dispatch(
            &mut ClientView::new(100),
            b"\x1b[B\x1b[B\r",
            &mut port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(port.selected, 2);
    assert_eq!(port.activations, 0);
}

#[test]
fn focus_metadata_preserves_input_order_and_an_armed_prefix() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    for packet in [b"before\x1b[".as_slice(), b"Oafter\x02\x1b[I".as_slice()] {
        dispatcher
            .dispatch(
                &mut view,
                packet,
                &mut port,
                Instant::now(),
                |_, _| Ok::<bool, io::Error>(false),
                &mut |_| Ok(()),
            )
            .unwrap();
    }
    assert!(view.prefix_active());
    assert_eq!(
        *calls.borrow(),
        [
            Call::Forward(b"before".to_vec()),
            Call::Focus(false),
            Call::Forward(b"after".to_vec()),
            Call::Focus(true)
        ]
    );
}

#[cfg(windows)]
#[test]
fn machine_picker_owns_closing_packet_and_split_paste_across_surface_switch() {
    for invalid in [false, true] {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut port = UnavailablePort(calls.clone());
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        view.machines.available = true;
        let now = Instant::now();
        let dispatch = |dispatcher: &mut InputDispatcher,
                        view: &mut ClientView,
                        port: &mut UnavailablePort,
                        packet: &[u8]| {
            dispatcher
                .dispatch(
                    view,
                    packet,
                    port,
                    now,
                    |_, _| Ok::<bool, io::Error>(false),
                    &mut |_| Ok(()),
                )
                .unwrap();
        };
        dispatch(&mut dispatcher, &mut view, &mut port, b"\x02Mignored");
        assert!(view.machines.open);
        dispatch(&mut dispatcher, &mut view, &mut port, b"\x1b[200~hidden");
        // An asynchronous candidate wins while a paste is still being decoded.
        view.machines.open = false;
        dispatcher.machine_surface_changed();
        dispatch(
            &mut dispatcher,
            &mut view,
            &mut port,
            if invalid {
                b"\xffpayload\x1b[201~visible"
            } else {
                b"payload\x1b[201~visible"
            },
        );
        assert_eq!(
            *calls.borrow(),
            [
                Call::Action(ClientAction::OpenMachines),
                Call::Forward(b"visible".to_vec())
            ]
        );
        calls.borrow_mut().clear();
        view.machines.show();
        dispatch(&mut dispatcher, &mut view, &mut port, b"\r\x1b[");
        assert!(view.machines.request.is_some());
        view.machines.open = false;
        dispatch(&mut dispatcher, &mut view, &mut port, b"Aok");
        assert_eq!(*calls.borrow(), [Call::Forward(b"ok".to_vec())]);
    }
}

#[cfg(windows)]
#[derive(Default)]
struct ModalLinkPort {
    probe: crate::terminal_input::PointerProbe,
    selections: Vec<crate::terminal_input::PointerSelectionIntent>,
    links: usize,
    forwarded: Vec<Vec<u8>>,
}

#[cfg(windows)]
impl TerminalInputPort for ModalLinkPort {
    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.push(bytes.to_vec());
        Ok(())
    }

    fn pointer_probe(&mut self) -> crate::terminal_input::PointerProbe {
        self.probe.clone()
    }

    fn pointer_selection(
        &mut self,
        _pane: &crate::terminal_session::state::PaneRef,
        intent: crate::terminal_input::PointerSelectionIntent,
    ) -> crate::terminal_input::PointerSelectionAdmission {
        self.selections.push(intent);
        crate::terminal_input::PointerSelectionAdmission::Admitted
    }

    fn pointer_link(
        &mut self,
        _source: crate::terminal_link_activation::PaintedLinkSource,
        _cell: crate::terminal_input::PointerSelectionCell,
    ) -> crate::terminal_input::PointerLinkAdmission {
        self.links += 1;
        crate::terminal_input::PointerLinkAdmission::Admitted
    }
}

#[cfg(windows)]
fn dispatcher_link_source(
    pane: crate::terminal_session::state::PaneRef,
) -> crate::terminal_link_activation::PaintedLinkSource {
    fn id(value: u128) -> String {
        format!("{value:032x}")
    }

    let tab = pane.anchor().tab_id().clone();
    let panel = pane.pane().panel().clone();
    crate::terminal_link_activation::PaintedLinkSource {
        pane,
        host_terminal: orchbee_client_server_protocol::HostTerminalId::parse(&id(27)).unwrap(),
        screen: crate::terminal_content::TerminalScreen::Primary,
        member: crate::terminal_link_activation::SnapshotMember {
            site: orchbee_workbench_protocol::SiteId::parse(&id(20)).unwrap(),
            context: orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(&id(21))
                .unwrap(),
            tab,
            panel,
            presentation: orchbee_workbench_protocol::TerminalPresentationId::parse(&id(22))
                .unwrap(),
            worker: orchbee_workbench_protocol::WorkerId::parse(&id(23)).unwrap(),
            terminal: orchbee_workbench_protocol::TerminalId::parse(&id(24)).unwrap(),
        },
        rows: crate::terminal_link_activation::PaintedRows {
            attachment: orchbee_terminal_protocol::AttachmentId::from_bytes([25; 16]).unwrap(),
            host_epoch: orchbee_terminal_protocol::HostEpoch::from_bytes([26; 16]).unwrap(),
            sequence: std::num::NonZeroU64::MIN,
            generation: std::num::NonZeroU64::MIN,
            revision: 1,
            columns: 10,
            start_column: 0,
            total_rows: 3,
            start_row: 0,
            row_count: 3,
            history: false,
        },
    }
}

#[cfg(windows)]
#[test]
fn machine_picker_retires_a_cancelled_link_release_before_the_next_pane_press() {
    use crate::terminal_input::{PointerFrameFacts, PointerPane, PointerProbe, PointerTab};
    use orchbee_terminal_protocol::{MouseFormat, MouseTracking};
    use ratatui::layout::Rect;

    let pane = pane_ref();
    let source = dispatcher_link_source(pane.clone());
    let mut port = ModalLinkPort {
        probe: PointerProbe {
            focus: Some(pane.clone()),
            desired: Some(pane.pane().clone()),
            panes: vec![PointerPane {
                right_click_reporting: false,
                right_click: Default::default(),
                record: pane.clone(),
                attached: true,
                controlling: Some(true),
                frame_available: true,
            }],
            tabs: vec![PointerTab {
                id: pane.anchor().tab_id().clone(),
                position: 1,
            }],
            selection_ready: true,
            frame: Some(PointerFrameFacts {
                tracking: MouseTracking::None,
                format: MouseFormat::Sgr,
                cell_pixel: (0, 0),
                alternate_screen: false,
                alternate_scroll: false,
                application_cursor: false,
            }),
            ..PointerProbe::default()
        },
        ..ModalLinkPort::default()
    };
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    view.machines.available = true;
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let publish = |dispatcher: &mut InputDispatcher| {
        let content = Rect::new(0, 0, 10, 3);
        let mut hits = crate::tui_pointer::HitMapBuilder::new();
        hits.record(
            crate::tui_pointer::HitLayer::Foreground,
            content,
            crate::tui_pointer::HitTarget::Pane {
                pane: pane.clone(),
                content,
            },
        );
        hits.record_link_source(content, source.clone());
        dispatcher.publish_hit_map(hits.finish());
    };
    let dispatch = |dispatcher: &mut InputDispatcher,
                    view: &mut ClientView,
                    port: &mut ModalLinkPort,
                    packet: &[u8]| {
        dispatcher
            .dispatch(
                view,
                packet,
                port,
                Instant::now(),
                |_, _| Ok::<bool, io::Error>(false),
                &mut |_| Ok(()),
            )
            .unwrap();
    };

    publish(&mut dispatcher);
    dispatch(&mut dispatcher, &mut view, &mut port, b"\x1b[<16;2;2M");
    dispatch(&mut dispatcher, &mut view, &mut port, b"\x02M");
    assert!(view.machines.open);
    dispatch(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;2;2m");
    assert_eq!(port.links, 0, "the modal-owned release cannot activate");

    view.machines.open = false;
    dispatcher.machine_surface_changed();
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    publish(&mut dispatcher);
    dispatch(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;2;2M");
    assert!(matches!(
        port.selections.as_slice(),
        [crate::terminal_input::PointerSelectionIntent::Start { .. }]
    ));
    assert!(port.forwarded.is_empty());
}

#[cfg(windows)]
#[test]
fn default_agent_cycle_opens_filtered_navigation_and_query_completion_is_captured() {
    for (shortcut, action) in [
        (b'{', ClientAction::PreviousAgent),
        (b'}', ClientAction::NextAgent),
    ] {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut port = UnavailablePort(Rc::clone(&calls));
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        view.machines.available = true;
        let now = Instant::now();
        for packet in [
            vec![2, shortcut, b'x'],
            b"/same\r".to_vec(),
            b"\x1b".to_vec(),
        ] {
            dispatcher
                .dispatch(
                    &mut view,
                    &packet,
                    &mut port,
                    now,
                    |_, _| Ok::<bool, io::Error>(false),
                    &mut |_| Ok(()),
                )
                .unwrap();
        }
        // Bare Escape is resolved after its bounded ambiguity window.
        dispatcher
            .dispatch(
                &mut view,
                &[],
                &mut port,
                now + Duration::from_secs(1),
                |_, _| Ok::<bool, io::Error>(false),
                &mut |_| Ok(()),
            )
            .unwrap();
        assert_eq!(*calls.borrow(), [Call::Action(action)]);
        assert!(view.machines.request.is_none());
        assert!(!view.machines.open);
        assert!(
            !view
                .machines
                .drive_agent_navigation(now + Duration::from_secs(2))
        );
    }
}

#[test]
fn focus_loss_cancels_prefix_and_preserves_the_next_terminal_key() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1b[O\x1b[Ix");
    assert!(!view.prefix_active());
    assert_eq!(forwarded_bytes(&calls), b"x");
}

#[test]
fn terminal_paste_retires_prefix_before_the_next_typed_key() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x02\x1b[200~hello\x1b[201~x",
    );
    assert!(!view.prefix_active());
    assert!(forwarded_bytes(&calls).ends_with(b"x"));
}

#[cfg(windows)]
#[test]
fn delivered_console_motion_survives_an_unavailable_vt_query_without_rearming_capture() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let now = Instant::now();
    dispatcher
        .begin_pointer_admission(now, &mut |_| Ok::<_, io::Error>(()))
        .unwrap();
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[?1003;0$y");
    assert_eq!(
        dispatcher.pointer_capture,
        PointerCaptureRequest::ButtonOnly
    );
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[200~\x1b[<35;5;5M\x1b[201~",
    );
    assert!(
        !dispatcher.native_motion_observed,
        "pasted sequences are not motion evidence"
    );
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<35;5;5M");
    assert!(dispatcher.native_motion_observed);
    assert_eq!(
        dispatcher.pointer_capture,
        PointerCaptureRequest::ButtonOnly
    );
    assert_eq!(
        dispatcher.pointer_observability(PointerCaptureRequest::ButtonOnly),
        MotionObservability::AnyMotion
    );
    dispatcher.set_pointer_capture(&mut view, &mut port, false);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<35;6;5M");
    assert_eq!(dispatcher.pointer_capture, PointerCaptureRequest::Release);
    assert_eq!(
        dispatcher.pointer_observability(PointerCaptureRequest::Release),
        MotionObservability::ButtonOnly
    );
}

fn pane_ref() -> crate::terminal_session::state::PaneRef {
    use crate::terminal_session::state::{AttachmentKey, PaneKey, PaneRef};
    use orchbee_workbench_protocol::{WorkbenchPanelId, WorkbenchTabId};
    PaneRef::new(
        PaneKey::new(
            AttachmentKey::new(
                0,
                WorkbenchTabId::parse("0000000000000000000000000000000a").unwrap(),
            ),
            WorkbenchPanelId::parse("0000000000000000000000000000000b").unwrap(),
        ),
        0,
    )
}
