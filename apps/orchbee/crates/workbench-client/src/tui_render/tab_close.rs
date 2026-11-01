//! Paints the admitted Tab close modal and returns token-bound control geometry and
//! presentation evidence. It does not accept a close decision or perform terminal lifecycle
//! effects.

use super::modal::{
    Dialog, DialogClass, DialogFooter, DialogRow, DialogTargets, ModalAction, dialog_layout,
    modal_key_label, paint_dialog,
};
use crate::action_projection::ActionTone;
use crate::client_view::{Binding, KeyCode, Modifiers};
use crate::localization::Locale;
use crate::tui_hit_map::{HitMapBuilder, HitTarget};
use crate::tui_presentation::ModalPresentation;
use crate::tui_theme::{Role, Theme};
use crate::workbench_client::CloseConfirmationView;
use crate::workbench_state::{CloseConfirmationToken, CloseStatusToken};
use ratatui::{Frame, layout::Rect};

pub(super) struct ModalDraw {
    pub(super) area: Rect,
    pub(super) prompt: ModalPresentation<CloseConfirmationToken>,
    pub(super) status: Option<CloseStatusToken>,
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    confirmation: &CloseConfirmationView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> ModalDraw {
    let status = confirmation.resume_status();
    let title = confirmation.set_count().map_or_else(
        || locale.close_title(confirmation.kind()).to_owned(),
        |count| locale.close_set_title(count),
    );
    let mut rows = Vec::new();
    if let Some(count) = confirmation.set_count() {
        // Names are previews, not target identities; each truncates to the dialog width.
        for (position, label) in confirmation
            .set_labels(locale)
            .unwrap_or_default()
            .into_iter()
            .take(3)
        {
            rows.push(DialogRow::Item(
                format!("#{position} · {label}").into(),
                Role::TextPrimary,
            ));
        }
        if count > 3 {
            rows.push(DialogRow::Item(
                locale.close_set_more(count - 3).into(),
                Role::TextSecondary,
            ));
        }
    } else {
        rows.push(DialogRow::Target(confirmation.target_label(locale)));
    }
    rows.push(DialogRow::Blank);
    rows.push(DialogRow::Prose(
        locale.close_warning(confirmation.impact()).into(),
    ));
    if let Some(status) = status.as_ref() {
        let label = status.target_label(locale);
        rows.push(DialogRow::Status(
            locale.native_close_status(status.text(&label)).into(),
            Role::StatusWarning,
        ));
        if let Some(progress) = confirmation.resume_set_status() {
            rows.push(DialogRow::Status(
                locale.close_set_status(progress).into(),
                Role::StatusPending,
            ));
        }
    }
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let confirm = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let actions = [
        ModalAction::new(
            locale.tab_close_cancel_label(),
            super::controls::ActionKind::Secondary,
            true,
        )
        .with_shortcut(&escape),
        // A confirmation opens with focus on its primary; Enter takes it.
        ModalAction::new(
            locale.tab_close_confirm_label(),
            super::controls::ActionKind::Primary,
            true,
        )
        .with_shortcut(&confirm)
        .with_tone(ActionTone::Destructive)
        .focused(true),
    ];
    let dialog = Dialog {
        title: &title,
        class: DialogClass::Narrow,
        rows,
        footer: DialogFooter::Actions(&actions),
    };
    let Some(layout) = dialog_layout(frame.area(), &dialog, theme) else {
        return ModalDraw {
            area: Rect::default(),
            prompt: ModalPresentation::NotPresented,
            status: None,
        };
    };
    let target = |confirm| HitTarget::TabClose {
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
    ModalDraw {
        area: layout.area,
        prompt: ModalPresentation::Presented(confirmation.token()),
        status: status.map(|status| status.token()),
    }
}
