use super::*;
use orchbee_workbench_protocol::{PaneGraphicsPlacement, WorkbenchRevision};
use ratatui::{Terminal, backend::TestBackend, style::Style, widgets::Paragraph};

fn layer(z_index: i32, placement: PaneGraphicsPlacement) -> PaneGraphicsLayer {
    PaneGraphicsLayer {
        layer_id: "primary".into(),
        z_index,
        width: 1,
        height: 2,
        placement,
        resource_revision: WorkbenchRevision::new(1),
        stream_id: None,
    }
}
const RED_BLUE: &[u8] = &[255, 0, 0, 255, 0, 0, 255, 255];

#[test]
fn negative_images_composite_in_z_order_without_stealing_a_terminal_half_block() {
    let area = Rect::new(0, 0, 2, 1);
    let mut buffer = Buffer::empty(area);
    buffer[(0, 0)]
        .set_symbol("▀")
        .set_fg(Color::White)
        .set_bg(Color::Rgb(0, 0, 0));
    let mut painted = vec![false; 2];
    let metadata = layer(
        -2,
        PaneGraphicsPlacement {
            grid_columns: 2,
            grid_rows: 1,
            ..Default::default()
        },
    );
    paint_layers(&mut buffer, area, &metadata, RED_BLUE, &mut painted);
    assert_eq!(buffer[(0, 0)].fg, Color::White);
    assert_eq!(buffer[(1, 0)].fg, Color::Rgb(255, 0, 0));
    paint_layers(
        &mut buffer,
        area,
        &PaneGraphicsLayer {
            z_index: -1,
            ..metadata
        },
        &[0, 255, 0, 255, 0, 255, 0, 255],
        &mut painted,
    );
    assert_eq!(buffer[(0, 0)].fg, Color::White);
    assert_eq!(buffer[(1, 0)].fg, Color::Rgb(0, 255, 0));
    assert_eq!(buffer[(1, 0)].bg, Color::Rgb(0, 255, 0));
}

#[test]
fn natural_fit_keeps_the_complete_extreme_aspect_image_inside_the_body() {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 24));
    let metadata = PaneGraphicsLayer {
        height: 4096,
        ..layer(0, PaneGraphicsPlacement::default())
    };
    let mut pixels = vec![0; 4096 * 4];
    for (row, pixel) in pixels.chunks_exact_mut(4).enumerate() {
        pixel.copy_from_slice(if row < 2048 {
            &[255, 0, 0, 255]
        } else {
            &[0, 0, 255, 255]
        });
    }
    paint(&mut buffer, Rect::new(0, 0, 80, 24), &metadata, &pixels);
    assert_eq!(buffer[(0, 0)].fg, Color::Rgb(255, 0, 0));
    assert_eq!(buffer[(0, 23)].bg, Color::Rgb(0, 0, 255));
    assert_eq!(buffer[(1, 0)].symbol(), " ");
}

#[test]
fn actual_frame_clips_to_panel_body_at_every_viewport_tier() {
    for (width, height) in [(80, 24), (120, 40), (60, 18), (32, 10), (8, 2)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let body = Rect::new(1, 1, width - 2, height - 1);
        let metadata = layer(
            0,
            PaneGraphicsPlacement {
                viewport_column: -1,
                viewport_row: -1,
                grid_columns: width,
                grid_rows: height,
            },
        );
        terminal
            .draw(|frame| {
                frame.render_widget(Paragraph::new("CHROME"), Rect::new(0, 0, width, 1));
                paint(frame.buffer_mut(), body, &metadata, RED_BLUE);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, 0)].symbol(), "C");
        assert_eq!(buffer[(1, 1)].symbol(), "▀");
        assert_eq!(buffer[(width - 1, 1)].symbol(), " ");
        assert_eq!(buffer[(0, height - 1)].symbol(), " ");
    }
}

#[test]
fn negative_layer_preserves_text_and_foreground_while_positive_layer_covers_it() {
    for z in [-1, 0] {
        let mut terminal = Terminal::new(TestBackend::new(4, 2)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_widget(
                    Paragraph::new("x")
                        .style(Style::new().fg(Color::Green).bg(Color::Rgb(0, 0, 0))),
                    frame.area(),
                );
                let area = frame.area();
                paint(
                    frame.buffer_mut(),
                    area,
                    &layer(
                        z,
                        PaneGraphicsPlacement {
                            grid_columns: 1,
                            grid_rows: 1,
                            ..Default::default()
                        },
                    ),
                    RED_BLUE,
                );
            })
            .unwrap();
        let cell = &terminal.backend().buffer()[(0, 0)];
        if z < 0 {
            assert_eq!(cell.symbol(), "x");
            assert_eq!(cell.fg, Color::Green);
            assert_eq!(cell.bg, Color::Rgb(127, 0, 127));
        } else {
            assert_eq!(cell.symbol(), "▀");
            assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
            assert_eq!(cell.bg, Color::Rgb(0, 0, 255));
        }
    }
}

#[test]
fn alpha_blends_known_rgb_and_preserves_completely_transparent_cells() {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 1));
    buffer[(0, 0)].set_symbol("x").set_bg(Color::Rgb(0, 100, 0));
    let metadata = layer(
        0,
        PaneGraphicsPlacement {
            grid_columns: 1,
            grid_rows: 1,
            ..Default::default()
        },
    );
    paint(
        &mut buffer,
        Rect::new(0, 0, 2, 1),
        &metadata,
        &[255, 0, 0, 128, 0, 0, 255, 0],
    );
    assert_eq!(buffer[(0, 0)].fg, Color::Rgb(128, 50, 0));
    assert_eq!(buffer[(0, 0)].bg, Color::Rgb(0, 100, 0));
    let original = buffer.clone();
    paint(
        &mut buffer,
        Rect::new(0, 0, 2, 1),
        &metadata,
        &[255, 255, 255, 0, 255, 255, 255, 0],
    );
    assert_eq!(buffer, original);
    assert_eq!(blend([255, 0, 0, 1], Color::Reset), Color::Reset);
}

#[test]
fn wide_terminal_head_and_spacer_remain_atomic() {
    let mut terminal = Terminal::new(TestBackend::new(4, 2)).unwrap();
    terminal
        .draw(|frame| {
            frame.render_widget(Paragraph::new("中"), frame.area());
            frame.buffer_mut()[(1, 0)].set_diff_option(CellDiffOption::Skip);
            let area = frame.area();
            paint(
                frame.buffer_mut(),
                area,
                &layer(
                    0,
                    PaneGraphicsPlacement {
                        grid_columns: 3,
                        grid_rows: 1,
                        ..Default::default()
                    },
                ),
                RED_BLUE,
            );
            assert_eq!(frame.buffer_mut()[(1, 0)].diff_option, CellDiffOption::Skip);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(0, 0)].symbol(), "中");
    assert_eq!(buffer[(1, 0)].symbol(), " ");
    assert_eq!(buffer[(2, 0)].symbol(), "▀");
}

#[test]
fn extreme_signed_placement_is_clipped_without_overflow() {
    for (x, y) in [(i32::MIN, i32::MAX), (i32::MAX, i32::MIN)] {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 5, 3));
        let before = buffer.clone();
        paint(
            &mut buffer,
            Rect::new(1, 1, 3, 2),
            &layer(
                0,
                PaneGraphicsPlacement {
                    viewport_column: x,
                    viewport_row: y,
                    grid_columns: u16::MAX,
                    grid_rows: u16::MAX,
                },
            ),
            RED_BLUE,
        );
        assert_eq!(buffer, before);
    }
}

#[test]
fn a_clear_recomposes_underlying_cells_in_the_next_real_draw() {
    let mut terminal = Terminal::new(TestBackend::new(4, 2)).unwrap();
    terminal
        .draw(|frame| {
            frame.render_widget(Paragraph::new("base"), frame.area());
            let area = frame.area();
            paint(
                frame.buffer_mut(),
                area,
                &layer(
                    0,
                    PaneGraphicsPlacement {
                        grid_columns: 1,
                        grid_rows: 1,
                        ..Default::default()
                    },
                ),
                RED_BLUE,
            );
        })
        .unwrap();
    assert_eq!(terminal.backend().buffer()[(0, 0)].symbol(), "▀");
    terminal
        .draw(|frame| frame.render_widget(Paragraph::new("base"), frame.area()))
        .unwrap();
    assert_eq!(terminal.backend().buffer()[(0, 0)].symbol(), "b");
}
