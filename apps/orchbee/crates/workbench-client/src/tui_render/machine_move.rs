//! Paints the question a Workspace's move asks before it stops a program, and records only the
//! controls tied to its token. It neither reads what runs nor moves the Workspace.

use super::controls::ActionKind;
use super::modal::{
    Dialog, DialogClass, DialogFooter, DialogRow, DialogTargets, ModalAction, dialog_layout,
    modal_key_label, paint_dialog,
};
use crate::action_projection::ActionTone;
use crate::client_view::{Binding, KeyCode, Modifiers};
use crate::localization::Locale;
use crate::machine_change::{Name, Stops};
use crate::machine_picker::MoveConfirmation;
use crate::terminal_text::sanitize_terminal_text;
use crate::terminal_title::display_name;
use crate::tui_hit_map::{HitMapBuilder, HitTarget};
use crate::tui_theme::{Role, Theme};
use ratatui::{Frame, widgets::Clear};

/// Paints `confirmation` over the whole viewport, and says whether the whole question was shown.
/// It looks and answers as the close-Workspace confirmation does; a window that cannot hold it
/// shows the TUI's size notice instead.
pub(super) fn render(
    frame: &mut Frame<'_>,
    confirmation: &MoveConfirmation,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> bool {
    let question = &confirmation.question;
    // An Agent by its name, else a title that names a program Orchbee knows; the rest are counted.
    let names: Vec<String> = match &question.stops {
        Stops::Busy { names, .. } => names
            .iter()
            .filter_map(|name| match name {
                Name::Agent(agent) => Some(sanitize_terminal_text(agent).into_owned()),
                Name::Title(title) => display_name(&sanitize_terminal_text(title), locale),
            })
            .collect(),
        Stops::Unread => Vec::new(),
    };
    let busy = match &question.stops {
        Stops::Busy { count, .. } => Some((*count, names.as_slice())),
        Stops::Unread => None,
    };
    let sentence = locale.machine_move_sentence(&sanitize_terminal_text(&question.to), busy);
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let actions = [
        ModalAction::new(
            locale.workspace_close_cancel_label(),
            ActionKind::Secondary,
            true,
        )
        .with_shortcut(&escape),
        ModalAction::new(locale.machine_move_confirm(), ActionKind::Primary, true)
            .with_shortcut(&enter)
            .with_tone(ActionTone::Destructive)
            .focused(true),
    ];
    let dialog = Dialog {
        title: locale.machine_move_title(),
        class: DialogClass::Narrow,
        rows: vec![
            DialogRow::Target(sanitize_terminal_text(&question.workspace)),
            DialogRow::Blank,
            // What stops is read whole before Move acts.
            DialogRow::Whole {
                text: sentence.into(),
                role: Role::TextPrimary,
                widen: false,
            },
            DialogRow::Prose(locale.machine_move_files().into()),
        ],
        footer: DialogFooter::Actions(&actions),
    };
    let target = |confirm| HitTarget::MachineMove {
        token: confirmation.token,
        confirm,
    };
    let targets = DialogTargets {
        close: target(false),
        actions: &[target(false), target(true)],
        dismiss_outside: false,
        scroll: None,
    };
    let viewport = frame.area();
    let Some(layout) = dialog_layout(viewport, &dialog, theme) else {
        // The question still owns every key and press; enlarging the window shows it again.
        hits.start_modal();
        frame.render_widget(Clear, viewport);
        super::render_too_small_notice(frame, viewport, locale.shell(), theme);
        return false;
    };
    paint_dialog(frame, &dialog, &layout, &targets, theme, hits);
    true
}
