//! `machine list`: this computer first, then each saved machine, with its state and what it has
//! free. States and the other machines' capacity come from the machine hub serving the data root
//! now, through one attachment that watches and holds nothing; the command starts, replaces and
//! waits for no hub, and connects nothing. Without a hub nothing carries a saved machine, which
//! reads as its record says, and this computer's memory and disk are read here: its processor share
//! needs readings seconds apart, which only a running hub keeps. A hub that cannot be read leaves
//! every state and other machine's capacity unknown.

use super::{Context, Error, Report, capacity, connect};
use crate::{
    cli_text::{CliText, escape::escape},
    localization::Locale,
    machines::{Catalog, LOCAL, Profile},
};
use orchbee_machine_protocol::{
    MachineId, Time,
    capacity::{Bytes, Capacity},
    hub::{CapacityOutcome, Catalog as HubCatalog, Event, MachineState, Reply, Request},
};
use orchbee_workbench_client::application::{
    machine_capacity,
    machine_hub::{HubClient, Notice},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

/// How long the hub's catalog and Machine states may take to arrive.
const STATES: Duration = Duration::from_millis(1_500);
/// How long a `capacity` answer may take; the hub waits up to 2 seconds for a Machine's.
const CAPACITY: Duration = Duration::from_millis(2_500);

/// One listed machine.
pub(super) struct Listed {
    /// The saved machine; `None` for this computer.
    profile: Option<Profile>,
    /// The machine's state; `None` when it could not be read. This computer has none.
    state: Option<MachineState>,
    /// What the machine has free and when that was read; `None` when it could not be read.
    capacity: Option<(Capacity, Time)>,
}

/// What the hub serving the data root reported.
pub(super) enum Hub {
    /// No hub serves the data root.
    Absent,
    /// A hub serves it and could not be read.
    Unreadable,
    Read {
        states: BTreeMap<String, MachineState>,
        /// By Machine id; `None` is this computer.
        capacities: BTreeMap<Option<String>, (Capacity, Time)>,
    },
}

/// Reads every Machine's state and what this computer and each `online` Machine have free from the
/// hub serving the data root now, within the command's deadline. Only an interrupt is an error.
pub(super) fn read(context: &mut Context) -> Result<Hub, Error> {
    match connect::watch_present(context) {
        Ok(Some(client)) => {
            let observed = connect::wait(context, {
                let client = Arc::clone(&client);
                move |deadline| observe(&client, deadline)
            });
            connect::close(client);
            match observed {
                Ok(hub) => Ok(hub),
                Err(Error::Cancelled) => Err(Error::Cancelled),
                Err(_) => Ok(Hub::Unreadable),
            }
        }
        Ok(None) => Ok(Hub::Absent),
        Err(Error::Cancelled) => Err(Error::Cancelled),
        Err(_) => Ok(Hub::Unreadable),
    }
}

/// Lists this computer and every machine of `store`'s catalog into `report`.
pub(super) fn list(
    catalog: Catalog,
    context: &mut Context,
    report: &mut Report,
) -> Result<(), Error> {
    let hub = read(context)?;
    let here = match &hub {
        Hub::Read { capacities, .. } => capacities.get(&None).cloned(),
        Hub::Absent | Hub::Unreadable => None,
    }
    .or_else(|| capacity::once().map(|sample| (sample.capacity, sample.sampled_at)));
    let mut machines = vec![Listed {
        profile: None,
        state: None,
        capacity: here,
    }];
    for profile in &catalog.profiles {
        let (state, capacity) = match &hub {
            Hub::Absent => (Some(recorded(profile)), None),
            Hub::Unreadable => (None, None),
            Hub::Read { states, capacities } => (
                states.get(profile.id.as_str()).cloned(),
                capacities
                    .get(&Some(profile.id.as_str().to_owned()))
                    .cloned(),
            ),
        };
        machines.push(Listed {
            profile: Some(profile.clone()),
            state,
            capacity,
        });
    }
    report.catalog = Some(catalog);
    report.machines = Some(machines);
    context.check()
}

/// The state of a saved machine that nothing carries.
fn recorded(profile: &Profile) -> MachineState {
    if !profile.draft.connection.supported() {
        MachineState::Unsupported { cause: None }
    } else if !profile.enabled {
        MachineState::Disabled
    } else {
        MachineState::Offline {
            cause: None,
            next_attempt: None,
        }
    }
}

/// Reads the catalog and every Machine's state, then asks for this computer's capacity and every
/// `online` Machine's at once, each within its bound and the command's `deadline`.
fn observe(client: &Arc<HubClient>, deadline: Instant) -> Hub {
    let states = states(client, deadline.min(Instant::now() + STATES));
    let mut asked = vec![None];
    asked.extend(
        states
            .iter()
            .filter(|(_, state)| matches!(state, MachineState::Online))
            .filter_map(|(id, _)| MachineId::new(id.as_str()).ok())
            .map(Some),
    );
    let until = deadline.min(Instant::now() + CAPACITY);
    let answers: Vec<_> = asked
        .into_iter()
        .map(|machine| {
            let key = machine.as_ref().map(|id| id.as_str().to_owned());
            let client = Arc::clone(client);
            let answer = thread::Builder::new()
                .name("machine-capacity".to_owned())
                .spawn(move || client.request(Request::Capacity { machine }, until));
            (key, answer)
        })
        .collect();
    let mut capacities = BTreeMap::new();
    for (key, answer) in answers {
        let Ok(answer) = answer else { continue };
        if let Ok(Ok(Reply::Capacity(CapacityOutcome::Sampled {
            capacity,
            sampled_at,
        }))) = answer.join()
        {
            capacities.insert(key, (capacity, sampled_at));
        }
    }
    Hub::Read { states, capacities }
}

/// Takes notices until the catalog and one state per Machine arrived, or `deadline`; a
/// re-attachment starts over with the new hub.
fn states(client: &HubClient, deadline: Instant) -> BTreeMap<String, MachineState> {
    let mut catalog: Option<HubCatalog> = None;
    let mut states = BTreeMap::new();
    loop {
        if let Some(catalog) = &catalog
            && catalog
                .machines()
                .iter()
                .all(|id| states.contains_key(id.as_str()))
        {
            return states;
        }
        match client.next(deadline) {
            Some(Notice::Event(Event::Catalog(next))) => catalog = Some(next),
            Some(Notice::Event(Event::Machine(machine))) => {
                states.insert(machine.machine.as_str().to_owned(), machine.state);
            }
            Some(Notice::Event(_)) => {}
            Some(Notice::Reattached(_)) => {
                catalog = None;
                states.clear();
            }
            Some(Notice::Ended(_)) | None => return states,
        }
    }
}

impl Listed {
    /// The machine's entry in `machines`: `null` where the source has none, `"unknown"` where it
    /// cannot say.
    pub(super) fn json(&self) -> Value {
        let state = match (&self.profile, &self.state) {
            (None, _) => Value::Null,
            (Some(_), Some(state)) => json!(state.name().as_str()),
            (Some(_), None) => json!("unknown"),
        };
        json!({
            "local": self.profile.is_none(),
            "id": self.profile.as_ref().map(|profile| profile.id.as_str()),
            "label": self.profile.as_ref().map(|profile| profile.draft.label.as_str()),
            "color": self.profile.as_ref().map(|profile| profile.color.as_str()),
            "state": state,
            "capacity": self.capacity.as_ref().map_or_else(|| json!("unknown"), capacity_json),
        })
    }
}

fn capacity_json((capacity, sampled_at): &(Capacity, Time)) -> Value {
    let bytes = |bytes: Bytes| json!({"totalBytes": bytes.total(), "freeBytes": bytes.free()});
    json!({
        "cpu": capacity.cpu.map_or_else(|| json!("unknown"), |cpu| json!({
            "logical": cpu.logical(),
            "freePercent": cpu.free_percent(),
        })),
        "memory": capacity.memory.map_or_else(|| json!("unknown"), bytes),
        "disk": capacity.disk.as_ref().map_or_else(|| json!("unknown"), |disk| json!({
            "path": disk.path.as_str(),
            "totalBytes": disk.bytes.total(),
            "freeBytes": disk.bytes.free(),
        })),
        "sampledAt": sampled_at.get(),
    })
}

/// The text table: this computer as `local`, then each saved machine.
pub(super) fn text(machines: &[Listed], locale: Locale) -> String {
    let words = CliText::new(locale);
    let rows: Vec<Vec<String>> = machines
        .iter()
        .map(|listed| {
            let free =
                machine_capacity::summary(listed.capacity.as_ref().map(|(capacity, _)| capacity));
            match &listed.profile {
                None => vec![
                    LOCAL.to_owned(),
                    words.cli_machine_this_computer().to_owned(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    free,
                ],
                Some(profile) => vec![
                    profile.id.as_str().to_owned(),
                    escape(&profile.draft.label),
                    profile.color.as_str().to_owned(),
                    escape(
                        profile
                            .draft
                            .ssh_target()
                            .unwrap_or_else(|| words.cli_machine_message("unsupported")),
                    ),
                    profile.draft.session.as_str().to_owned(),
                    words
                        .cli_machine_hub_machine_state(listed.state.as_ref())
                        .to_owned(),
                    free,
                ],
            }
        })
        .collect();
    super::super::present::columns(&words.cli_machine_list_headers(), &rows)
}
