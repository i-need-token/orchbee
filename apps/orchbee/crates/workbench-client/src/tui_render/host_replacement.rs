//! Paints the question a click on Update asks before a Terminal Host replacement restarts the
//! terminals, and records only the controls tied to its token. It neither reads the Session nor
//! starts the replacement.

use super::controls::ActionKind;
use super::modal::{
    Dialog, DialogClass, DialogFooter, DialogRow, DialogTargets, ModalAction, dialog_layout,
    modal_key_label, paint_dialog,
};
use crate::action_projection::ActionTone;
use crate::client_view::{Binding, KeyCode, Modifiers};
use crate::localization::Locale;
use crate::machine_picker::HostReplacementQuestion;
use crate::tui_hit_map::{HitMapBuilder, HitTarget};
use crate::tui_theme::{Role, Theme};
use ratatui::{Frame, widgets::Clear};

/// Paints `question` over the whole viewport, and says whether the whole question was shown. It
/// looks and answers as a move's question does; a window that cannot hold it shows the TUI's size
/// notice instead.
pub(super) fn render(
    frame: &mut Frame<'_>,
    question: &HostReplacementQuestion,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> bool {
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let actions = [
        ModalAction::new(
            locale.workspace_close_cancel_label(),
            ActionKind::Secondary,
            true,
        )
        .with_shortcut(&escape),
        ModalAction::new(locale.host_replacement_confirm(), ActionKind::Primary, true)
            .with_shortcut(&enter)
            .with_tone(ActionTone::Destructive)
            .focused(true),
    ];
    let dialog = Dialog {
        title: locale.host_replacement_title(),
        class: DialogClass::Narrow,
        // What restarts is read whole before Restart acts.
        rows: vec![DialogRow::Whole {
            text: question.staying.map_or_else(
                || locale.host_replacement_sentence().into(),
                |count| locale.host_replacement_staying(count).into(),
            ),
            role: Role::TextPrimary,
            widen: false,
        }],
        footer: DialogFooter::Actions(&actions),
    };
    let target = |confirm| HitTarget::HostReplacement {
        token: question.token,
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
