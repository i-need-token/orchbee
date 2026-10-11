use super::*;
use crate::tui_pointer::{HitLayer, HitMapBuilder, HitTarget};
use ratatui::layout::Rect;

fn navigation_view(columns: u16) -> (ClientView, crate::machine_picker::SidebarTarget) {
    use orchbee_workbench_protocol::*;
    let contexts: Vec<_> = [1, 2]
        .into_iter()
        .map(|id| crate::machine_discovery::Context {
            target: crate::terminal_session::NativeContextTarget {
                site_id: SiteId::parse(&format!("{:032x}", 1)).unwrap(),
                location: crate::terminal_session::NativeContextLocation::Workspace {
                    workspace_id: WorkbenchWorkspaceId::parse(&format!("{id:032x}")).unwrap(),
                    terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{id:032x}"))
                        .unwrap(),
                },
            },
            project_id: Some(WorkbenchProjectId::parse(&format!("{:032x}", 2)).unwrap()),
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
                        |_, _| Ok::<bool, io::Error>(false),
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
                    |_, _| Ok::<bool, io::Error>(false),
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
                    |_, _| Ok::<bool, io::Error>(false),
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
fn sidebar_backdrop_retirement_settles_before_paint_with_or_without_packet_tail() {
    use crate::terminal_input::InputDispatcher;
    for button in [0, 1, 2, 64, 65] {
        for with_tail in [false, true] {
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
            let mut packet = format!("\x1b[<{button};51;2M");
            if with_tail {
                packet.push_str("\rtail\x1b[");
            }
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
                    if with_tail { b"13ufresh" } else { b"fresh" },
                    &mut surface,
                    now,
                    |_, _| Ok::<bool, io::Error>(false),
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
fn sidebar_width_retires_same_packet_release_when_press_or_motion_observes_host_resize() {
    use crate::{terminal_input::InputDispatcher, tui_layout::SidebarWidth};
    for resized in [dimensions(100, 40), dimensions(120, 28)] {
        for during_motion in [false, true] {
            let initial = dimensions(120, 40);
            let mut samples = vec![Ok(Some(initial))];
            if during_motion {
                samples.push(Ok(Some(initial)));
            }
            samples.extend([Ok(Some(resized)), Ok(Some(resized)), Ok(Some(resized))]);
            let viewport = QueueViewportSource {
                dimensions: std::cell::RefCell::new(samples.into()),
            };
            let mut size = initial;
            let mut terminal = test_terminal(initial);
            let (mut view, _) = navigation_view(initial.columns);
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
            let mut dispatcher = InputDispatcher::default();
            dispatcher.set_pointer_capture(&mut view, &mut surface, true);
            let theme = crate::tui_theme::Theme::terminal_native();
            let chrome = super::super::Chrome {
                locale: Locale::EnUs,
                theme: &theme,
            };
            super::super::draw_surface(
                &mut terminal,
                &mut view,
                chrome,
                &mut surface,
                &mut dispatcher,
                &mut |_| Ok(()),
            )
            .unwrap();
            dispatcher
                .dispatch(
                    &mut view,
                    if during_motion {
                        b"\x1b[<0;30;5M\x1b[<32;51;5M\x1b[<0;36;5m"
                    } else {
                        b"\x1b[<0;30;5M\x1b[<0;36;5m"
                    },
                    &mut surface,
                    Instant::now(),
                    |view, surface| {
                        super::super::synchronize_viewport(
                            &mut terminal,
                            &viewport,
                            view,
                            surface,
                            &mut size,
                            None,
                        )
                    },
                    &mut |_| Ok(()),
                )
                .unwrap();
            assert_eq!(size, resized);
            assert!(dispatcher.sidebar_width_preview().is_none());
            assert_eq!(view.machines.sidebar_width(), SidebarWidth::DEFAULT);
            assert!(view.machines.request.is_none());
            assert!(
                !surface
                    .events
                    .iter()
                    .any(|event| matches!(event, SurfaceEvent::Forward(_)))
            );

            // Only a new successfully painted source and a new press can commit.
            super::super::draw_surface(
                &mut terminal,
                &mut view,
                chrome,
                &mut surface,
                &mut dispatcher,
                &mut |_| Ok(()),
            )
            .unwrap();
            dispatcher
                .dispatch(
                    &mut view,
                    b"\x1b[<0;30;5M\x1b[<0;36;5m",
                    &mut surface,
                    Instant::now(),
                    |view, surface| {
                        super::super::synchronize_viewport(
                            &mut terminal,
                            &viewport,
                            view,
                            surface,
                            &mut size,
                            None,
                        )
                    },
                    &mut |_| Ok(()),
                )
                .unwrap();
            assert_eq!(view.machines.sidebar_width().columns(), 36);
            assert!(dispatcher.sidebar_width_preview().is_none());
            assert!(view.machines.request.is_none());
        }
    }
}
