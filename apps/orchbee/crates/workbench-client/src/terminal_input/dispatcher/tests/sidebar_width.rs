//! Painted keyboard/drag decisions share layout and packet fences.
use super::*;
use crate::{
    client_view::ClientActionDisposition,
    tui_hit_map::{
        FrameHitMap, GlobalChromeAction, HitLayer, HitMapBuilder, HitTarget, InteractionSpec,
        InteractionTarget, PrimaryContract,
    },
    tui_layout::{DrawnSidebarWidth, SidebarWidth},
};
use ratatui::layout::Rect;

#[derive(Default)]
struct Port {
    bytes: Vec<u8>,
    settings: usize,
}
impl TerminalInputPort for Port {
    fn apply_client_action(
        &mut self,
        command: crate::workbench_client::InputAction,
    ) -> Option<ClientActionDisposition> {
        let action = command.intent();
        if action == ClientAction::OpenSettings {
            self.settings += 1;
        }
        Some(ClientActionDisposition::KeepNavigator)
    }
    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
}
fn fixture() -> (InputDispatcher, ClientView, Port) {
    let mut view = ClientView::new(120);
    view.machines.available = true;
    view.machines.observe_agent_sidebar(true);
    (InputDispatcher::default(), view, Port::default())
}
fn map(view: &ClientView) -> FrameHitMap {
    let target = DrawnSidebarWidth::new(Rect::new(0, 0, 120, 40), view.sidebar_surface()).unwrap();
    let mut hits = HitMapBuilder::new();
    hits.record(
        HitLayer::Chrome,
        target.edge(),
        HitTarget::SidebarResize(target),
    );
    hits.finish()
}

#[test]
fn sidebar_width_keyboard_requires_paint_but_not_mouse_capture_and_fences_geometry_tail() {
    let (mut dispatcher, mut view, mut port) = fixture();
    dispatcher.set_pointer_capture(&mut view, &mut port, false);
    assert!(view.machines.enter_sidebar_navigation());
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[1;2C");
    assert_eq!(
        view.machines.sidebar_width(),
        SidebarWidth::DEFAULT,
        "no successful frame"
    );
    dispatcher.publish_hit_map(map(&view));
    let mut layouts = Vec::new();
    dispatcher
        .dispatch(
            &mut view,
            b"\x1b[1;2C\rtail\x1b[",
            &mut port,
            Instant::now(),
            |view, _| {
                layouts.push(view.machines.sidebar_width().columns());
                Ok::<bool, io::Error>(false)
            },
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(layouts, [32]);
    assert!(view.machines.sidebar_navigation_active());
    assert!(view.machines.request.is_none());
    assert!(port.bytes.is_empty());
    dispatcher.publish_hit_map(map(&view));
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"1;2C");
    assert_eq!(
        view.machines.sidebar_width().columns(),
        32,
        "old split key cannot resize after paint"
    );
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"0");
    assert_eq!(view.machines.sidebar_width(), SidebarWidth::DEFAULT);
    assert!(!dispatcher.pointer_capture_enabled());
    assert!(port.bytes.is_empty());
}

#[test]
fn sidebar_width_release_commits_layout_once_and_releases_fresh_input_after_paint() {
    for tail in [false, true] {
        let (mut dispatcher, mut view, mut port) = fixture();
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatcher.publish_hit_map(map(&view));
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            b"\x1b[<0;30;5M\x1b[<32;51;5M",
        );
        assert!(dispatcher.sidebar_width_preview().is_some());
        assert_eq!(view.machines.sidebar_width(), SidebarWidth::DEFAULT);
        let mut observed = Vec::new();
        let packet = if tail {
            b"\x1b[<0;36;5m\rtail\x1b[".as_slice()
        } else {
            b"\x1b[<0;36;5m".as_slice()
        };
        dispatcher
            .dispatch(
                &mut view,
                packet,
                &mut port,
                Instant::now(),
                |view, _| {
                    observed.push(view.machines.sidebar_width().columns());
                    Ok::<bool, io::Error>(false)
                },
                &mut |_| Ok(()),
            )
            .unwrap();
        assert_eq!(observed, [36]);
        assert!(dispatcher.sidebar_width_preview().is_none());
        assert!(view.machines.request.is_none());
        assert!(port.bytes.is_empty());
        dispatcher.publish_hit_map(map(&view));
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            if tail { b"13ufresh" } else { b"fresh" },
        );
        assert_eq!(port.bytes, b"fresh");
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;60;5m");
        assert_eq!(view.machines.sidebar_width().columns(), 36);
    }
}

#[test]
fn sidebar_width_escape_and_competing_action_cancel_without_changing_preference() {
    for end in [b"\x1b[27u".as_slice(), b"\x02?", b"\x1b[<36;51;5M"] {
        let (mut dispatcher, mut view, mut port) = fixture();
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatcher.publish_hit_map(map(&view));
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            b"\x1b[<0;30;5M\x1b[<32;51;5M",
        );
        dispatch_all(&mut dispatcher, &mut view, &mut port, end);
        assert!(dispatcher.sidebar_width_preview().is_none());
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;51;5m");
        assert_eq!(view.machines.sidebar_width(), SidebarWidth::DEFAULT);
        assert!(port.bytes.is_empty());
    }
}

#[test]
fn sidebar_width_focus_loss_rolls_back_and_fences_the_late_release_packet() {
    let (mut dispatcher, mut view, mut port) = fixture();
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher.publish_hit_map(map(&view));
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;30;5M\x1b[<32;51;5M",
    );
    assert!(dispatcher.sidebar_width_preview().is_some());

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[O\x1b[I");
    assert!(dispatcher.sidebar_width_preview().is_none());
    assert_eq!(view.machines.sidebar_width(), SidebarWidth::DEFAULT);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;51;5mtail\x1b[200~paste\x1b[201~\x1b[",
    );
    assert_eq!(view.machines.sidebar_width(), SidebarWidth::DEFAULT);
    assert!(port.bytes.is_empty());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"A");
    assert!(port.bytes.is_empty());
    dispatcher.publish_hit_map(map(&view));
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"fresh");
    assert_eq!(port.bytes, b"fresh");
}

#[test]
fn sidebar_restore_uses_the_existing_toggle_on_exact_release() {
    let (mut dispatcher, mut view, mut port) = fixture();
    view.commit(
        view.decide_action(ClientAction::ToggleBusinessSidebar),
        ClientActionDisposition::KeepNavigator,
    );
    assert_eq!(
        view.sidebar_surface(),
        crate::client_view::SidebarSurface::Hidden
    );
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let rect = Rect::new(100, 39, 8, 1);
    let mut hits = HitMapBuilder::with_pointer(dispatcher.pointer_projection());
    hits.record_interaction(
        HitLayer::Foreground,
        rect,
        rect,
        InteractionTarget::GlobalChrome(GlobalChromeAction::RestoreSidebar),
        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
    );
    dispatcher.publish_hit_map(hits.finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;101;40M");
    assert_eq!(
        view.sidebar_surface(),
        crate::client_view::SidebarSurface::Hidden,
        "press only arms the restore control"
    );
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;101;40mtail",
    );
    assert_ne!(
        view.sidebar_surface(),
        crate::client_view::SidebarSurface::Hidden
    );
    assert!(!view.machines.sidebar_navigation_active());
    assert!(view.machines.request.is_none());
    assert!(port.bytes.is_empty());
}

#[test]
fn sidebar_restore_release_revalidates_machine_availability() {
    let (mut dispatcher, mut view, mut port) = fixture();
    view.commit(
        view.decide_action(ClientAction::ToggleBusinessSidebar),
        ClientActionDisposition::KeepNavigator,
    );
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let rect = Rect::new(100, 39, 8, 1);
    let mut hits = HitMapBuilder::with_pointer(dispatcher.pointer_projection());
    hits.record_interaction(
        HitLayer::Foreground,
        rect,
        rect,
        InteractionTarget::GlobalChrome(GlobalChromeAction::RestoreSidebar),
        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
    );
    dispatcher.publish_hit_map(hits.finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;101;40M");
    view.machines.available = false;
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;101;40mtail",
    );
    assert_eq!(
        view.sidebar_surface(),
        crate::client_view::SidebarSurface::Hidden
    );
    assert!(port.bytes.is_empty());
}

#[test]
fn sidebar_width_end_keeps_packet_input_owned_until_canonical_paint() {
    use crate::{localization::Locale, tui_theme::Theme};
    use ratatui::{Terminal, backend::TestBackend};
    for navigation in [false, true] {
        for ending in [b"\x1b[27u".as_slice(), b"\x1b[<0;30;5m"] {
            let (mut dispatcher, mut view, mut port) = fixture();
            dispatcher.set_pointer_capture(&mut view, &mut port, true);
            if navigation {
                assert!(view.machines.enter_sidebar_navigation());
                dispatcher
                    .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
                        Ok::<(), io::Error>(())
                    })
                    .unwrap();
            }
            dispatcher.publish_hit_map(map(&view));
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;30;5M");
            let drag = *dispatcher.sidebar_width_preview().unwrap();
            let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
            let mut hits = HitMapBuilder::new();
            let theme = Theme::terminal_native();
            terminal
                .draw(|frame| {
                    crate::tui_render::render_terminal_shell(
                        frame,
                        &view,
                        Locale::EnUs,
                        None,
                        &theme,
                        &mut hits,
                    );
                    assert!(crate::tui_render::render_width_preview(
                        frame, &view, &drag, &theme, &hits
                    ));
                })
                .unwrap();
            dispatcher.publish_hit_map(hits.finish());
            let footer: String = (0..28)
                .map(|x| terminal.backend().buffer()[(x, 39)].symbol())
                .collect();
            assert!(footer.contains(Locale::EnUs.settings_title()));
            let focused = view.machines.sidebar_focused_target().cloned();
            let mut packet = ending.to_vec();
            packet.extend_from_slice(b"\x1b[<0;3;40M");
            dispatch_all(&mut dispatcher, &mut view, &mut port, &packet);
            assert_eq!(port.settings, 0);
            assert_eq!(view.machines.sidebar_width(), SidebarWidth::DEFAULT);
            assert_eq!(view.machines.sidebar_navigation_active(), navigation);
            assert_eq!(view.machines.sidebar_focused_target(), focused.as_ref());
            assert!(view.machines.request.is_none());
            assert!(port.bytes.is_empty());
            // Rebuild the canonical map after the preview has ended.
            let mut hits = HitMapBuilder::new();
            terminal
                .draw(|frame| {
                    crate::tui_render::render_terminal_shell(
                        frame,
                        &view,
                        Locale::EnUs,
                        None,
                        &theme,
                        &mut hits,
                    );
                })
                .unwrap();
            dispatcher.publish_hit_map(hits.finish());
            dispatch_all(
                &mut dispatcher,
                &mut view,
                &mut port,
                b"\x1b[<0;3;40M\x1b[<0;3;40m",
            );
            assert_eq!(port.settings, 1);
        }
    }
}
