//! Painted mode controls share the decoder and keyboard resize authority.
use crate::client_view::ClientView;
use crate::terminal_input::{InputDispatcher, TerminalInputPort, TerminalInputUnavailable};
use crate::{
    client_view::PanelDirection,
    localization::Locale,
    tui_hit_map::{FrameHitMap, HitMapBuilder, HitTarget, ResizeControl},
    tui_layout::DrawnResize,
    tui_theme::Theme,
};
use ratatui::{Terminal, backend::TestBackend};
use std::{io, time::Instant};

#[derive(Default)]
struct Port {
    bytes: Vec<u8>,
    adjustments: Vec<(DrawnResize, PanelDirection, bool)>,
}
impl Port {
    fn click_resize_control(
        &mut self,
        target: &DrawnResize,
        direction: PanelDirection,
        fine: bool,
    ) {
        self.adjustments.push((target.clone(), direction, fine));
    }
}

impl TerminalInputPort for Port {
    fn surface_action(
        &mut self,
        action: crate::terminal_input::SurfaceAction<'_>,
    ) -> Option<crate::client_view::ClientActionDisposition> {
        use crate::terminal_input::{LayoutAction, SurfaceAction};
        match action {
            SurfaceAction::Layout(LayoutAction::ResizeControl {
                target,
                direction,
                fine,
            }) => {
                self.click_resize_control(target, direction, fine);
                None
            }
            _ => None,
        }
    }

    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
}

fn control_cell(
    map: &FrameHitMap,
    width: u16,
    height: u16,
    accepts: impl Fn(&ResizeControl) -> bool,
) -> (u16, u16) {
    (0..height).flat_map(|row| (0..width).map(move |column| (column, row))).find(|&(column, row)| {
        matches!(map.hit_target_at(column, row), Some(HitTarget::Resize(control)) if accepts(control))
    }).expect("fully painted control")
}

fn dispatch_all(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    port: &mut Port,
    bytes: &[u8],
) {
    dispatcher
        .dispatch(
            view,
            bytes,
            port,
            Instant::now(),
            |_, _| Ok::<_, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
}

#[test]
fn resize_toolbar_uses_matching_release_cancels_stale_geometry_and_restores_input() {
    for (width, height) in [(40, 10), (80, 24), (120, 40)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                Theme::orchbee_light(),
                Theme::orchbee_dark(),
                Theme::terminal_native(),
            ] {
                let (native, _, _, _) =
                    crate::workbench_client::ui::tests::publication_hint::ready_split_ui();
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                let mut dispatcher = InputDispatcher::default();
                let mut view = ClientView::new(width);
                let mut port = Port::default();
                dispatcher.set_pointer_capture(&mut view, &mut port, true);
                dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02P");
                assert!(view.resize_mode());
                let mut paint = |view: &ClientView| {
                    let mut hits = HitMapBuilder::new();
                    terminal
                        .draw(|frame| {
                            crate::tui_render::render_terminal_shell(
                                frame,
                                view,
                                locale,
                                Some(native.view()),
                                &theme,
                                &mut hits,
                            );
                        })
                        .unwrap();
                    hits.finish()
                };
                let map = paint(&view);
                let (x, y) = control_cell(&map, width, height, |control| {
                    matches!(
                        control,
                        ResizeControl::Adjust {
                            direction: PanelDirection::Right,
                            fine: false,
                            ..
                        }
                    )
                });
                dispatcher.publish_hit_map(map);
                let press = format!("\x1b[<0;{};{}M", x + 1, y + 1);
                let release = format!("\x1b[<0;{};{}m", x + 1, y + 1);
                dispatch_all(&mut dispatcher, &mut view, &mut port, press.as_bytes());
                assert!(port.adjustments.is_empty());
                dispatch_all(
                    &mut dispatcher,
                    &mut view,
                    &mut port,
                    format!("{release}owned").as_bytes(),
                );
                assert_eq!(port.adjustments.len(), 1);
                assert!(!port.adjustments[0].2);
                assert!(port.bytes.is_empty());

                dispatcher.publish_hit_map(paint(&view));
                dispatch_all(&mut dispatcher, &mut view, &mut port, press.as_bytes());
                dispatcher
                    .dispatch(
                        &mut view,
                        release.as_bytes(),
                        &mut port,
                        Instant::now(),
                        |_, _| Ok::<_, io::Error>(true),
                        &mut |_| Ok(()),
                    )
                    .unwrap();
                assert_eq!(port.adjustments.len(), 1, "geometry changed after paint");

                let map = paint(&view);
                let (x, y) = control_cell(&map, width, height, |control| {
                    matches!(control, ResizeControl::Step(false))
                });
                dispatcher.publish_hit_map(map);
                dispatch_all(
                    &mut dispatcher,
                    &mut view,
                    &mut port,
                    format!("\x1b[<0;{};{}M\x1b[<0;{};{}m", x + 1, y + 1, x + 1, y + 1).as_bytes(),
                );
                assert!(view.resize_pointer_fine);
                let map = paint(&view);
                control_cell(&map, width, height, |control| {
                    matches!(control, ResizeControl::Adjust { fine: true, .. })
                });
                let (x, y) = control_cell(&map, width, height, |control| {
                    matches!(control, ResizeControl::Exit)
                });
                dispatcher.publish_hit_map(map);
                dispatch_all(
                    &mut dispatcher,
                    &mut view,
                    &mut port,
                    format!(
                        "\x1b[<0;{};{}M\x1b[<0;{};{}mowned",
                        x + 1,
                        y + 1,
                        x + 1,
                        y + 1
                    )
                    .as_bytes(),
                );
                assert!(!view.resize_mode());
                assert!(port.bytes.is_empty());
                dispatcher.publish_hit_map(paint(&view));
                dispatch_all(&mut dispatcher, &mut view, &mut port, b"fresh");
                assert_eq!(port.bytes, b"fresh");
            }
        }
    }
}
