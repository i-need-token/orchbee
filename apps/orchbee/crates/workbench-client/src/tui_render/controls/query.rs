//! One-row overlay query slot: the activation key at rest and `label: draft` while editing.

use crate::{
    terminal_text::{
        display_width, project_terminal_text, sanitize_terminal_text, truncate_display,
    },
    tui_theme::{GlyphMode, Role, Theme},
};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

/// Draft cells an editing query keeps before its label starts to truncate.
const MIN_DRAFT_WIDTH: usize = 8;

/// Paints the one-cell rule that separates a query slot from the content it filters.
pub(crate) fn render_divider(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let glyph = match theme.glyphs.mode() {
        GlyphMode::Unicode => "─",
        #[cfg(test)]
        GlyphMode::Simplified => "-",
    };
    frame.render_widget(
        Paragraph::new(glyph.repeat(usize::from(area.width)))
            .style(theme.style(Role::FrameDivider)),
        area,
    );
}

/// The owner's current query presentation; the component never parses either text field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QueryField<'a> {
    /// Not editing and the query is empty: the formatted activation key and its localized prompt,
    /// both quiet so the resting slot never competes with the content below it.
    Rest { key: &'a str, prompt: &'a str },
    /// Not editing while a query remains applied: the label and that query.
    Retained { label: &'a str, query: &'a str },
    /// Editing: the label, the raw draft, and the raw cursor byte offset inside it.
    Editing {
        label: &'a str,
        query: &'a str,
        cursor: usize,
    },
}

/// Paints one query row across `area` and returns the painted row.
///
/// While editing, the label keeps secondary hierarchy, the draft scrolls to keep its cursor
/// visible, and the terminal cursor follows the draft cursor. A row that cannot keep the label and
/// one draft cell refuses before painting.
pub(crate) fn render_query_field(
    frame: &mut Frame<'_>,
    area: Rect,
    field: QueryField<'_>,
    theme: &Theme,
) -> Option<Rect> {
    if area.is_empty() || frame.area().intersection(area) != area {
        return None;
    }
    let row = Rect::new(area.x, area.y, area.width, 1);
    let width = usize::from(row.width);
    match field {
        QueryField::Rest { key, prompt } => {
            let key = sanitize_terminal_text(key);
            let key_width = display_width(&key);
            if key_width == 0 || key_width > width {
                return None;
            }
            let prompt = truncate_display(
                &sanitize_terminal_text(prompt),
                width.saturating_sub(key_width + 1),
                theme.glyphs.ellipsis,
            );
            frame.render_widget(
                Paragraph::new(format!("{key} {prompt}")).style(theme.style(Role::TextSecondary)),
                row,
            );
        }
        QueryField::Retained { label, query } => {
            let text = format!(
                "{}: {}",
                sanitize_terminal_text(label),
                sanitize_terminal_text(query)
            );
            frame.render_widget(
                Paragraph::new(truncate_display(&text, width, theme.glyphs.ellipsis))
                    .style(theme.style(Role::TextSecondary)),
                row,
            );
        }
        QueryField::Editing {
            label,
            query,
            cursor,
        } => {
            // The label yields only what the draft needs to stay legible beside it.
            let label = truncate_display(
                &sanitize_terminal_text(label),
                width.saturating_sub(MIN_DRAFT_WIDTH + 2).max(1),
                theme.glyphs.ellipsis,
            );
            let prefix = format!("{label}: ");
            let prefix_width = display_width(&prefix);
            // The draft keeps at least one cell, which also hosts the cursor at its end.
            let value_width = width.checked_sub(prefix_width).filter(|width| *width > 0)?;
            let projection = project_terminal_text(query);
            let displayed = projection.display();
            let cursor_column = display_width(&displayed[..projection.cursor_boundary(cursor)]);
            let left = cursor_column.saturating_sub(value_width.saturating_sub(1));
            let value_area = Rect::new(
                row.x + u16::try_from(prefix_width).ok()?,
                row.y,
                u16::try_from(value_width).ok()?,
                1,
            );
            frame.render_widget(
                Paragraph::new(prefix).style(theme.style(Role::TextSecondary)),
                row,
            );
            frame.render_widget(
                Paragraph::new(displayed.as_ref())
                    .scroll((0, u16::try_from(left).unwrap_or(u16::MAX)))
                    .style(theme.style(Role::TextPrimary)),
                value_area,
            );
            frame.set_cursor_position((
                value_area.x + u16::try_from(cursor_column - left).unwrap_or(0),
                value_area.y,
            ));
        }
    }
    Some(row)
}
