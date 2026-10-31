//! Bounded Project choices and text fields publish controls only after a complete draw.
use super::modal::{
    ModalAction, description_line, modal_key_label, modal_layout, paint_modal_action,
    paint_modal_close, paint_modal_frame, pointer_presentation, set_modal_cursor,
    single_modal_layout, status_line,
};
mod project_delete;
use crate::client_view::{
    Binding, Bindings, ClientAction, KeyCode, KeyLabelGlyphs, Modifiers, PrefixAction, PrefixDesc,
};
use crate::localization::{DisplayPhase, Locale};
use crate::terminal_text::{
    display_width, sanitize_terminal_text, truncate_display, truncate_display_middle,
};
use crate::tui_hit_map::{
    HitLayer, HitMapBuilder, HitTarget, InteractionAvailability, ModalControl,
};
use crate::tui_layout::centered_overlay;
use crate::tui_presentation::ModalPresentation;
use crate::tui_theme::{GlyphMode, Role, Theme};
use crate::workbench_state::workspace_setup::{
    LocationLabelStatus, SetupDraftView, SetupStage, SetupStatus, SetupToken,
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span, Text},
    widgets::{Block, Padding, Paragraph},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DirectoryGuidance {
    fact: String,
    action: PrefixAction,
    description: PrefixDesc,
}

pub(super) fn directory_guidance(
    locale: Locale,
    phase: DisplayPhase,
    retired: bool,
    path: Option<&str>,
    unconfigured: bool,
) -> Option<DirectoryGuidance> {
    if retired || phase != DisplayPhase::Empty {
        return None;
    }
    path.map(|path| DirectoryGuidance {
        fact: locale.setup_directory_ready(&sanitize_terminal_text(path)),
        action: PrefixAction::Client(ClientAction::CreateTab),
        description: PrefixDesc::CreateTab,
    })
    .or_else(|| {
        unconfigured.then(|| DirectoryGuidance {
            fact: locale.setup_directory_needed().to_owned(),
            action: PrefixAction::Client(ClientAction::RequestWorkspaceDirectory),
            description: PrefixDesc::WorkspaceDirectory,
        })
    })
}

pub(super) fn render_directory_guidance(
    frame: &mut Frame<'_>,
    area: Rect,
    guidance: &DirectoryGuidance,
    bindings: &Bindings,
    locale: Locale,
    admitted: bool,
    theme: &Theme,
) {
    if area.is_empty() || frame.area().intersection(area) != area {
        return;
    }
    let fact = crate::terminal_text::wrap_text(&Text::from(guidance.fact.as_str()), area.width);
    let fact_rows = fact.lines.len();
    frame.render_widget(
        Paragraph::new(fact).style(theme.style(Role::TextSecondary)),
        area,
    );
    if !admitted {
        return;
    }
    let glyphs = match theme.glyphs.mode() {
        GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
        #[cfg(any(test, feature = "test-support"))]
        GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
    };
    let Some(key) = bindings
        .action_labels_with_glyphs(guidance.action.clone(), glyphs)
        .into_iter()
        .next()
    else {
        return;
    };
    let Ok(fact_rows) = u16::try_from(fact_rows) else {
        return;
    };
    if fact_rows >= area.height {
        return;
    }
    let Some(hint_y) = area.y.checked_add(fact_rows) else {
        return;
    };
    let hint_area = Rect::new(area.x, hint_y, area.width, 1);
    let label = (locale.shell().prefix_desc)(guidance.description);
    let _ = super::controls::render_key_hints(
        frame,
        hint_area,
        &[super::controls::KeyHint::new(&key, label)],
        theme,
    );
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    draft: &SetupDraftView<'_>,
    status: Option<SetupStatus>,
    content_area: Rect,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<SetupToken>) {
    if draft.stage() == SetupStage::ProjectDeleteConfirm {
        return project_delete::render(
            frame,
            locale,
            project_delete::Decision {
                token: draft.token(),
                project: draft.project_label().unwrap_or(""),
            },
            content_area,
            theme,
            hits,
        );
    }
    if matches!(
        draft.stage(),
        SetupStage::QuickProject | SetupStage::ChooseWorkspaceProject
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
        SetupStage::QuickName | SetupStage::QuickDirectory
    ) {
        return render_quick(frame, locale, draft, content_area, theme, hits);
    }
    if matches!(
        draft.stage(),
        SetupStage::PreparingWorkspace | SetupStage::DissolveProject
    ) {
        let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
        let hints = [];
        let action = ModalAction::new(
            locale.setup_cancel_label(),
            super::controls::ActionKind::Secondary,
            true,
        )
        .with_shortcut(&escape);
        let title = locale.setup_title(draft.stage());
        let text = Text::from(vec![
            Line::from(""),
            description_line(locale.quick_workspace_preparing_fact(), theme),
            Line::from(""),
            Line::from(""),
        ]);
        let Some(layout) = single_modal_layout(
            frame.area(),
            content_area,
            title,
            &text,
            &hints,
            action,
            theme,
        ) else {
            return (Rect::default(), ModalPresentation::NotPresented);
        };
        hits.start_modal();
        paint_modal_frame(frame, layout.area, title, theme);
        frame.render_widget(
            Paragraph::new(crate::terminal_text::wrap_text(&text, layout.inner.width)),
            layout.inner,
        );
        paint_modal_close(
            frame,
            layout.close,
            HitTarget::Setup {
                token: draft.token(),
                control: ModalControl::Cancel,
            },
            theme,
            hits,
        );
        paint_modal_action(
            frame,
            Rect::new(
                layout.action.x,
                layout.action.y,
                layout.inner.right() - layout.action.x,
                1,
            ),
            layout.action,
            action,
            HitTarget::Setup {
                token: draft.token(),
                control: ModalControl::Cancel,
            },
            theme,
            hits,
        );
        hits.record(
            HitLayer::OverlaySurface,
            layout.area,
            HitTarget::OverlaySurface,
        );
        return (layout.area, ModalPresentation::Presented(draft.token()));
    }
    let rename = match draft.stage() {
        SetupStage::ProjectRename => Some(true),
        SetupStage::WorkspaceRename => Some(false),
        _ => None,
    };
    let title = locale.setup_title(draft.stage());
    let mut lines = vec![Line::from("")];
    let context_budget = usize::from(content_area.width.saturating_sub(10)).min(72);
    // The editable line's index inside `lines`, when the stage owns a text
    // field; the picker stage has none.
    let field_index;
    {
        if let Some(project) = draft.project_label() {
            lines.push(description_line(
                truncate_display(
                    &format!(
                        "{}: {}",
                        locale.setup_project(),
                        sanitize_terminal_text(project)
                    ),
                    context_budget,
                    theme.glyphs.ellipsis,
                ),
                theme,
            ));
            lines.push(Line::from(""));
        }
        if matches!(
            draft.stage(),
            SetupStage::DirectoryPath
                | SetupStage::WorkspaceRename
                | SetupStage::WorkspaceProjectName
        ) && let Some(workspace) = draft.workspace_label()
        {
            lines.push(description_line(
                truncate_display(
                    &format!(
                        "{}: {}",
                        locale.shell().workspaces,
                        sanitize_terminal_text(locale.workspace_display_label(workspace))
                    ),
                    context_budget,
                    theme.glyphs.ellipsis,
                ),
                theme,
            ));
            lines.push(Line::from(""));
        }
        // The field's one visible row is sized like the rest: the windowed
        // projection never takes more than the inner width it is given.
        lines.push(Line::from(if draft.stage() == SetupStage::DirectoryPath {
            locale.quick_workspace_directory()
        } else {
            locale.quick_workspace_name()
        }));
        field_index = Some(lines.len());
        lines.push(Line::from(""));
        lines.push(description_line(
            rename.map_or_else(
                || {
                    if draft.stage() == SetupStage::TabGroupName {
                        locale.setup_group_name_hint()
                    } else {
                        locale.setup_input_hint(draft.stage() == SetupStage::DirectoryPath)
                    }
                },
                |project| {
                    if !project && draft.text().trim().is_empty() {
                        locale.workspace_name_required()
                    } else {
                        locale.location_rename_hint(project)
                    }
                },
            ),
            theme,
        ));
    }
    lines.push(Line::from(""));
    if draft.faulted() {
        lines.push(status_line(locale.setup_fault(), Role::StatusError, theme));
        lines.push(Line::from(""));
    }
    if let Some(
        status @ (SetupStatus::NotApplied
        | SetupStatus::CommittedNeedRefresh
        | SetupStatus::OutcomeUnknown
        | SetupStatus::LocationLabel {
            status:
                LocationLabelStatus::NotApplied
                | LocationLabelStatus::CommittedNeedRefresh
                | LocationLabelStatus::OutcomeUnknown,
            ..
        }),
    ) = status
    {
        let role = if matches!(
            status,
            SetupStatus::NotApplied
                | SetupStatus::LocationLabel {
                    status: LocationLabelStatus::NotApplied,
                    ..
                }
        ) {
            Role::StatusError
        } else {
            Role::StatusWarning
        };
        lines.push(status_line(locale.setup_status(status), role, theme));
        lines.push(Line::from(""));
    }
    lines.extend([Line::from(""), Line::from("")]);
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let backspace = modal_key_label(theme, &Binding::new(KeyCode::Backspace, Modifiers::NONE));
    let clear = modal_key_label(theme, &Binding::new(KeyCode::Char('u'), Modifiers::CTRL));
    let hints = [
        super::controls::KeyHint::new(&backspace, locale.setup_delete_hint()),
        super::controls::KeyHint::new(&clear, locale.setup_clear_hint()),
    ];
    let actions = [
        ModalAction::new(
            locale.setup_cancel_label(),
            super::controls::ActionKind::Secondary,
            true,
        )
        .with_shortcut(&escape),
        ModalAction::new(
            locale.setup_submit_label(),
            super::controls::ActionKind::Primary,
            !draft.faulted(),
        )
        .with_shortcut(&enter),
    ];
    let placeholder = Text::from(lines);
    let Some(candidate) = modal_layout(
        frame.area(),
        content_area,
        title,
        &placeholder,
        &hints,
        actions,
        theme,
    ) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let Some(field_width) = candidate.inner.width.checked_sub(4) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let Some(field_index) = field_index else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let Some(window) = draft.field_window(usize::from(field_width)) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let mut lines = placeholder.lines;
    let mut displayed = window.display().to_owned();
    displayed
        .push_str(&" ".repeat(usize::from(field_width).saturating_sub(display_width(&displayed))));
    lines[field_index] = Line::from(vec![
        Span::styled("[ ", theme.style(Role::AccentFocus)),
        Span::styled(displayed, theme.style(Role::TextPrimary)),
        Span::styled(" ]", theme.style(Role::AccentFocus)),
    ]);
    let text = Text::from(lines);
    let Some(layout) = modal_layout(
        frame.area(),
        content_area,
        title,
        &text,
        &hints,
        actions,
        theme,
    ) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    hits.start_modal();
    paint_modal_frame(frame, layout.area, title, theme);
    frame.render_widget(
        Paragraph::new(crate::terminal_text::wrap_text(&text, layout.inner.width)),
        layout.inner,
    );
    paint_modal_close(
        frame,
        layout.close,
        HitTarget::Setup {
            token: draft.token(),
            control: ModalControl::Cancel,
        },
        theme,
        hits,
    );
    let rendered_hints = super::controls::render_overlay_hints(frame, layout.hints, &hints, theme)
        .expect("admitted setup hints must paint");
    debug_assert!(layout.hints.intersection(rendered_hints) == rendered_hints);
    for ((rect, action), control) in layout
        .actions
        .into_iter()
        .zip(actions)
        .zip([ModalControl::Cancel, ModalControl::Submit])
    {
        paint_modal_action(
            frame,
            Rect::new(rect.x, rect.y, layout.inner.right() - rect.x, 1),
            rect,
            action,
            HitTarget::Setup {
                token: draft.token(),
                control,
            },
            theme,
            hits,
        );
    }
    hits.record(
        HitLayer::OverlaySurface,
        layout.area,
        HitTarget::OverlaySurface,
    );
    set_modal_cursor(
        frame,
        &text,
        layout.inner,
        field_index,
        window.cursor_column() + 2,
    );
    (layout.area, ModalPresentation::Presented(draft.token()))
}

struct ProjectPicker<'a> {
    token: SetupToken,
    stage: SetupStage,
    workspace: &'a str,
    count: usize,
    selected: usize,
}

struct ProjectChoice<'a> {
    position: usize,
    label: &'a str,
}

#[derive(Clone, Copy)]
struct PickerRowGeometry {
    row: Rect,
    navigation: Rect,
    primary: Rect,
}

#[derive(Clone, Copy)]
struct FieldRowGeometry {
    row: Rect,
    navigation: Rect,
    primary: Rect,
    content: Rect,
}

impl FieldRowGeometry {
    fn resolve(text: &Text<'_>, inner: Rect, label_index: usize, theme: &Theme) -> Option<Self> {
        let label = super::controls::text_row(text, inner, label_index);
        let field = super::controls::text_row(text, inner, label_index.checked_add(1)?);
        if label.height != 1 || field.height != 1 || field.y != label.bottom() {
            return None;
        }
        let row = Rect::new(label.x, label.y, label.width, 2);
        if inner.intersection(row) != row {
            return None;
        }
        let navigation_width =
            u16::try_from(display_width(theme.glyphs.navigation_marker).max(1)).ok()?;
        let primary_x = row.x.checked_add(navigation_width)?;
        let content_x = primary_x.checked_add(1)?;
        let navigation = Rect::new(row.x, row.y, navigation_width, row.height);
        let primary = Rect::new(primary_x, row.y, 1, row.height);
        let content = Rect::new(
            content_x,
            row.y,
            row.right().checked_sub(content_x)?,
            row.height,
        );
        (!content.is_empty()
            && row.intersection(navigation) == navigation
            && row.intersection(primary) == primary
            && row.intersection(content) == content)
            .then_some(Self {
                row,
                navigation,
                primary,
                content,
            })
    }

    fn prefix_width(self) -> usize {
        usize::from(self.content.x.saturating_sub(self.row.x))
    }
}

impl PickerRowGeometry {
    fn resolve(row: Rect, theme: &Theme) -> Option<Self> {
        let navigation_width =
            u16::try_from(display_width(theme.glyphs.navigation_marker).max(1)).ok()?;
        let primary_x = row.x.checked_add(navigation_width)?.checked_add(1)?;
        let primary = Rect::new(
            primary_x,
            row.y,
            row.right().checked_sub(primary_x)?,
            row.height,
        );
        let navigation = Rect::new(row.x, row.y, navigation_width, row.height);
        (!row.is_empty()
            && !primary.is_empty()
            && row.intersection(navigation) == navigation
            && row.intersection(primary) == primary)
            .then_some(Self {
                row,
                navigation,
                primary,
            })
    }

    fn prefix_width(self) -> usize {
        usize::from(self.primary.x.saturating_sub(self.row.x))
    }
}

fn render_projects<'a>(
    frame: &mut Frame<'_>,
    locale: Locale,
    picker: ProjectPicker<'_>,
    content: Rect,
    theme: &Theme,
    hits: &mut HitMapBuilder,
    choice: impl Fn(usize) -> Option<ProjectChoice<'a>>,
) -> (Rect, ModalPresentation<SetupToken>) {
    let count = picker.count;
    let area = centered_overlay(
        content,
        78.min(content.width.saturating_sub(4)),
        u16::try_from(count.saturating_add(7))
            .unwrap_or(u16::MAX)
            .min(content.height.saturating_sub(2)),
    );
    let inner = overlay_block(theme).inner(area);
    if area.is_empty()
        || frame.area().intersection(area) != area
        || inner.width < 20
        || inner.height < 6
        || count == 0
    {
        return (Rect::default(), ModalPresentation::NotPresented);
    }
    let capacity = usize::from(inner.height - 5);
    let start = picker
        .selected
        .saturating_sub(capacity / 2)
        .min(count.saturating_sub(capacity));
    let budget = usize::from(inner.width);
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let up = modal_key_label(theme, &Binding::new(KeyCode::Up, Modifiers::NONE));
    let down = modal_key_label(theme, &Binding::new(KeyCode::Down, Modifiers::NONE));
    let vertical = format!("{up}/{down}");
    let hints = [
        super::controls::KeyHint::new(&enter, locale.setup_choose_hint()),
        super::controls::KeyHint::new(&vertical, locale.setup_select_hint()),
    ];
    let cancel = ModalAction::new(
        locale.setup_cancel_label(),
        super::controls::ActionKind::Secondary,
        true,
    )
    .with_shortcut(&escape);
    let close = match super::controls::frame_close_rect(frame.area(), area) {
        Some(close) => close,
        None => return (Rect::default(), ModalPresentation::NotPresented),
    };
    let hint_row = Rect::new(inner.x, inner.bottom() - 2, inner.width, 1);
    let action_row = Rect::new(inner.x, inner.bottom() - 1, inner.width, 1);
    let hints_valid = super::controls::measure_overlay_hints(&hints, theme).is_some();
    let first_hint_fits = super::controls::measure_overlay_hints(&hints[..1], theme)
        .is_some_and(|width| width <= usize::from(hint_row.width));
    let Some(action) = super::controls::action_rect(
        frame.area(),
        action_row,
        cancel.spec(super::controls::PointerPresentation::Rest),
        theme,
    ) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    if !hints_valid || !first_hint_fits {
        return (Rect::default(), ModalPresentation::NotPresented);
    }

    let Some(list_y) = inner.y.checked_add(2) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let Ok(list_height) = u16::try_from(capacity) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let list = Rect::new(inner.x, list_y, inner.width, list_height);
    if inner.intersection(list) != list {
        return (Rect::default(), ModalPresentation::NotPresented);
    }
    let row_geometries = (0..capacity)
        .map(|offset| {
            let y = list_y.checked_add(u16::try_from(offset).ok()?)?;
            PickerRowGeometry::resolve(Rect::new(inner.x, y, inner.width, 1), theme)
        })
        .collect::<Option<Vec<_>>>();
    let Some(row_geometries) = row_geometries else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let choices = (start..count.min(start + capacity))
        .map(|index| {
            choice(index).map(|project| {
                let prefix = format!("{}. ", project.position);
                let geometry = row_geometries[index - start];
                let label_budget = usize::from(geometry.primary.width);
                let label = format!(
                    "{prefix}{}",
                    truncate_display(
                        &sanitize_terminal_text(project.label),
                        label_budget.saturating_sub(display_width(&prefix)),
                        theme.glyphs.ellipsis,
                    )
                );
                (index, label)
            })
        })
        .collect::<Vec<_>>();
    let mut lines = vec![
        Line::from(""),
        description_line(
            truncate_display(
                &sanitize_terminal_text(picker.workspace),
                budget,
                theme.glyphs.ellipsis,
            ),
            theme,
        ),
    ];
    for (offset, projected) in choices.iter().enumerate() {
        let geometry = row_geometries[offset];
        let label = projected.as_ref().map_or("", |(_, label)| label.as_str());
        lines.push(
            Line::from(format!("{}{}", " ".repeat(geometry.prefix_width()), label,))
                .style(theme.style(Role::TextPrimary)),
        );
    }
    while lines.len() < usize::from(inner.height - 3) {
        lines.push(Line::from(""));
    }
    lines.push(description_line(
        format!("{}/{}", picker.selected + 1, count),
        theme,
    ));
    lines.extend([Line::from(""), Line::from("")]);

    hits.start_modal();
    paint_modal_frame(frame, area, locale.setup_title(picker.stage), theme);
    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
    paint_modal_close(
        frame,
        close,
        HitTarget::Setup {
            token: picker.token,
            control: ModalControl::Cancel,
        },
        theme,
        hits,
    );
    let rendered_hints = super::controls::render_overlay_hints(frame, hint_row, &hints, theme)
        .expect("admitted setup picker hints must paint");
    debug_assert!(hint_row.intersection(rendered_hints) == rendered_hints);
    paint_modal_action(
        frame,
        action_row,
        action,
        cancel,
        HitTarget::Setup {
            token: picker.token,
            control: ModalControl::Cancel,
        },
        theme,
        hits,
    );
    for (offset, projected) in choices.into_iter().enumerate() {
        let Some((index, _)) = projected else {
            continue;
        };
        let geometry = row_geometries[offset];
        let target = HitTarget::Setup {
            token: picker.token,
            control: ModalControl::Choice(index),
        };
        let availability = InteractionAvailability::Enabled;
        let pointer =
            hits.projected_modal_pointer(HitLayer::Overlay, geometry.row, &target, availability);
        let provisional = index == picker.selected;
        let state = super::controls::InteractionState::new(
            if provisional {
                super::controls::FocusState::Focused
            } else {
                super::controls::FocusState::Unfocused
            },
            pointer_presentation(pointer),
            if provisional {
                super::controls::NavigationState::Provisional
            } else {
                super::controls::NavigationState::None
            },
            super::controls::CurrentState::NotCurrent,
            super::controls::CheckedState::Unchecked,
            super::controls::Availability::Enabled,
            super::controls::OperationState::Idle,
        );
        let painted = super::controls::paint_row_interaction(
            frame,
            geometry.row,
            super::controls::RowTracks::new(
                super::controls::RowFocusCue::EnclosingFrame,
                geometry.primary,
            )
            .with_provisional(geometry.navigation),
            &[],
            super::controls::ControlSurface::Overlay,
            state,
            theme,
        )
        .expect("admitted setup picker row must paint");
        assert_eq!(painted, geometry.row);
        hits.record_modal_control(HitLayer::Overlay, geometry.row, target, availability);
    }
    hits.record(
        HitLayer::OverlaySurface,
        list,
        HitTarget::ListScroll(crate::tui_hit_map::ListScrollTarget::Setup(picker.token)),
    );
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
    (area, ModalPresentation::Presented(picker.token))
}

/// Label rows of the quick setup fields; each editable row follows its label.
const QUICK_NAME_LABEL: usize = 3;
const QUICK_DIRECTORY_LABEL: usize = 6;

fn render_quick(
    frame: &mut Frame<'_>,
    locale: Locale,
    draft: &SetupDraftView<'_>,
    content_area: Rect,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<SetupToken>) {
    let project = draft.project_label().unwrap_or(locale.setup_ungrouped());
    let directory_active = draft.stage() == SetupStage::QuickDirectory;
    let title = locale.setup_title(draft.stage());
    let mut lines = vec![
        Line::from(""),
        description_line(
            format!(
                "{}: {}",
                locale.setup_project(),
                sanitize_terminal_text(project)
            ),
            theme,
        ),
        Line::from(""),
        Line::from(locale.quick_workspace_name()),
        Line::from(""),
        Line::from(""),
        Line::from(locale.quick_workspace_directory()),
        Line::from(""),
        description_line(locale.quick_workspace_directory_hint(), theme),
    ];
    if draft.faulted() {
        lines.push(status_line(locale.setup_fault(), Role::StatusError, theme));
        lines.push(Line::from(""));
    }
    let choices_start = lines.len();
    let selected = draft.selected_directory().unwrap_or(0);
    let first = selected
        .saturating_sub(1)
        .min(draft.directory_choices().len().saturating_sub(2));
    let choices: Vec<_> = draft
        .directory_choices()
        .iter()
        .enumerate()
        .skip(first)
        .take(2)
        .collect();
    for (index, path) in &choices {
        let marker = if draft.selected_directory() == Some(*index) {
            theme.glyphs.active_marker
        } else {
            " "
        };
        // A path keeps its distinguishing tail: it truncates in the middle.
        lines.push(Line::from(format!(
            "{marker} {} {}",
            index + 1,
            truncate_display_middle(
                &sanitize_terminal_text(path.as_str()),
                usize::from(content_area.width.saturating_sub(16).min(44)),
                theme.glyphs.ellipsis
            )
        )));
    }
    // The shared modal reserves its final two text rows for hints and actions.
    // Directory choices are content and must never occupy those control rows.
    lines.extend([Line::from(""), Line::from("")]);
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let tab = modal_key_label(theme, &Binding::new(KeyCode::Tab, Modifiers::NONE));
    let clear = modal_key_label(theme, &Binding::new(KeyCode::Char('u'), Modifiers::CTRL));
    let hints = [
        super::controls::KeyHint::new(&tab, locale.setup_next_field_hint()),
        super::controls::KeyHint::new(&clear, locale.setup_clear_hint()),
    ];
    let actions = [
        ModalAction::new(
            locale.setup_cancel_label(),
            super::controls::ActionKind::Secondary,
            true,
        )
        .with_shortcut(&escape),
        ModalAction::new(
            locale.setup_submit_label(),
            super::controls::ActionKind::Primary,
            !draft.faulted(),
        )
        .with_shortcut(&enter),
    ];
    let placeholder = Text::from(lines);
    let Some(candidate) = modal_layout(
        frame.area(),
        content_area,
        title,
        &placeholder,
        &hints,
        actions,
        theme,
    ) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let Some(name_geometry) =
        FieldRowGeometry::resolve(&placeholder, candidate.inner, QUICK_NAME_LABEL, theme)
    else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let Some(directory_geometry) =
        FieldRowGeometry::resolve(&placeholder, candidate.inner, QUICK_DIRECTORY_LABEL, theme)
    else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    if name_geometry.content.width < 5 || directory_geometry.content.width < 5 {
        return (Rect::default(), ModalPresentation::NotPresented);
    }
    let field_width = usize::from(name_geometry.content.width - 4);
    let Some(name) = draft.quick_window(false, field_width) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let Some(directory) = draft.quick_window(true, field_width) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let mut lines = placeholder.lines;
    for (label_index, label, window, active, geometry) in [
        (
            QUICK_NAME_LABEL,
            locale.quick_workspace_name(),
            &name,
            !directory_active,
            name_geometry,
        ),
        (
            QUICK_DIRECTORY_LABEL,
            locale.quick_workspace_directory(),
            &directory,
            directory_active,
            directory_geometry,
        ),
    ] {
        let prefix = " ".repeat(geometry.prefix_width());
        lines[label_index] = Line::from(format!(
            "{prefix}{}",
            truncate_display(
                label,
                usize::from(geometry.content.width),
                theme.glyphs.ellipsis,
            )
        ))
        .style(theme.style(Role::TextPrimary));
        let mut value = window.display().to_owned();
        value.push_str(&" ".repeat(field_width.saturating_sub(display_width(&value))));
        let bracket_role = if active {
            Role::AccentFocus
        } else {
            Role::TextSecondary
        };
        lines[label_index + 1] = Line::from(vec![
            Span::raw(prefix),
            Span::styled("[ ", theme.style(bracket_role)),
            Span::styled(value, theme.style(Role::TextPrimary)),
            Span::styled(" ]", theme.style(bracket_role)),
        ]);
    }
    let text = Text::from(lines);
    let Some(layout) = modal_layout(
        frame.area(),
        content_area,
        title,
        &text,
        &hints,
        actions,
        theme,
    ) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    if layout.inner != candidate.inner {
        return (Rect::default(), ModalPresentation::NotPresented);
    }

    hits.start_modal();
    paint_modal_frame(frame, layout.area, title, theme);
    frame.render_widget(
        Paragraph::new(crate::terminal_text::wrap_text(&text, layout.inner.width)),
        layout.inner,
    );
    paint_modal_close(
        frame,
        layout.close,
        HitTarget::Setup {
            token: draft.token(),
            control: ModalControl::Cancel,
        },
        theme,
        hits,
    );
    let rendered_hints = super::controls::render_overlay_hints(frame, layout.hints, &hints, theme)
        .expect("admitted quick setup hints must paint");
    debug_assert!(layout.hints.intersection(rendered_hints) == rendered_hints);
    for ((rect, action), control) in layout
        .actions
        .into_iter()
        .zip(actions)
        .zip([ModalControl::Cancel, ModalControl::Submit])
    {
        paint_modal_action(
            frame,
            Rect::new(rect.x, rect.y, layout.inner.right() - rect.x, 1),
            rect,
            action,
            HitTarget::Setup {
                token: draft.token(),
                control,
            },
            theme,
            hits,
        );
    }
    for (active, geometry) in [
        (!directory_active, name_geometry),
        (directory_active, directory_geometry),
    ] {
        let target = HitTarget::Setup {
            token: draft.token(),
            control: ModalControl::NextField,
        };
        let pointer = if active {
            super::controls::PointerPresentation::Rest
        } else {
            pointer_presentation(hits.projected_modal_pointer(
                HitLayer::Overlay,
                geometry.row,
                &target,
                InteractionAvailability::Enabled,
            ))
        };
        let state = super::controls::InteractionState::new(
            if active {
                super::controls::FocusState::Focused
            } else {
                super::controls::FocusState::Unfocused
            },
            pointer,
            if active {
                super::controls::NavigationState::Provisional
            } else {
                super::controls::NavigationState::None
            },
            super::controls::CurrentState::NotCurrent,
            super::controls::CheckedState::Unchecked,
            super::controls::Availability::Enabled,
            super::controls::OperationState::Idle,
        );
        let bands = [super::controls::RowContentBand::explicit(geometry.content)];
        let painted = super::controls::paint_row_interaction(
            frame,
            geometry.row,
            super::controls::RowTracks::new(
                super::controls::RowFocusCue::EnclosingFrame,
                geometry.primary,
            )
            .with_provisional(geometry.navigation),
            &bands,
            super::controls::ControlSurface::Overlay,
            state,
            theme,
        )
        .expect("admitted quick setup field row must paint");
        assert_eq!(painted, geometry.row);
        if !active {
            hits.record_modal_control(
                HitLayer::Overlay,
                geometry.row,
                target,
                InteractionAvailability::Enabled,
            );
        }
    }
    for (row, (index, _)) in choices.iter().enumerate() {
        let rect = super::controls::text_row(&text, layout.inner, choices_start + row);
        if rect.height == 1 && layout.inner.intersection(rect) == rect {
            hits.record_modal_control(
                HitLayer::Overlay,
                rect,
                HitTarget::Setup {
                    token: draft.token(),
                    control: ModalControl::Choice(*index),
                },
                InteractionAvailability::Enabled,
            );
        }
    }
    hits.record(
        HitLayer::OverlaySurface,
        layout.area,
        HitTarget::OverlaySurface,
    );
    let (field_index, window, geometry) = if directory_active {
        (QUICK_DIRECTORY_LABEL + 1, &directory, directory_geometry)
    } else {
        (QUICK_NAME_LABEL + 1, &name, name_geometry)
    };
    set_modal_cursor(
        frame,
        &text,
        layout.inner,
        field_index,
        geometry.prefix_width() + 2 + window.cursor_column(),
    );
    (layout.area, ModalPresentation::Presented(draft.token()))
}

fn overlay_block(theme: &Theme) -> Block<'static> {
    super::overlay_surface(theme, Padding::horizontal(2))
}

#[cfg(test)]
mod tests;
