//! The near side of a remote stop: the hub of this data root takes the current catalog, then is
//! asked for `stop-session`. A Machine the hub does not carry `online`, or a hub that is closing,
//! is brought `online` under the `machine connect` rules and asked once more, as a routed command
//! is. Once a request may have been sent, only the hub's answer settles the outcome.

use super::{
    super::{
        super::{Remote, Report, connect},
        Refusal,
    },
    Context, Error, Stopped, mapped,
};
use orchbee_machine_protocol::{
    MachineId,
    hub::{Reply, Request, StopSessionOutcome},
};
use orchbee_workbench_client::application::machine_hub::{HubClient, RequestFailure};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// How long the hub may take to start, take the catalog and bring the Machine `online`.
const CONNECT: Duration = Duration::from_secs(90);
/// How long the hub may take to answer `stop-session`: its run watches the Session for 65
/// seconds, and the exchange in progress then takes at most 30 more.
const ANSWER: Duration = Duration::from_secs(120);

/// Asks the hub to stop the remote Session of the Machine `machine`.
pub(super) fn stop(machine: &str, context: &mut Context) -> Result<Stopped, Refusal> {
    let machine = MachineId::new(machine).map_err(|_| mapped(Error::Internal))?;
    context.deadline = Instant::now() + CONNECT;
    let client = connect::open(context).map_err(mapped)?;
    let result = carry(&client, &machine, context);
    connect::close(client);
    result
}

fn carry(
    client: &Arc<HubClient>,
    machine: &MachineId,
    context: &mut Context,
) -> Result<Stopped, Refusal> {
    // The hub learns a machine saved after it started; taking the catalog restarts nothing.
    connect::ask_reload(client, context).map_err(mapped)?;
    match ask(client, machine, context)? {
        // Nothing was asked of the Session: the Machine is not `online`, or the hub was closing.
        Stopped {
            outcome: StopSessionOutcome::NotOnline | StopSessionOutcome::Stopping,
            interrupted: false,
        } => {}
        stopped => return Ok(stopped),
    }
    context.deadline = Instant::now() + CONNECT;
    let mut report = Report {
        catalog: None,
        id: None,
        changed: false,
        committed: false,
        remote: Remote::NotContacted,
        remote_version: None,
    };
    connect::ask_connect(client, machine, context, &mut report).map_err(mapped)?;
    ask(client, machine, context)
}

/// Sends one `stop-session` and waits for the hub's answer. An interrupt before the request is
/// sent ends the command with nothing asked; after that every end but an answer is `unknown`.
fn ask(
    client: &Arc<HubClient>,
    machine: &MachineId,
    context: &mut Context,
) -> Result<Stopped, Refusal> {
    context.deadline = Instant::now() + ANSWER;
    context.check().map_err(mapped)?;
    let unknown = |interrupted| Stopped {
        outcome: StopSessionOutcome::Unknown,
        interrupted,
    };
    let request = Request::StopSession {
        machine: machine.clone(),
    };
    match connect::ask(client, request, context) {
        Ok(Ok(Reply::StopSession(outcome))) => Ok(Stopped {
            outcome,
            interrupted: false,
        }),
        // The hub never received the request.
        Ok(Err(RequestFailure::Unsendable | RequestFailure::Unsupported)) => {
            Err(mapped(Error::Internal))
        }
        Err(Error::Cancelled) => Ok(unknown(true)),
        // A lost connection, an ended owner, an elapsed wait, another reply or a wait that could
        // not finish leaves the request possibly acted on.
        Ok(_) | Err(_) => Ok(unknown(false)),
    }
}
