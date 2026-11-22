//! Shared sidebar text projection; no observation or navigation authority.
use super::{ClientView, Line, Locale, Modifier, Role, Span, Theme, truncate_display};
use crate::{
    machine_discovery::display_label,
    machine_picker::{Choice, Row},
};
use orchbee_workbench_protocol::MetadataState;
use ratatui::style::Style;

/// A machine name shortens in its middle and keeps its last three cells, because fleet names
/// differ at their end: `gpu-render-farm-07` becomes `gpu-ren…-07`, never `gpu-render-…`.
pub(super) fn truncate_machine_name(name: &str, budget: usize, ellipsis: &str) -> String {
    const TAIL: usize = 3;
    if crate::terminal_text::display_width(name) <= budget {
        return name.to_owned();
    }
    let tail = TAIL + crate::terminal_text::display_width(ellipsis);
    if budget <= tail {
        return truncate_display(name, budget, ellipsis);
    }
    let mut text = truncate_display(name, budget - tail, "");
    text.push_str(&crate::terminal_text::truncate_display_start(
        name, tail, ellipsis,
    ));
    text
}

/// Cells a remote Workspace's machine name gets beside its name, in the marker's order of
/// giving way: both whole; a machine name over twelve cells shortens first, to twelve; then the
/// Workspace name, to four cells; then the machine name again, to four cells. `text` is what
/// the line's other text wants and `text_floor` the cells its Workspace name keeps.
pub(super) fn marker_cells(full: usize, text: usize, text_floor: usize, available: usize) -> usize {
    const CAP: usize = 12;
    const FLOOR: usize = 4;
    let cap = full.min(CAP);
    if text + full <= available {
        full
    } else if text + cap <= available {
        available - text
    } else if text_floor + cap <= available {
        cap
    } else {
        available
            .saturating_sub(text_floor)
            .clamp(full.min(FLOOR), cap)
    }
}

fn semantic_role(style: Style, theme: &Theme) -> Option<Role> {
    [
        Role::TextPrimary,
        Role::TextSecondary,
        Role::TextMuted,
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
///
/// A bold reverse band, Monochrome's provisional selection, is the exception: a host that
/// brightens bold text shows its cells on a lighter background than plain ones, so every cell
/// that is not explicitly plain takes the band's emphasis and the row keeps one background.
pub(super) fn compose_lines(
    lines: &mut [Line<'static>],
    interaction: Option<Role>,
    emphasize_identity: bool,
    theme: &Theme,
) {
    let band_emphasis = interaction.is_some_and(|role| {
        theme
            .style(role)
            .add_modifier
            .contains(Modifier::REVERSED | Modifier::BOLD)
    });
    for (index, line) in lines.iter_mut().enumerate() {
        let line_role = semantic_role(line.style, theme);
        let composed =
            theme.content_on_surface(Role::SurfaceChrome, interaction, line_role, line.style);
        line.style = if band_emphasis {
            composed
        } else {
            composed.remove_modifier(Modifier::BOLD)
        };
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
            if !explicitly_plain && (content_bold || identity_bold || band_emphasis) {
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
        MetadataState::Idle => ("agent-idle", Role::StatusIdle),
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
