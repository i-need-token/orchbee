//! Shared sidebar text projection; no observation or navigation authority.
use super::{ClientView, Line, Locale, Modifier, Role, Span, Theme, truncate_display};
use crate::{
    machine_discovery::display_label,
    machine_picker::{Choice, Row},
};
use orchbee_workbench_protocol::MetadataState;
use ratatui::style::Style;

fn semantic_role(style: Style, theme: &Theme) -> Option<Role> {
    [
        Role::TextPrimary,
        Role::TextSecondary,
        Role::TextDisabled,
        Role::HintKey,
        Role::HintLabel,
        Role::StatusSuccess,
        Role::StatusWarning,
        Role::StatusError,
        Role::StatusInfo,
        Role::StatusPending,
        Role::StatusWorking,
        Role::StatusUnread,
    ]
    .into_iter()
    .find(|role| {
        let semantic = theme.style(*role);
        style.fg == semantic.fg
            && style.add_modifier.contains(Modifier::DIM)
                == semantic.add_modifier.contains(Modifier::DIM)
    })
}

/// Compose one row surface without inheriting the row painter's bold across supporting lines.
/// Explicit token emphasis remains authoritative; pressed emphasis belongs to identity.
pub(super) fn compose_lines(
    lines: &mut [Line<'static>],
    interaction: Option<Role>,
    emphasize_identity: bool,
    theme: &Theme,
) {
    for (index, line) in lines.iter_mut().enumerate() {
        let line_role = semantic_role(line.style, theme);
        line.style = theme
            .content_on_surface(Role::SurfaceChrome, interaction, line_role, line.style)
            .remove_modifier(Modifier::BOLD);
        for span in &mut line.spans {
            let content_bold = span.style.add_modifier.contains(Modifier::BOLD);
            let explicitly_plain = span.style.sub_modifier.contains(Modifier::BOLD);
            let role = semantic_role(span.style, theme);
            span.style =
                theme.content_on_surface(Role::SurfaceChrome, interaction, role, span.style);
            // Modifier-only surfaces can add bold to each span as well as the Line. Preserve
            // authored span emphasis and interaction fallbacks, without promoting current names
            // or metadata. Current context retains its fill, underline and independent marker.
            let identity_bold = index == 0
                && matches!(role, None | Some(Role::TextPrimary))
                && (emphasize_identity
                    || interaction != Some(Role::SurfaceCurrent)
                        && span.style.add_modifier.contains(Modifier::BOLD));
            if !explicitly_plain && (content_bold || identity_bold) {
                span.style = span.style.add_modifier(Modifier::BOLD);
            } else {
                span.style = span.style.remove_modifier(Modifier::BOLD);
            }
        }
    }
}

pub(super) fn state_role(state: orchbee_workbench_protocol::MetadataState) -> (&'static str, Role) {
    match state {
        MetadataState::Blocked => ("agent-blocked", Role::StatusWarning),
        MetadataState::Done => ("agent-done", Role::StatusSuccess),
        MetadataState::Error => ("agent-error", Role::StatusError),
        MetadataState::Working => ("agent-working", Role::StatusWorking),
        MetadataState::Idle => ("agent-idle", Role::TextSecondary),
        MetadataState::Unknown => ("agent-unknown", Role::TextSecondary),
    }
}
/// Working cycles through the theme's equal-width frames at the Sidebar phase; other states are
/// static.
pub(super) fn state_glyph(
    state: MetadataState,
    theme: &Theme,
    working_frame: usize,
) -> &'static str {
    let frames = theme.glyphs.working_frames;
    if state == MetadataState::Working && !frames.is_empty() {
        frames[working_frame % frames.len()]
    } else {
        theme.agent_glyph(state)
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
