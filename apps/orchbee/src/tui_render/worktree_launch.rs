//! Draw the complete Worktree decision before arming its input token.
use super::modal::{
    ModalAction, ModalLayout, SingleModalLayout, description_line, modal_key_label, modal_layout,
    paint_modal_action, paint_modal_close, pointer_presentation, set_modal_cursor,
    single_modal_layout, status_line, title_line,
};
use super::{
    Clear, Frame, Line, Locale, ModalPresentation, Padding, Paragraph, Rect, Theme, Wrap,
    display_width, truncate_display,
};
use crate::client_view::{Binding, KeyCode, Modifiers};
use crate::terminal_session::{
    WorktreeMode, WorktreeNotice, WorktreeStage, WorktreeToken, WorktreeView,
};
use crate::terminal_text::sanitize_terminal_text;
use crate::tui_pointer::{
    HitLayer, HitMapBuilder, HitTarget, InteractionAvailability, ModalControl,
};
use crate::tui_theme::Role;
use ratatui::text::{Span, Text};
use unicode_segmentation::UnicodeSegmentation;

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    view: WorktreeView<'_>,
    content: Rect,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<WorktreeToken>) {
    if view.stage() == WorktreeStage::Checkout {
        return render_checkouts(
            frame,
            locale,
            CheckoutPicker {
                token: view.token(),
                mode: view.mode(),
                source: view.source_path().unwrap_or(""),
                count: view.choice_count(),
                selected: view.selected(),
            },
            content,
            theme,
            hits,
            |index| {
                view.choice(index).map(|(label, path)| CheckoutChoice {
                    label,
                    path,
                    openable: view.choice_openable(index),
                })
            },
        );
    }
    render_form(frame, locale, view, content, theme, hits)
}

#[derive(Clone, Copy)]
enum WorktreeLayout {
    Pair(ModalLayout),
    Single(SingleModalLayout),
}

impl WorktreeLayout {
    const fn area(self) -> Rect {
        match self {
            Self::Pair(layout) => layout.area,
            Self::Single(layout) => layout.area,
        }
    }

    const fn inner(self) -> Rect {
        match self {
            Self::Pair(layout) => layout.inner,
            Self::Single(layout) => layout.inner,
        }
    }

    const fn close(self) -> Rect {
        match self {
            Self::Pair(layout) => layout.close,
            Self::Single(layout) => layout.close,
        }
    }

    const fn hints(self) -> Rect {
        match self {
            Self::Pair(layout) => layout.hints,
            Self::Single(layout) => layout.hints,
        }
    }
}

#[derive(Clone, Copy)]
struct WorktreeFieldGeometry {
    row: Rect,
    navigation: Rect,
    primary: Rect,
    content: Rect,
}

impl WorktreeFieldGeometry {
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

fn resolve_worktree_layout(
    bounds: Rect,
    content: Rect,
    text: &Text<'_>,
    hints: &[super::controls::KeyHint<'_>],
    cancel: ModalAction<'_>,
    submit: Option<ModalAction<'_>>,
    theme: &Theme,
) -> Option<WorktreeLayout> {
    match submit {
        Some(submit) => modal_layout(bounds, content, text, hints, [cancel, submit], theme)
            .map(WorktreeLayout::Pair),
        None => single_modal_layout(bounds, content, text, hints, cancel, theme)
            .map(WorktreeLayout::Single),
    }
}

fn notice_role(notice: WorktreeNotice) -> Role {
    match notice {
        WorktreeNotice::Preparing | WorktreeNotice::Pending => Role::StatusPending,
        WorktreeNotice::Opened | WorktreeNotice::Removed => Role::StatusSuccess,
        WorktreeNotice::EntryPending => Role::StatusWorking,
        WorktreeNotice::InvalidInput
        | WorktreeNotice::InputLimit
        | WorktreeNotice::SourceUnavailable
        | WorktreeNotice::ListingUnavailable
        | WorktreeNotice::ConfigurationUnavailable
        | WorktreeNotice::NotSubmitted
        | WorktreeNotice::Failed => Role::StatusError,
        WorktreeNotice::Unknown | WorktreeNotice::SourceChanged => Role::StatusWarning,
        WorktreeNotice::PresentationUnavailable
        | WorktreeNotice::EntryDeferred
        | WorktreeNotice::Busy
        | WorktreeNotice::EntryFailed => Role::StatusInfo,
    }
}

fn render_form(
    frame: &mut Frame<'_>,
    locale: Locale,
    view: WorktreeView<'_>,
    content: Rect,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<WorktreeToken>) {
    let stage = view.stage();
    let removal = view.removal_target();
    let confirming = matches!(stage, WorktreeStage::Remove | WorktreeStage::ResumeRemove);
    let budget = usize::from(content.width.saturating_sub(10)).min(72);
    let mut lines = vec![
        title_line(locale.worktree_title(view.mode()), theme),
        Line::from(""),
    ];
    if let Some((label, path, branch, terminals)) = removal {
        lines[0] = title_line(
            locale.worktree_remove_title(&sanitize_terminal_text(label)),
            theme,
        );
        // The complete exact path must fit before a destructive decision can be armed.
        lines.push(Line::from(sanitize_terminal_text(path).into_owned()));
        lines.push(Line::from(locale.worktree_remove_scope(terminals)));
        lines.push(description_line(
            locale.worktree_remove_branch(branch.map(sanitize_terminal_text).as_deref()),
            theme,
        ));
    } else if let Some(source) = view.source_path() {
        lines.push(description_line(
            path_text(source, budget, theme.glyphs.ellipsis),
            theme,
        ));
    }
    let mut field_labels = None;
    if matches!(stage, WorktreeStage::Branch | WorktreeStage::Base) {
        let branch = lines.len();
        lines.extend([
            Line::from(locale.worktree_field(WorktreeStage::Branch)),
            Line::from(""),
        ]);
        lines.push(Line::from(""));
        let base = lines.len();
        lines.extend([
            Line::from(locale.worktree_field(WorktreeStage::Base)),
            Line::from(""),
        ]);
        field_labels = Some([branch, base]);
    }
    if let Some(id) = view.operation_id() {
        lines.push(description_line(
            truncate_display(
                &format!("orchbee worktree status {}", sanitize_terminal_text(id)),
                budget,
                theme.glyphs.ellipsis,
            ),
            theme,
        ));
    }
    if let Some(workspace) = view.workspace().filter(|_| removal.is_none()) {
        lines.push(description_line(
            truncate_display(
                &format!(
                    "{}: {}",
                    locale.worktree_workspace(),
                    sanitize_terminal_text(workspace),
                ),
                budget,
                theme.glyphs.ellipsis,
            ),
            theme,
        ));
    }
    if view.notice() != WorktreeNotice::Preparing || stage == WorktreeStage::Preparing {
        let notice = if removal.is_some() && view.notice() == WorktreeNotice::Failed {
            view.removal_failure().map_or_else(
                || locale.worktree_stage_notice(view.notice(), stage),
                |reason| locale.worktree_remove_failure(reason),
            )
        } else {
            locale.worktree_stage_notice(view.notice(), stage)
        };
        lines.push(status_line(notice, notice_role(view.notice()), theme));
    }
    if let Some(progress) = view.removal_progress() {
        lines.push(Line::from(
            locale.worktree_remove_workspace_state(&progress.workspace),
        ));
        if let Some(restoration) = &progress.restoration {
            lines.push(Line::from(locale.worktree_remove_restoration(restoration)));
        }
    }
    lines.extend([Line::from(""), Line::from(""), Line::from("")]);

    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let refresh = modal_key_label(theme, &Binding::new(KeyCode::Char('r'), Modifiers::NONE));
    let tab = modal_key_label(theme, &Binding::new(KeyCode::Tab, Modifiers::NONE));
    let clear = modal_key_label(theme, &Binding::new(KeyCode::Char('u'), Modifiers::CTRL));
    let mut hints = Vec::new();
    match stage {
        WorktreeStage::Branch | WorktreeStage::Base => {
            hints.extend([
                super::controls::KeyHint::new(&tab, locale.setup_next_field_hint()),
                super::controls::KeyHint::new(&clear, locale.setup_clear_hint()),
            ]);
        }
        WorktreeStage::Waiting
        | WorktreeStage::Result
        | WorktreeStage::Preparing
        | WorktreeStage::Remove
        | WorktreeStage::ResumeRemove => {}
        WorktreeStage::Checkout => unreachable!("checkout has its own picker surface"),
    }
    let cancel = ModalAction::new(
        locale.worktree_cancel_label(stage),
        super::controls::ActionKind::Secondary,
        true,
    )
    .with_shortcut(&escape);
    let submit = (stage != WorktreeStage::Preparing).then(|| {
        if removal.is_some() {
            let resume = view.can_resume_remove();
            let label = locale.worktree_remove_submit(stage, resume);
            let action = ModalAction::new(label, super::controls::ActionKind::Primary, true);
            return if confirming || resume {
                action
                    .with_shortcut(&enter)
                    .with_tone(crate::action_projection::ActionTone::Destructive)
            } else {
                action.with_shortcut(&refresh)
            };
        }
        ModalAction::new(
            locale.worktree_submit_label(stage),
            super::controls::ActionKind::Primary,
            true,
        )
        .with_shortcut(
            if matches!(stage, WorktreeStage::Branch | WorktreeStage::Base) {
                &enter
            } else {
                &refresh
            },
        )
    });
    let placeholder = Text::from(lines);
    let Some(candidate) = resolve_worktree_layout(
        frame.area(),
        content,
        &placeholder,
        &hints,
        cancel,
        submit,
        theme,
    ) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };

    let mut lines = placeholder.lines.clone();
    let mut field_rows = None;
    let mut active_window = None;
    if let Some([branch_label, base_label]) = field_labels {
        let Some(branch_geometry) =
            WorktreeFieldGeometry::resolve(&placeholder, candidate.inner(), branch_label, theme)
        else {
            return (Rect::default(), ModalPresentation::NotPresented);
        };
        let Some(base_geometry) =
            WorktreeFieldGeometry::resolve(&placeholder, candidate.inner(), base_label, theme)
        else {
            return (Rect::default(), ModalPresentation::NotPresented);
        };
        if branch_geometry.content.width < 5
            || branch_geometry.content.width != base_geometry.content.width
        {
            return (Rect::default(), ModalPresentation::NotPresented);
        }
        let field_width = usize::from(branch_geometry.content.width - 4);
        let Some(window) = view.field_window(field_width) else {
            return (Rect::default(), ModalPresentation::NotPresented);
        };
        for (label_index, label, raw_value, active, geometry) in [
            (
                branch_label,
                locale.worktree_field(WorktreeStage::Branch),
                view.branch(),
                stage == WorktreeStage::Branch,
                branch_geometry,
            ),
            (
                base_label,
                locale.worktree_field(WorktreeStage::Base),
                view.base(),
                stage == WorktreeStage::Base,
                base_geometry,
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
            let mut value = if active {
                window.display().to_owned()
            } else {
                truncate_display(
                    &sanitize_terminal_text(raw_value),
                    field_width,
                    theme.glyphs.ellipsis,
                )
            };
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
        field_rows = Some([
            (stage == WorktreeStage::Branch, branch_geometry),
            (stage == WorktreeStage::Base, base_geometry),
        ]);
        active_window = Some((
            if stage == WorktreeStage::Branch {
                branch_label + 1
            } else {
                base_label + 1
            },
            window,
            if stage == WorktreeStage::Branch {
                branch_geometry
            } else {
                base_geometry
            },
        ));
    }
    let text = Text::from(lines);
    let Some(layout) =
        resolve_worktree_layout(frame.area(), content, &text, &hints, cancel, submit, theme)
    else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    if layout.inner() != candidate.inner() {
        return (Rect::default(), ModalPresentation::NotPresented);
    }

    hits.start_modal();
    frame.render_widget(Clear, layout.area());
    frame.render_widget(
        super::overlay_surface(theme, true, Padding::horizontal(2)),
        layout.area(),
    );
    frame.render_widget(
        Paragraph::new(text.clone()).wrap(Wrap { trim: false }),
        layout.inner(),
    );
    paint_modal_close(
        frame,
        layout.close(),
        HitTarget::Worktree {
            token: view.token(),
            control: ModalControl::Cancel,
        },
        theme,
        hits,
    );
    if !hints.is_empty() {
        let rendered_hints =
            super::controls::render_key_hints(frame, layout.hints(), &hints, theme)
                .expect("admitted worktree hints must paint");
        debug_assert!(layout.hints().intersection(rendered_hints) == rendered_hints);
    }
    match layout {
        WorktreeLayout::Pair(layout) => {
            let primary = if matches!(stage, WorktreeStage::Waiting | WorktreeStage::Result)
                && !view.can_resume_remove()
            {
                ModalControl::Refresh
            } else {
                ModalControl::Submit
            };
            for ((rect, action), control) in layout
                .actions
                .into_iter()
                .zip([cancel, submit.expect("pair layout has a submit action")])
                .zip([ModalControl::Cancel, primary])
            {
                paint_modal_action(
                    frame,
                    Rect::new(rect.x, rect.y, layout.inner.right() - rect.x, 1),
                    rect,
                    action,
                    HitTarget::Worktree {
                        token: view.token(),
                        control,
                    },
                    theme,
                    hits,
                );
            }
        }
        WorktreeLayout::Single(layout) => {
            paint_modal_action(
                frame,
                Rect::new(
                    layout.action.x,
                    layout.action.y,
                    layout.inner.right() - layout.action.x,
                    1,
                ),
                layout.action,
                cancel,
                HitTarget::Worktree {
                    token: view.token(),
                    control: ModalControl::Cancel,
                },
                theme,
                hits,
            );
        }
    }
    if let Some(rows) = field_rows {
        for (active, geometry) in rows {
            let target = HitTarget::Worktree {
                token: view.token(),
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
            .expect("admitted worktree field row must paint");
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
    }
    hits.record(
        HitLayer::OverlaySurface,
        layout.area(),
        HitTarget::OverlaySurface,
    );
    if let Some((field_index, window, geometry)) = active_window {
        set_modal_cursor(
            frame,
            &text,
            layout.inner(),
            field_index,
            geometry.prefix_width() + 2 + window.cursor_column(),
        );
    }
    (layout.area(), ModalPresentation::Presented(view.token()))
}

struct CheckoutPicker<'a> {
    token: WorktreeToken,
    mode: WorktreeMode,
    source: &'a str,
    count: usize,
    selected: usize,
}
struct CheckoutChoice<'a> {
    label: String,
    path: &'a str,
    openable: bool,
}

#[derive(Clone, Copy)]
struct CheckoutRowGeometry {
    row: Rect,
    navigation: Rect,
    primary: Rect,
    label: Rect,
    path: Rect,
}

impl CheckoutRowGeometry {
    fn resolve(row: Rect, theme: &Theme) -> Option<Self> {
        if row.height != 2 {
            return None;
        }
        let navigation_width =
            u16::try_from(display_width(theme.glyphs.navigation_marker).max(1)).ok()?;
        let primary_x = row.x.checked_add(navigation_width)?;
        let content_x = primary_x.checked_add(1)?;
        let content_width = row.right().checked_sub(content_x)?;
        let navigation = Rect::new(row.x, row.y, navigation_width, row.height);
        let primary = Rect::new(primary_x, row.y, 1, row.height);
        let label = Rect::new(content_x, row.y, content_width, 1);
        let path = Rect::new(content_x, row.y + 1, content_width, 1);
        (!row.is_empty()
            && !label.is_empty()
            && row.intersection(navigation) == navigation
            && row.intersection(primary) == primary
            && row.intersection(label) == label
            && row.intersection(path) == path)
            .then_some(Self {
                row,
                navigation,
                primary,
                label,
                path,
            })
    }

    fn prefix_width(self) -> usize {
        usize::from(self.label.x.saturating_sub(self.row.x))
    }
}

struct RenderedCheckout {
    index: usize,
    label: String,
    path: String,
    openable: bool,
}

fn render_checkouts<'a>(
    frame: &mut Frame<'_>,
    locale: Locale,
    picker: CheckoutPicker<'_>,
    content: Rect,
    theme: &Theme,
    hits: &mut HitMapBuilder,
    choice: impl Fn(usize) -> Option<CheckoutChoice<'a>>,
) -> (Rect, ModalPresentation<WorktreeToken>) {
    let count = picker.count;
    let area = crate::tui_layout::centered_overlay(
        content,
        84.min(content.width.saturating_sub(4)),
        u16::try_from(count.saturating_mul(2).saturating_add(7))
            .unwrap_or(u16::MAX)
            .min(content.height.saturating_sub(2)),
    );
    let block = super::overlay_surface(theme, true, Padding::horizontal(2));
    let inner = block.inner(area);
    if area.is_empty()
        || frame.area().intersection(area) != area
        || inner.width < 20
        || inner.height < 7
        || count == 0
    {
        return (Rect::default(), ModalPresentation::NotPresented);
    }
    let capacity = usize::from((inner.height - 5) / 2);
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
        locale.worktree_cancel_label(WorktreeStage::Checkout),
        super::controls::ActionKind::Secondary,
        true,
    )
    .with_shortcut(&escape);
    let close = match super::controls::close_rect(frame.area(), area) {
        Some(close) => close,
        None => return (Rect::default(), ModalPresentation::NotPresented),
    };
    let hint_row = Rect::new(inner.x, inner.bottom() - 2, inner.width, 1);
    let action_row = Rect::new(inner.x, inner.bottom() - 1, inner.width, 1);
    let hints_valid = super::controls::measure_key_hints(&hints, theme).is_some();
    let first_hint_fits = super::controls::measure_key_hints(&hints[..1], theme)
        .is_some_and(|width| width <= usize::from(hint_row.width));
    let Some(action) = super::controls::action_rect(
        frame.area(),
        action_row,
        cancel.spec(super::controls::PointerPresentation::Rest),
        theme,
    ) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    if !hints_valid || !first_hint_fits || capacity == 0 {
        return (Rect::default(), ModalPresentation::NotPresented);
    }
    let list = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        u16::try_from(capacity.saturating_mul(2)).unwrap_or(u16::MAX),
    );
    if inner.intersection(list) != list {
        return (Rect::default(), ModalPresentation::NotPresented);
    }
    let row_geometries = (0..capacity)
        .map(|offset| {
            CheckoutRowGeometry::resolve(
                Rect::new(
                    inner.x,
                    inner.y + 2 + u16::try_from(offset.saturating_mul(2)).ok()?,
                    inner.width,
                    2,
                ),
                theme,
            )
        })
        .collect::<Option<Vec<_>>>();
    let Some(row_geometries) = row_geometries else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let choices = (start..count.min(start + capacity))
        .map(|index| {
            let CheckoutChoice {
                openable,
                label,
                path,
            } = choice(index).unwrap_or_else(|| CheckoutChoice {
                label: String::new(),
                path: "",
                openable: false,
            });
            let label = if openable {
                label
            } else {
                format!("{} · {label}", locale.setup_choice_unavailable())
            };
            let geometry = row_geometries[index - start];
            RenderedCheckout {
                index,
                label: truncate_display(
                    &sanitize_terminal_text(&label),
                    usize::from(geometry.label.width),
                    theme.glyphs.ellipsis,
                ),
                path: path_text(
                    path,
                    usize::from(geometry.path.width),
                    theme.glyphs.ellipsis,
                ),
                openable,
            }
        })
        .collect::<Vec<_>>();
    let mut lines = vec![
        title_line(
            truncate_display(
                locale.worktree_title(picker.mode),
                budget,
                theme.glyphs.ellipsis,
            ),
            theme,
        ),
        description_line(
            path_text(picker.source, budget, theme.glyphs.ellipsis),
            theme,
        ),
    ];
    for (offset, choice) in choices.iter().enumerate() {
        let geometry = row_geometries[offset];
        let prefix = " ".repeat(geometry.prefix_width());
        let label_role = if choice.openable {
            Role::TextPrimary
        } else {
            Role::TextDisabled
        };
        let path_role = if choice.openable {
            Role::TextSecondary
        } else {
            Role::TextDisabled
        };
        lines.push(Line::from(format!("{prefix}{}", choice.label)).style(theme.style(label_role)));
        lines.push(Line::from(format!("{prefix}{}", choice.path)).style(theme.style(path_role)));
    }
    // Fixed footer rows do not participate in checkout scrolling.
    while lines.len() < usize::from(inner.height - 3) {
        lines.push(Line::from(""));
    }
    lines.push(description_line(
        format!("{}/{}", picker.selected + 1, count),
        theme,
    ));
    lines.extend([Line::from(""), Line::from("")]);

    hits.start_modal();
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
    paint_modal_close(
        frame,
        close,
        HitTarget::Worktree {
            token: picker.token,
            control: ModalControl::Cancel,
        },
        theme,
        hits,
    );
    let rendered_hints = super::controls::render_key_hints(frame, hint_row, &hints, theme)
        .expect("admitted worktree picker hints must paint");
    debug_assert!(hint_row.intersection(rendered_hints) == rendered_hints);
    paint_modal_action(
        frame,
        action_row,
        action,
        cancel,
        HitTarget::Worktree {
            token: picker.token,
            control: ModalControl::Cancel,
        },
        theme,
        hits,
    );
    for (offset, choice) in choices.into_iter().enumerate() {
        let geometry = row_geometries[offset];
        let target = HitTarget::Worktree {
            token: picker.token,
            control: ModalControl::Choice(choice.index),
        };
        let availability = if choice.openable {
            InteractionAvailability::Enabled
        } else {
            InteractionAvailability::Disabled
        };
        let pointer =
            hits.projected_modal_pointer(HitLayer::Overlay, geometry.row, &target, availability);
        let provisional = choice.index == picker.selected;
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
            if choice.openable {
                super::controls::Availability::Enabled
            } else {
                super::controls::Availability::Disabled
            },
            super::controls::OperationState::Idle,
        );
        let bands = [
            super::controls::RowContentBand::semantic(geometry.label, Role::TextPrimary),
            super::controls::RowContentBand::semantic(geometry.path, Role::TextSecondary),
        ];
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
        .expect("admitted worktree checkout row must paint");
        assert_eq!(painted, geometry.row);
        hits.record_modal_control(HitLayer::Overlay, geometry.row, target, availability);
    }
    hits.record(
        HitLayer::OverlaySurface,
        list,
        HitTarget::ListScroll(crate::tui_pointer::ListScrollTarget::Worktree(picker.token)),
    );
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
    (area, ModalPresentation::Presented(picker.token))
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

#[cfg(test)]
mod tests;
