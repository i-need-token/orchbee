use super::*;
use crate::client_view::{CopyModeKey, KeyEvent, copy_mode_key};
use crate::terminal_copy_mode::{CopyModeCursor, CopySurfaceToken, EpisodeId, Surface};
use crate::tui_pointer::{HitLayer, HitMapBuilder, HitTarget};
use ratatui::layout::Rect;
use std::num::NonZeroU64;

#[derive(Default)]
struct CopyPort {
    active: bool,
    refuse: bool,
    forwarded: Vec<u8>,
    keys: Vec<KeyEvent>,
    pastes: Vec<Option<String>>,
    actions: Vec<ClientAction>,
    pointer_cells: Vec<(CopyModeCursor, bool)>,
}

#[test]
fn timed_out_paste_reports_one_copy_refusal_and_keeps_its_tail_captured() {
    let mut port = CopyPort {
        active: true,
        ..CopyPort::default()
    };
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(80);
    let now = Instant::now();
    dispatcher
        .dispatch(
            &mut view,
            b"\x1b[200~partial",
            &mut port,
            now,
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    dispatcher
        .synchronize(
            &mut view,
            &mut port,
            now + Duration::from_millis(60),
            &mut |_| Ok::<_, io::Error>(()),
        )
        .unwrap();
    assert_eq!(port.pastes, [None]);
    dispatcher
        .dispatch(
            &mut view,
            b"q\x02U\x1b[201~",
            &mut port,
            now + Duration::from_millis(61),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(port.pastes, [None]);
    assert!(port.keys.is_empty());
    assert!(port.forwarded.is_empty());
    assert!(port.active);
    dispatcher
        .dispatch(
            &mut view,
            b"q",
            &mut port,
            now + Duration::from_millis(62),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(!port.active);
}

impl TerminalInputPort for CopyPort {
    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(bytes);
        Ok(())
    }
    fn apply_client_action(&mut self, action: ClientAction) -> Option<ClientActionDisposition> {
        if action == ClientAction::EnterCopyMode {
            self.active = !self.refuse;
        }
        self.actions.push(action);
        Some(ClientActionDisposition::KeepNavigator)
    }
    fn copy_mode_active(&self) -> bool {
        self.active
    }
    fn capture_copy_mode_key(&mut self, event: &KeyEvent) -> bool {
        self.keys.push(*event);
        if copy_mode_key(*event) == Some(CopyModeKey::Exit) {
            self.active = false;
        }
        true
    }
    fn capture_copy_mode_paste(&mut self, text: Option<&str>) -> bool {
        self.pastes.push(text.map(str::to_owned));
        true
    }
    fn cancel_copy_mode(&mut self) -> bool {
        std::mem::take(&mut self.active)
    }
    fn copy_pointer_cell(
        &mut self,
        _token: CopySurfaceToken,
        cell: CopyModeCursor,
        start: bool,
    ) -> bool {
        self.pointer_cells.push((cell, start));
        true
    }
}

#[test]
fn copy_exit_packet_cannot_fall_through_to_retained_resize_mode() {
    let mut port = CopyPort {
        active: true,
        ..CopyPort::default()
    };
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(80);
    let transition = view.decide_action(ClientAction::EnterResizeMode);
    let action = transition.action().unwrap();
    let disposition = port.apply_client_action(action).unwrap();
    view.commit(transition, disposition);
    assert!(port.active);
    assert!(view.resize_mode());

    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"qtail\x1b[200~paste\x1b[201~",
    );
    assert!(!port.active);
    assert!(view.resize_mode());
    assert_eq!(port.keys.len(), 1);
    assert_eq!(copy_mode_key(port.keys[0]), Some(CopyModeKey::Exit));
    assert!(port.pastes.is_empty());
    assert!(port.forwarded.is_empty());
    assert_eq!(port.actions, [ClientAction::EnterResizeMode]);

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[C");
    assert_eq!(
        port.actions,
        [
            ClientAction::EnterResizeMode,
            ClientAction::ResizePane {
                direction: crate::client_view::PanelDirection::Right,
                fine: false,
            },
        ]
    );
}

fn copy_surface_map() -> (crate::tui_pointer::FrameHitMap, Rect) {
    let area = Rect::new(10, 5, 8, 3);
    let surface = Surface::checked(
        crate::terminal_content::TerminalScreen::Primary,
        NonZeroU64::new(7).unwrap(),
        11,
        area.width,
        40,
        20,
        area.height,
    )
    .unwrap();
    let token = CopySurfaceToken {
        episode: EpisodeId::new(NonZeroU64::new(3).unwrap()),
        surface,
    };
    let mut hits = HitMapBuilder::new();
    // Copy content is native chrome, matching the production renderer's
    // admission layer rather than an overlay-only surface.
    hits.record(
        HitLayer::ChromeSurface,
        area,
        HitTarget::CopySurface {
            token,
            area,
            origin: CopyModeCursor { column: 0, row: 0 },
        },
    );
    (hits.finish(), area)
}

#[test]
fn copy_drag_release_runs_before_packet_cleanup_and_cancelled_drag_cleans_once() {
    for cancelled in [false, true] {
        let mut port = CopyPort::default();
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(80);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02U");
        assert!(port.active && view.copy_mode_active());
        let (map, area) = copy_surface_map();
        dispatcher.publish_hit_map(map);
        let x = area.x + 2;
        let y = area.y + 1;
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            format!("\x1b[<0;{};{}M", x + 1, y + 1).as_bytes(),
        );
        assert_eq!(
            port.pointer_cells,
            [(CopyModeCursor { column: 2, row: 1 }, true)]
        );

        if cancelled {
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[O\x1b[I");
        }
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            format!("\x1b[<0;{};{}mtail\x1b[200~paste", x + 1, y + 1).as_bytes(),
        );
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[201~");
        assert!(port.forwarded.is_empty());
        assert!(port.pastes.is_empty());
        if cancelled {
            assert_eq!(
                port.pointer_cells,
                [(CopyModeCursor { column: 2, row: 1 }, true)],
                "cleanup cannot replay a cancelled copy effect"
            );
        } else {
            assert_eq!(
                port.pointer_cells,
                [
                    (CopyModeCursor { column: 2, row: 1 }, true),
                    (CopyModeCursor { column: 2, row: 1 }, false)
                ],
                "the active Copy owner must see release before packet cleanup"
            );
        }

        dispatch_all(&mut dispatcher, &mut view, &mut port, b"q");
        assert!(!port.active);
        dispatcher.publish_hit_map(HitMapBuilder::new().finish());
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            format!("\x1b[<0;{};{}morphan", x + 1, y + 1).as_bytes(),
        );
        assert_eq!(port.forwarded, b"orphan", "cancelled={cancelled}");
    }
}

#[test]
fn raw_search_packet_and_carriage_return_are_owned_by_copy_mode() {
    let mut port = CopyPort::default();
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(80);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02U");
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"/foo\r");
    assert!(port.forwarded.is_empty());
    assert_eq!(port.keys.len(), 5);
    assert_eq!(
        copy_mode_key(*port.keys.last().unwrap()),
        Some(CopyModeKey::Copy)
    );
    assert!(port.active);
}

#[test]
fn entry_exit_packets_and_fragmented_exit_tail_never_reach_the_pane() {
    for refuse in [false, true] {
        let mut port = CopyPort {
            refuse,
            ..CopyPort::default()
        };
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"before\x02Utail");
        assert_eq!(port.forwarded, b"before");
        assert_eq!(port.actions, [ClientAction::EnterCopyMode]);
        assert_eq!(view.copy_mode_active(), !refuse);
        if !refuse {
            dispatch_all(
                &mut dispatcher,
                &mut view,
                &mut port,
                b"hj\x02\x02\x1b[999~",
            );
            assert_eq!(
                port.actions.len(),
                1,
                "prefix and unknown sequences stay captured"
            );
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"qUNSAFE\x1b[");
            assert!(!port.active);
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"A");
            assert_eq!(
                port.forwarded, b"before",
                "owned partial CSI stays owned across reads"
            );
        }
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"fresh");
        assert_eq!(port.forwarded, b"beforefresh");
    }
}

#[test]
fn atomic_paste_release_and_mode_help_keep_exclusive_capture() {
    let mut port = CopyPort::default();
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(80);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02U");
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[200~q\x02U\x1b[201~",
    );
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[200~\xff\x1b[201~",
    );
    assert_eq!(port.pastes, [Some("q\u{2}U".into()), None]);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[113;1:3u");
    assert!(port.active, "release cannot exit mode");
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1bOP");
    assert_eq!(
        view.overlay(),
        Some(crate::client_view::Overlay::ShortcutHelp)
    );
    let before = port.keys.len();
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[6~");
    assert_eq!(
        port.keys.len(),
        before,
        "help owns paging before the copy cursor"
    );
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"?");
    assert_eq!(
        port.keys.len(),
        before,
        "Help keeps ordinary search input away from the copy cursor"
    );
    assert_eq!(view.help_filter(), "?");
    assert!(port.active, "Help remains above the underlying copy mode");
    assert!(port.forwarded.is_empty());
    assert!(!view.prefix_active());
}

#[test]
fn configured_entry_and_disabled_default_share_dispatch_and_discovery() {
    for (profile, input, enters) in [
        ("[keys]\ncopy_mode='f2'", b"\x1bOQ".as_slice(), true),
        ("[keys]\ncopy_mode=[]", b"\x02U".as_slice(), false),
    ] {
        let mut view = ClientView::new(100);
        view.configure_keys(&orchbee_local_runtime::KeySettings::from_profile(profile).unwrap());
        let mut port = CopyPort::default();
        let mut dispatcher = InputDispatcher::default();
        dispatch_all(&mut dispatcher, &mut view, &mut port, input);
        assert_eq!(port.active, enters);
        let entries = crate::localization::Locale::EnUs.shortcut_help_entries(
            &view,
            true,
            crate::client_view::KeyLabelGlyphs::Unicode,
        );
        let copy_title =
            crate::localization::Locale::EnUs.menu_action(&crate::action_menu::MenuAction::Copy);
        if enters {
            assert!(
                entries
                    .iter()
                    .any(|entry| entry.key == "F2" && entry.title == copy_title),
                "configured F2 copy binding should discover the shared action title {copy_title:?}"
            );
        } else {
            assert!(
                !entries.iter().any(|entry| entry.title == copy_title),
                "disabling the default copy binding should remove {copy_title:?} from discovery"
            );
        }
    }
}
