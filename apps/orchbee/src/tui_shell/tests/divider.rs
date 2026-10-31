//! Successful-frame admission and recovery when an old preview has no target in a new frame.
use super::*;
use crate::{
    client_view::Modifiers,
    terminal_input::{MouseAction, MouseButton, MouseEvent},
    tui_layout::DividerDrag,
    tui_pointer::HitMapBuilder,
};
use orchbee_workbench_client::test_support::localization::LocaleTestAccess;
use orchbee_workbench_client::test_support::machine_picker::PickerSourceTestAccess;
use orchbee_workbench_client::test_support::machine_picker::PickerTestAccess;
use orchbee_workbench_client::test_support::terminal_graphics::ProbeTestAccess;
use orchbee_workbench_client::test_support::tui_graphics::SurfaceTestAccess;
use orchbee_workbench_client::test_support::tui_layout::DividerDragTestAccess;
use orchbee_workbench_client::test_support::tui_layout::DividerSource;
use orchbee_workbench_client::test_support::tui_layout::DividerSourceFixture;
use orchbee_workbench_client::test_support::tui_layout::DrawnDivider;
use orchbee_workbench_client::test_support::tui_layout::DrawnDividerFixture;
use orchbee_workbench_client::test_support::tui_layout::DrawnDividerTestAccess;
use orchbee_workbench_client::test_support::tui_layout::DrawnSidebarWidthTestAccess;
use orchbee_workbench_client::test_support::tui_layout::SidebarWidthTestAccess;
use orchbee_workbench_client::test_support::tui_pointer::HelpControl;
use orchbee_workbench_client::test_support::tui_pointer::HitLayer;
use orchbee_workbench_client::test_support::tui_pointer::HitMapBuilderTestAccess;
use orchbee_workbench_client::test_support::tui_pointer::HitTarget;
use orchbee_workbench_client::test_support::tui_pointer::InteractionSpec;
use orchbee_workbench_client::test_support::tui_pointer::InteractionSpecTestAccess;
use orchbee_workbench_client::test_support::tui_pointer::InteractionTarget;
use orchbee_workbench_client::test_support::tui_pointer::MotionObservability;
use orchbee_workbench_client::test_support::tui_pointer::PointerOutcome;
use orchbee_workbench_client::test_support::tui_pointer::PointerState;
use orchbee_workbench_client::test_support::tui_pointer::PointerStateTestAccess;
use orchbee_workbench_client::test_support::tui_pointer::PrimaryContract;
use orchbee_workbench_client::test_support::tui_pointer::ProjectedPointer;
use orchbee_workbench_protocol::{
    ClientSplitRatio, PanelOrientation, SessionId, WorkbenchPanelId, WorkbenchPanelSlotId,
    WorkbenchRevision, WorkbenchSplitId, WorkbenchTabId, WorkbenchTerminalContextId,
};
use ratatui::{layout::Rect, widgets::Paragraph};

fn fixture_divider(viewport: Rect) -> DrawnDivider {
    DrawnDivider::from(DrawnDividerFixture {
        source: DividerSource::from(DividerSourceFixture {
            session: SessionId::parse(&format!("{:032x}", 1)).unwrap(),
            context: WorkbenchTerminalContextId::parse(&format!("{:032x}", 2)).unwrap(),
            tab: WorkbenchTabId::parse(&format!("{:032x}", 3)).unwrap(),
            focus: WorkbenchPanelId::parse(&format!("{:032x}", 4)).unwrap(),
            revision: WorkbenchRevision::new(1),
        }),
        viewport,
        split: WorkbenchSplitId::parse(&format!("{:032x}", 5)).unwrap(),
        first: WorkbenchPanelSlotId::parse(&format!("{:032x}", 6)).unwrap(),
        second: WorkbenchPanelSlotId::parse(&format!("{:032x}", 7)).unwrap(),
        orientation: PanelOrientation::Horizontal,
        ratio: ClientSplitRatio::default(),
        area: Rect::new(0, 0, 101, 30),
        divider: Rect::new(50, 0, 1, 30),
    })
}

#[derive(Default)]
struct Surface {
    viewport: Option<Rect>,
    require_geometry: bool,
    require_control: bool,
    control_samples: usize,
    presentation_checks: Vec<bool>,
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
    fn pending_resize_projection(
        &mut self,
        _: Rect,
        _: orchbee_workbench_client::test_support::client_view::SidebarSurface,
        allowed: bool,
    ) -> crate::tui_layout::ResizeProjection {
        self.presentation_checks.push(allowed);
        crate::tui_layout::ResizeProjection::Canonical
    }
}

#[test]
fn zero_host_samples_cannot_apply_new_pixels_to_the_retained_viewport() {
    use orchbee_workbench_client::test_support::terminal_graphics::Probe;
    use orchbee_workbench_client::test_support::terminal_graphics::Reply;
    let initial = dimensions(120, 40);
    let mut size = initial;
    let mut terminal = test_terminal(initial);
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([Ok(None), Ok(None)])),
    };
    let mut probe = Probe::default();
    probe.activate(1);
    let now = Instant::now();
    assert!(probe.query((initial.columns, initial.rows), now).is_some());
    probe.reply(Reply::Cell(Some((16, 32))), now);
    let measurement = probe.facts().measurement.unwrap();
    assert_ne!(measurement.apply(initial), initial);
    let mut view = ClientView::new(initial.columns);
    let mut surface = OrderedSurface::default();
    let canonical = crate::tui_layout::ResizeProjection::Canonical;
    for _ in 0..2 {
        assert!(
            !super::super::synchronize_viewport(
                &mut terminal,
                &viewport,
                &mut view,
                &mut surface,
                &mut size,
                Some(measurement),
                (&canonical, true),
            )
            .unwrap()
        );
        assert_eq!(size, initial);
    }
    let geometry = super::super::terminal_geometry(initial, &view)
        .unwrap()
        .unwrap();
    assert_eq!(
        surface.events,
        [
            SurfaceEvent::Observe(geometry),
            SurfaceEvent::Observe(geometry)
        ]
    );
}

#[test]
fn external_pixel_resize_retires_the_lease_while_owned_observation_does_not_sample_the_host() {
    let initial = dimensions(120, 40);
    let resized = WindowDimensions {
        pixel_width: initial.pixel_width + 120,
        ..initial
    };
    let mut size = initial;
    let mut terminal = test_terminal(initial);
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([Ok(Some(resized))])),
    };
    let mut view = ClientView::new(120);
    let mut surface = Surface::default();
    let canonical = crate::tui_layout::ResizeProjection::Canonical;
    super::super::observe_surface_geometry(&size, &view, &mut surface, (&canonical, true)).unwrap();
    assert_eq!(
        viewport.dimensions.borrow().len(),
        1,
        "own geometry cannot consume a host resize sample"
    );
    assert_eq!(surface.presentation_checks, [true]);
    assert!(
        super::super::synchronize_viewport(
            &mut terminal,
            &viewport,
            &mut view,
            &mut surface,
            &mut size,
            None,
            (&canonical, true),
        )
        .unwrap()
    );
    assert_eq!(surface.presentation_checks, [true, false]);
    assert_eq!((size.columns, size.rows), (initial.columns, initial.rows));
    assert_eq!(size.pixel_width, resized.pixel_width);
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
fn divider_preview_cannot_leak_into_a_machine_capture_frame_before_input_reconciliation() {
    const WIDTH: u16 = 120;
    const HEIGHT: u16 = 40;

    let mut preview_terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
    let mut preview_surface = Surface::default();
    let mut preview_view = ClientView::new(WIDTH);
    let mut preview_dispatcher = crate::terminal_input::InputDispatcher::default();
    let target = fixture_divider(Rect::new(0, 0, WIDTH, HEIGHT));
    let mut hits = HitMapBuilder::new();
    hits.record(
        HitLayer::Foreground,
        target.divider(),
        HitTarget::Divider(target),
    );
    preview_dispatcher.set_pointer_capture(&mut preview_view, &mut preview_surface, true);
    preview_dispatcher.publish_hit_map(hits.finish());
    preview_dispatcher
        .dispatch(
            &mut preview_view,
            b"\x1b[<0;51;6M",
            &mut preview_surface,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(preview_dispatcher.divider_preview().is_some());

    // The Client capture changes after the pointer packet and before the dispatcher samples the
    // new owner. `draw_surface` must still project the new top owner for this exact frame.
    preview_view.machines.available = true;
    preview_view.machines.show();
    let machine_source = preview_view
        .machines
        .picker_source()
        .expect("the higher owner must be a live Machine picker episode");
    assert!(
        machine_source
            .drawn(orchbee_workbench_client::test_support::machine_picker::PickerAction::Row(0))
            .is_some(),
        "the real default Machine snapshot contains its Local row"
    );
    assert!(
        machine_source
            .drawn(orchbee_workbench_client::test_support::machine_picker::PickerAction::Row(1))
            .is_none(),
        "the source snapshot contains exactly one Machine row"
    );
    assert!(
        preview_dispatcher.divider_preview().is_some(),
        "installing the higher owner has not yet reconciled dispatcher gesture state"
    );
    let mut control_view = preview_view.clone();
    let mut control_surface = Surface::default();
    let mut control_dispatcher = crate::terminal_input::InputDispatcher::default();
    control_dispatcher.set_pointer_capture(&mut control_view, &mut control_surface, true);
    let theme = crate::tui_theme::Theme::terminal_native();
    let chrome = super::super::Chrome {
        locale: Locale::EnUs,
        theme: &theme,
    };

    super::super::draw_surface(
        &mut preview_terminal,
        &mut preview_view,
        chrome,
        &mut preview_surface,
        &mut preview_dispatcher,
        &mut |_| Ok(()),
    )
    .unwrap();
    assert!(
        preview_dispatcher.divider_preview().is_none(),
        "successful publication retires the covered gesture after the guarded frame is painted"
    );

    let mut control_terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
    super::super::draw_surface(
        &mut control_terminal,
        &mut control_view,
        chrome,
        &mut control_surface,
        &mut control_dispatcher,
        &mut |_| Ok(()),
    )
    .unwrap();
    assert!(control_dispatcher.divider_preview().is_none());
    assert_eq!(
        preview_view.machines.picker_source(),
        Some(machine_source.clone()),
        "painting the guarded frame must preserve its exact Machine source"
    );
    assert_eq!(
        control_view.machines.picker_source(),
        Some(machine_source),
        "the clean control frame must use that same Machine source"
    );
    assert_eq!(
        preview_terminal.backend().buffer(),
        control_terminal.backend().buffer(),
        "a live lower drag must leave no glyph, style, or status residue in the captured frame"
    );
    let text = preview_terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.contains(&format!(
        "{} 1/1",
        Locale::EnUs.machine_picker_text("machines-mode")
    )));
    assert!(text.contains(Locale::EnUs.machine_picker_text("local")));
    assert!(text.contains(Locale::EnUs.machine_picker_text("current")));
}

#[test]
fn divider_rejected_preview_preserves_the_draw_pointer_projection() {
    let viewport = Rect::new(0, 0, 120, 40);
    let drag = DividerDrag::begin(fixture_divider(viewport), 50, 5).unwrap();
    let interaction_area = Rect::new(2, 2, 5, 1);
    let interaction = InteractionTarget::Fixture(HitTarget::Help {
        page: 0,
        action: HelpControl::Close,
    });
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let theme = crate::tui_theme::Theme::terminal_native();

    for pressed in [false, true] {
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        let view = ClientView::new(120);
        let mut surface = Surface::default();
        let mut pointer = PointerState::default();
        pointer.set_capture(true);
        if !pressed {
            pointer.set_motion_observability(MotionObservability::AnyMotion);
        }

        let mut hits = HitMapBuilder::with_pointer(pointer.projection());
        terminal
            .draw(|frame| {
                let (_, preview_painted) = super::super::divider::render(
                    frame,
                    &view,
                    super::super::Chrome {
                        locale: Locale::EnUs,
                        theme: &theme,
                    },
                    &surface,
                    None,
                    super::super::divider::Presentation {
                        projection: &crate::tui_layout::ResizeProjection::Canonical,
                        preview: None,
                    },
                    &mut hits,
                );
                assert!(!preview_painted);
                hits.start_modal();
                frame.render_widget(Paragraph::new("[x]"), interaction_area);
                hits.record_interaction(
                    HitLayer::Overlay,
                    interaction_area,
                    interaction_area,
                    interaction.clone(),
                    spec,
                );
            })
            .unwrap();
        pointer.publish(hits.finish());
        assert!(!pointer.take_projection_redraw());

        let outcome = pointer.event(
            &mut ClientView::new(120),
            &mut surface,
            Instant::now(),
            MouseEvent {
                action: if pressed {
                    MouseAction::Press
                } else {
                    MouseAction::Motion
                },
                button: if pressed {
                    MouseButton::Left
                } else {
                    MouseButton::None
                },
                mods: Modifiers::NONE,
                column: interaction_area.x,
                row: interaction_area.y,
            },
        );
        assert!(matches!(
            (pressed, outcome),
            (true, PointerOutcome::Handled) | (false, PointerOutcome::IdleMotion { changed: true })
        ));

        let expected = if pressed {
            ProjectedPointer::Pressed
        } else {
            ProjectedPointer::Hovered
        };
        let mut hits = HitMapBuilder::with_pointer(pointer.projection());
        terminal
            .draw(|frame| {
                let (_, preview_painted) = super::super::divider::render(
                    frame,
                    &view,
                    super::super::Chrome {
                        locale: Locale::EnUs,
                        theme: &theme,
                    },
                    &surface,
                    None,
                    super::super::divider::Presentation {
                        projection: &crate::tui_layout::ResizeProjection::Divider {
                            source: drag.target().clone(),
                            ratio: drag.ratio(),
                        },
                        preview: Some(super::super::divider::Preview::Divider(&drag)),
                    },
                    &mut hits,
                );
                assert!(
                    !preview_painted,
                    "the old divider is absent from this frame"
                );
                hits.start_modal();
                assert_eq!(
                    hits.projected_pointer(
                        HitLayer::Overlay,
                        interaction_area,
                        interaction_area,
                        &interaction,
                        spec,
                    ),
                    expected,
                    "candidate reset must retain this draw's immutable projection"
                );
                frame.render_widget(Paragraph::new("[x]"), interaction_area);
                hits.record_interaction(
                    HitLayer::Overlay,
                    interaction_area,
                    interaction_area,
                    interaction.clone(),
                    spec,
                );
            })
            .unwrap();
        pointer.publish(hits.finish());
        assert!(
            !pointer.take_projection_redraw(),
            "resetting a rejected candidate must not fabricate a follow-up frame"
        );
    }
}

#[test]
fn divider_lost_preview_restores_graphics_in_the_same_flushed_frame_without_another_event() {
    use orchbee_workbench_client::test_support::terminal_graphics::Probe;
    use orchbee_workbench_client::test_support::terminal_graphics::Reply;
    use orchbee_workbench_client::test_support::workbench_client::PaneGraphicsCanvas;
    use orchbee_workbench_protocol::{
        PaneGraphicsLayer, PaneGraphicsPlacement, PaneGraphicsTarget, TerminalPresentationId,
    };
    for sidebar in [false, true] {
        let size = dimensions(120, 40);
        let mut backend = FixedViewportBackend::new(
            Vec::<u8>::new(),
            size,
            Arc::new(orchbee_workbench_client::test_support::terminal_lifecycle::synchronized_update_lease()),
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
        let target = fixture_divider(Rect::new(0, 0, 120, 40));
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
                target.divider(),
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
                orchbee_workbench_client::test_support::tui_layout::SidebarWidth::DEFAULT
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
            &SessionId::parse(&format!("{:032x}", 1)).unwrap(),
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
