use super::*;
use crate::{
    client_view::Modifiers,
    terminal_input::{MouseAction, MouseButton, MouseEvent},
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
};
use ratatui::{layout::Rect, widgets::Paragraph};

fn navigation_view(columns: u16) -> (ClientView, crate::machine_picker::SidebarTarget) {
    use orchbee_workbench_protocol::*;
    let contexts: Vec<_> = [1, 2]
        .into_iter()
        .map(|id| crate::machine_discovery::Context {
            target: crate::terminal_session::NativeContextTarget {
                site_id: SiteId::parse(&format!("{:032x}", 1)).unwrap(),
                location: crate::terminal_session::NativeContextLocation::Workspace {
                    project_id: WorkbenchProjectId::parse(&format!("{:032x}", 2)).unwrap(),
                    workspace_id: WorkbenchWorkspaceId::parse(&format!("{id:032x}")).unwrap(),
                    terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{id:032x}"))
                        .unwrap(),
                },
            },
            project: Some("Project".into()),
            workspace: Some(format!("Workspace {id}")),
        })
        .collect();
    let current = contexts[0].target.clone();
    let target = crate::machine_picker::SidebarTarget::Context {
        machine: crate::machine_picker::Choice::Local,
        target: contexts[1].target.clone(),
    };
    let mut view = ClientView::new(columns);
    view.machines.available = true;
    view.machines.observe_local(contexts);
    view.machines
        .observe_focus(crate::machine_picker::Choice::Local, Some(current), None);
    (view, target)
}

#[test]
fn sidebar_hidden_layout_retires_navigation_before_already_read_enter_or_split_csi() {
    use crate::terminal_input::InputDispatcher;
    for resized in [dimensions(70, 24), dimensions(120, 6), dimensions(50, 24)] {
        for split in [false, true] {
            let initial = dimensions(120, 24);
            let mut size = initial;
            let mut terminal = test_terminal(initial);
            let viewport = QueueViewportSource {
                dimensions: std::cell::RefCell::new(VecDeque::from([
                    Ok(Some(initial)),
                    Ok(Some(resized)),
                ])),
            };
            let (mut view, target) = navigation_view(initial.columns);
            let mut surface = OrderedSurface::default();
            super::super::synchronize_viewport(
                &mut terminal,
                &viewport,
                &mut view,
                &mut surface,
                &mut size,
                None,
            )
            .unwrap();
            assert!(view.machines.enter_sidebar_navigation());
            assert!(view.machines.focus_sidebar_target(&target));
            let mut dispatcher = InputDispatcher::default();
            let now = Instant::now();
            dispatcher
                .synchronize(&mut view, &mut surface, now, &mut |_| {
                    Ok::<(), io::Error>(())
                })
                .unwrap();
            dispatcher.publish_hit_map(HitMapBuilder::new().finish());
            if split {
                dispatcher
                    .dispatch(
                        &mut view,
                        b"\x1b[",
                        &mut surface,
                        now,
                        |_, _| Ok::<(), io::Error>(()),
                        &mut |_| Ok(()),
                    )
                    .unwrap();
            }
            // The real shell samples layout after reading bytes and before dispatch.
            assert!(
                super::super::synchronize_viewport(
                    &mut terminal,
                    &viewport,
                    &mut view,
                    &mut surface,
                    &mut size,
                    None
                )
                .unwrap()
            );
            dispatcher.invalidate_hit_map(&mut surface);
            assert!(!view.machines.sidebar_navigation_active());
            dispatcher
                .dispatch(
                    &mut view,
                    if split { b"13utail" } else { b"\rtail" },
                    &mut surface,
                    now,
                    |_, _| Ok::<(), io::Error>(()),
                    &mut |_| Ok(()),
                )
                .unwrap();
            assert!(view.machines.request.is_none());
            assert!(
                !surface
                    .events
                    .iter()
                    .any(|event| matches!(event, SurfaceEvent::Forward(_)))
            );
            dispatcher.publish_hit_map(HitMapBuilder::new().finish());
            dispatcher
                .dispatch(
                    &mut view,
                    b"fresh",
                    &mut surface,
                    now,
                    |_, _| Ok::<(), io::Error>(()),
                    &mut |_| Ok(()),
                )
                .unwrap();
            let forwarded: Vec<_> = surface
                .events
                .iter()
                .filter_map(|event| match event {
                    SurfaceEvent::Forward(bytes) => Some(bytes.as_slice()),
                    _ => None,
                })
                .collect();
            assert_eq!(forwarded, [b"fresh".as_slice()]);
        }
    }
}

#[test]
fn sidebar_backdrop_press_retires_before_enter_and_keeps_split_fragment_owned_after_draw() {
    use crate::terminal_input::InputDispatcher;
    for button in [0, 1, 2, 64, 65] {
        let initial = dimensions(70, 24);
        let mut size = initial;
        let mut terminal = test_terminal(initial);
        let viewport = QueueViewportSource {
            dimensions: std::cell::RefCell::new(VecDeque::from([Ok(Some(initial)), Ok(None)])),
        };
        let (mut view, target) = navigation_view(initial.columns);
        view.commit(
            view.decide_action(ClientAction::FocusBusinessSidebar),
            SessionActionDisposition::KeepNavigator,
        );
        let mut surface = OrderedSurface::default();
        super::super::synchronize_viewport(
            &mut terminal,
            &viewport,
            &mut view,
            &mut surface,
            &mut size,
            None,
        )
        .unwrap();
        assert!(view.machines.focus_sidebar_target(&target));
        let mut dispatcher = InputDispatcher::default();
        let now = Instant::now();
        dispatcher.set_pointer_capture(&mut view, &mut surface, true);
        dispatcher
            .synchronize(&mut view, &mut surface, now, &mut |_| {
                Ok::<(), io::Error>(())
            })
            .unwrap();
        let mut hits = HitMapBuilder::new();
        hits.mark_overlay_active();
        hits.record(
            HitLayer::OverlaySurface,
            Rect::new(0, 0, 30, 23),
            HitTarget::OverlaySurface,
        );
        dispatcher.publish_hit_map(hits.finish());
        let packet = format!("\x1b[<{button};51;2M\rtail\x1b[");
        dispatcher
            .dispatch(
                &mut view,
                packet.as_bytes(),
                &mut surface,
                now,
                |view, surface| {
                    super::super::synchronize_viewport(
                        &mut terminal,
                        &viewport,
                        view,
                        surface,
                        &mut size,
                        None,
                    )
                    .map(|_| ())
                },
                &mut |_| Ok(()),
            )
            .unwrap();
        assert_eq!(view.overlay(), None);
        assert!(!view.machines.sidebar_navigation_active());
        assert!(view.machines.request.is_none());
        assert!(
            !surface
                .events
                .iter()
                .any(|event| matches!(event, SurfaceEvent::Forward(_)))
        );
        dispatcher.publish_hit_map(HitMapBuilder::new().finish());
        dispatcher
            .dispatch(
                &mut view,
                b"13ufresh",
                &mut surface,
                now,
                |_, _| Ok::<(), io::Error>(()),
                &mut |_| Ok(()),
            )
            .unwrap();
        let forwarded: Vec<_> = surface
            .events
            .iter()
            .filter_map(|event| match event {
                SurfaceEvent::Forward(bytes) => Some(bytes.as_slice()),
                _ => None,
            })
            .collect();
        assert_eq!(forwarded, [b"fresh".as_slice()]);
    }
}

#[test]
fn action_menu_status_button_remains_visible_and_does_not_open_the_machine_picker() {
    for width in [24, 32, 50, 80, 120] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let height = 24;
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut view = ClientView::new(width);
            view.machines.available = true;
            let theme = crate::tui_theme::Theme::terminal_native();
            let chrome = super::super::Chrome {
                locale,
                theme: &theme,
            };
            let button = Rect::new(width - 6, height - 1, 6, 1);
            let mut hits = HitMapBuilder::new();
            hits.record(HitLayer::Foreground, button, HitTarget::ActionMenuButton);
            let status = Rect::new(0, height - 1, width - 7, 1);
            let indicator = view.machines.indicator_area(locale, status);
            hits.record_machine_indicator(indicator);
            terminal
                .draw(|frame| {
                    frame.render_widget(Paragraph::new(locale.menu_button()), button);
                    super::super::render_machine_surface(frame, &mut view, chrome, false, &hits);
                })
                .unwrap();
            let row: String = (button.x..button.right())
                .map(|x| terminal.backend().buffer()[(x, button.y)].symbol())
                .collect();
            assert_eq!(row.replace(' ', ""), locale.menu_button());
            let packet = |column| MouseEvent {
                action: MouseAction::Press,
                button: MouseButton::Left,
                mods: Modifiers::NONE,
                column,
                row: height - 1,
            };
            for column in button.x..button.right() {
                assert!(!view.machines.pointer(packet(column)));
                assert!(!view.machines.open);
            }
            assert!(view.machines.pointer(packet(indicator.unwrap().x)));
            assert!(view.machines.open);
        }
    }
}
