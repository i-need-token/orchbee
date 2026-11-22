//! Paints the card over the terminals of a machine that is not connected: the terminal panel is
//! cleared, so no other machine's terminals show around it, and the card sits in it with the
//! machine's state, its one action and Not now. It records only the controls tied to its token.

use super::controls::ActionKind;
use super::modal::{
    Dialog, DialogClass, DialogFooter, DialogRow, DialogTargets, ModalAction, dialog_layout,
    modal_key_label, paint_dialog,
};
use crate::client_view::{Binding, KeyCode, Modifiers};
use crate::localization::Locale;
use crate::machine_picker::Picker;
use crate::tui_hit_map::{HitMapBuilder, HitTarget};
use crate::tui_theme::Theme;
use ratatui::{Frame, layout::Rect, widgets::Clear};

impl Picker {
    /// Paints the card in the terminal `panel`, while one shows and the picker is closed.
    pub fn render_card(
        &self,
        frame: &mut Frame<'_>,
        panel: Rect,
        locale: Locale,
        theme: &Theme,
        hits: &mut HitMapBuilder,
    ) {
        if !self.card_shown() || panel.width == 0 || panel.height == 0 {
            return;
        }
        let Some(card) = self.card() else {
            return;
        };
        let title = locale.machine_card_title(card.machine);
        let rows: Vec<_> = locale
            .machine_card_lines(card.machine)
            .into_iter()
            .map(|line| DialogRow::Prose(line.into()))
            .collect();
        let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
        let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
        let not_now = ModalAction::new(locale.machine_card_not_now(), ActionKind::Secondary, true)
            .with_shortcut(&escape)
            .focused(card.action.is_none());
        let mut actions = vec![not_now];
        let target = |act| HitTarget::MachineCard {
            token: card.token,
            act,
        };
        let mut targets = vec![target(false)];
        if let Some(action) = card.action {
            actions.push(
                ModalAction::new(locale.machine_action(action), ActionKind::Primary, true)
                    .with_shortcut(&enter)
                    .focused(true),
            );
            targets.push(target(true));
        }
        let dialog = Dialog {
            title: &title,
            class: DialogClass::Narrow,
            rows,
            footer: DialogFooter::Actions(&actions),
        };
        let dialog_targets = DialogTargets {
            close: target(false),
            actions: &targets,
            dismiss_outside: false,
            scroll: None,
        };
        frame.render_widget(Clear, panel);
        if let Some(layout) = dialog_layout(panel, &dialog, theme) {
            paint_dialog(frame, &dialog, &layout, &dialog_targets, theme, hits);
        } else {
            // The card still owns every key and press; enlarging the window shows it again.
            hits.start_modal();
        }
    }
}
