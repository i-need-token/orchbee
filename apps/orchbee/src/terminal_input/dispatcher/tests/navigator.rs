use super::*;
use crate::client_view::navigator::{Admission, Catalog, Row, Source, Target};
use crate::terminal_session::NativeContextLocation;
use crate::tui_pointer::{
    HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, PrimaryContract,
};
use orchbee_workbench_protocol::{
    SiteId, WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId,
};
use ratatui::layout::Rect;

struct Port {
    calls: Rc<RefCell<Vec<Call>>>,
    catalog: Catalog,
    attempts: Vec<Row>,
    refuse: bool,
}
impl Port {
    fn ready() -> Self {
        let location = NativeContextLocation::Standalone(
            WorkbenchTerminalContextId::parse(&format!("{:032x}", 1)).unwrap(),
        );
        let tabs: Vec<_> = (1..=20)
            .map(|id| WorkbenchTabId::parse(&format!("{id:032x}")).unwrap())
            .collect();
        Self {
            calls: Rc::new(RefCell::new(Vec::new())),
            attempts: Vec::new(),
            refuse: false,
            catalog: Catalog {
                source: Source {
                    site: SiteId::parse(&format!("{:032x}", 1)).unwrap(),
                    revision: WorkbenchRevision::new(1),
                    location: location.clone(),
                },
                locations: vec![location],
                current_tab: Some(tabs[0].clone()),
                tabs,
            },
        }
    }
}
impl TerminalInputPort for Port {
    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.calls.borrow_mut().push(Call::Forward(bytes.to_vec()));
        Ok(())
    }
    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        self.calls.borrow_mut().push(Call::Action(action));
        Some(SessionActionDisposition::KeepNavigator)
    }
    fn navigator_catalog(&self) -> Option<Catalog> {
        Some(self.catalog.clone())
    }
    fn activate_navigator(&mut self, row: &Row) -> Admission {
        self.attempts.push(row.clone());
        if self.refuse {
            Admission::Refused
        } else if Some(&row.target)
            == self
                .catalog
                .current_tab
                .as_ref()
                .map(|tab| Target::Tab(tab.clone()))
                .as_ref()
        {
            Admission::ExactCurrent
        } else {
            Admission::Admitted(1)
        }
    }
}
fn paint(dispatcher: &mut InputDispatcher, view: &ClientView) {
    let mut hits = HitMapBuilder::with_pointer(dispatcher.pointer_projection());
    hits.mark_overlay_active();
    let source = view.navigator.draw_source();
    hits.start_navigator(source.clone(), view.navigator.focus.clone(), 5);
    if let Some(source) = source {
        let rect = Rect::new(38, 2, 3, 1);
        hits.record_interaction(
            HitLayer::Overlay,
            rect,
            rect,
            InteractionTarget::NavigatorClose(source),
            InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
        );
    }
    for (rect, tabs) in [
        (Rect::new(2, 3, 20, 5), true),
        (Rect::new(22, 3, 20, 5), false),
    ] {
        if let Some(row) = view.navigator.scroll_row(tabs) {
            hits.record(
                HitLayer::OverlaySurface,
                rect,
                HitTarget::ListScroll(crate::tui_pointer::ListScrollTarget::Navigator(row)),
            );
        }
    }
    hits.record(
        HitLayer::OverlaySurface,
        Rect::new(2, 2, 40, 8),
        HitTarget::OverlaySurface,
    );
    let rows = view.navigator.catalog.as_ref().unwrap();
    for (index, tab) in rows.tabs.iter().take(3).enumerate() {
        hits.record_navigator_row(
            Rect::new(2, 3 + index as u16, 20, 1),
            view.navigator.row(Target::Tab(tab.clone())).unwrap(),
        );
    }
    dispatcher.publish_hit_map(hits.finish());
}
fn opened() -> (InputDispatcher, ClientView, Port) {
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(80);
    let mut port = Port::ready();
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02wopening-tail");
    assert!(view.navigator_active());
    paint(&mut dispatcher, &view);
    (dispatcher, view, port)
}

#[test]
fn navigator_wheel_stays_in_its_column_and_never_activates_then_close_restores_input() {
    let (mut d, mut v, mut p) = opened();
    dispatch_all(
        &mut d,
        &mut v,
        &mut p,
        b"\x1b[<65;3;4M\x1b[<65;3;4M\x1b[<65;3;4M",
    );
    assert_eq!(
        v.navigator.focus,
        Some(Target::Tab(p.catalog.tabs[5].clone()))
    );
    assert!(p.attempts.is_empty());
    paint(&mut d, &v);
    dispatch_all(&mut d, &mut v, &mut p, b"\x1b[<64;3;4M");
    assert_eq!(
        v.navigator.focus,
        Some(Target::Tab(p.catalog.tabs[2].clone()))
    );
    paint(&mut d, &v);
    dispatch_all(&mut d, &mut v, &mut p, b"\x1b[<65;23;4M");
    assert_eq!(
        v.navigator.focus,
        Some(Target::Location(p.catalog.source.location.clone()))
    );
    paint(&mut d, &v);
    dispatch_all(&mut d, &mut v, &mut p, b"\x1b[<65;1;1M");
    assert!(v.navigator_active(), "outside wheel cannot dismiss");
    dispatch_all(&mut d, &mut v, &mut p, b"\x1b[<0;39;3M\x1b[<0;39;3mtail");
    assert!(!v.navigator_active());
    assert!(p.attempts.is_empty());
    d.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut d, &mut v, &mut p, b"fresh");
    assert_eq!(forwarded_bytes(&p.calls), b"fresh");
}
fn escape(dispatcher: &mut InputDispatcher, view: &mut ClientView, port: &mut Port) {
    dispatch_all(dispatcher, view, port, b"\x1b");
    dispatcher
        .synchronize(
            view,
            port,
            Instant::now() + Duration::from_secs(1),
            &mut |_| Ok::<_, io::Error>(()),
        )
        .unwrap();
}

#[test]
fn enter_requires_focused_fresh_paint_and_acceptance_is_one_shot() {
    let (mut d, mut v, mut p) = opened();
    dispatch_all(&mut d, &mut v, &mut p, b"\x1b[B\r");
    assert!(
        p.attempts.is_empty(),
        "moving invalidates the previous focused receipt"
    );
    paint(&mut d, &v);
    dispatch_all(&mut d, &mut v, &mut p, b"\r\rhidden\x03");
    assert_eq!(p.attempts.len(), 1);
    assert_eq!(p.attempts[0].target, Target::Tab(p.catalog.tabs[1].clone()));
    assert!(!v.navigator_active());
    dispatch_all(&mut d, &mut v, &mut p, b"polled-before-paint");
    assert!(forwarded_bytes(&p.calls).is_empty());
    d.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut d, &mut v, &mut p, b"fresh");
    assert_eq!(forwarded_bytes(&p.calls), b"fresh");
    assert_eq!(
        p.calls
            .borrow()
            .iter()
            .filter(|call| matches!(call, Call::Action(_)))
            .count(),
        1
    );
}

#[test]
fn pointer_uses_exact_other_row_and_owns_late_release_beyond_repaint() {
    let (mut d, mut v, mut p) = opened();
    dispatch_all(&mut d, &mut v, &mut p, b"\x1b[<0;3;5M");
    assert!(p.attempts.is_empty(), "press only arms the painted row");
    assert!(v.navigator_active());
    paint(&mut d, &v);
    dispatch_all(&mut d, &mut v, &mut p, b"\x1b[<0;3;5mtail");
    assert_eq!(p.attempts[0].target, Target::Tab(p.catalog.tabs[1].clone()));
    assert_eq!(d.navigator.release, None);
    d.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut d, &mut v, &mut p, b"fresh");
    assert_eq!(p.attempts.len(), 1);
    assert_eq!(forwarded_bytes(&p.calls), b"fresh");
}

#[test]
fn navigator_source_replacement_cancels_an_armed_row_without_rehitting() {
    let (mut d, mut v, mut p) = opened();
    dispatch_all(&mut d, &mut v, &mut p, b"\x1b[<0;3;5M");
    assert!(p.attempts.is_empty());

    p.catalog.source.revision = WorkbenchRevision::new(2);
    d.synchronize(&mut v, &mut p, Instant::now(), &mut |_| {
        Ok::<_, io::Error>(())
    })
    .unwrap();
    paint(&mut d, &v);
    dispatch_all(&mut d, &mut v, &mut p, b"\x1b[<0;3;5mtail");

    assert!(v.navigator_active());
    assert!(p.attempts.is_empty());
    assert!(forwarded_bytes(&p.calls).is_empty());
}

#[test]
fn backdrop_variants_no_click_through_and_new_press_or_focus_loss_releases_ownership() {
    for (press, closes) in [
        (&b"\x1b[<64;1;1M"[..], false),
        (b"\x1b[<4;1;1M", false),
        (b"\x1b[<2;3;5M", false),
        (b"\x1b[<16;3;5M", false),
        (b"\x1b[<2;1;1M", true),
        (b"\x1b[<16;1;1M", true),
    ] {
        let (mut d, mut v, mut p) = opened();
        dispatch_all(&mut d, &mut v, &mut p, press);
        assert_eq!(!v.navigator_active(), closes);
        assert!(p.attempts.is_empty());
        assert!(forwarded_bytes(&p.calls).is_empty());
    }
    for clear in [b"\x1b[<0;9;9M".as_slice(), b"\x1b[O".as_slice()] {
        let (mut d, mut v, mut p) = opened();
        dispatch_all(&mut d, &mut v, &mut p, b"\x1b[<2;1;1M");
        d.publish_hit_map(HitMapBuilder::new().finish());
        dispatch_all(&mut d, &mut v, &mut p, clear);
        assert_eq!(d.navigator.release, None);
    }
}

#[test]
fn refresh_revokes_paint_refusal_stays_and_focus_loss_cancels_without_replacement() {
    let (mut d, mut v, mut p) = opened();
    p.catalog.source.revision = WorkbenchRevision::new(2);
    dispatch_all(&mut d, &mut v, &mut p, b"\r\x1b[<0;3;4M");
    assert!(p.attempts.is_empty());
    assert!(v.navigator_active());
    paint(&mut d, &v);
    p.refuse = true;
    dispatch_all(&mut d, &mut v, &mut p, b"\r");
    assert!(v.navigator_active());
    assert_eq!(p.attempts.len(), 1);
    p.catalog.tabs.remove(0);
    dispatch_all(&mut d, &mut v, &mut p, b"must-not-reach-shell");
    assert!(!v.navigator_active());
    assert!(forwarded_bytes(&p.calls).is_empty());
}

#[test]
fn captured_utf8_csi_and_paste_fragments_cannot_escape_after_retirement() {
    for (start, end) in [
        (&b"\xe4"[..], &b"\xb8\xad"[..]),
        (b"\x1b[", b"A"),
        (b"\x1b[200~partial", b"tail\x1b[201~"),
    ] {
        let (mut d, mut v, mut p) = opened();
        dispatch_all(&mut d, &mut v, &mut p, start);
        v.dismiss_overlay();
        d.synchronize(&mut v, &mut p, Instant::now(), &mut |_| {
            Ok::<_, io::Error>(())
        })
        .unwrap();
        d.publish_hit_map(HitMapBuilder::new().finish());
        dispatch_all(&mut d, &mut v, &mut p, end);
        assert!(forwarded_bytes(&p.calls).is_empty());
        dispatch_all(&mut d, &mut v, &mut p, b"fresh");
        assert_eq!(forwarded_bytes(&p.calls), b"fresh");
    }
}

#[test]
fn local_prefix_unknown_double_prefix_paste_and_escape_never_dispatch_actions() {
    let (mut d, mut v, mut p) = opened();
    dispatch_all(
        &mut d,
        &mut v,
        &mut p,
        b"\x02\x02\x02c\x1b[200~\x02w\x1b[201~\x02",
    );
    assert!(v.navigator_active());
    escape(&mut d, &mut v, &mut p);
    assert!(!v.navigator_active());
    assert!(!v.prefix_active());
    assert!(forwarded_bytes(&p.calls).is_empty());
    assert_eq!(
        p.calls
            .borrow()
            .iter()
            .filter(|call| matches!(call, Call::Action(_)))
            .count(),
        1
    );
}

#[test]
fn stale_resize_and_unpresented_frame_never_authorize_or_leave_invisible_capture() {
    let (mut d, mut v, mut p) = opened();
    d.invalidate_hit_map(&mut p);
    dispatch_all(&mut d, &mut v, &mut p, b"\x1b[<0;3;4M\r");
    assert!(p.attempts.is_empty());
    d.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut d, &mut v, &mut p, b"stale");
    assert!(!v.navigator_active());
    assert!(forwarded_bytes(&p.calls).is_empty());
}

#[test]
fn paste_lease_failure_prevents_navigator_and_refresh_admission() {
    let mut d = InputDispatcher::default();
    let mut v = ClientView::new(80);
    let mut p = Port::ready();
    let result = d.dispatch(
        &mut v,
        b"\x02w",
        &mut p,
        Instant::now(),
        |_, _| Ok::<_, io::Error>(false),
        &mut |mode| {
            if matches!(mode, ModeRequest::BracketedPaste(true)) {
                Err(io::Error::other("lease refused"))
            } else {
                Ok(())
            }
        },
    );
    assert!(result.is_err());
    assert!(!v.navigator_active());
    assert!(p.calls.borrow().is_empty());
}

#[test]
fn remapped_direct_and_disabled_toggles_stay_beneath_local_capture() {
    use orchbee_local_site_runtime::{
        WindowsKeyChord, WindowsKeyOverride, WindowsKeySettings, WindowsKeyTrigger,
    };
    let chord = |key: &str, ctrl| WindowsKeyChord {
        key: key.into(),
        ctrl,
        alt: false,
        shift: false,
        super_key: false,
    };
    for (prefix, triggers, close) in [
        (
            true,
            vec![WindowsKeyTrigger {
                prefix: true,
                chord: chord("n", false),
            }],
            &b"\x01n"[..],
        ),
        (
            false,
            vec![WindowsKeyTrigger {
                prefix: false,
                chord: chord("f1", false),
            }],
            &b"\x1bOP"[..],
        ),
        (false, Vec::new(), &b"\x01w"[..]),
    ] {
        let (mut d, mut v, mut p) = opened();
        let disabled = triggers.is_empty();
        v.configure_keys(&WindowsKeySettings {
            commands: Vec::new(),
            prefix: chord("a", true),
            overrides: vec![WindowsKeyOverride {
                action: "workspace_picker",
                triggers,
            }],
        });
        if !prefix && !disabled {
            dispatch_all(&mut d, &mut v, &mut p, b"\x01\x1bOP");
            assert!(
                v.navigator_active(),
                "direct toggle cannot escape an armed local prefix"
            );
        }
        dispatch_all(&mut d, &mut v, &mut p, close);
        assert_eq!(v.navigator_active(), disabled);
        if disabled {
            escape(&mut d, &mut v, &mut p);
            assert!(!v.navigator_active());
        }
        assert!(forwarded_bytes(&p.calls).is_empty());
        assert_eq!(
            p.calls
                .borrow()
                .iter()
                .filter(|call| matches!(call, Call::Action(_)))
                .count(),
            1
        );
    }
}
