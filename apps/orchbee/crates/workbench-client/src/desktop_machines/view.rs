//! The whole view the desktop page shows of the saved machines: Settings › Machines' facts, each
//! with the words the TUI says them in, in the page's locale, beside the values the page acts on.
//
// module-layer: presentation

use crate::{
    localization::Locale,
    machine_capacity::parts,
    machine_settings::{Action, Facts, Machine, State},
};
use orchbee_machine_protocol::capacity::Capacity;
use serde_json::{Value, json};

pub(super) fn project(
    locale: Locale,
    facts: &Facts,
    hub: Option<&'static str>,
    unreadable: bool,
    notice: Option<&(u64, String)>,
) -> Value {
    json!({
        "catalog": if unreadable { "unreadable" } else { "read" },
        "hub": hub.map(|key| locale.machine_picker_text(key)),
        "local": capacity(locale, facts.local.as_ref()),
        "passwords": facts.passwords,
        "added": facts.added,
        "notice": notice.map(|(number, text)| json!({ "number": number, "text": text })),
        "machines": facts
            .machines
            .iter()
            .map(|machine| self::machine(locale, machine))
            .collect::<Vec<_>>(),
    })
}

fn machine(locale: Locale, machine: &Machine) -> Value {
    json!({
        "machine": machine.id,
        "label": machine.label,
        "address": machine.address,
        "color": machine.color.as_str(),
        "state": state(machine.state),
        "stopping": machine.stopping,
        "word": locale.machine_state_word(machine),
        "status": locale.machine_status(machine),
        "lines": locale.machine_status_lines(&machine.explanation, &machine.label),
        "action": machine.action().map(|action| json!({
            "act": self::action(action),
            "label": locale.machine_action(action),
        })),
        "version": machine.version(),
        "capacity": capacity(locale, machine.capacity.as_ref()),
        "workspaces": machine.workspaces,
        "terminals": machine.terminals,
        "agents": machine.agents,
        "stopConfirmation": locale.machine_stop_confirmation(machine),
        "removeDescription": locale.machine_remove_description(machine.workspaces),
    })
}

/// The short form's three facts and the long sentence; `null` when nothing was read.
fn capacity(locale: Locale, capacity: Option<&Capacity>) -> Value {
    capacity.map_or(Value::Null, |capacity| {
        json!({
            "parts": parts(capacity)
                .iter()
                .map(|part| json!({ "text": part.text, "low": part.low }))
                .collect::<Vec<_>>(),
            "long": locale.machine_capacity_long(Some(capacity)),
        })
    })
}

const fn state(state: State) -> &'static str {
    match state {
        State::Online => "online",
        State::Connecting => "connecting",
        State::Reconnecting => "reconnecting",
        State::Offline => "offline",
        State::Disabled => "disabled",
        State::NeedsLogin => "needs-login",
        State::NeedsUpdate => "needs-update",
        State::LocalNeedsUpdate => "local-needs-update",
        State::Other(key) => key,
    }
}

/// The intent's `act` that the machine's button carries out.
const fn action(action: Action) -> &'static str {
    match action {
        Action::Disconnect => "disconnect",
        Action::Connect => "connect",
        Action::Retry => "retry",
        Action::RetryNow => "retry-now",
        Action::LogIn => "log-in",
    }
}
