use super::*;
use crate::client_view::{CopyModeKey, KeyEvent, copy_mode_key};

#[derive(Default)]
struct CopyPort {
    active: bool,
    refuse: bool,
    forwarded: Vec<u8>,
    keys: Vec<KeyEvent>,
    pastes: Vec<Option<String>>,
    actions: Vec<ClientAction>,
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
    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        if action == ClientAction::EnterCopyMode {
            self.active = !self.refuse;
        }
        self.actions.push(action);
        Some(SessionActionDisposition::KeepNavigator)
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
        copy_mode_key(*port.keys.last().unwrap()),
        Some(CopyModeKey::Search(
            crate::terminal_copy::CopySearchDirection::Backward
        ))
    );
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
        view.configure_keys(
            &orchbee_local_site_runtime::WindowsKeySettings::from_profile(profile).unwrap(),
        );
        let mut port = CopyPort::default();
        let mut dispatcher = InputDispatcher::default();
        dispatch_all(&mut dispatcher, &mut view, &mut port, input);
        assert_eq!(port.active, enters);
        let rows = crate::localization::Locale::EnUs
            .shell()
            .configured_shortcut_rows(&view);
        if !enters {
            assert!(
                rows.iter().any(
                    |(key, desc)| key.trim() == "Unbound" && desc == "Enter keyboard copy mode"
                )
            );
        }
    }
}
