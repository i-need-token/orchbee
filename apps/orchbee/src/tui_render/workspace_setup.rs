use super::tab_close::{modal_area, set_modal_cursor, wrapped_text_fits};
use crate::{
    localization::Locale,
    terminal_session::{
        ModalPresentation, NativeDisplayPhase, NativeLocationLabelStatus, NativeSetupDraftView,
        NativeSetupStage, NativeSetupStatus, NativeSetupToken,
    },
    terminal_text::sanitize_terminal_text,
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Text},
    widgets::{Block, Clear, Padding, Paragraph, Wrap},
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
    content_area: Rect,
    theme: &Theme,
) -> (Rect, ModalPresentation<NativeSetupToken>) {
    if matches!(
        draft.stage(),
        NativeSetupStage::QuickName | NativeSetupStage::QuickDirectory
    ) {
        return render_quick(frame, locale, draft, content_area, theme);
    }
    if draft.stage() == NativeSetupStage::PreparingWorkspace {
        let text = Text::from(vec![
            Line::from(locale.setup_title(draft.stage())),
            Line::from(""),
            Line::from(locale.quick_workspace_preparing_actions()),
        ]);
        let (area, complete) = render_content(
            frame,
            content_area,
            locale.setup_resize_hint(),
            text,
            None,
            theme,
        );
        return (
            area,
            if complete {
                ModalPresentation::Presented(draft.token())
            } else {
                ModalPresentation::NotPresented
            },
        );
    }
    let rename = match draft.stage() {
        NativeSetupStage::ProjectRename => Some(true),
        NativeSetupStage::WorkspaceRename => Some(false),
        _ => None,
    };
    let mut lines = vec![
        Line::from(locale.setup_title(draft.stage())),
        Line::from(""),
    ];
    // The editable line's index inside `lines`, when the stage owns a text
    // field; the picker stage has none.
    let mut field_index = None;
    if matches!(
        draft.stage(),
        NativeSetupStage::ChooseProject | NativeSetupStage::QuickProject
    ) {
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
        // The field's one visible row is sized like the rest: the windowed
        // projection never takes more than the inner width it is given.
        field_index = Some(lines.len());
        lines.push(Line::from(""));
        lines.push(Line::from(""));
        lines.push(Line::from(rename.map_or_else(
            || {
                if draft.stage() == NativeSetupStage::TabGroupName {
                    locale.setup_group_name_hint()
                } else {
                    locale.setup_input_hint(draft.stage() == NativeSetupStage::DirectoryPath)
                }
            },
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
        || {
            locale.setup_actions(matches!(
                draft.stage(),
                NativeSetupStage::ChooseProject | NativeSetupStage::QuickProject
            ))
        },
        |project| locale.location_rename_actions(project),
    )));
    let resize_hint = rename.map_or_else(
        || locale.setup_resize_hint(),
        |project| locale.location_rename_resize_hint(project),
    );
    let (area, complete) = render_content(
        frame,
        content_area,
        resize_hint,
        Text::from(lines),
        field_index.map(|index| (index, draft)),
        theme,
    );
    (
        area,
        if complete {
            ModalPresentation::Presented(draft.token())
        } else {
            ModalPresentation::NotPresented
        },
    )
}

fn render_quick(
    frame: &mut Frame<'_>,
    locale: Locale,
    draft: &NativeSetupDraftView<'_>,
    content_area: Rect,
    theme: &Theme,
) -> (Rect, ModalPresentation<NativeSetupToken>) {
    let project = draft
        .project_label()
        .unwrap_or(locale.cli_default_project_label());
    let directory_active = draft.stage() == NativeSetupStage::QuickDirectory;
    let mut lines = vec![
        Line::from(locale.setup_title(draft.stage())),
        Line::from(""),
        Line::from(format!(
            "{}: {}",
            locale.setup_project(),
            sanitize_terminal_text(project)
        )),
        Line::from(""),
        Line::from(format!(
            "{}{}",
            if directory_active { "  " } else { "> " },
            locale.quick_workspace_name()
        )),
        Line::from(""),
        Line::from(format!(
            "{}{}",
            if directory_active { "> " } else { "  " },
            locale.quick_workspace_directory()
        )),
        Line::from(""),
        Line::from(""),
        Line::from(locale.quick_workspace_actions()),
    ];
    if draft.faulted() {
        lines.push(Line::from(locale.setup_fault()));
    }
    let area = modal_area(content_area, &Text::from(lines.clone()));
    let block = overlay_block(theme);
    let inner = block.inner(area);
    let name = draft.quick_window(false, usize::from(inner.width));
    let directory = draft.quick_window(true, usize::from(inner.width));
    if let Some(window) = &name {
        lines[5] = Line::from(window.display().to_owned());
    }
    if let Some(window) = &directory {
        lines[7] = Line::from(window.display().to_owned());
    }
    let text = Text::from(lines);
    let complete = name.is_some() && directory.is_some() && wrapped_text_fits(&text, inner);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if complete {
        frame.render_widget(
            Paragraph::new(text.clone()).wrap(Wrap { trim: false }),
            inner,
        );
        if let Some(window) = if directory_active {
            directory.as_ref()
        } else {
            name.as_ref()
        } {
            set_modal_cursor(
                frame,
                &text,
                inner,
                if directory_active { 7 } else { 5 },
                window.cursor_column(),
            );
        }
    } else {
        frame.render_widget(
            Paragraph::new(locale.setup_resize_hint())
                .style(theme.style(Role::StatusWarning))
                .wrap(Wrap { trim: false }),
            inner,
        );
    }
    (
        area,
        if complete {
            ModalPresentation::Presented(draft.token())
        } else {
            ModalPresentation::NotPresented
        },
    )
}

fn overlay_block(theme: &Theme) -> Block<'static> {
    super::overlay_surface(theme, true, Padding::horizontal(2))
}

fn render_content(
    frame: &mut Frame<'_>,
    content_area: Rect,
    resize_hint: &str,
    text: Text<'_>,
    field: Option<(usize, &NativeSetupDraftView<'_>)>,
    theme: &Theme,
) -> (Rect, bool) {
    let area = modal_area(content_area, &text);
    let block = overlay_block(theme);
    let inner = block.inner(area);
    // The raw draft stays exact; what the modal draws is its safe projection
    // inside the inner width, with the raw cursor mapped through the same
    // projection — never a raw byte offset applied to sanitized text.
    let window = field.and_then(|(_, draft)| draft.field_window(usize::from(inner.width)));
    let mut lines = text.lines;
    if let (Some((index, _)), Some(window)) = (field, &window) {
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
        if let (Some((index, _)), Some(window)) = (field, &window) {
            set_modal_cursor(frame, &text, inner, index, window.cursor_column());
        }
        (area, true)
    } else {
        frame.render_widget(
            Paragraph::new(resize_hint)
                .style(theme.style(Role::StatusWarning))
                .wrap(Wrap { trim: false }),
            inner,
        );
        (area, false)
    }
}

#[cfg(test)]
mod tests;
