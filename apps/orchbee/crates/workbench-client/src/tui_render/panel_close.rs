//! Paints the admitted Panel close confirmation and returns only controls and draw proof tied
//! to its exact confirmation token. It does not submit closure or infer physical disposal from
//! the displayed state.

use super::modal::{
    Dialog, DialogClass, DialogFooter, DialogRow, DialogTargets, ModalAction, dialog_layout,
    modal_key_label, paint_dialog,
};
use crate::client_view::{Binding, KeyCode, Modifiers};
use crate::localization::Locale;
use crate::tui_hit_map::{HitMapBuilder, HitTarget};
use crate::tui_presentation::ModalPresentation;
use crate::tui_theme::Theme;
use crate::workbench_client::PanelCloseConfirmationView;
use crate::workbench_state::PanelCloseConfirmationToken;
use ratatui::{Frame, layout::Rect};

pub(super) struct PanelModalDraw {
    pub(super) area: Rect,
    pub(super) prompt: ModalPresentation<PanelCloseConfirmationToken>,
}

/// The localized decision a Panel close presents, resolved before geometry.
struct PanelCloseCopy {
    target: String,
    tab: String,
    dissolves_tab: bool,
    consequence: String,
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    confirmation: &PanelCloseConfirmationView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> PanelModalDraw {
    let copy = PanelCloseCopy {
        target: locale
            .panel_close_target(confirmation.panel_position(), confirmation.panel_label()),
        tab: locale.panel_close_tab(&confirmation.tab_label(locale)),
        dissolves_tab: confirmation.dissolves_tab(),
        consequence: locale.close_warning(confirmation.impact()),
    };
    render_prepared(frame, locale, confirmation.token(), &copy, theme, hits)
}

/// The same atomic admission and paint the production compositor uses, from resolved copy.
fn render_prepared(
    frame: &mut Frame<'_>,
    locale: Locale,
    token: PanelCloseConfirmationToken,
    copy: &PanelCloseCopy,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> PanelModalDraw {
    let mut rows = vec![
        DialogRow::Target(copy.target.as_str().into()),
        DialogRow::Context(copy.tab.as_str().into()),
        DialogRow::Blank,
    ];
    if copy.dissolves_tab {
        rows.push(DialogRow::Prose(locale.panel_close_last_panel().into()));
    }
    rows.push(DialogRow::Prose(copy.consequence.as_str().into()));
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let confirm = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let actions = [
        ModalAction::new(
            locale.panel_close_cancel_label(),
            super::controls::ActionKind::Secondary,
            true,
        )
        .with_shortcut(&escape),
        // Closing a Panel ends or discards its terminals, as closing a Tab does.
        ModalAction::new(
            locale.panel_close_confirm_label(),
            super::controls::ActionKind::Primary,
            true,
        )
        .with_shortcut(&confirm)
        .with_tone(crate::action_projection::ActionTone::Destructive)
        .focused(true),
    ];
    let dialog = Dialog {
        title: locale.panel_close_title(),
        class: DialogClass::Narrow,
        rows,
        footer: DialogFooter::Actions(&actions),
    };
    let Some(layout) = dialog_layout(frame.area(), &dialog, theme) else {
        return PanelModalDraw {
            area: Rect::default(),
            prompt: ModalPresentation::NotPresented,
        };
    };
    let target = |confirm| HitTarget::PanelClose { token, confirm };
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
    PanelModalDraw {
        area: layout.area,
        prompt: ModalPresentation::Presented(token),
    }
}

#[cfg(test)]
mod tests;
