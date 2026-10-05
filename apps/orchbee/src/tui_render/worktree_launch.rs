//! Draw the complete Worktree decision before arming its input token.
use super::tab_close::{modal_area, set_modal_cursor, wrapped_text_fits};
use super::{
    Block, Borders, Clear, Frame, Line, Locale, ModalPresentation, Padding, Paragraph, Rect, Role,
    Theme, Wrap, display_width, truncate_display,
};
use crate::terminal_session::{WorktreeNotice, WorktreeStage, WorktreeToken, WorktreeView};
use crate::terminal_text::sanitize_terminal_text;
use ratatui::text::Text;
use unicode_segmentation::UnicodeSegmentation;

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    view: WorktreeView<'_>,
    content: Rect,
    theme: &Theme,
) -> (Rect, ModalPresentation<WorktreeToken>) {
    let stage = view.stage();
    let budget = usize::from(content.width.saturating_sub(10)).min(72);
    let mut lines = vec![
        Line::from(locale.worktree_title(view.mode())),
        Line::from(""),
    ];
    if let Some(source) = view.source_path() {
        lines.push(Line::from(path_text(source, budget, theme.glyphs.ellipsis)));
    }
    let mut field = None;
    match stage {
        WorktreeStage::Branch | WorktreeStage::Base => {
            lines.push(Line::from(locale.worktree_field(WorktreeStage::Branch)));
            let branch = lines.len();
            lines.push(Line::from(truncate_display(
                view.branch(),
                budget,
                theme.glyphs.ellipsis,
            )));
            lines.push(Line::from(locale.worktree_field(WorktreeStage::Base)));
            let base = lines.len();
            lines.push(Line::from(truncate_display(
                view.base(),
                budget,
                theme.glyphs.ellipsis,
            )));
            field = Some(if stage == WorktreeStage::Base {
                base
            } else {
                branch
            });
        }
        WorktreeStage::Project | WorktreeStage::Checkout => {
            lines.push(Line::from(locale.worktree_field(stage)));
            let start = view.selected().saturating_sub(1);
            for index in start..view.choice_count().min(start + 3) {
                if let Some((label, identity)) = view.choice(index) {
                    lines.push(Line::from(format!(
                        "{} {}",
                        if index == view.selected() { ">" } else { " " },
                        truncate_display(&label, budget.saturating_sub(2), theme.glyphs.ellipsis)
                    )));
                    lines.push(Line::from(if stage == WorktreeStage::Project {
                        identity.to_owned()
                    } else {
                        path_text(identity, budget, theme.glyphs.ellipsis)
                    }));
                }
            }
            lines.push(Line::from(format!(
                "{}/{}",
                view.selected() + 1,
                view.choice_count()
            )));
        }
        _ => {}
    }
    if let Some(id) = view.operation_id() {
        lines.push(Line::from(format!("orchbee worktree status {id}")));
    }
    if let Some(project) = view.retained_project() {
        lines.push(Line::from(format!(
            "{}: {project}",
            locale.worktree_project()
        )));
    }
    if let Some(request) = view.project_request() {
        lines.push(Line::from(format!(
            "{}: {request}",
            locale.worktree_request()
        )));
    }
    if let Some(workspace) = view.workspace() {
        lines.push(Line::from(format!(
            "{}: {workspace}",
            locale.worktree_workspace()
        )));
    }
    if view.notice() != WorktreeNotice::Preparing || stage == WorktreeStage::Preparing {
        lines.push(Line::from(
            locale.worktree_stage_notice(view.notice(), stage),
        ));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(locale.worktree_actions(stage)));
    let area = modal_area(content, &Text::from(lines.clone()));
    let block = Block::new()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(2))
        .style(
            theme
                .style(Role::SurfaceOverlay)
                .fg(theme.color(Role::TextPrimary)),
        )
        .border_style(theme.style(Role::AccentFocus));
    let inner = block.inner(area);
    let window = field.and_then(|_| view.field_window(usize::from(inner.width)));
    if let (Some(index), Some(window)) = (field, window.as_ref()) {
        lines[index] = Line::from(window.display().to_owned());
    }
    let text = Text::from(lines);
    let complete = wrapped_text_fits(&text, inner);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if complete {
        frame.render_widget(
            Paragraph::new(text.clone()).wrap(Wrap { trim: false }),
            inner,
        );
        if let (Some(index), Some(window)) = (field, window.as_ref()) {
            set_modal_cursor(frame, &text, inner, index, window.cursor_column());
        }
    } else {
        frame.render_widget(
            Paragraph::new(locale.setup_resize_hint()).wrap(Wrap { trim: false }),
            inner,
        );
    }
    (
        area,
        if complete {
            ModalPresentation::Presented(view.token())
        } else {
            ModalPresentation::NotPresented
        },
    )
}
fn path_text(path: &str, budget: usize, marker: &str) -> String {
    let clean = sanitize_terminal_text(path);
    if display_width(&clean) <= budget {
        return clean.into_owned();
    }
    let available = budget.saturating_sub(display_width(marker));
    let prefix = truncate_display(&clean, available / 2, "");
    let mut suffix = Vec::new();
    let mut width = 0;
    for g in clean.graphemes(true).rev() {
        let next = width + display_width(g);
        if next > available - available / 2 {
            break;
        }
        suffix.push(g);
        width = next;
    }
    format!(
        "{prefix}{marker}{}",
        suffix.into_iter().rev().collect::<String>()
    )
}
