use super::{
    Dialog, DialogClass, DialogFooter, DialogRow, DialogTargets, ModalAction, dialog_layout,
    paint_dialog, paint_field, unwrapped_text_fits,
};
use crate::terminal_text::sanitize_terminal_text;
use crate::tui_hit_map::{HitMapBuilder, HitTarget};
use crate::tui_render::controls::ActionKind;
use crate::tui_theme::{Role, Theme};
use crate::workbench_state::PanelCloseConfirmationToken;
use ratatui::{Terminal, backend::TestBackend, layout::Rect, text::Text};

#[test]
fn combining_text_is_measured_after_terminal_sanitization() {
    let safe = sanitize_terminal_text("e\u{301}\x1b[31m\n界");
    assert!(!safe.contains('\x1b'));
    assert!(!safe.contains('\n'));
    let width = u16::try_from(Text::from(safe.as_ref()).width()).unwrap();
    assert!(unwrapped_text_fits(&safe, Rect::new(0, 0, width, 1)));
    assert!(!unwrapped_text_fits(&safe, Rect::new(0, 0, width - 1, 1)));
}

fn actions() -> [ModalAction<'static>; 2] {
    [
        ModalAction::new("Cancel", ActionKind::Secondary, true).with_shortcut("Esc"),
        ModalAction::new("Rename", ActionKind::Primary, true)
            .with_shortcut("Enter")
            .focused(true),
    ]
}

/// Target, blank, field, help and two lines of prose: eleven rows with both spacers.
fn dialog<'a>(actions: &'a [ModalAction<'a>]) -> Dialog<'a> {
    Dialog {
        title: "Rename Tab",
        class: DialogClass::Narrow,
        rows: vec![
            DialogRow::Target("build".into()),
            DialogRow::Blank,
            DialogRow::Field { minimum: 24 },
            DialogRow::Help(Some("Leave blank for an unnamed group.".into())),
            DialogRow::Prose(
                "Ends 2 terminals, closes its Workspace and deletes the checkout.".into(),
            ),
        ],
        footer: DialogFooter::Actions(actions),
    }
}

fn close_target() -> HitTarget {
    HitTarget::PanelClose {
        token: PanelCloseConfirmationToken::for_test(1),
        confirm: false,
    }
}

/// Paints the dialog into a fresh window and returns its frame and the painted rows.
fn paint(width: u16, height: u16, dialog: &Dialog<'_>) -> Option<(Rect, Vec<String>, bool)> {
    let theme = Theme::terminal_native();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    let mut area = None;
    terminal
        .draw(|frame| {
            let Some(layout) = dialog_layout(frame.area(), dialog, &theme) else {
                return;
            };
            let targets = [close_target(), close_target()];
            paint_dialog(
                frame,
                dialog,
                &layout,
                &DialogTargets {
                    close: close_target(),
                    actions: &targets,
                    dismiss_outside: false,
                    scroll: None,
                },
                &theme,
                &mut hits,
            );
            area = Some(layout.area);
        })
        .unwrap();
    let area = area?;
    let map = hits.finish();
    // The close control sits on the top edge, at its right end.
    let close_on_edge =
        (area.x..area.right()).any(|x| map.hit_target_at(x, area.y) == Some(&close_target()));
    let rows = terminal
        .backend()
        .buffer()
        .content()
        .chunks(usize::from(width))
        .map(|row| row.iter().map(|cell| cell.symbol()).collect())
        .collect();
    Some((area, rows, close_on_edge))
}

#[test]
fn dialogs_center_in_the_whole_window_within_their_class_width() {
    let theme = Theme::terminal_native();
    let actions = actions();
    let mut dialog = dialog(&actions);
    let (area, rows, close_on_edge) = paint(120, 40, &dialog).unwrap();
    // Every required line is shorter than the Narrow minimum: content 40, outer 46.
    assert_eq!(area.width, 46);
    assert_eq!((area.x, area.y), ((120 - 46) / 2, (40 - area.height) / 2));
    assert_eq!(area.height, 11);
    assert!(rows[usize::from(area.y)].contains("Rename Tab"));
    assert!(close_on_edge);
    // A path field takes the class maximum; Medium starts wider than Narrow.
    dialog.rows[2] = DialogRow::Field {
        minimum: DialogClass::Narrow.path_width(),
    };
    let viewport = Rect::new(0, 0, 120, 40);
    assert_eq!(
        dialog_layout(viewport, &dialog, &theme).unwrap().area.width,
        56 + 6
    );
    dialog.rows[2] = DialogRow::Field { minimum: 24 };
    dialog.class = DialogClass::Medium;
    assert_eq!(
        dialog_layout(viewport, &dialog, &theme).unwrap().area.width,
        48 + 6
    );
}

#[test]
fn small_windows_drop_the_margin_then_degrade_each_axis_and_refuse_below_the_minimum() {
    let theme = Theme::terminal_native();
    let actions = actions();
    let dialog = dialog(&actions);
    let at = |height| dialog_layout(Rect::new(0, 0, 80, height), &dialog, &theme);
    // The one-row margin goes first: eleven rows fit in thirteen with it and in eleven without.
    assert_eq!(at(13).unwrap().area.y, 1);
    assert_eq!(at(11).unwrap().area.y, 0);
    // Then blank rows, help, and prose beyond one line, in that order.
    let eight = at(8).unwrap();
    assert!(eight.row(1).is_none() && eight.row(3).is_some());
    let seven = at(7).unwrap();
    assert!(seven.row(3).is_none() && seven.row(4).unwrap().height == 2);
    let six = at(6).unwrap();
    assert!(six.row(0).is_some() && six.row(2).is_some() && six.row(4).unwrap().height == 1);
    assert!(at(5).is_none());

    // Width: no margin at the exact outer width, then narrower than the class while the complete
    // action row fits, then the buttons lose their key text, then refusal.
    let mut refused = false;
    let mut bare = false;
    for width in (8..=46).rev() {
        match paint(width, 24, &dialog) {
            Some((area, rows, close_on_edge)) => {
                assert!(!refused, "{width}: refusal is final as the window narrows");
                assert_eq!(area.width, width.min(46));
                let footer = &rows[usize::from(area.bottom() - 2)];
                assert!(
                    footer.contains("Cancel") && footer.contains("Rename"),
                    "{footer}"
                );
                bare |= !footer.contains("Esc");
                assert!(close_on_edge);
            }
            None => refused = true,
        }
    }
    assert!(bare && refused);
}

#[test]
fn fields_use_the_band_where_the_theme_has_one_and_brackets_otherwise() {
    for (theme, band) in [
        (Theme::rgb_test_dark(), true),
        (Theme::terminal_native(), false),
    ] {
        assert_eq!(theme.field_band(), band);
        let mut terminal = Terminal::new(TestBackend::new(12, 1)).unwrap();
        terminal
            .draw(|frame| paint_field(frame, Rect::new(0, 0, 12, 1), "name", false, &theme))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
        if band {
            assert_eq!(text, " name       ");
            let fill = theme.style(Role::SurfaceField).bg;
            assert!(fill.is_some());
            assert!((0..12).all(|x| buffer[(x, 0)].style().bg == fill));
        } else {
            assert_eq!(text, "[ name     ]");
        }
    }
}

/// A shared frame owns its fill even when the covered terminal supplies wide-cell metadata.
#[test]
fn overlay_frames_replace_their_underlay_and_repair_crossing_wide_cells() {
    use ratatui::{
        buffer::{Buffer, CellDiffOption},
        widgets::{Padding, Widget},
    };
    for (width, height) in [(80, 24), (120, 36)] {
        for theme in [
            Theme::terminal_native(),
            Theme::orchbee_dark(),
            Theme::orchbee_light(),
        ] {
            let viewport = Rect::new(0, 0, width, height);
            let area = crate::tui_layout::place_overlay(viewport, 92, height).unwrap();
            let mut canonical = Buffer::empty(viewport);
            super::overlay_block("Reference", area.width, &theme).render(area, &mut canonical);
            let mut covered = Buffer::empty(viewport);
            for y in 0..height {
                covered.set_string(
                    0,
                    y,
                    "M".repeat(usize::from(width)),
                    theme.style(Role::TextPrimary),
                );
            }
            for y in area.y..area.bottom() {
                covered.set_string(area.x - 1, y, "界", theme.style(Role::TextPrimary));
                covered[(area.x, y)].set_diff_option(CellDiffOption::Skip);
                covered.set_string(area.right() - 1, y, "界", theme.style(Role::TextPrimary));
                covered[(area.right(), y)].set_diff_option(CellDiffOption::Skip);
            }
            super::titled_overlay(&theme, Padding::horizontal(2), "Reference", area.width)
                .render(area, &mut covered);
            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    assert_eq!(
                        covered[(x, y)],
                        canonical[(x, y)],
                        "{width}x{height} ({x},{y})"
                    );
                }
                for x in [area.x - 1, area.right()] {
                    assert_eq!(covered[(x, y)].symbol(), " ");
                    assert_eq!(covered[(x, y)].diff_option, CellDiffOption::None);
                }
            }
        }
    }
}
