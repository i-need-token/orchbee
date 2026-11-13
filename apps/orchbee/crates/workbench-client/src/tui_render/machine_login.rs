//! Paints the login overlay: the Machine, `ssh`'s own prompt escaped and bounded, Orchbee's line
//! for the question, and the field; or how a login that ended without the person ended. A secret
//! field shows one mask glyph per typed character and nothing of the characters; the painter reads
//! only their count, or a new host's typed text while it begins `yes`. A remote pane whose link
//! was lost carries one line offering Enter to log in again.

use super::controls::ActionKind;
use super::modal::{
    Dialog, DialogClass, DialogFooter, DialogRow, DialogTargets, FieldTone, ModalAction,
    NAME_FIELD_WIDTH, dialog_layout, field_value_area, modal_key_label, paint_dialog, paint_field,
    status_line,
};
use crate::client_view::{Binding, KeyCode, Modifiers};
use crate::localization::{Locale, LoginLabel, MachineQuestion};
use crate::machine_login::{AskNotice, Ending};
use crate::machine_picker::{LoginControl, LoginOverlay, Picker};
use crate::machine_question::{CUT, shown_lines};
use crate::terminal_text::{display_width, sanitize_terminal_text, truncate_display};
use crate::tui_hit_map::{HitMapBuilder, HitTarget};
use crate::tui_theme::{Role, Theme};
use orchbee_machine_protocol::hub::LoginEnd;
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Clear, Paragraph},
};
use std::borrow::Cow;

impl Picker {
    /// Paints the lost-link line across the middle of the remote pane's `panel`, while nothing of
    /// the picker covers it.
    pub fn render_lost_link(
        &self,
        frame: &mut Frame<'_>,
        panel: Rect,
        locale: Locale,
        theme: &Theme,
    ) {
        if !self.lost_link || self.captures() || panel.width < 4 || panel.height == 0 {
            return;
        }
        let bullet = display_width(theme.glyphs.status_bullet) + 1;
        let text = truncate_display(
            locale.machine_link_lost(),
            usize::from(panel.width).saturating_sub(bullet),
            theme.glyphs.ellipsis,
        );
        let width = u16::try_from(display_width(&text) + bullet)
            .unwrap_or(u16::MAX)
            .min(panel.width);
        let line = Rect::new(
            panel.x + (panel.width - width) / 2,
            panel.y + panel.height / 2,
            width,
            1,
        );
        frame.render_widget(Clear, line);
        frame.render_widget(
            Paragraph::new(status_line(text, Role::StatusWarning, theme)),
            line,
        );
    }
}

/// Paints `login` over the whole viewport for the Machine named `label`, and says whether the
/// whole question was shown: Orchbee's asking sentence and a new host's fingerprint whole, `ssh`'s
/// own lines kept from the last one up with the cut marked (at least its asking line for a
/// question only `ssh` words), the field and the buttons. A window that cannot hold them shows the
/// TUI's size notice instead, and the answer cannot be submitted until one can.
pub(super) fn render(
    frame: &mut Frame<'_>,
    login: &LoginOverlay,
    label: &str,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> bool {
    let presented = &login.presented;
    let machine = format!(
        "{label} {} {}",
        theme.glyphs.middle_dot,
        sanitize_terminal_text(&presented.target)
    );
    let shown = match presented.outcome {
        Some(ending) => render_ended(frame, login, &machine, ending, locale, theme, hits),
        None => render_question(frame, login, &machine, locale, theme, hits),
    };
    if !shown {
        // The overlay still owns every key and press; enlarging the window shows it again.
        let viewport = frame.area();
        hits.start_modal();
        frame.render_widget(Clear, viewport);
        super::render_too_small_notice(frame, viewport, locale.shell(), theme);
    }
    shown
}

/// Paints the overlay of a login that ended without the person: how it ended, and Close.
fn render_ended(
    frame: &mut Frame<'_>,
    login: &LoginOverlay,
    machine: &str,
    ending: Ending,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> bool {
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let actions = [ModalAction::new(
        locale.machine_login_label(LoginLabel::Close),
        ActionKind::Primary,
        true,
    )
    .with_shortcut(&enter)];
    let close = || HitTarget::MachineLogin {
        token: login.presented.token,
        control: LoginControl::Close,
    };
    let targets = DialogTargets {
        close: close(),
        actions: &[close()],
        dismiss_outside: false,
        scroll: None,
    };
    let role = if ending == Ending::Hub(LoginEnd::Succeeded) {
        Role::StatusSuccess
    } else {
        Role::StatusError
    };
    let dialog = Dialog {
        title: locale.machine_login_label(LoginLabel::Title),
        class: DialogClass::Medium,
        rows: vec![
            DialogRow::Target(machine.into()),
            DialogRow::Blank,
            DialogRow::Status(locale.machine_login_outcome(ending).into(), role),
        ],
        footer: DialogFooter::Actions(&actions),
    };
    let Some(layout) = dialog_layout(frame.area(), &dialog, theme) else {
        return false;
    };
    paint_dialog(frame, &dialog, &layout, &targets, theme, hits);
    true
}

/// Paints an open question; false when the window cannot hold the whole question.
fn render_question(
    frame: &mut Frame<'_>,
    login: &LoginOverlay,
    machine: &str,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> bool {
    let presented = &login.presented;
    let question = MachineQuestion::of(&presented.kind);
    let host_key = login.host_key();
    let said = locale.machine_question_said();
    let mut prompt = shown_lines(presented.prompt.as_str());
    let bounded = prompt.first().is_some_and(|line| line == CUT);
    if bounded {
        prompt.remove(0);
    }
    let prompt: Vec<String> = prompt
        .into_iter()
        .map(|line| format!("{said}{line}"))
        .collect();
    let cut_mark = format!("{said}{CUT}");
    // A question only `ssh` words asks in its last line, which must stay.
    let fewest = usize::from(matches!(question, MachineQuestion::Other) && !prompt.is_empty());
    let mut asked = Vec::new();
    let whole =
        |text: Cow<'static, str>, role: Role, widen: bool| DialogRow::Whole { text, role, widen };
    match &question {
        MachineQuestion::HostKey {
            algorithm,
            fingerprint,
        } => {
            asked.push(whole(
                locale
                    .machine_question_new_host_lead(&presented.target, algorithm)
                    .into(),
                Role::TextPrimary,
                false,
            ));
            asked.push(whole(
                (*fingerprint).to_owned().into(),
                Role::TextPrimary,
                true,
            ));
            asked.push(whole(
                locale.machine_question_type_yes().trim_end().into(),
                Role::AccentFocus,
                false,
            ));
        }
        _ => asked.push(whole(
            locale
                .machine_question_asked(&presented.target, &question, false)
                .trim_end()
                .to_owned()
                .into(),
            Role::AccentFocus,
            false,
        )),
    }
    let status = if presented.sending {
        DialogRow::Status(
            locale.machine_login_label(LoginLabel::Sending).into(),
            Role::StatusPending,
        )
    } else if login.type_yes {
        DialogRow::Error(locale.machine_login_label(LoginLabel::TypeYes).into())
    } else {
        match presented.notice {
            Some(AskNotice::Repeated) => {
                DialogRow::Error(locale.machine_login_repeated(&question).into())
            }
            Some(AskNotice::Inadmissible) => {
                DialogRow::Error(locale.machine_answer_undelivered(true).into())
            }
            None => DialogRow::Help(
                (!host_key).then(|| locale.machine_login_label(LoginLabel::Privacy).into()),
            ),
        }
    };
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let actions = [
        ModalAction::new(
            locale.machine_login_label(LoginLabel::Cancel),
            ActionKind::Secondary,
            true,
        )
        .with_shortcut(&escape),
        ModalAction::new(
            locale.machine_login_label(if host_key {
                LoginLabel::Trust
            } else {
                LoginLabel::LogIn
            }),
            ActionKind::Primary,
            !presented.sending,
        )
        .with_shortcut(&enter),
    ];
    let target = |control| HitTarget::MachineLogin {
        token: presented.token,
        control,
    };
    let targets = DialogTargets {
        close: target(LoginControl::Close),
        actions: &[target(LoginControl::Cancel), target(LoginControl::Submit)],
        // A stray press never ends a login.
        dismiss_outside: false,
        scroll: None,
    };
    // `ssh`'s last `kept` lines; a mark stands for every earlier line that went.
    let compose = |kept: usize| {
        let mut rows = vec![DialogRow::Target(machine.into())];
        if kept > 0 {
            rows.push(DialogRow::Blank);
            if bounded || kept < prompt.len() {
                rows.push(DialogRow::Item(
                    cut_mark.as_str().into(),
                    Role::TextSecondary,
                ));
            }
            rows.extend(
                prompt[prompt.len() - kept..]
                    .iter()
                    .map(|line| DialogRow::Whole {
                        text: line.as_str().into(),
                        role: Role::TextSecondary,
                        widen: false,
                    }),
            );
        }
        rows.push(DialogRow::Blank);
        rows.extend(asked.iter().map(copy_row));
        rows.push(DialogRow::Field {
            minimum: NAME_FIELD_WIDTH,
        });
        rows.push(copy_row(&status));
        Dialog {
            title: locale.machine_login_label(LoginLabel::Title),
            class: DialogClass::Medium,
            rows,
            footer: DialogFooter::Actions(&actions),
        }
    };
    let viewport = frame.area();
    let Some((dialog, layout)) = (fewest..=prompt.len()).rev().find_map(|kept| {
        let dialog = compose(kept);
        dialog_layout(viewport, &dialog, theme).map(|layout| (dialog, layout))
    }) else {
        return false;
    };
    let Some(field) = dialog
        .rows
        .iter()
        .position(|row| matches!(row, DialogRow::Field { .. }))
        .and_then(|index| layout.row(index))
    else {
        return false;
    };
    paint_dialog(frame, &dialog, &layout, &targets, theme, hits);
    let value_area = field_value_area(field, theme);
    let width = usize::from(value_area.width);
    let value = match login.shown_yes() {
        Some(yes) => sanitize_terminal_text(&yes).into_owned(),
        None => theme
            .glyphs
            .secret_mask
            .repeat(presented.field.characters().min(width)),
    };
    paint_field(frame, field, &value, FieldTone::Value, theme);
    let column = u16::try_from(display_width(&value))
        .unwrap_or(u16::MAX)
        .min(value_area.width.saturating_sub(1));
    frame.set_cursor_position((value_area.x.saturating_add(column), value_area.y));
    true
}

/// A second borrow of a row, so one composition can be tried with fewer of `ssh`'s lines.
fn copy_row<'a>(row: &'a DialogRow<'a>) -> DialogRow<'a> {
    match row {
        DialogRow::Whole { text, role, widen } => DialogRow::Whole {
            text: Cow::Borrowed(text),
            role: *role,
            widen: *widen,
        },
        DialogRow::Error(text) => DialogRow::Error(Cow::Borrowed(text)),
        DialogRow::Status(text, role) => DialogRow::Status(Cow::Borrowed(text), *role),
        DialogRow::Help(text) => DialogRow::Help(text.as_deref().map(Cow::Borrowed)),
        _ => DialogRow::Blank,
    }
}
