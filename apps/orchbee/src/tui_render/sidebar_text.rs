//! Shared, style-preserving sidebar text projection; no observation or navigation authority.
use super::*;
use crate::{
    machine_discovery::display_label,
    machine_picker::{Choice, Row},
};
use orchbee_workbench_protocol::MetadataState;
pub(super) fn state_role(state: orchbee_workbench_protocol::MetadataState) -> (&'static str, Role) {
    match state {
        MetadataState::Blocked => ("agent-blocked", Role::StatusWarning),
        MetadataState::Done => ("agent-done", Role::StatusSuccess),
        MetadataState::Working => ("agent-working", Role::StatusInfo),
        MetadataState::Idle => ("agent-idle", Role::TextPrimary),
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
