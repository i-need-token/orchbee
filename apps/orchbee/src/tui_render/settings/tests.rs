//! Display-width boundaries and exact row hit regions use the same completed frame.
use super::*;
use crate::native_settings::{Basis, Editor, Values};
use orchbee_workbench_protocol::{ConfigurationThemeName, NativeSettingsSource, WorkbenchRevision};
use ratatui::{Terminal, backend::TestBackend};
fn draft() -> View {
    let mut editor = Editor::default();
    let token = editor.open().unwrap();
    assert!(editor.loaded(
        token,
        Basis {
            revision: WorkbenchRevision::new(1),
            source: NativeSettingsSource::parse("missing").unwrap(),
            values: Values {
                theme: ConfigurationThemeName::OneLight,
                reminders: true,
                sound: false
            }
        }
    ));
    editor.view().unwrap()
}
#[test]
fn settings_rows_and_explicit_save_fit_both_locales_themes_and_admitted_boundaries() {
    for (width, height) in [(36, 10), (58, 10), (80, 24), (120, 40)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for mut theme in [
                Theme::terminal_native(),
                Theme::one_light(),
                Theme::one_dark(),
            ] {
                for monochrome in [false, true] {
                    if monochrome {
                        theme = theme.quantize(crate::tui_theme::ColorLevel::Monochrome);
                    }
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let mut hits = HitMapBuilder::new();
                    let view = draft();
                    let mut receipt = ModalPresentation::NotPresented;
                    terminal
                        .draw(|frame| {
                            receipt =
                                render(frame, frame.area(), view, false, locale, &theme, &mut hits);
                        })
                        .unwrap();
                    assert_eq!(receipt, ModalPresentation::Presented(view.token()));
                    let text: String = terminal
                        .backend()
                        .buffer()
                        .content
                        .iter()
                        .map(|cell| cell.symbol())
                        .collect();
                    assert!(text.contains("One Light"));
                    assert!(
                        text.replace(' ', "")
                            .contains(&locale.settings_row(Row::Save).replace(' ', "")),
                        "{width}x{height}: {text}"
                    );
                    assert!(text.contains("Esc"));
                    let map = hits.finish();
                    let x = (width - width.min(58)) / 2 + 2;
                    let y = (height - 10) / 2 + 1;
                    for row in Row::ALL {
                        assert_eq!(
                            map.hit_target_at(x, y + row.index() as u16),
                            Some(&HitTarget::SettingsRow {
                                token: view.token(),
                                row
                            })
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn settings_refuses_incomplete_geometry_and_delays_pending_copy() {
    for (width, height) in [(35, 10), (36, 9), (1, 1)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut result = ModalPresentation::Presented(draft().token());
        terminal
            .draw(|frame| {
                result = render(
                    frame,
                    frame.area(),
                    draft(),
                    false,
                    Locale::EnUs,
                    &Theme::terminal_native(),
                    &mut HitMapBuilder::new(),
                );
            })
            .unwrap();
        assert_eq!(result, ModalPresentation::NotPresented);
    }
    let mut editor = Editor::default();
    editor.open().unwrap();
    for pending in [false, true] {
        let mut terminal = Terminal::new(TestBackend::new(58, 10)).unwrap();
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    editor.view().unwrap(),
                    pending,
                    Locale::EnUs,
                    &Theme::terminal_native(),
                    &mut HitMapBuilder::new(),
                );
            })
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert_eq!(text.contains("[..] Opening"), pending);
    }
}
