//! Paints the login overlay: the Machine, `ssh`'s own prompt escaped and bounded, Orchbee's line
//! for the question, and the field. A secret field shows one mask glyph per typed character and
//! nothing of the characters; the painter reads only their count. A remote pane whose link was
//! lost carries one line offering Enter to log in again.

use super::controls::ActionKind;
use super::modal::{
    Dialog, DialogClass, DialogFooter, DialogRow, DialogTargets, FieldTone, ModalAction,
    NAME_FIELD_WIDTH, dialog_layout, field_value_area, modal_key_label, paint_dialog, paint_field,
    status_line,
};
use crate::client_view::{Binding, KeyCode, Modifiers};
use crate::localization::{Locale, LoginLabel, MachineQuestion};
use crate::machine_login::AskNotice;
use crate::machine_picker::{LoginControl, LoginOverlay, Picker};
use crate::machine_question::shown_lines;
use crate::terminal_text::{display_width, sanitize_terminal_text, truncate_display};
use crate::tui_hit_map::{HitMapBuilder, HitTarget};
use crate::tui_theme::{Role, Theme};
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

/// Paints `login` over the whole viewport for the Machine named `label`. Under height pressure
/// `ssh`'s prompt goes first; a window that cannot hold the rest paints nothing.
pub(super) fn render(
    frame: &mut Frame<'_>,
    login: &LoginOverlay,
    label: &str,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    let presented = &login.presented;
    let question = MachineQuestion::of(&presented.kind);
    let host_key = login.host_key();
    let said = locale.machine_question_said();
    let prompt: Vec<String> = shown_lines(presented.prompt.as_str())
        .into_iter()
        .map(|line| format!("{said}{line}"))
        .collect();
    let machine = format!(
        "{label} {} {}",
        theme.glyphs.middle_dot,
        sanitize_terminal_text(&presented.target)
    );
    let mut asked = Vec::new();
    let label_text = match &question {
        MachineQuestion::HostKey {
            algorithm,
            fingerprint,
        } => {
            asked.push(DialogRow::Prose(
                locale
                    .machine_question_new_host(&presented.target, algorithm, fingerprint)
                    .into(),
            ));
            Cow::Borrowed(locale.machine_question_type_yes().trim_end())
        }
        _ => Cow::Owned(
            locale
                .machine_question_asked(&presented.target, &question, false)
                .trim_end()
                .to_owned(),
        ),
    };
    asked.push(DialogRow::Label(label_text, true));
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
    let compose = |with_prompt: bool| {
        let mut rows = vec![DialogRow::Target(machine.as_str().into())];
        if with_prompt && !prompt.is_empty() {
            rows.push(DialogRow::Blank);
            rows.extend(
                prompt
                    .iter()
                    .map(|line| DialogRow::Item(line.as_str().into(), Role::TextSecondary)),
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
    let (dialog, layout) = [true, false].into_iter().find_map(|with_prompt| {
        let dialog = compose(with_prompt);
        dialog_layout(viewport, &dialog, theme).map(|layout| (dialog, layout))
    })?;
    let field = dialog
        .rows
        .iter()
        .position(|row| matches!(row, DialogRow::Field { .. }))
        .and_then(|index| layout.row(index))?;
    paint_dialog(frame, &dialog, &layout, &targets, theme, hits);
    let value_area = field_value_area(field, theme);
    let width = usize::from(value_area.width);
    let value = if host_key {
        sanitize_terminal_text(&login.yes).into_owned()
    } else {
        theme
            .glyphs
            .secret_mask
            .repeat(presented.field.characters().min(width))
    };
    paint_field(frame, field, &value, FieldTone::Value, theme);
    let column = u16::try_from(display_width(&value))
        .unwrap_or(u16::MAX)
        .min(value_area.width.saturating_sub(1));
    frame.set_cursor_position((value_area.x.saturating_add(column), value_area.y));
    Some(layout.area)
}

/// A second borrow of a text row, so one composition can be tried with and without the prompt.
fn copy_row<'a>(row: &'a DialogRow<'a>) -> DialogRow<'a> {
    match row {
        DialogRow::Prose(text) => DialogRow::Prose(Cow::Borrowed(text)),
        DialogRow::Label(text, focused) => DialogRow::Label(Cow::Borrowed(text), *focused),
        DialogRow::Error(text) => DialogRow::Error(Cow::Borrowed(text)),
        DialogRow::Status(text, role) => DialogRow::Status(Cow::Borrowed(text), *role),
        DialogRow::Help(text) => DialogRow::Help(text.as_deref().map(Cow::Borrowed)),
        _ => DialogRow::Blank,
    }
}
