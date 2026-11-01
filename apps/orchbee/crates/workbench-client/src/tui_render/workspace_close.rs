//! Paints the admitted Workspace close confirmation and records only controls tied to its
//! captured token. It does not decide membership or submit Workspace closure.

use super::modal::{
    Dialog, DialogClass, DialogFooter, DialogRow, DialogTargets, ModalAction, dialog_layout,
    modal_key_label, paint_dialog,
};
use crate::client_view::{Binding, KeyCode, Modifiers};
use crate::localization::Locale;
use crate::tui_hit_map::{HitMapBuilder, HitTarget};
use crate::tui_presentation::ModalPresentation;
use crate::tui_theme::Theme;
use crate::workbench_state::workspace_close::{
    WorkspaceCloseConfirmationView, WorkspaceCloseToken,
};
use ratatui::{Frame, layout::Rect};

/// Closing a Workspace always confirms: it ends the terminals of every Tab at once.
pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    confirmation: WorkspaceCloseConfirmationView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<WorkspaceCloseToken>) {
    let count = confirmation.project_member_count();
    let title = locale.workspace_close_title(count);
    let [ends, kept] = locale.workspace_close_consequence(count.is_some());
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let confirm_key = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let actions = [
        ModalAction::new(
            locale.workspace_close_cancel_label(),
            super::controls::ActionKind::Secondary,
            true,
        )
        .with_shortcut(&escape),
        // The button repeats the title's verb; the menu offers this same action as Close Workspace.
        ModalAction::new(
            locale.workspace_close_confirm_label(),
            super::controls::ActionKind::Primary,
            true,
        )
        .with_shortcut(&confirm_key)
        .with_tone(crate::action_projection::ActionTone::Destructive)
        .focused(true),
    ];
    let dialog = Dialog {
        title: &title,
        class: DialogClass::Narrow,
        rows: vec![
            DialogRow::Target(confirmation.label().into()),
            DialogRow::Blank,
            DialogRow::Prose(ends.into()),
            DialogRow::Prose(kept.into()),
        ],
        footer: DialogFooter::Actions(&actions),
    };
    let Some(layout) = dialog_layout(frame.area(), &dialog, theme) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let target = |confirm| HitTarget::WorkspaceClose {
        token: confirmation.token(),
        confirm,
    };
    paint_dialog(
        frame,
        &dialog,
        &layout,
        &DialogTargets {
            close: target(false),
            actions: &[target(false), target(true)],
            dismiss_outside: false,
            scroll: None,
        },
        theme,
        hits,
    );
    (
        layout.area,
        ModalPresentation::Presented(confirmation.token()),
    )
}
