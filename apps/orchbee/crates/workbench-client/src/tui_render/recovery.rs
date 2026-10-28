//! Geometry feedback with a complete, owner-supplied local return hint.

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

use crate::{
    client_view::{
        Binding, KeyCode, KeyLabelGlyphs, LabelStyle, Modifiers, format_binding_with_glyphs,
    },
    terminal_text::display_width,
    tui_theme::{GlyphMode, Role, Theme},
};

use super::controls::{KeyHint, measure_key_hints, render_key_hints};

/// The informational message yields space to the complete local return path.
/// This is a keyboard hint, not an actionable pointer control or admission proof.
pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    notice: &str,
    local_return: Option<&str>,
    theme: &Theme,
) {
    if area.is_empty() || frame.area().intersection(area) != area {
        return;
    }
    frame.render_widget(Clear, area);
    frame
        .buffer_mut()
        .set_style(area, theme.style(Role::SurfaceChrome));
    let key_glyphs = match theme.glyphs.mode() {
        GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
        #[cfg(any(test, feature = "test-support"))]
        GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
    };
    let key = format_binding_with_glyphs(
        &Binding::new(KeyCode::Esc, Modifiers::NONE),
        LabelStyle::for_host(),
        key_glyphs,
    );
    let hint = local_return.map(|label| KeyHint::new(&key, label));
    let hint_width = hint
        .as_ref()
        .and_then(|hint| measure_key_hints(&[*hint], theme));
    let hint_width = hint_width
        .and_then(|width| u16::try_from(width).ok())
        .filter(|width| *width <= area.width);
    if let (Some(hint), Some(width)) = (hint, hint_width) {
        let hint_area = Rect::new(area.right() - width, area.y, width, 1);
        render_key_hints(frame, hint_area, &[hint], theme);
    }
    let marker = format!("{} ", theme.glyphs.status_bullet);
    let notice_width = display_width(&marker).checked_add(display_width(notice));
    let remaining = area
        .width
        .saturating_sub(hint_width.map_or(0, |width| width.saturating_add(2)));
    if notice_width.is_some_and(|width| width <= usize::from(remaining)) {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(marker, theme.style(Role::StatusWarning)),
                Span::styled(notice, theme.style(Role::TextPrimary)),
            ])),
            Rect::new(area.x, area.y, remaining, 1),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{localization::Locale, terminal_text::display_width, tui_theme::GlyphSet};
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn narrowing_preserves_complete_local_return_and_never_crops_a_key_hint() {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for glyphs in [GlyphSet::UTF8, GlyphSet::ASCII] {
                let mut theme = Theme::rgb_test_dark();
                theme.glyphs = glyphs;
                let label = locale.recovery_verb(crate::localization::RecoveryVerb::Close);
                let complete_hint = format!("Esc {label}");
                for width in [3, 8, 9, 16, 40, 80] {
                    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
                    terminal
                        .draw(|frame| {
                            render(
                                frame,
                                frame.area(),
                                locale.presentation_notice(true),
                                Some(label),
                                &theme,
                            );
                        })
                        .unwrap();
                    let row = terminal
                        .backend()
                        .buffer()
                        .content()
                        .iter()
                        .map(|cell| cell.symbol())
                        .collect::<String>();
                    let normalized = row.replace(' ', "");
                    if display_width(&complete_hint) <= usize::from(width) {
                        assert!(
                            normalized.ends_with(&complete_hint.replace(' ', "")),
                            "{locale:?} {width}: {row:?}"
                        );
                    } else {
                        assert!(!row.contains("Esc"), "{locale:?} {width}: {row:?}");
                    }
                    assert!(
                        !row.contains("Ctrl+"),
                        "recovery must remain local: {row:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn first_refusal_has_no_phantom_local_return_and_clipped_offer_is_untouched() {
        let theme = Theme::rgb_test_dark();
        let mut terminal = Terminal::new(TestBackend::new(80, 1)).unwrap();
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    Locale::EnUs.presentation_notice(false),
                    None,
                    &theme,
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Cannot open here: enlarge the terminal."));
        assert!(!text.contains("Esc"));
        let marker_width = display_width(theme.glyphs.status_bullet);
        assert_eq!(buffer[(0, 0)].fg, theme.color(Role::StatusWarning));
        assert_eq!(
            buffer[(u16::try_from(marker_width + 1).unwrap(), 0)].fg,
            theme.color(Role::TextPrimary)
        );

        terminal
            .draw(|frame| {
                frame
                    .buffer_mut()
                    .set_string(0, 0, "unchanged", theme.style(Role::TextPrimary));
                let before = frame.buffer_mut().clone();
                render(
                    frame,
                    Rect::new(79, 0, 2, 1),
                    "refused",
                    Some("Close"),
                    &theme,
                );
                assert_eq!(*frame.buffer_mut(), before);
            })
            .unwrap();
    }
}
