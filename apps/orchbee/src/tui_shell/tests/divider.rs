//! Successful-frame admission and recovery when an old preview has no target in a new frame.
use super::*;
use crate::{
    tui_layout::{DividerSource, DrawnDivider},
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
};
use orchbee_workbench_protocol::{
    ClientSplitRatio, PanelOrientation, SiteId, WorkbenchPanelId, WorkbenchPanelSlotId,
    WorkbenchRevision, WorkbenchSplitId, WorkbenchTabId, WorkbenchTerminalContextId,
};
use ratatui::layout::Rect;

#[derive(Default)]
struct Surface {
    viewport: Option<Rect>,
    require_geometry: bool,
    require_control: bool,
    control_samples: usize,
}
impl TerminalInputPort for Surface {
    fn forward(&mut self, _: &[u8]) -> Result<(), TerminalInputUnavailable> {
        panic!()
    }
    fn pointer_divider_ready(
        &mut self,
        _: &DrawnDivider,
    ) -> Result<(), crate::client_view::Notice> {
        Ok(())
    }
}
impl super::super::TerminalSurface for Surface {
    fn client_control_tick(&mut self, _: &mut ClientView, _: Rect, _: Locale) -> bool {
        self.control_samples += 1;
        false
    }
    fn observe_client_viewport(&mut self, _: &ClientView, area: Rect) {
        self.viewport = Some(area);
    }
    fn after_draw(&mut self) {
        if self.require_control {
            assert!(
                self.control_samples > 0,
                "a fresh surface must initialize its controller before its first published frame"
            );
        }
        if self.require_geometry {
            assert_eq!(self.viewport, Some(Rect::new(0, 0, 120, 40)));
        }
    }
}

#[test]
fn divider_every_successful_surface_draw_observes_controller_geometry_before_publication() {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let theme = crate::tui_theme::Theme::terminal_native();
    let mut dispatcher = crate::terminal_input::InputDispatcher::default();
    let mut view = ClientView::new(120);
    for _surface_switch in 0..2 {
        let mut surface = Surface {
            require_geometry: true,
            ..Surface::default()
        };
        super::super::draw_surface(
            &mut terminal,
            &mut view,
            super::super::Chrome {
                locale: Locale::EnUs,
                theme: &theme,
            },
            &mut surface,
            &mut dispatcher,
            &mut |_| Ok(()),
        )
        .unwrap();
    }
}

#[test]
fn divider_first_published_frame_initializes_control_without_waiting_for_idle_input() {
    struct HostViewport(WindowDimensions);
    impl ViewportSource for HostViewport {
        fn window_dimensions(&self) -> io::Result<Option<WindowDimensions>> {
            Ok(Some(self.0))
        }
    }
    let size = dimensions(120, 40);
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let mut inputs = QueueInputSource {
        inputs: VecDeque::from([Ok(Some(b"\x02d".to_vec()))]),
    };
    let mut surface = Surface {
        require_geometry: true,
        require_control: true,
        ..Surface::default()
    };
    let theme = crate::tui_theme::Theme::terminal_native();
    assert!(matches!(
        super::super::run_event_loop(
            &mut terminal,
            super::super::Chrome {
                locale: Locale::EnUs,
                theme: &theme
            },
            &mut inputs,
            &mut surface,
            &FixedTermination(false),
            &HostViewport(size),
            size,
            true,
        ),
        Ok(LoopExit::User)
    ));
}

#[test]
fn divider_lost_preview_restores_graphics_in_the_same_flushed_frame_without_another_event() {
    use crate::terminal_graphics::{Probe, Reply};
    use crate::terminal_session::PaneGraphicsCanvas;
    use orchbee_workbench_protocol::{
        PaneGraphicsLayer, PaneGraphicsPlacement, PaneGraphicsTarget, TerminalPresentationId,
    };
    for sidebar in [false, true] {
        let size = dimensions(120, 40);
        let mut backend = FixedViewportBackend::new(
            Vec::<u8>::new(),
            size,
            Arc::new(SynchronizedUpdateLease::new()),
        );
        backend.enable_graphics_probe(31);
        let mut terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Fixed(to_area(size)),
            },
        )
        .unwrap();
        let graphics = terminal.backend().graphics_surface().unwrap();
        let now = Instant::now();
        let mut probe = Probe::default();
        probe.activate(31);
        probe.query((120, 40), now).unwrap();
        probe.reply(Reply::Cell(Some((8, 16))), now);
        probe.reply(
            Reply::Kitty {
                image_id: 31,
                placement_id: None,
                success: true,
            },
            now,
        );
        graphics.update(probe.facts(), now, Vec::new(), false);
        let mut surface = Surface::default();
        let mut view = ClientView::new(120);
        let mut dispatcher = crate::terminal_input::InputDispatcher::default();
        let target = DrawnDivider {
            source: DividerSource {
                site: SiteId::parse(&format!("{:032x}", 1)).unwrap(),
                context: WorkbenchTerminalContextId::parse(&format!("{:032x}", 2)).unwrap(),
                tab: WorkbenchTabId::parse(&format!("{:032x}", 3)).unwrap(),
                focus: WorkbenchPanelId::parse(&format!("{:032x}", 4)).unwrap(),
                revision: WorkbenchRevision::new(1),
            },
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
        if sidebar {
            view.machines.available = true;
            view.machines.observe_agent_sidebar(true);
            let target =
                crate::tui_layout::DrawnSidebarWidth::new(to_area(size), view.sidebar_surface())
                    .unwrap();
            hits.record(
                HitLayer::Chrome,
                target.edge(),
                HitTarget::SidebarResize(target),
            );
        } else {
            hits.record(
                HitLayer::Foreground,
                target.divider,
                HitTarget::Divider(target),
            );
        }
        dispatcher.set_pointer_capture(&mut view, &mut surface, true);
        dispatcher.publish_hit_map(hits.finish());
        dispatcher
            .dispatch(
                &mut view,
                if sidebar {
                    b"\x1b[<0;30;6M"
                } else {
                    b"\x1b[<0;51;6M"
                },
                &mut surface,
                now,
                |_, _| Ok::<bool, io::Error>(false),
                &mut |_| Ok(()),
            )
            .unwrap();
        assert_eq!(dispatcher.divider_preview().is_some(), !sidebar);
        assert_eq!(dispatcher.sidebar_width_preview().is_some(), sidebar);
        let theme = crate::tui_theme::Theme::terminal_native();
        if sidebar {
            super::super::draw_surface(
                &mut terminal,
                &mut view,
                super::super::Chrome {
                    locale: Locale::ZhCn,
                    theme: &theme,
                },
                &mut surface,
                &mut dispatcher,
                &mut |_| Ok(()),
            )
            .unwrap();
            assert!(dispatcher.sidebar_width_preview().is_some());
            assert_eq!(
                view.machines.sidebar_width(),
                crate::tui_layout::SidebarWidth::DEFAULT
            );
            // The next successful candidate loses the exact painted border.
            view.machines.available = false;
        }
        super::super::draw_surface(
            &mut terminal,
            &mut view,
            super::super::Chrome {
                locale: Locale::EnUs,
                theme: &theme,
            },
            &mut surface,
            &mut dispatcher,
            &mut |_| Ok(()),
        )
        .unwrap();
        assert!(dispatcher.divider_preview().is_none());
        assert!(dispatcher.sidebar_width_preview().is_none());
        let canvas = PaneGraphicsCanvas::fixture(
            PaneGraphicsTarget {
                panel_id: WorkbenchPanelId::parse(&format!("{:032x}", 4)).unwrap(),
                presentation_id: Some(
                    TerminalPresentationId::parse(&format!("{:032x}", 8)).unwrap(),
                ),
            },
            WorkbenchRevision::new(1),
            vec![(
                PaneGraphicsLayer {
                    layer_id: "primary".into(),
                    z_index: 0,
                    width: 1,
                    height: 1,
                    placement: PaneGraphicsPlacement::default(),
                    resource_revision: WorkbenchRevision::new(1),
                    stream_id: None,
                },
                vec![255; 4],
            )],
        );
        graphics.select(
            &SiteId::parse(&format!("{:032x}", 1)).unwrap(),
            &canvas,
            Rect::new(30, 4, 10, 10),
        );
        graphics.finish_frame(&[canvas.draw_for_test()], false);
        let mut bytes = Vec::new();
        graphics
            .write_frame(&mut bytes, ratatui::layout::Position::new(0, 0))
            .unwrap();
        assert!(String::from_utf8(bytes).unwrap().contains("\x1b_G"));
    }
}
