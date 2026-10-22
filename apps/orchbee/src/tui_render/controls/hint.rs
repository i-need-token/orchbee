//! Ordered keyboard hints with semantic key and localized-label styling.

use crate::{
    terminal_text::{display_width, sanitize_terminal_text},
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

/// One owner-ordered shortcut hint.
///
/// `key` is an already formatted effective binding label. `label` is its localized verb. The
/// component paints both fields but never parses either one to recover an action or binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct KeyHint<'a> {
    key: &'a str,
    label: &'a str,
}

impl<'a> KeyHint<'a> {
    pub(crate) const fn new(key: &'a str, label: &'a str) -> Self {
        Self { key, label }
    }
}

/// Visual boundary between adjacent hints, chosen by the surface that owns the hint line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Separator {
    /// A middle dot for persistent chrome such as the status line and sidebar footer.
    Dot,
    /// A spaced vertical rule for an overlay footer.
    Rule,
}

#[derive(Debug, Eq, PartialEq)]
struct ResolvedHint {
    key: String,
    label: String,
    width: usize,
}

#[derive(Debug, Eq, PartialEq)]
struct ResolvedHints {
    items: Vec<ResolvedHint>,
    separator: String,
    separator_width: usize,
    width: usize,
}

/// Measures the complete ordered hint line after terminal-safe projection.
///
/// Empty or unsafe-to-empty fields and arithmetic overflow reject the whole set.
pub(crate) fn measure_key_hints(items: &[KeyHint<'_>], theme: &Theme) -> Option<usize> {
    resolve_hints(items, Separator::Dot, theme).map(|resolved| resolved.width)
}

/// Measures an overlay footer, whose hints are separated by spaced vertical rules.
pub(crate) fn measure_overlay_hints(items: &[KeyHint<'_>], theme: &Theme) -> Option<usize> {
    resolve_hints(items, Separator::Rule, theme).map(|resolved| resolved.width)
}

/// Paints the longest ordered prefix of complete hints and returns its exact extent.
///
/// The owner supplies priority order, normally with the local exit path first. Every item is
/// validated before mutation. If the first item does not fit, or if the offered area is clipped,
/// this function returns `None` without painting.
pub(crate) fn render_key_hints(
    frame: &mut Frame<'_>,
    area: Rect,
    items: &[KeyHint<'_>],
    theme: &Theme,
) -> Option<Rect> {
    render_hints(frame, area, items, Separator::Dot, theme)
}

/// Paints an overlay footer with the same whole-item prefix contract as [`render_key_hints`].
pub(crate) fn render_overlay_hints(
    frame: &mut Frame<'_>,
    area: Rect,
    items: &[KeyHint<'_>],
    theme: &Theme,
) -> Option<Rect> {
    render_hints(frame, area, items, Separator::Rule, theme)
}

fn render_hints(
    frame: &mut Frame<'_>,
    area: Rect,
    items: &[KeyHint<'_>],
    separator: Separator,
    theme: &Theme,
) -> Option<Rect> {
    if area.is_empty() || frame.area().intersection(area) != area {
        return None;
    }
    let resolved = resolve_hints(items, separator, theme)?;
    let mut width: usize = 0;
    let mut visible: usize = 0;
    for item in &resolved.items {
        let separator_width = if visible == 0 {
            0
        } else {
            resolved.separator_width
        };
        let next = width
            .checked_add(separator_width)?
            .checked_add(item.width)?;
        if next > usize::from(area.width) {
            break;
        }
        width = next;
        visible += 1;
    }
    if visible == 0 {
        return None;
    }
    let width = u16::try_from(width).ok()?;
    let rect = Rect::new(area.x, area.y, width, 1);
    if area.intersection(rect) != rect || frame.area().intersection(rect) != rect {
        return None;
    }

    let mut spans = Vec::with_capacity(visible.saturating_mul(3));
    for (index, item) in resolved.items.into_iter().take(visible).enumerate() {
        if index > 0 {
            spans.push(Span::styled(
                resolved.separator.clone(),
                theme.style(Role::HintLabel),
            ));
        }
        spans.push(Span::styled(item.key, theme.style(Role::HintKey)));
        spans.push(Span::styled(
            format!(" {}", item.label),
            theme.style(Role::HintLabel),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), rect);
    Some(rect)
}

fn resolve_hints(
    items: &[KeyHint<'_>],
    separator: Separator,
    theme: &Theme,
) -> Option<ResolvedHints> {
    if items.is_empty() {
        return None;
    }
    let (separator_glyph, padding) = match separator {
        Separator::Dot => (theme.glyphs.middle_dot, " "),
        Separator::Rule => (theme.glyphs.vertical_rule, "  "),
    };
    let separator_glyph = sanitize_terminal_text(separator_glyph);
    let separator_glyph_width = display_width(&separator_glyph);
    if separator_glyph_width == 0 {
        return None;
    }
    let separator = format!("{padding}{separator_glyph}{padding}");
    let separator_width = separator_glyph_width.checked_add(padding.len() * 2)?;
    let mut resolved = Vec::with_capacity(items.len());
    let mut width: usize = 0;
    for (index, item) in items.iter().enumerate() {
        let key = sanitize_terminal_text(item.key);
        let label = sanitize_terminal_text(item.label);
        let key_width = display_width(&key);
        let label_width = display_width(&label);
        if key_width == 0 || label_width == 0 {
            return None;
        }
        let item_width = key_width.checked_add(1)?.checked_add(label_width)?;
        if index > 0 {
            width = width.checked_add(separator_width)?;
        }
        width = width.checked_add(item_width)?;
        resolved.push(ResolvedHint {
            key: key.into_owned(),
            label: label.into_owned(),
            width: item_width,
        });
    }
    Some(ResolvedHints {
        items: resolved,
        separator,
        separator_width,
        width,
    })
}

#[cfg(test)]
mod tests;
