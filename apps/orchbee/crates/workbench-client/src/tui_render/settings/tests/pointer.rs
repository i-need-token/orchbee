//! Real Settings geometry and mixed content roles under admitted hover and press reports.
use super::*;
use crate::{
    client_view::Modifiers,
    terminal_input::{MouseAction, MouseButton, MouseEvent},
    tui_hit_map::PointerProjection,
    tui_pointer::{MotionObservability, PointerOutcome, PointerState},
};
use ratatui::style::Modifier;

fn paint(
    editor: &Editor,
    locale: Locale,
    theme: &Theme,
    pointer: PointerProjection,
) -> (Terminal<TestBackend>, FrameHitMap, SettingsDraw) {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let mut hits = HitMapBuilder::with_pointer(pointer);
    let mut receipt = ModalPresentation::NotPresented;
    terminal
        .draw(|frame| {
            receipt = render(
                frame,
                frame.area(),
                editor.view().unwrap(),
                false,
                locale,
                theme,
                &mut hits,
            );
        })
        .unwrap();
    let ModalPresentation::Presented(draw) = receipt else {
        panic!("full Settings geometry is admitted");
    };
    (terminal, hits.finish(), draw)
}

fn assert_pointer_band(editor: &mut Editor, locale: Locale, theme: &Theme, target: HitTarget) {
    let View::Draft(before) = editor.view().unwrap() else {
        panic!("draft");
    };
    let values = before.values.clone();
    let selected = before.selected;
    let picker = before.picker;
    let hovered_surface = match &target {
        HitTarget::SettingsControl {
            control: Control::Choose(choice),
            ..
        } if picker
            .is_some_and(|picker| Choice::options(picker.row())[picker.cursor()] == *choice) =>
        {
            Role::SurfaceSelection
        }
        _ => Role::SurfaceHover,
    };
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.set_motion_observability(MotionObservability::AnyMotion);
    let (_, hits, draw) = paint(editor, locale, theme, pointer.projection());
    editor.presented(draw);
    let target_cells = cells_for(&hits, &target, 120, 40);
    let y = target_cells.iter().map(|(_, y)| *y).max().unwrap();
    let mut band_cells = target_cells.clone();
    // A numeric row owns its whole band, including independently published minus/plus targets.
    if let HitTarget::SettingsRow { token, row } = &target {
        for forward in [false, true] {
            band_cells.extend(cells_for(
                &hits,
                &HitTarget::SettingsControl {
                    token: *token,
                    control: Control::Adjust { row: *row, forward },
                },
                120,
                40,
            ));
        }
    }
    band_cells.retain(|(_, row)| *row == y);
    let left = band_cells.iter().map(|(x, _)| *x).min().unwrap();
    let right = band_cells.iter().map(|(x, _)| *x).max().unwrap() + 1;
    pointer.publish(hits);
    for pressed in [false, true] {
        let outcome = pointer.settings_event(MouseEvent {
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
            column: left,
            row: y,
        });
        assert_eq!(
            outcome,
            if pressed {
                PointerOutcome::Handled
            } else {
                PointerOutcome::IdleMotion { changed: true }
            }
        );
        let (terminal, hits, next_draw) = paint(editor, locale, theme, pointer.projection());
        assert_eq!(next_draw, draw, "pointer feedback cannot change admission");
        assert_eq!(cells_for(&hits, &target, 120, 40), target_cells);
        let surface = if pressed {
            Role::SurfacePressed
        } else {
            hovered_surface
        };
        let expected = theme.paired_pointer_surfaces().style(surface);
        let buffer = terminal.backend().buffer();
        for x in left..right {
            // Continuation cells of a wide glyph are not separately emitted by the backend.
            if (left..x).rev().take(4).any(|start| {
                let width = display_width(buffer[(start, y)].symbol());
                width > 1 && usize::from(start) + width > usize::from(x)
            }) {
                continue;
            }
            let cell = &buffer[(x, y)];
            assert_eq!(
                cell.bg,
                expected.bg.unwrap(),
                "{target:?} {surface:?} {x},{y}"
            );
            let reversed = expected.add_modifier.contains(Modifier::REVERSED);
            assert_eq!(cell.modifier.contains(Modifier::REVERSED), reversed);
            if reversed {
                assert_eq!(cell.fg, expected.fg.unwrap(), "one displayed reverse band");
                // DIM applied before the host swaps the pair would darken a token's background.
                assert!(!cell.modifier.contains(Modifier::DIM), "{target:?} {x},{y}");
            }
        }
        let View::Draft(after) = editor.view().unwrap() else {
            panic!("pointer presentation retains the draft");
        };
        assert_eq!(after.values, &values);
        assert_eq!(after.selected, selected);
        assert_eq!(after.picker, picker);
        pointer.publish(hits);
    }
}

#[test]
fn mixed_setting_rows_and_footer_actions_keep_one_complete_hover_and_pressed_band() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::orchbee_light(),
            Theme::orchbee_dark(),
            Theme::orchbee_dark().quantize(ColorLevel::Ansi16),
            Theme::terminal_native().quantize(ColorLevel::Monochrome),
        ] {
            let values = UserPreferencesValues {
                completion_reminder_minimum_seconds: 1,
                ..UserPreferencesValues::default()
            };
            let mut editor = editor_with(values);
            // Enable Save while retaining a checkbox accent, secondary brackets/numeric value,
            // a disabled decrement, action shortcuts and the ordinary Application theme row.
            select_and_present(&mut editor, Row::Sound, locale, &theme);
            editor.edit(Edit::Adjust(true));
            for row in [
                Row::Theme,
                Row::CopyOnSelect,
                Row::CompletionThreshold,
                Row::Save,
            ] {
                editor.edit(Edit::Select(row));
                let token = editor.view().unwrap().token();
                assert_pointer_band(
                    &mut editor,
                    locale,
                    &theme,
                    HitTarget::SettingsRow { token, row },
                );
            }
            let token = editor.view().unwrap().token();
            assert_pointer_band(
                &mut editor,
                locale,
                &theme,
                HitTarget::SettingsControl {
                    token,
                    control: Control::Close,
                },
            );
        }
    }
}

#[test]
fn picker_labels_checked_markers_and_navigation_share_the_winning_pointer_band() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::orchbee_light(),
            Theme::orchbee_dark(),
            Theme::orchbee_dark().quantize(ColorLevel::Ansi16),
            Theme::terminal_native().quantize(ColorLevel::Monochrome),
        ] {
            let mut editor = editor_with(UserPreferencesValues::default());
            select_and_present(&mut editor, Row::Theme, locale, &theme);
            editor.edit(Edit::Activate);
            for choice in Choice::options(Row::Theme) {
                let token = editor.view().unwrap().token();
                assert_pointer_band(
                    &mut editor,
                    locale,
                    &theme,
                    HitTarget::SettingsControl {
                        token,
                        control: Control::Choose(*choice),
                    },
                );
            }
        }
    }
}
