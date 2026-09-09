use super::{EMPHASIS, SIDEBAR_BACKGROUND, TEXT, WARNING, tab_close::wrapped_text_fits};
use crate::{
    localization::Locale,
    terminal_session::{
        NativeDisplayPhase, NativeLocationLabelStatus, NativeSetupDraftView, NativeSetupStage,
        NativeSetupStatus, NativeSetupToken,
    },
    terminal_text::sanitize_terminal_text,
    tui_layout::centered_overlay,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Text},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
};

pub(super) fn directory_guidance(
    locale: Locale,
    phase: NativeDisplayPhase,
    retired: bool,
    path: Option<&str>,
    unconfigured: bool,
) -> Option<String> {
    if retired || phase != NativeDisplayPhase::Empty {
        return None;
    }
    path.map(|path| locale.setup_directory_ready(&sanitize_terminal_text(path)))
        .or_else(|| unconfigured.then(|| locale.setup_directory_needed().to_owned()))
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    draft: &NativeSetupDraftView<'_>,
    status: Option<NativeSetupStatus>,
) -> (Rect, Option<NativeSetupToken>) {
    let rename = match draft.stage() {
        NativeSetupStage::ProjectRename => Some(true),
        NativeSetupStage::WorkspaceRename => Some(false),
        _ => None,
    };
    let mut lines = vec![
        Line::from(locale.setup_title(draft.stage())),
        Line::from(""),
    ];
    if draft.stage() == NativeSetupStage::ChooseProject {
        for index in 0..draft.project_count() {
            if let Some(project) = draft.project_at(index) {
                let marker = if project.selected() { ">" } else { " " };
                let label =
                    sanitize_terminal_text(project.label().unwrap_or(locale.setup_new_project()));
                lines.push(Line::from(format!(
                    "{marker} {}. {label}",
                    project.position()
                )));
            }
        }
    } else {
        if let Some(project) = draft.project_label() {
            lines.push(Line::from(format!(
                "{}: {}",
                locale.setup_project(),
                sanitize_terminal_text(project)
            )));
            lines.push(Line::from(""));
        }
        if matches!(
            draft.stage(),
            NativeSetupStage::DirectoryPath | NativeSetupStage::WorkspaceRename
        ) && let Some(workspace) = draft.workspace_label()
        {
            lines.push(Line::from(format!(
                "{}: {}",
                locale.shell().workspaces,
                sanitize_terminal_text(workspace)
            )));
            lines.push(Line::from(""));
        }
        lines.push(Line::from(
            sanitize_terminal_text(draft.text()).into_owned(),
        ));
        lines.push(Line::from(""));
        lines.push(Line::from(rename.map_or_else(
            || locale.setup_input_hint(draft.stage() == NativeSetupStage::DirectoryPath),
            |project| locale.location_rename_hint(project),
        )));
    }
    lines.push(Line::from(""));
    if draft.known_project_created() {
        lines.push(Line::from(locale.setup_project_retained()));
        lines.push(Line::from(""));
    }
    if draft.faulted() {
        lines.push(Line::from(locale.setup_fault()));
        lines.push(Line::from(""));
    }
    if let Some(
        status @ (NativeSetupStatus::NotApplied
        | NativeSetupStatus::CommittedNeedRefresh
        | NativeSetupStatus::OutcomeUnknown
        | NativeSetupStatus::LocationLabel {
            status:
                NativeLocationLabelStatus::NotApplied
                | NativeLocationLabelStatus::CommittedNeedRefresh
                | NativeLocationLabelStatus::OutcomeUnknown,
            ..
        }),
    ) = status
    {
        lines.push(Line::from(locale.setup_status(status)));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(rename.map_or_else(
        || locale.setup_actions(draft.stage() == NativeSetupStage::ChooseProject),
        |project| locale.location_rename_actions(project),
    )));
    let resize_hint = rename.map_or_else(
        || locale.setup_resize_hint(),
        |project| locale.location_rename_resize_hint(project),
    );
    let (area, complete) = render_content(frame, resize_hint, Text::from(lines));
    (area, complete.then(|| draft.token()))
}

fn render_content(frame: &mut Frame<'_>, resize_hint: &str, text: Text<'_>) -> (Rect, bool) {
    let area = centered_overlay(frame.area(), u16::MAX, u16::MAX);
    let block = Block::new()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(2))
        .style(Style::new().bg(SIDEBAR_BACKGROUND).fg(TEXT))
        .border_style(Style::new().fg(EMPHASIS));
    let inner = block.inner(area);
    let complete = wrapped_text_fits(&text, inner);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if complete {
        frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
        (area, true)
    } else {
        frame.render_widget(
            Paragraph::new(resize_hint)
                .style(Style::new().fg(WARNING))
                .wrap(Wrap { trim: false }),
            inner,
        );
        (area, false)
    }
}

#[cfg(test)]
mod tests;
