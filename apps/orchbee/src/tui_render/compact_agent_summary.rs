//! Compact top-strip summary of the existing machine-sidebar Agent observations.
use super::{Frame, Locale, Paragraph, Rect, Role, Span, Theme, display_width, truncate_display};
use crate::machine_picker::{AgentSourceCoverage, CompactAgentSummary};

const MIN_TAB_STRIP_CELLS: u16 = 7;
const MAX_SUMMARY_CELLS: usize = 48;

pub(super) fn render(
    frame: &mut Frame<'_>,
    title_area: Rect,
    summary: CompactAgentSummary<'_>,
    locale: Locale,
    theme: &Theme,
) -> Rect {
    if title_area.height == 0 || title_area.width <= MIN_TAB_STRIP_CELLS {
        return title_area;
    }
    let offered =
        usize::from(title_area.width.saturating_sub(MIN_TAB_STRIP_CELLS)).min(MAX_SUMMARY_CELLS);
    if offered == 0 {
        return title_area;
    }
    let label = summary_text(summary, locale, offered, theme);
    let width = u16::try_from(display_width(&label)).unwrap_or(u16::MAX);
    if width == 0 {
        return title_area;
    }
    let summary_area = Rect::new(
        title_area.right().saturating_sub(width),
        title_area.y,
        width,
        1,
    );
    frame.render_widget(
        Paragraph::new(Span::styled(label, theme.style(summary_role(summary))))
            .alignment(ratatui::layout::Alignment::Right),
        summary_area,
    );
    Rect::new(
        title_area.x,
        title_area.y,
        title_area.width.saturating_sub(width),
        title_area.height,
    )
}

fn summary_role(summary: CompactAgentSummary<'_>) -> Role {
    match summary {
        CompactAgentSummary::Observed {
            coverage: AgentSourceCoverage::Unavailable,
            ..
        }
        | CompactAgentSummary::Unavailable => Role::StatusWarning,
        CompactAgentSummary::Observed {
            coverage: AgentSourceCoverage::Loading,
            ..
        } => Role::StatusWarning,
        CompactAgentSummary::Loading => Role::TextSecondary,
        CompactAgentSummary::Observed { row, .. } => {
            super::sidebar_text::state_role(row.display_state()).1
        }
        CompactAgentSummary::Empty | CompactAgentSummary::NoMatches => Role::TextSecondary,
    }
}

fn summary_text(
    summary: CompactAgentSummary<'_>,
    locale: Locale,
    width: usize,
    theme: &Theme,
) -> String {
    let text = |key| locale.machine_picker_text(key).to_owned();
    match summary {
        CompactAgentSummary::Observed { row, coverage } => {
            let (state_key, _) = super::sidebar_text::state_role(row.display_state());
            let state = text(state_key);
            let full = format!("{}: {state}", text("agent-summary-observed"));
            let short = format!("{}: {state}", text("agent-summary-label"));
            let detail = match coverage {
                AgentSourceCoverage::Complete => None,
                AgentSourceCoverage::Loading => Some(text("agent-summary-sources-loading")),
                AgentSourceCoverage::Unavailable => Some(text("agent-summary-source-unavailable")),
            };
            if let Some(detail) = detail {
                let full = format!("{full} · {detail}");
                let short = format!("{short} · {detail}");
                if display_width(&full) <= width {
                    return append_detail(&full, row.title(), width, theme);
                }
                if display_width(&short) <= width {
                    return append_detail(&short, row.title(), width, theme);
                }
                let marker = match coverage {
                    AgentSourceCoverage::Loading => "~",
                    AgentSourceCoverage::Unavailable => "!",
                    AgentSourceCoverage::Complete => "",
                };
                let marked = format!("{short}{marker}");
                if display_width(&marked) <= width {
                    return marked;
                }
                let state = format!("{state}{marker}");
                return if display_width(&state) <= width {
                    state
                } else {
                    truncate_display(&state, width, "")
                };
            }
            if display_width(&full) <= width {
                return append_detail(&full, row.title(), width, theme);
            }
            if display_width(&short) <= width {
                return append_detail(&short, row.title(), width, theme);
            }
            if display_width(&state) <= width {
                state
            } else {
                truncate_display(&state, width, "")
            }
        }
        CompactAgentSummary::Empty
        | CompactAgentSummary::NoMatches
        | CompactAgentSummary::Loading
        | CompactAgentSummary::Unavailable => {
            let (long_state, short_state) = match summary {
                CompactAgentSummary::Empty => {
                    (text("agent-summary-none"), text("agent-summary-none-short"))
                }
                CompactAgentSummary::NoMatches => (
                    text("agent-summary-no-matches"),
                    text("agent-summary-no-matches"),
                ),
                CompactAgentSummary::Loading => {
                    (text("agent-summary-loading"), text("agent-summary-loading"))
                }
                CompactAgentSummary::Unavailable => (
                    text("agent-summary-unavailable"),
                    text("agent-summary-unavailable"),
                ),
                CompactAgentSummary::Observed { .. } => unreachable!(),
            };
            let short = format!("{}: {short_state}", text("agent-summary-label"));
            let full = if matches!(summary, CompactAgentSummary::Empty) {
                long_state
            } else {
                format!("{}: {long_state}", text("agent-summary-observed"))
            };
            if display_width(&full) <= width {
                full
            } else if display_width(&short) <= width {
                short
            } else if display_width(&short_state) <= width {
                short_state
            } else {
                truncate_display(&short_state, width, "")
            }
        }
    }
}

fn append_detail(base: &str, detail: &str, width: usize, theme: &Theme) -> String {
    let safe = crate::terminal_text::sanitize_terminal_text(detail);
    let detail = safe.as_ref();
    let base_width = display_width(base);
    if detail.is_empty() || width.saturating_sub(base_width) <= 3 {
        return base.to_owned();
    }
    let detail_width = width - base_width - 3;
    let detail = truncate_display(detail, detail_width, theme.glyphs.ellipsis);
    if detail.is_empty() {
        base.to_owned()
    } else {
        format!("{base} · {detail}")
    }
}

#[cfg(test)]
mod tests;
