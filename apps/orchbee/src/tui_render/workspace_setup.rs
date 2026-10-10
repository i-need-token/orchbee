//! Bounded Project choices and text fields publish controls only after a complete draw.
use super::tab_close::{modal_area, set_modal_cursor, wrapped_text_fits};
use crate::{
    localization::Locale,
    terminal_session::{
        ModalPresentation, NativeDisplayPhase, NativeLocationLabelStatus, NativeSetupDraftView,
        NativeSetupStage, NativeSetupStatus, NativeSetupToken,
    },
    terminal_text::sanitize_terminal_text,
    terminal_text::{display_width, truncate_display},
    tui_layout::centered_overlay,
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget, ModalControl},
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
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<NativeSetupToken>) {
    hits.start_modal();
    if matches!(
        draft.stage(),
        NativeSetupStage::QuickProject | NativeSetupStage::ChooseWorkspaceProject
    ) {
        return render_projects(
            frame,
            locale,
            ProjectPicker {
                token: draft.token(),
                stage: draft.stage(),
                workspace: draft.workspace_label().unwrap_or(""),
                count: draft.project_count(),
                selected: draft.selected_project(),
            },
            content_area,
            theme,
            hits,
            |index| {
                draft.project_at(index).map(|project| ProjectChoice {
                    position: project.position(),
                    selected: project.selected(),
                    label: project.label().unwrap_or(if project.ungrouped() {
                        locale.setup_ungrouped()
                    } else {
                        locale.setup_new_project()
                    }),
                })
            },
        );
    }
    if matches!(
        draft.stage(),
        NativeSetupStage::QuickName | NativeSetupStage::QuickDirectory
    ) {
        return render_quick(frame, locale, draft, content_area, theme);
    }
    if matches!(
        draft.stage(),
        NativeSetupStage::PreparingWorkspace | NativeSetupStage::DissolveProject
    ) {
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
    let context_budget = usize::from(content_area.width.saturating_sub(10)).min(72);
    // The editable line's index inside `lines`, when the stage owns a text
    // field; the picker stage has none.
    let field_index;
    {
        if let Some(project) = draft.project_label() {
            lines.push(Line::from(truncate_display(
                &format!(
                    "{}: {}",
                    locale.setup_project(),
                    sanitize_terminal_text(project)
                ),
                context_budget,
                theme.glyphs.ellipsis,
            )));
            lines.push(Line::from(""));
        }
        if matches!(
            draft.stage(),
            NativeSetupStage::DirectoryPath
                | NativeSetupStage::WorkspaceRename
                | NativeSetupStage::WorkspaceProjectName
        ) && let Some(workspace) = draft.workspace_label()
        {
            lines.push(Line::from(truncate_display(
                &format!(
                    "{}: {}",
                    locale.shell().workspaces,
                    sanitize_terminal_text(workspace)
                ),
                context_budget,
                theme.glyphs.ellipsis,
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
        || locale.setup_actions(false),
        |project| locale.location_rename_actions(project),
    )));
    lines.push(Line::from(format!(
        "{}  {}",
        locale.setup_cancel_button(),
        locale.setup_submit_button()
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
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
    if complete {
        let inner = overlay_block(theme).inner(area);
        record_buttons(hits, draft.token(), inner, locale);
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

struct ProjectPicker<'a> {
    token: NativeSetupToken,
    stage: NativeSetupStage,
    workspace: &'a str,
    count: usize,
    selected: usize,
}

struct ProjectChoice<'a> {
    position: usize,
    selected: bool,
    label: &'a str,
}

fn render_projects<'a>(
    frame: &mut Frame<'_>,
    locale: Locale,
    picker: ProjectPicker<'_>,
    content: Rect,
    theme: &Theme,
    hits: &mut HitMapBuilder,
    choice: impl Fn(usize) -> Option<ProjectChoice<'a>>,
) -> (Rect, ModalPresentation<NativeSetupToken>) {
    let count = picker.count;
    let area = centered_overlay(
        content,
        78.min(content.width.saturating_sub(4)),
        u16::try_from(count.saturating_add(7))
            .unwrap_or(u16::MAX)
            .min(content.height.saturating_sub(2)),
    );
    let block = overlay_block(theme);
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if inner.width < 20 || inner.height < 6 || count == 0 {
        hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
        frame.render_widget(
            Paragraph::new(locale.setup_resize_hint()).wrap(Wrap { trim: false }),
            inner,
        );
        return (area, ModalPresentation::NotPresented);
    }
    hits.record(
        HitLayer::OverlaySurface,
        area,
        HitTarget::Setup {
            token: picker.token,
            control: ModalControl::Surface,
        },
    );
    let capacity = usize::from(inner.height - 5);
    let start = picker
        .selected
        .saturating_sub(capacity / 2)
        .min(count.saturating_sub(capacity));
    let budget = usize::from(inner.width);
    let mut lines = vec![
        Line::from(truncate_display(
            locale.setup_title(picker.stage),
            budget,
            theme.glyphs.ellipsis,
        )),
        Line::from(truncate_display(
            &sanitize_terminal_text(picker.workspace),
            budget,
            theme.glyphs.ellipsis,
        )),
    ];
    for index in start..count.min(start + capacity) {
        if let Some(project) = choice(index) {
            let prefix = format!(
                "{} {}. ",
                if project.selected { ">" } else { " " },
                project.position
            );
            lines.push(
                Line::from(format!(
                    "{prefix}{}",
                    truncate_display(
                        &sanitize_terminal_text(project.label),
                        budget.saturating_sub(display_width(&prefix)),
                        theme.glyphs.ellipsis
                    )
                ))
                .style(theme.style(if project.selected {
                    Role::SurfaceSelection
                } else {
                    Role::TextPrimary
                })),
            );
            hits.record(
                HitLayer::Overlay,
                Rect::new(
                    inner.x,
                    inner.y + 2 + u16::try_from(index - start).unwrap_or(0),
                    inner.width,
                    1,
                ),
                HitTarget::Setup {
                    token: picker.token,
                    control: ModalControl::Choice(index),
                },
            );
        } else {
            lines.push(Line::from(""));
        }
    }
    lines.push(Line::from(format!("{}/{}", picker.selected + 1, count)));
    lines.push(Line::from(
        if display_width(locale.setup_picker_actions()) <= budget {
            locale.setup_picker_actions()
        } else {
            locale.setup_picker_compact_actions()
        },
    ));
    lines.push(Line::from(locale.setup_cancel_button()));
    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
    hits.record(
        HitLayer::Overlay,
        Rect::new(
            inner.x,
            inner.bottom() - 1,
            u16::try_from(display_width(locale.setup_cancel_button())).unwrap_or(0),
            1,
        ),
        HitTarget::Setup {
            token: picker.token,
            control: ModalControl::Cancel,
        },
    );
    (area, ModalPresentation::Presented(picker.token))
}

fn record_buttons(hits: &mut HitMapBuilder, token: NativeSetupToken, inner: Rect, locale: Locale) {
    let cancel_width = u16::try_from(display_width(locale.setup_cancel_button())).unwrap_or(0);
    let submit_width = u16::try_from(display_width(locale.setup_submit_button())).unwrap_or(0);
    if inner.height == 0 || inner.width < cancel_width + 2 + submit_width {
        return;
    }
    hits.record(
        HitLayer::Overlay,
        Rect::new(inner.x, inner.bottom() - 1, cancel_width, 1),
        HitTarget::Setup {
            token,
            control: ModalControl::Cancel,
        },
    );
    hits.record(
        HitLayer::Overlay,
        Rect::new(
            inner.x + cancel_width + 2,
            inner.bottom() - 1,
            submit_width,
            1,
        ),
        HitTarget::Setup {
            token,
            control: ModalControl::Submit,
        },
    );
}

fn render_quick(
    frame: &mut Frame<'_>,
    locale: Locale,
    draft: &NativeSetupDraftView<'_>,
    content_area: Rect,
    theme: &Theme,
) -> (Rect, ModalPresentation<NativeSetupToken>) {
    let project = draft.project_label().unwrap_or(locale.setup_ungrouped());
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
