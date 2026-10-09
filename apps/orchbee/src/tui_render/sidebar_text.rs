//! Shared sidebar text and row-height projection; no observation or navigation authority.
use super::*;
use crate::{
    machine_discovery::display_label,
    machine_picker::{Choice, Row},
};
use orchbee_workbench_protocol::MetadataState;

pub(super) fn list_height(heights: impl IntoIterator<Item = u16>, gap: u16, chrome: u16) -> u16 {
    let mut total = chrome;
    let mut populated = false;
    for height in heights {
        if populated {
            total = total.saturating_add(gap);
        }
        total = total.saturating_add(height);
        populated = true;
    }
    if populated {
        total
    } else {
        total.saturating_add(1)
    }
}

/// Keep section actions and counts out of the item hierarchy. The full heading
/// retains the section's existing picker target, including the blank cells.
pub(super) fn section_heading(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    count: &str,
    action: &str,
    theme: &Theme,
) {
    let action_width = display_width(action).min(usize::from(area.width));
    let left_width = usize::from(area.width).saturating_sub(action_width + 2);
    let count = if count.is_empty() {
        String::new()
    } else {
        format!(" {count}")
    };
    let title = truncate_display(
        title,
        left_width.saturating_sub(display_width(&count)),
        theme.glyphs.ellipsis,
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                title,
                theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD),
            ),
            Span::styled(count, theme.style(Role::TextSecondary)),
        ])),
        Rect::new(area.x, area.y, left_width as u16, 1),
    );
    frame.render_widget(
        Paragraph::new(action)
            .alignment(Alignment::Right)
            .style(theme.style(Role::HintLabel)),
        Rect::new(
            area.right() - action_width as u16,
            area.y,
            action_width as u16,
            1,
        ),
    );
}

pub(super) fn state_role(state: orchbee_workbench_protocol::MetadataState) -> (&'static str, Role) {
    match state {
        MetadataState::Blocked => ("agent-blocked", Role::StatusWarning),
        MetadataState::Done => ("agent-done", Role::StatusSuccess),
        MetadataState::Working => ("agent-working", Role::StatusInfo),
        MetadataState::Idle => ("agent-idle", Role::TextSecondary),
        MetadataState::Unknown => ("agent-unknown", Role::TextSecondary),
    }
}
/// Preserve style boundaries while truncating the entire line at a grapheme boundary.
pub(super) fn fit(line: Line<'static>, width: usize, ellipsis: &str) -> Line<'static> {
    let text = line
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<String>();
    let clipped = truncate_display(&text, width, ellipsis);
    if clipped == text {
        return line;
    }
    let mut remaining = clipped
        .strip_suffix(ellipsis)
        .map_or(clipped.len(), str::len);
    let mut spans = Vec::new();
    for span in line.spans {
        let length = remaining.min(span.content.len());
        if length > 0 {
            spans.push(Span::styled(span.content[..length].to_owned(), span.style));
        }
        remaining -= length;
        if remaining == 0 {
            break;
        }
    }
    if clipped.ends_with(ellipsis) {
        spans.push(Span::styled(ellipsis.to_owned(), line.style));
    }
    Line { spans, ..line }
}

pub(super) fn machine_label(view: &ClientView, row: &Row, locale: Locale) -> String {
    if row.choice == Choice::Local {
        return locale.machine_picker_text("local").into();
    }
    let label = display_label(&row.label);
    if let Choice::Remote(id) = &row.choice
        && view
            .machines
            .rows
            .iter()
            .filter(|other| other.enabled && display_label(&other.label) == label)
            .count()
            > 1
    {
        let width = (8..=32).step_by(4).find(|width| view.machines.rows.iter().filter(|other|
            matches!(&other.choice, Choice::Remote(other_id) if other.enabled && other_id.as_str()[..*width] == id.as_str()[..*width]))
            .count() == 1).unwrap_or(32);
        return format!("{label} [{}]", &id.as_str()[..width]);
    }
    label
}
