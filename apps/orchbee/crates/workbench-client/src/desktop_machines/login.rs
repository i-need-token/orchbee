//! The login question as the desktop page shows it, worded as the TUI's overlay words it: the
//! machine, `ssh`'s own lines, Orchbee's asking sentence, the remember box, and the line beside the
//! field; or how a login that ended without the person ended. Nothing typed is ever in it.
//
// module-layer: presentation

use crate::{
    localization::{Locale, LoginLabel, MachineQuestion, RememberText},
    machine_login::{AskNotice, Ending, Presented, RememberBox},
    machine_question::{CUT, shown_lines},
};
use orchbee_machine_protocol::hub::{LoginEnd, QuestionKind};
use serde_json::{Value, json};

/// The question `presented` of the machine named `label`.
pub(super) fn project(locale: Locale, presented: &Presented, label: &str) -> Value {
    let (login, question) = presented.token.ids();
    let host_key = matches!(presented.kind, QuestionKind::HostKey { .. });
    let asked = MachineQuestion::of(&presented.kind);
    let said = locale.machine_question_said();
    let remember = match presented.remember {
        RememberBox::Offered {
            ticked,
            until_restart,
        } => json!({
            "label": locale.machine_remember_text(if until_restart {
                RememberText::RememberUntilRestart
            } else {
                RememberText::Remember
            }),
            "ticked": ticked,
        }),
        RememberBox::Absent | RememberBox::Unavailable | RememberBox::NeedsNewerSsh => Value::Null,
    };
    let remember_line = match presented.remember {
        RememberBox::Unavailable => Some(locale.machine_remember_text(RememberText::Unavailable)),
        RememberBox::NeedsNewerSsh => {
            Some(locale.machine_remember_text(RememberText::NeedsNewerSsh))
        }
        RememberBox::Absent | RememberBox::Offered { .. } => None,
    };
    let notice = presented.notice.map(|notice| match notice {
        AskNotice::Repeated => locale.machine_login_repeated(&asked),
        AskNotice::Inadmissible => locale.machine_answer_undelivered(true),
        AskNotice::RememberedRejected => locale.machine_remember_text(RememberText::Rejected),
    });
    let ended = presented.outcome.map(|ending| {
        json!({
            "text": locale.machine_login_outcome(ending),
            "succeeded": ending == Ending::Hub(LoginEnd::Succeeded),
        })
    });
    json!({
        "login": login,
        "question": question,
        "machine": format!("{label} · {}", presented.target),
        "kind": match presented.kind {
            QuestionKind::Password => "password",
            QuestionKind::KeyPassphrase => "passphrase",
            QuestionKind::HostKey { .. } => "host-key",
            _ => "other",
        },
        "said": shown_lines(presented.prompt.as_str())
            .into_iter()
            .map(|line| if line == CUT { format!("{said}{CUT}") } else { format!("{said}{line}") })
            .collect::<Vec<_>>(),
        "asked": asked_lines(locale, &presented.target, &asked),
        "remember": remember,
        "rememberLine": remember_line,
        "notice": notice,
        "sending": presented.sending,
        "ended": ended,
        "labels": {
            "title": locale.machine_login_label(LoginLabel::Title),
            "cancel": locale.machine_login_label(LoginLabel::Cancel),
            "submit": locale.machine_login_label(if host_key { LoginLabel::Trust } else { LoginLabel::LogIn }),
            "close": locale.machine_login_label(LoginLabel::Close),
            "sending": locale.machine_login_label(LoginLabel::Sending),
            "typeYes": locale.machine_login_label(LoginLabel::TypeYes),
            "privacy": (!host_key).then(|| locale.machine_login_label(LoginLabel::Privacy)),
            "rememberPrivacy": locale.machine_remember_text(RememberText::Privacy),
        },
    })
}

/// Orchbee's asking sentence: a new host's lead, its whole fingerprint and what to type, or the
/// one line naming what is asked.
fn asked_lines(locale: Locale, target: &str, asked: &MachineQuestion<'_>) -> Vec<String> {
    match asked {
        MachineQuestion::HostKey {
            algorithm,
            fingerprint,
        } => vec![
            locale.machine_question_new_host_lead(target, algorithm),
            (*fingerprint).to_owned(),
            locale.machine_question_type_yes().trim_end().to_owned(),
        ],
        _ => vec![
            locale
                .machine_question_asked(target, asked, false)
                .trim_end()
                .to_owned(),
        ],
    }
}
