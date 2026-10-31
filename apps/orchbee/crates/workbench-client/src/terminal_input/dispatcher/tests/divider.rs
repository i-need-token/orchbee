//! Native decoder timelines for drag ownership, one release CAS, Esc and prefix boundaries.
use super::*;
use crate::tui_hit_map::{HitLayer, HitMapBuilder, HitTarget};
use crate::tui_layout::{DividerSource, DrawnDivider};
use orchbee_workbench_protocol::{
    ClientSplitRatio, PanelOrientation, SessionId, WorkbenchPanelId, WorkbenchPanelSlotId,
    WorkbenchRevision, WorkbenchSplitId, WorkbenchTabId, WorkbenchTerminalContextId,
};
use ratatui::layout::Rect;

struct Port {
    bytes: Vec<u8>,
    submissions: usize,
    menus: usize,
}
impl Port {
    fn pointer_divider_resize(&mut self, _: &DrawnDivider, _: ClientSplitRatio) {
        self.submissions += 1;
    }
}

impl TerminalInputPort for Port {
    fn surface_action(
        &mut self,
        action: crate::terminal_input::SurfaceAction<'_>,
    ) -> Option<crate::client_view::ClientActionDisposition> {
        use crate::terminal_input::{LayoutAction, SurfaceAction};
        match action {
            SurfaceAction::Layout(LayoutAction::Divider { target, ratio }) => {
                self.pointer_divider_resize(target, ratio);
                None
            }
            _ => None,
        }
    }

    fn open_action_menu(
        &mut self,
        _: crate::action_menu::MenuTarget,
        _: Option<crate::action_menu::MenuAnchor>,
        _: bool,
    ) -> bool {
        self.menus += 1;
        true
    }

    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn pointer_divider_ready(
        &mut self,
        _: &DrawnDivider,
    ) -> Result<(), crate::client_view::Notice> {
        Ok(())
    }
}
fn fixture() -> (InputDispatcher, ClientView, Port) {
    let source = DividerSource {
        session: SessionId::parse(&format!("{:032x}", 1)).unwrap(),
        context: WorkbenchTerminalContextId::parse(&format!("{:032x}", 2)).unwrap(),
        tab: WorkbenchTabId::parse(&format!("{:032x}", 3)).unwrap(),
        focus: WorkbenchPanelId::parse(&format!("{:032x}", 4)).unwrap(),
        revision: WorkbenchRevision::new(1),
    };
    let target = DrawnDivider {
        source,
        viewport: Rect::new(0, 0, 120, 40),
        split: WorkbenchSplitId::parse(&format!("{:032x}", 5)).unwrap(),
        first: WorkbenchPanelSlotId::parse(&format!("{:032x}", 6)).unwrap(),
        second: WorkbenchPanelSlotId::parse(&format!("{:032x}", 7)).unwrap(),
        orientation: PanelOrientation::Horizontal,
        ratio: ClientSplitRatio::default(),
        area: Rect::new(0, 0, 101, 30),
        divider: Rect::new(50, 0, 1, 30),
    };
    let mut hits = HitMapBuilder::new();
    hits.record(
        HitLayer::Foreground,
        target.divider,
        HitTarget::Divider(target),
    );
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    let mut port = Port {
        bytes: Vec::new(),
        submissions: 0,
        menus: 0,
    };
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher.publish_hit_map(hits.finish());
    (dispatcher, view, port)
}

#[test]
fn divider_decoded_escape_cancels_and_fences_only_the_late_release_packet() {
    let (mut dispatcher, mut view, mut port) = fixture();
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;51;6M\x1b[<32;61;6M",
    );
    assert!(dispatcher.divider_preview().is_some());
    assert_eq!(port.submissions, 0);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b");
    dispatcher
        .synchronize(
            &mut view,
            &mut port,
            Instant::now() + Duration::from_secs(1),
            &mut |_| Ok::<(), io::Error>(()),
        )
        .unwrap();
    assert!(dispatcher.divider_preview().is_none());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;61;6mx");
    assert!(port.bytes.is_empty());
    assert_eq!(port.submissions, 0);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"fresh");
    assert_eq!(port.bytes, b"fresh");
}

#[test]
fn divider_decoded_release_is_once_and_every_transport_boundary_cancels() {
    for ending in 0..5 {
        let (mut dispatcher, mut view, mut port) = fixture();
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            b"\x1b[<0;51;6M\x1b[<32;61;6M",
        );
        match ending {
            0 => {}
            1 => dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02?"),
            2 => dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<36;61;6M"),
            3 => dispatcher.invalidate_hit_map(&mut port),
            _ => {
                dispatcher.set_pointer_capture(&mut view, &mut port, false);
                dispatcher.set_pointer_capture(&mut view, &mut port, true);
            }
        }
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            b"\x1b[<0;61;6m\x1b[<0;61;6mtail",
        );
        assert_eq!(port.submissions, usize::from(ending == 0));
        assert!(port.bytes.is_empty());
        assert!(dispatcher.divider_preview().is_none());
    }
}

#[test]
fn divider_focus_loss_rolls_back_and_late_release_fences_only_its_packet() {
    let (mut dispatcher, mut view, mut port) = fixture();
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;51;6M\x1b[<32;61;6M",
    );
    assert!(dispatcher.divider_preview().is_some());

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[O");
    assert!(dispatcher.divider_preview().is_none());
    assert_eq!(port.submissions, 0);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[I");
    assert!(dispatcher.divider_preview().is_none());

    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;61;6mtail\x1b[200~paste\x1b[201~\x1b[",
    );
    assert_eq!(port.submissions, 0);
    assert!(port.bytes.is_empty());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"A");
    assert!(port.bytes.is_empty());
    dispatcher.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"fresh");
    assert_eq!(port.bytes, b"fresh");
}

#[test]
fn divider_menu_chord_and_direct_binding_refuse_without_canceling_the_drag() {
    use orchbee_local_runtime::{KeyChord, KeyOverride, KeySettings, KeyTrigger};

    for binding in 0..3 {
        let (mut dispatcher, mut view, mut port) = fixture();
        let prefix: &[u8] = match binding {
            0 => b"\x02",
            1 => b"\x01",
            _ => b"",
        };
        let mut settings = KeySettings::default();
        if binding == 1 {
            settings.prefix.key = "a".into();
        } else if binding == 2 {
            settings.overrides.push(KeyOverride {
                action: "menu",
                triggers: vec![KeyTrigger {
                    prefix: false,
                    chord: KeyChord {
                        key: "f2".into(),
                        ctrl: false,
                        alt: false,
                        shift: false,
                        super_key: false,
                    },
                }],
            });
        }
        view.configure_keys(&settings);
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            b"\x1b[<0;51;6M\x1b[<32;61;6M",
        );
        let preview = |dispatcher: &InputDispatcher| {
            dispatcher
                .divider_preview()
                .map(|drag| (drag.target.clone(), drag.ratio, drag.preview))
        };
        let original = preview(&dispatcher).unwrap();
        if !prefix.is_empty() {
            dispatch_all(&mut dispatcher, &mut view, &mut port, prefix);
            assert!(view.prefix_active());
            assert_eq!(preview(&dispatcher).as_ref(), Some(&original));
        }
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1bOQ");
        assert!(!view.prefix_active());
        assert_eq!(port.menus, 0);
        assert_eq!(preview(&dispatcher).as_ref(), Some(&original));
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            b"\x1b[<0;61;6m\x1b[<0;61;6m",
        );
        assert_eq!(port.submissions, 1);
        assert!(dispatcher.divider_preview().is_none());
        assert!(port.bytes.is_empty());
        // The identical binding is admitted once the genuine gesture ends.
        dispatch_all(&mut dispatcher, &mut view, &mut port, prefix);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1bOQ");
        assert_eq!(port.menus, 1);
    }
}
