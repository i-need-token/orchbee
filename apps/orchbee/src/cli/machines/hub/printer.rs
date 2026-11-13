//! `orchbee --machine-hub-state-v1`: attaches with `watch` alone, so it holds nothing, starting the
//! hub when none runs, and prints the hub's state, its process id, the catalog revision, and each
//! Machine's state as localized text.

use super::lifetime::Moment;
use crate::{cli::Output, cli_text::CliText, localization::Locale};
use orchbee_machine_protocol::{
    MachineId,
    hub::{Catalog, Event, HubState, MachineState},
};
use orchbee_workbench_client::application::machine_hub::{
    AttachFailure, Attachment, HubClient, Incompatible, Notice, data_root, running,
};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

/// The bound on reaching the hub, a start included.
const ATTACH: Duration = Duration::from_secs(10);
/// The bound on the attachment's events, and on the `query`.
const READ: Duration = Duration::from_secs(5);

/// Why the printer printed no state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StateFailure {
    DataRoot,
    Unavailable,
    Denied,
    Closing,
    Lost,
    Incompatible(Incompatible),
    Timeout,
}

impl StateFailure {
    pub(super) const fn status(&self) -> u8 {
        match self {
            Self::DataRoot => 1,
            Self::Denied => 3,
            Self::Unavailable | Self::Closing => 4,
            Self::Lost => 5,
            Self::Incompatible(_) => 6,
            Self::Timeout => 7,
        }
    }
}

impl From<AttachFailure> for StateFailure {
    fn from(failure: AttachFailure) -> Self {
        match failure {
            AttachFailure::DataRoot => Self::DataRoot,
            AttachFailure::Unavailable => Self::Unavailable,
            AttachFailure::Denied => Self::Denied,
            AttachFailure::Closing => Self::Closing,
            AttachFailure::Incompatible(incompatible) => Self::Incompatible(incompatible),
            AttachFailure::Lost => Self::Lost,
            AttachFailure::Timeout => Self::Timeout,
        }
    }
}

/// What the attachment reported.
struct Report {
    hub: HubState,
    catalog: Catalog,
    machines: BTreeMap<String, MachineState>,
}

pub(super) fn run(locale: Locale) -> Output {
    let text = CliText::new(locale);
    match collect() {
        Ok((report, pid)) => Output {
            status: 0,
            stdout: render(text, &report, pid).into_bytes(),
            stderr: String::new(),
            finalization: None,
        },
        Err(failure) => Output {
            status: failure.status(),
            stdout: Vec::new(),
            stderr: format!("{}\n", text.cli_machine_hub_failure(&failure)),
            finalization: None,
        },
    }
}

fn collect() -> Result<(Report, u32), StateFailure> {
    let attachment = Attachment {
        presence: false,
        asks: false,
        watch: true,
    };
    let (client, attached) = HubClient::open(attachment, Instant::now() + ATTACH)?;
    let report = watch(&client, attached.hub, Instant::now() + READ);
    client.close();
    let report = report?;
    let pid = running(&data_root()?, Instant::now() + READ)
        .ok()
        .flatten()
        .ok_or(StateFailure::Unavailable)?;
    Ok((report, pid))
}

/// Takes notices until the catalog and one `machine` per id arrived; a re-attachment starts over
/// with the new hub.
fn watch(
    client: &HubClient,
    attached: HubState,
    deadline: Instant,
) -> Result<Report, StateFailure> {
    let mut hub = attached;
    let mut catalog: Option<Catalog> = None;
    let mut machines: BTreeMap<String, MachineState> = BTreeMap::new();
    loop {
        if let Some(catalog) = catalog.as_ref()
            && catalog
                .machines()
                .iter()
                .all(|id: &MachineId| machines.contains_key(id.as_str()))
        {
            return Ok(Report {
                hub,
                catalog: catalog.clone(),
                machines,
            });
        }
        match client.next(deadline) {
            Some(Notice::Event(Event::Hub(state))) => hub = state,
            Some(Notice::Event(Event::Catalog(next))) => catalog = Some(next),
            Some(Notice::Event(Event::Machine(machine))) => {
                machines.insert(machine.machine.as_str().to_owned(), machine.state);
            }
            Some(Notice::Event(_)) => {}
            Some(Notice::Reattached(attached)) => {
                hub = attached.hub;
                catalog = None;
                machines.clear();
            }
            Some(Notice::Ended(failure)) => return Err(failure.into()),
            None => return Err(StateFailure::Timeout),
        }
    }
}

fn render(text: CliText, report: &Report, pid: u32) -> String {
    let state = match report.hub {
        HubState::Held => text.cli_machine_hub_held().to_owned(),
        HubState::Expiring { expires_at } => {
            match expires_at.get().saturating_sub(Moment::now().wall) {
                // Past `expiresAt` an open route keeps the hub until it ends.
                0 => text.cli_machine_hub_overdue().to_owned(),
                remaining => text.cli_machine_hub_expiring(remaining.div_ceil(1_000)),
            }
        }
        HubState::Closing => text.cli_machine_hub_closing().to_owned(),
    };
    let mut out = text.cli_machine_hub_summary(&state, pid, report.catalog.revision().get());
    if report.catalog.machines().is_empty() {
        out.push_str(text.cli_machine_message("empty"));
        out.push('\n');
    }
    for id in report.catalog.machines() {
        let reported = report.machines.get(id.as_str());
        let state = reported.map_or_else(
            || text.cli_machine_hub_machine_state(None),
            |state| text.cli_machine_hub_machine_state(Some(state)),
        );
        out.push_str(&format!("{id}  {state}"));
        // The key a person compares before trusting a host, or before checking a changed one.
        if let Some(
            MachineState::Untrusted { host_key }
            | MachineState::KeyChanged {
                host_key: Some(host_key),
            },
        ) = reported
        {
            out.push_str(&format!(
                "  {} {}",
                host_key.algorithm.as_str(),
                host_key.fingerprint.as_str()
            ));
        }
        out.push('\n');
    }
    out
}
