//! Paints the notice of a command login offered to this TUI: the Machine, how many commands wait,
//! what declining does, and the two choices. A notice the person accepted says the login is
//! starting until its question replaces it, or why the login ended, with Close, if it ended first.

use super::controls::ActionKind;
use super::modal::{
    Dialog, DialogClass, DialogFooter, DialogRow, DialogTargets, ModalAction, dialog_layout,
    modal_key_label, paint_dialog,
};
use crate::client_view::{Binding, KeyCode, Modifiers};
use crate::localization::{Locale, LoginLabel, WaitingLabel};
use crate::machine_login::{Ending, Withdrawn};
use crate::machine_picker::{WaitingControl, WaitingNotice};
use crate::terminal_text::sanitize_terminal_text;
use crate::tui_hit_map::{HitMapBuilder, HitTarget};
use crate::tui_theme::{Role, Theme};
use orchbee_machine_protocol::hub::LoginEnd;
use ratatui::{Frame, widgets::Clear};

/// Paints `notice` over the whole viewport for the Machine named `label`, and says whether the
/// whole notice was shown. A window that cannot hold it shows the TUI's size notice instead.
pub(super) fn render(
    frame: &mut Frame<'_>,
    notice: &WaitingNotice,
    label: &str,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> bool {
    let offered = &notice.offered;
    let machine = format!(
        "{label} {} {}",
        theme.glyphs.middle_dot,
        sanitize_terminal_text(&offered.target)
    );
    let target = |control| HitTarget::MachineWaiting {
        login: offered.login,
        control,
    };
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    if let Some(why) = offered.ended {
        let role = match why {
            Withdrawn::Ended(Ending::Hub(LoginEnd::Succeeded)) => Role::StatusSuccess,
            Withdrawn::Taken => Role::StatusWarning,
            Withdrawn::Ended(_) => Role::StatusError,
        };
        let close = [ModalAction::new(
            locale.machine_login_label(LoginLabel::Close),
            ActionKind::Primary,
            true,
        )
        .with_shortcut(&enter)];
        let dialog = Dialog {
            title: locale.machine_login_label(LoginLabel::Title),
            class: DialogClass::Narrow,
            rows: vec![
                DialogRow::Target(machine.into()),
                DialogRow::Blank,
                DialogRow::Status(locale.machine_waiting_ended(why).into(), role),
            ],
            footer: DialogFooter::Actions(&close),
        };
        let targets = DialogTargets {
            close: target(WaitingControl::Close),
            actions: &[target(WaitingControl::Close)],
            dismiss_outside: false,
            scroll: None,
        };
        return paint(frame, &dialog, &targets, locale, theme, hits);
    }
    let choosable = !offered.accepted;
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let actions = [
        ModalAction::new(
            locale.machine_waiting_label(WaitingLabel::Decline),
            ActionKind::Secondary,
            choosable,
        )
        .with_shortcut(&escape),
        ModalAction::new(
            locale.machine_waiting_label(WaitingLabel::LogIn),
            ActionKind::Primary,
            choosable,
        )
        .with_shortcut(&enter),
    ];
    let targets = DialogTargets {
        close: target(WaitingControl::Close),
        actions: &[
            target(WaitingControl::Decline),
            target(WaitingControl::Accept),
        ],
        // A stray press never fails a command.
        dismiss_outside: false,
        scroll: None,
    };
    let several = offered.waiters != 1;
    let mut rows = vec![DialogRow::Target(machine.into()), DialogRow::Blank];
    // A claimed login stays after its last command left.
    if offered.waiters > 0 {
        rows.push(DialogRow::Whole {
            text: locale.machine_waiting_count(offered.waiters).into(),
            role: Role::TextPrimary,
            widen: false,
        });
        rows.push(DialogRow::Whole {
            text: locale.machine_waiting_consequence(several).into(),
            role: Role::TextSecondary,
            widen: false,
        });
    }
    if offered.accepted {
        rows.push(DialogRow::Status(
            locale.machine_waiting_label(WaitingLabel::Starting).into(),
            Role::StatusPending,
        ));
    }
    let dialog = Dialog {
        title: locale.machine_login_label(LoginLabel::Title),
        class: DialogClass::Narrow,
        rows,
        footer: DialogFooter::Actions(&actions),
    };
    paint(frame, &dialog, &targets, locale, theme, hits)
}

/// Paints `dialog` over the whole viewport, and says whether it was shown whole.
fn paint(
    frame: &mut Frame<'_>,
    dialog: &Dialog<'_>,
    targets: &DialogTargets<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> bool {
    let viewport = frame.area();
    let Some(layout) = dialog_layout(viewport, dialog, theme) else {
        // The notice still owns every key and press; enlarging the window shows it again.
        hits.start_modal();
        frame.render_widget(Clear, viewport);
        super::render_too_small_notice(frame, viewport, locale.shell(), theme);
        return false;
    };
    paint_dialog(frame, dialog, &layout, targets, theme, hits);
    true
}
