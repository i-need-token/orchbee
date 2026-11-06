//! Draw the complete Worktree decision before arming its input token.
use super::controls::{self, ActionKind, KeyHint};
use super::modal::{
    Dialog, DialogClass, DialogFooter, DialogRow, DialogTargets, ModalAction, NAME_FIELD_WIDTH,
    dialog_layout, field_value_area, modal_key_label, paint_dialog, paint_field,
    pointer_presentation,
};
use super::{Frame, Locale, ModalPresentation, Rect, Theme, display_width, truncate_display};
use crate::action_projection::ActionTone;
use crate::client_view::{Binding, KeyCode, Modifiers};
use crate::terminal_text::{sanitize_terminal_text, truncate_display_middle};
use crate::tui_hit_map::{
    HitLayer, HitMapBuilder, HitTarget, InteractionAvailability, ModalControl,
};
use crate::tui_theme::Role;
use crate::workbench_state::worktree_launch::{
    WorktreeMode, WorktreeNotice, WorktreeStage, WorktreeToken, WorktreeView,
};
use std::borrow::Cow;

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    view: WorktreeView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<WorktreeToken>) {
    match view.stage() {
        WorktreeStage::Checkout => render_checkouts(
            frame,
            locale,
            &CheckoutPicker {
                token: view.token(),
                mode: view.mode(),
                source: view.source_path().unwrap_or(""),
                count: view.choice_count(),
                selected: view.selected(),
            },
            theme,
            hits,
            |index| {
                view.choice(index).map(|(label, path)| CheckoutChoice {
                    label,
                    path,
                    openable: view.choice_openable(index),
                })
            },
        ),
        WorktreeStage::Branch | WorktreeStage::Base => {
            render_form(frame, locale, &view, theme, hits)
        }
        WorktreeStage::Preparing
        | WorktreeStage::Remove
        | WorktreeStage::ResumeRemove
        | WorktreeStage::AbandonRemove
        | WorktreeStage::Waiting
        | WorktreeStage::Result => render_task(frame, locale, &view, theme, hits),
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
        | WorktreeNotice::NotRepository
        | WorktreeNotice::ConfigurationUnavailable
        | WorktreeNotice::NotSubmitted
        | WorktreeNotice::Failed
        | WorktreeNotice::ClientInCheckout => Role::StatusError,
        WorktreeNotice::Unknown | WorktreeNotice::SourceChanged => Role::StatusWarning,
        WorktreeNotice::PresentationUnavailable
        | WorktreeNotice::EntryDeferred
        | WorktreeNotice::Busy
        | WorktreeNotice::EntryFailed => Role::StatusInfo,
    }
}

fn worktree_target(view: &WorktreeView<'_>, control: ModalControl) -> HitTarget {
    HitTarget::Worktree {
        token: view.token(),
        control,
    }
}

/// The New Worktree form: Branch and Base under the source checkout. Editing keys live in Help.
fn render_form(
    frame: &mut Frame<'_>,
    locale: Locale,
    view: &WorktreeView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<WorktreeToken>) {
    const BRANCH: usize = 3;
    const BASE: usize = 7;
    let stage = view.stage();
    let errors = [view.field_error(false), view.field_error(true)];
    let help = |error: Option<crate::localization::WorktreeFieldError>, base: bool| match error {
        Some(error) => DialogRow::Error(locale.worktree_field_error(error, base).into()),
        None => DialogRow::Help((!base).then(|| locale.worktree_branch_help().into())),
    };
    let mut rows = vec![
        DialogRow::TargetPath(native_path(view.source_path().unwrap_or(""))),
        DialogRow::Blank,
        DialogRow::Label(
            locale.worktree_field(WorktreeStage::Branch).into(),
            stage == WorktreeStage::Branch,
        ),
        DialogRow::Field {
            minimum: NAME_FIELD_WIDTH,
        },
        help(errors[0], false),
        DialogRow::Blank,
        DialogRow::Label(
            locale.worktree_field(WorktreeStage::Base).into(),
            stage == WorktreeStage::Base,
        ),
        DialogRow::Field {
            minimum: NAME_FIELD_WIDTH,
        },
        help(errors[1], true),
    ];
    // A refusal that names no field, such as a destination the branch cannot form.
    if let Some(notice) = view
        .stated_notice()
        .filter(|_| errors.iter().all(Option::is_none))
    {
        rows.push(DialogRow::Status(
            locale.worktree_stage_notice(notice, stage).into(),
            notice_role(notice),
        ));
    }
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let actions = [
        ModalAction::new(
            locale.worktree_cancel_label(stage),
            ActionKind::Secondary,
            true,
        )
        .with_shortcut(&escape),
        ModalAction::new(
            locale.worktree_submit_label(view.mode()),
            ActionKind::Primary,
            errors.iter().all(Option::is_none),
        )
        .with_shortcut(&enter),
    ];
    let dialog = Dialog {
        title: locale.worktree_title(view.mode()),
        class: DialogClass::Medium,
        rows,
        footer: DialogFooter::Actions(&actions),
    };
    let Some(layout) = dialog_layout(frame.area(), &dialog, theme) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let (Some(branch_row), Some(base_row)) = (layout.row(BRANCH), layout.row(BASE)) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let width = usize::from(field_value_area(branch_row, theme).width);
    let Some(window) = view.field_window(width) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    paint_dialog(
        frame,
        &dialog,
        &layout,
        &DialogTargets {
            close: worktree_target(view, ModalControl::Close),
            actions: &[
                worktree_target(view, ModalControl::Cancel),
                worktree_target(view, ModalControl::Submit),
            ],
            dismiss_outside: view.branch().is_empty() && view.base().is_empty(),
            scroll: None,
        },
        theme,
        hits,
    );
    let ellipsis = theme.glyphs.ellipsis;
    for (row, raw, active, placeholder) in [
        (
            branch_row,
            view.branch(),
            stage == WorktreeStage::Branch,
            None,
        ),
        (
            base_row,
            view.base(),
            stage == WorktreeStage::Base,
            Some(locale.worktree_base_placeholder()),
        ),
    ] {
        match placeholder.filter(|_| raw.is_empty()) {
            Some(placeholder) => paint_field(frame, row, placeholder, true, theme),
            None if active => paint_field(frame, row, window.display(), false, theme),
            None => paint_field(
                frame,
                row,
                &truncate_display(&sanitize_terminal_text(raw), width, ellipsis),
                false,
                theme,
            ),
        }
        // A press on the other field moves the caret there.
        if !active {
            hits.record_modal_control(
                HitLayer::Overlay,
                row,
                worktree_target(view, ModalControl::NextField),
                InteractionAvailability::Enabled,
            );
        }
    }
    let value = field_value_area(
        if stage == WorktreeStage::Branch {
            branch_row
        } else {
            base_row
        },
        theme,
    );
    let column = u16::try_from(window.cursor_column())
        .unwrap_or(u16::MAX)
        .min(value.width.saturating_sub(1));
    frame.set_cursor_position((value.x.saturating_add(column), value.y));
    (layout.area, ModalPresentation::Presented(view.token()))
}

/// Every non-editing step: reading the source, the removal decisions, a running operation, and
/// a settled result. A running operation keeps its disabled primary and closes with `Close`;
/// closing never stops it, and its result returns as a status notice.
fn render_task(
    frame: &mut Frame<'_>,
    locale: Locale,
    view: &WorktreeView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<WorktreeToken>) {
    let stage = view.stage();
    let removal = view.removal_target();
    let decision = matches!(
        stage,
        WorktreeStage::Remove | WorktreeStage::ResumeRemove | WorktreeStage::AbandonRemove
    );
    let title = match stage {
        WorktreeStage::Remove | WorktreeStage::ResumeRemove => locale.worktree_remove_title(),
        WorktreeStage::AbandonRemove => locale.worktree_remove_abandon_title(),
        _ => locale.worktree_title(view.mode()),
    };
    let mut rows = Vec::new();
    if let Some((_, path, ..)) = removal {
        rows.push(DialogRow::TargetPath(native_path(path)));
        rows.push(DialogRow::Blank);
    } else if let Some(source) = view.source_path() {
        rows.push(DialogRow::TargetPath(native_path(source)));
        rows.push(DialogRow::Blank);
    }
    match (stage, removal) {
        (WorktreeStage::Remove | WorktreeStage::ResumeRemove, Some((_, _, branch, terminals))) => {
            rows.push(DialogRow::Prose(
                locale.worktree_remove_scope(terminals).into(),
            ));
            rows.push(DialogRow::Prose(
                locale
                    .worktree_remove_branch(branch.map(sanitize_terminal_text).as_deref())
                    .into(),
            ));
        }
        (WorktreeStage::AbandonRemove, _) => {
            rows.push(DialogRow::Prose(
                locale.worktree_remove_abandon_notice().into(),
            ));
        }
        _ => {}
    }
    if let Some(notice) = view.stated_notice().filter(|_| !decision) {
        let text = match view.removal_failure() {
            Some(
                orchbee_workbench_protocol::WorktreeOperationFailure::LaunchRequestTooLarge {},
            ) if notice == WorktreeNotice::Failed => locale.launch_request_too_large(),
            Some(reason) if removal.is_some() && notice == WorktreeNotice::Failed => {
                locale.worktree_remove_failure(reason)
            }
            _ => locale.worktree_stage_notice(notice, stage),
        };
        rows.push(DialogRow::Status(text.into(), notice_role(notice)));
        // An unknown result has one recovery, the status command.
        if notice == WorktreeNotice::Unknown
            && let Some(id) = view.operation_id()
        {
            rows.push(DialogRow::Command(
                format!("orchbee worktree status {}", sanitize_terminal_text(id)).into(),
            ));
        }
    }
    if let Some(progress) = view.removal_progress().filter(|_| !decision) {
        if let Some(workspace) = view.held_close() {
            rows.push(DialogRow::Prose(locale.worktree_remove_held_close().into()));
            rows.push(DialogRow::Command(
                format!(
                    "orchbee workspace close {} --confirm",
                    sanitize_terminal_text(workspace)
                )
                .into(),
            ));
        } else {
            rows.push(DialogRow::Prose(
                locale
                    .worktree_remove_workspace_state(&progress.workspace)
                    .into(),
            ));
        }
        if let Some(restoration) = &progress.restoration {
            rows.push(DialogRow::Prose(
                locale.worktree_remove_restoration(restoration).into(),
            ));
        }
    }
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let abandon_key = modal_key_label(theme, &Binding::new(KeyCode::Char('x'), Modifiers::NONE));
    let cancel = ModalAction::new(
        locale.worktree_cancel_label(stage),
        ActionKind::Secondary,
        true,
    )
    .with_shortcut(&escape);
    let mut actions = vec![cancel];
    let mut controls = vec![ModalControl::Cancel];
    match stage {
        WorktreeStage::Remove | WorktreeStage::ResumeRemove => {
            actions.push(
                ModalAction::new(
                    locale.worktree_remove_submit(stage),
                    ActionKind::Primary,
                    true,
                )
                .with_shortcut(&enter)
                .with_tone(ActionTone::Destructive)
                .focused(true),
            );
            controls.push(ModalControl::Submit);
        }
        // Stop Tracking forgets only this Client's receipt, so it is not destructive.
        WorktreeStage::AbandonRemove => {
            actions.push(
                ModalAction::new(
                    locale.worktree_remove_submit(stage),
                    ActionKind::Primary,
                    true,
                )
                .with_shortcut(&enter)
                .focused(true),
            );
            controls.push(ModalControl::Submit);
        }
        // The running operation's primary stays in place, disabled.
        WorktreeStage::Waiting => {
            actions.push(
                ModalAction::new(
                    removal.map_or_else(
                        || locale.worktree_submit_label(view.mode()),
                        |_| locale.worktree_remove_submit(WorktreeStage::Remove),
                    ),
                    ActionKind::Primary,
                    false,
                )
                .with_shortcut(&enter),
            );
            controls.push(ModalControl::Submit);
        }
        WorktreeStage::Result => {
            if view.can_abandon_remove() {
                actions.insert(
                    1,
                    ModalAction::new(
                        locale.worktree_remove_abandon_label(),
                        ActionKind::Secondary,
                        true,
                    )
                    .with_shortcut(&abandon_key),
                );
                controls.insert(1, ModalControl::Abandon);
            }
            if view.can_resume_remove() {
                actions.push(
                    ModalAction::new(
                        locale.worktree_remove_submit(stage),
                        ActionKind::Primary,
                        true,
                    )
                    .with_shortcut(&enter)
                    .with_tone(ActionTone::Destructive)
                    .focused(true),
                );
                controls.push(ModalControl::Submit);
            } else if actions.len() == 1
                && let Some(only) = actions.first_mut()
            {
                // A settled notice's only button takes the focus.
                *only = only.focused(true);
            }
        }
        WorktreeStage::Preparing | WorktreeStage::Branch | WorktreeStage::Base => {}
        WorktreeStage::Checkout => {}
    }
    let targets: Vec<_> = controls
        .iter()
        .map(|control| worktree_target(view, *control))
        .collect();
    let dialog = Dialog {
        title,
        class: DialogClass::Narrow,
        rows,
        footer: DialogFooter::Actions(&actions),
    };
    let Some(layout) = dialog_layout(frame.area(), &dialog, theme) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    paint_dialog(
        frame,
        &dialog,
        &layout,
        &DialogTargets {
            close: worktree_target(view, ModalControl::Close),
            actions: &targets,
            // A settled result loses nothing; a decision and a running operation stay open.
            dismiss_outside: matches!(stage, WorktreeStage::Result | WorktreeStage::Preparing),
            scroll: None,
        },
        theme,
        hits,
    );
    (layout.area, ModalPresentation::Presented(view.token()))
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
}

/// The Open Worktree picker: each checkout is its branch over its path. A picker has a hint row
/// instead of buttons, and a press outside closes it.
fn render_checkouts<'a>(
    frame: &mut Frame<'_>,
    locale: Locale,
    picker: &CheckoutPicker<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
    choice: impl Fn(usize) -> Option<CheckoutChoice<'a>>,
) -> (Rect, ModalPresentation<WorktreeToken>) {
    let count = picker.count;
    if count == 0 {
        return (Rect::default(), ModalPresentation::NotPresented);
    }
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let up = modal_key_label(theme, &Binding::new(KeyCode::Up, Modifiers::NONE));
    let down = modal_key_label(theme, &Binding::new(KeyCode::Down, Modifiers::NONE));
    let vertical = format!("{up}{down}");
    let hints = [
        KeyHint::new(&escape, locale.setup_close_hint()),
        KeyHint::new(&enter, locale.worktree_choose_hint()),
        KeyHint::new(&vertical, locale.setup_select_hint()),
    ];
    let dialog = Dialog {
        title: locale.worktree_title(picker.mode),
        class: DialogClass::Medium,
        rows: vec![
            DialogRow::TargetPath(native_path(picker.source)),
            DialogRow::Blank,
            DialogRow::List {
                len: count.saturating_mul(2),
            },
        ],
        footer: DialogFooter::Hints(&hints),
    };
    let Some(layout) = dialog_layout(frame.area(), &dialog, theme) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let Some(list) = layout.row(2) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let capacity = usize::from(list.height / 2);
    if capacity == 0 {
        return (Rect::default(), ModalPresentation::NotPresented);
    }
    let start = picker
        .selected
        .saturating_sub(capacity / 2)
        .min(count.saturating_sub(capacity));
    let rows = (start..count.min(start + capacity))
        .map(|index| {
            let offset = u16::try_from((index - start).saturating_mul(2)).ok()?;
            let geometry = CheckoutRowGeometry::resolve(
                Rect::new(list.x, list.y.checked_add(offset)?, list.width, 2),
                theme,
            )?;
            let CheckoutChoice {
                label,
                path,
                openable,
            } = choice(index).unwrap_or(CheckoutChoice {
                label: String::new(),
                path: "",
                openable: false,
            });
            let label = if openable {
                label
            } else {
                format!("{} · {label}", locale.setup_choice_unavailable())
            };
            Some((index, geometry, label, path, openable))
        })
        .collect::<Option<Vec<_>>>();
    let Some(rows) = rows else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    paint_dialog(
        frame,
        &dialog,
        &layout,
        &DialogTargets {
            close: HitTarget::Worktree {
                token: picker.token,
                control: ModalControl::Close,
            },
            actions: &[],
            dismiss_outside: true,
            scroll: Some(HitTarget::ListScroll(
                crate::tui_hit_map::ListScrollTarget::Worktree(picker.token),
            )),
        },
        theme,
        hits,
    );
    let ellipsis = theme.glyphs.ellipsis;
    for (index, geometry, label, path, openable) in rows {
        let (label_role, path_role) = if openable {
            (Role::TextPrimary, Role::TextSecondary)
        } else {
            (Role::TextDisabled, Role::TextDisabled)
        };
        frame.render_widget(
            ratatui::widgets::Paragraph::new(truncate_display(
                &sanitize_terminal_text(&label),
                usize::from(geometry.label.width),
                ellipsis,
            ))
            .style(theme.style(label_role)),
            geometry.label,
        );
        frame.render_widget(
            ratatui::widgets::Paragraph::new(truncate_display_middle(
                &native_path(path),
                usize::from(geometry.path.width),
                ellipsis,
            ))
            .style(theme.style(path_role)),
            geometry.path,
        );
        let target = HitTarget::Worktree {
            token: picker.token,
            control: ModalControl::Choice(index),
        };
        let availability = if openable {
            InteractionAvailability::Enabled
        } else {
            InteractionAvailability::Disabled
        };
        let pointer =
            hits.projected_modal_pointer(HitLayer::Overlay, geometry.row, &target, availability);
        let provisional = index == picker.selected;
        let state = controls::InteractionState::new(
            if provisional {
                controls::FocusState::Focused
            } else {
                controls::FocusState::Unfocused
            },
            pointer_presentation(pointer),
            if provisional {
                controls::NavigationState::Provisional
            } else {
                controls::NavigationState::None
            },
            controls::CurrentState::NotCurrent,
            controls::CheckedState::Unchecked,
            if openable {
                controls::Availability::Enabled
            } else {
                controls::Availability::Disabled
            },
            controls::OperationState::Idle,
        );
        let bands = [
            controls::RowContentBand::semantic(geometry.label, label_role),
            controls::RowContentBand::semantic(geometry.path, path_role),
        ];
        let painted = controls::paint_row_interaction(
            frame,
            geometry.row,
            controls::RowTracks::new(controls::RowFocusCue::EnclosingFrame, geometry.primary)
                .with_provisional(geometry.navigation),
            &bands,
            controls::ControlSurface::Overlay,
            state,
            theme,
        )
        .expect("admitted worktree checkout row must paint");
        assert_eq!(painted, geometry.row);
        hits.record_modal_control(HitLayer::Overlay, geometry.row, target, availability);
    }
    (layout.area, ModalPresentation::Presented(picker.token))
}

/// Git reports Windows checkouts with forward slashes; display uses the native separator the
/// user sees elsewhere. Identities and submitted values keep the reported text.
fn native_path(path: &str) -> Cow<'_, str> {
    if path.contains('/') {
        Cow::Owned(sanitize_terminal_text(path).replace('/', "\\"))
    } else {
        sanitize_terminal_text(path)
    }
}

#[cfg(test)]
mod tests;
