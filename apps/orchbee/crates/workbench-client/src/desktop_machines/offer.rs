//! The notice of a command login the hub offers the window, worded as the TUI's notice words it:
//! the machine, how many commands wait and what declining does to them, the login starting once
//! accepted, or why an accepted login ended before its question.
//
// module-layer: presentation

use crate::{
    localization::{Locale, LoginLabel, WaitingLabel},
    machine_login::{Ending, Offered, Withdrawn},
};
use orchbee_machine_protocol::hub::LoginEnd;
use serde_json::{Value, json};

/// The notice of `offered`, for the machine named `label`.
pub(super) fn project(locale: Locale, offered: &Offered, label: &str) -> Value {
    let ended = offered.ended.map(|why| {
        let tone = match why {
            Withdrawn::Ended(Ending::Hub(LoginEnd::Succeeded)) => "success",
            Withdrawn::Taken => "warning",
            Withdrawn::Ended(_) => "error",
        };
        json!({ "text": locale.machine_waiting_ended(why), "tone": tone })
    });
    // A claimed login stays after its last command left, without the count.
    let waiting = offered.waiters > 0;
    json!({
        "login": offered.login.get(),
        "machine": format!("{label} · {}", offered.target),
        "count": waiting.then(|| locale.machine_waiting_count(offered.waiters)),
        "consequence": waiting
            .then(|| locale.machine_waiting_consequence(offered.waiters != 1)),
        "accepted": offered.accepted,
        "ended": ended,
        "labels": {
            "title": locale.machine_login_label(LoginLabel::Title),
            "decline": locale.machine_waiting_label(WaitingLabel::Decline),
            "logIn": locale.machine_waiting_label(WaitingLabel::LogIn),
            "starting": locale.machine_waiting_label(WaitingLabel::Starting),
            "close": locale.machine_login_label(LoginLabel::Close),
        },
    })
}
