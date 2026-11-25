//! The whole view the desktop page shows of the saved machines: Settings › Machines' facts, each
//! with the words the TUI says them in, in the page's locale, beside the values the page acts on.
//
// module-layer: presentation

use super::add::{Add, Check};
use crate::{
    action_menu::MenuHeading,
    client_actions::MenuAction,
    localization::{Locale, MachineText, ManageText},
    machine_add::Stage,
    machine_capacity::{parts, summary},
    machine_change::Band,
    machine_discovery::display_label,
    machine_picker::ManageProblem,
    machine_settings::{Action, Facts, Machine, State},
    machine_spaces::Checkpoint,
    machine_status,
    machines::ProfileId,
    workbench_projection::ClientContextLocation,
};
use orchbee_machine_protocol::capacity::Capacity;
use serde_json::{Value, json};
use std::sync::Arc;

/// The members other modules project: what asks the person over whatever holds the window, the
/// login question or the notice of a command login offered to it, and the Workspaces each machine
/// listed when last read.
pub(super) struct Projected {
    pub(super) login: Option<Value>,
    pub(super) offer: Option<Value>,
    pub(super) seen: Value,
}

pub(super) fn project(
    locale: Locale,
    facts: &Facts,
    hub: Option<&'static str>,
    unreadable: bool,
    notice: Option<&(u64, String)>,
    projected: Projected,
    add: Option<&Add>,
) -> Value {
    json!({
        "catalog": if unreadable { "unreadable" } else { "read" },
        "hub": hub.map(|key| locale.machine_picker_text(key)),
        "local": capacity(locale, facts.local.as_ref()),
        "passwords": facts.passwords,
        "added": facts.added,
        "notice": notice.map(|(number, text)| json!({ "number": number, "text": text })),
        "login": projected.login,
        "offer": projected.offer,
        "seen": projected.seen,
        "change": change(locale, facts),
        "add": json!({
            "labels": {
                "addMachine": locale.machine_manage_text(ManageText::AddMachine),
                "title": locale.machine_manage_text(ManageText::AddTitle),
                "address": locale.machine_manage_text(ManageText::Address),
                "hosts": locale.machine_manage_text(ManageText::Hosts),
                "cancel": locale.machine_manage_text(ManageText::Cancel),
                "add": locale.machine_manage_text(ManageText::Add),
            },
            "form": add.map(|add| self::add(locale, add)),
        }),
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
        "color": machine.color.to_string(),
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
        "notConnected": locale.machine_not_connected(&machine.label),
        "removeDescription": locale.machine_remove_description(machine.workspaces),
        "card": json!({
            "title": locale.machine_card_title(machine),
            "lines": locale.machine_card_lines(machine),
            "notNow": locale.machine_card_not_now(),
        }),
    })
}

/// How the add in progress stands: the check running or the one that failed with its lines, the
/// problem that kept it from starting, the label of the primary button, and the machine it saved.
fn add(locale: Locale, add: &Add) -> Value {
    let mut status = None;
    let mut lines = Vec::new();
    let mut submit = locale.machine_manage_text(ManageText::Add);
    let mut blocked = false;
    match &add.check {
        None => {}
        Some(Check::Starting) => status = Some(("pending", locale.machine_checking(Stage::Ssh))),
        Some(Check::Checking(stage)) => status = Some(("pending", locale.machine_checking(*stage))),
        Some(Check::Failed {
            stage,
            explanation,
            retry,
        }) => {
            status = Some(("error", locale.machine_check_failed(*stage)));
            lines = locale.machine_status_lines(explanation, &add.label);
            if *retry {
                submit = locale.machine_status_action(
                    explanation.action.unwrap_or(machine_status::Action::Retry),
                );
            }
            blocked = !retry;
        }
        Some(Check::Saving) => status = Some(("pending", locale.machine_saving().to_owned())),
    }
    let problem = add.problem.map(|problem| {
        json!({
            "field": match problem {
                // The name comes from the address: the form asks for no other.
                ManageProblem::Address | ManageProblem::Name => Some("address"),
                ManageProblem::Save => None,
            },
            "text": locale.machine_manage_problem(problem),
        })
    });
    json!({
        "hosts": add
            .hosts
            .iter()
            .map(|host| json!({ "host": host, "label": display_label(host) }))
            .collect::<Vec<_>>(),
        "running": add.running(),
        "status": status.map(|(tone, text)| json!({ "tone": tone, "text": text })),
        "lines": lines,
        "problem": problem,
        "submit": submit,
        "blocked": blocked,
        "saved": add.saved.as_ref().map(|machine| machine.as_str()),
        "color": add.color.to_string(),
    })
}

/// The short form, its three facts (none when nothing was read) and the long sentence.
fn capacity(locale: Locale, capacity: Option<&Capacity>) -> Value {
    json!({
        "summary": summary(capacity),
        "parts": capacity.map_or_else(Vec::new, |capacity| {
            parts(capacity)
                .iter()
                .map(|part| json!({ "text": part.text, "low": part.low }))
                .collect::<Vec<_>>()
        }),
        "long": locale.machine_capacity_long(capacity),
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

/// The Workspaces each enabled saved machine listed when last read, this run or an earlier one,
/// which a sidebar keeps for a machine that is not connected: Projects and Workspaces in that
/// Session's order, by ID and name.
pub(super) fn seen(checkpoints: &[(ProfileId, Arc<Checkpoint>)]) -> Value {
    Value::Array(
        checkpoints
            .iter()
            .map(|(machine, checkpoint)| {
                json!({
                    "machine": machine.as_str(),
                    "projects": checkpoint.projects.iter().map(|project| json!({
                        "id": project.project_id.as_str(),
                        "name": project.label,
                    })).collect::<Vec<_>>(),
                    "workspaces": checkpoint.rows.iter().filter_map(|row| {
                        let ClientContextLocation::Workspace { workspace_id, .. } = &row.target.location
                        else {
                            return None;
                        };
                        Some(json!({
                            "id": workspace_id.as_str(),
                            "name": row.label.as_deref()?,
                            "projectId": row.project_id.as_ref().map(|id| id.as_str()),
                        }))
                    }).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

/// Change machine's words, and its list as the TUI orders it when it opens: This computer (null),
/// then every saved machine by band and name.
fn change(locale: Locale, facts: &Facts) -> Value {
    let rows = crate::machine_change::rows(&crate::machine_change::Facts {
        here: None,
        local: facts.local.clone(),
        machines: facts.machines.clone(),
        moving: false,
    });
    json!({
        "labels": {
            "title": MenuAction::ChangeMachine.title(locale),
            "heading": locale.menu_heading(MenuHeading::RunsOn),
            "note": locale.machine_text(MachineText::ColumnNote),
            "here": locale.machine_change_here(),
            "find": locale.machine_change_find(),
            "noMatch": locale.machine_change_no_match(),
            "manage": MenuAction::ManageMachines.title(locale),
        },
        "rows": rows.iter().map(|row| json!({
            "machine": row.id(),
            "band": match row.band {
                Band::Room => "room",
                Band::Low => "low",
                Band::Login => "login",
                Band::Unusable => "unusable",
            },
        })).collect::<Vec<_>>(),
    })
}
