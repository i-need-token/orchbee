//! A Workspace's Change machine list in the action menu: the rows it opens with, which of them can
//! be chosen, and the move a choice hands to the machine manager, which runs it. The rows keep the
//! order they opened with; newer facts change only what they say.
//! `apps/orchbee/docs/machines.md#change-machine` owns the rules.

use super::episode::{Intent, MachineChoices, Target};
use super::{
    ClientContextLocation, MenuAction, MenuUnavailable, WorkbenchClientState, WorkbenchClientUi,
    intent_availability, visible_unavailable,
};
use crate::machine_change::{Band, Facts, Request, Row};

/// The rows' actions in list order, then Manage machines….
pub(super) fn actions(choices: &MachineChoices) -> Vec<MenuAction> {
    (0..choices.rows.len())
        .map_while(|index| u16::try_from(index).ok())
        .map(MenuAction::MoveToMachine)
        .chain([MenuAction::ManageMachines])
        .collect()
}

/// The name a row carries to the renderer: a saved machine's name, empty for This computer, which
/// the renderer names in the interface language.
pub(super) fn row_label(row: &Row) -> String {
    row.machine
        .as_ref()
        .map_or_else(String::new, |machine| machine.label.clone())
}

/// The color of the machine this Client's Session runs on; none for this computer.
pub(super) fn here_color(facts: Option<&Facts>) -> Option<crate::machine_color::MachineColor> {
    let facts = facts?;
    let here = facts.here.as_deref()?;
    facts
        .machines
        .iter()
        .find(|machine| machine.id == here)
        .map(|machine| machine.color)
}

/// The name of the machine this Client's Session runs on; empty for this computer.
pub(super) fn here_label(facts: Option<&Facts>) -> String {
    facts
        .and_then(|facts| {
            let here = facts.here.as_deref()?;
            facts
                .machines
                .iter()
                .find(|machine| machine.id == here)
                .map(|machine| machine.label.clone())
        })
        .unwrap_or_default()
}

/// Why a row cannot be chosen now. The row of the machine the Workspace runs on stays choosable,
/// since choosing it changes nothing.
pub(super) fn unavailable(intent: &Intent, action: &MenuAction) -> Option<MenuUnavailable> {
    let MenuAction::MoveToMachine(index) = action else {
        return None;
    };
    let choices = intent.machines()?;
    let row = choices.rows.get(usize::from(*index))?;
    if row.current {
        None
    } else if choices.moving {
        Some(MenuUnavailable::OperationBusy)
    } else if row.band == Band::Unusable {
        // The row's state word says why.
        Some(MenuUnavailable::MachineUnavailable)
    } else {
        None
    }
}

/// The move a chosen row asks for; none for the machine the Workspace already runs on.
pub(super) fn request(
    intent: &Intent,
    index: u16,
    state: Option<&WorkbenchClientState>,
) -> Option<Request> {
    let row = intent.machines()?.rows.get(usize::from(index))?;
    if row.current {
        return None;
    }
    let Target::Workspace {
        location: location @ ClientContextLocation::Workspace { workspace_id, .. },
        ..
    } = intent.target()
    else {
        return None;
    };
    let label = state
        .and_then(|state| {
            state
                .cache()
                .navigation()
                .iter()
                .find(|context| &context.location == location)
        })
        .map(super::workspace_title)
        .unwrap_or_default();
    Some(Request {
        workspace: workspace_id.as_str().to_owned(),
        label,
        to: row.id().map(str::to_owned),
        to_label: row_label(row),
    })
}

impl WorkbenchClientUi {
    /// The rows a Change machine list opens with, from the newest facts.
    pub(super) fn machine_choices(&self) -> MachineChoices {
        let facts = self.machine_facts.clone().unwrap_or_default();
        MachineChoices {
            rows: crate::machine_change::rows(&facts),
            moving: facts.moving,
        }
    }

    /// Takes the machine manager's newest facts; an open list shows what changed. True when the
    /// list needs a new draw.
    pub(in crate::workbench_client) fn observe_machine_change(&mut self, facts: Facts) -> bool {
        if self.machine_facts.as_ref() == Some(&facts) {
            return false;
        }
        let changed = self.menus.refresh_machines(&facts);
        self.machine_facts = Some(facts);
        if changed && let Some(intent) = self.menus.intent() {
            let facts = self.row_facts(intent.target());
            let state = self.state.as_ref();
            let (menu, intent, discovery) = self.menus.menu_with_facts();
            if let Some(intent) = intent {
                menu.refresh_availability(|action| {
                    visible_unavailable(intent_availability(
                        intent, state, discovery, action, facts,
                    ))
                });
            }
        }
        changed
    }

    /// The move the person chose in the list, once.
    pub(in crate::workbench_client) fn take_machine_change_request(&mut self) -> Option<Request> {
        self.machine_request.take()
    }

    /// Whether an open menu offers Change machine or is its list, so the machine manager keeps
    /// what the machines have free current.
    pub(in crate::workbench_client) fn machine_change_wanted(&self) -> bool {
        self.menus.view().is_some_and(|menu| {
            menu.actions.iter().any(|action| {
                matches!(
                    action,
                    MenuAction::ChangeMachine | MenuAction::MoveToMachine(_)
                )
            })
        })
    }

    /// Whether the machine list itself is open, the only place that shows free capacity; a
    /// menu that only offers Change machine reads no capacity, so it never starts the hub.
    pub(in crate::workbench_client) fn machine_list_open(&self) -> bool {
        self.menus.view().is_some_and(|menu| {
            menu.actions
                .iter()
                .any(|action| matches!(action, MenuAction::MoveToMachine(_)))
        })
    }
}
