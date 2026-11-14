//! The near side of a routed command: the hub of this data root carries the Machine, and one hub
//! `route` channel carries the packet and its reply. The route opens first, so a Machine already
//! `online` serves it without a `connect`, which would restart the hub's countdown; only a hub
//! that answers `not-online` has the Machine brought `online` under the `machine connect` rules.
//! An interrupt resets the channel and leaves the Machine's connection to the hub.

use super::{
    super::{Context, Error, Remote, Report, connect},
    INPUT_SOURCE, Outcome, Refusal, mapped,
};
use orchbee_machine_protocol::{
    MachineId, ResetReason,
    hub::{Channel, Outcome as ChannelOutcome},
};
use orchbee_workbench_client::application::machine_hub::{
    ChannelDelivery, ChannelFailure, ChannelHandle, HubClient,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// How long the Machine may take to come `online`, and the route channel to open.
const CONNECT: Duration = Duration::from_secs(90);
/// How often a wait looks for an interrupt.
const POLL: Duration = Duration::from_millis(50);

/// How one route attempt ended without a reply.
enum Failed {
    /// The hub reset the channel, which states that nothing was dispatched.
    Reset(ResetReason),
    Refused(Refusal),
}

impl Failed {
    fn refusal(self) -> Refusal {
        match self {
            Self::Reset(reason) => refused(reason),
            Self::Refused(refusal) => refusal,
        }
    }
}

/// Sends `packet` to the Machine `machine` and returns the reply packet's bytes. `outcome` turns
/// `unknown` once the packet's `end` is sent and stays so unless the hub states no effect. A login
/// the hub offers to an interface writes `waiting` on standard error and waits for the person.
pub(super) fn route(
    machine: &str,
    waiting: &str,
    packet: &[u8],
    limit: usize,
    context: &mut Context,
    outcome: &mut Outcome,
) -> Result<Vec<u8>, Refusal> {
    let machine = MachineId::new(machine).map_err(|_| mapped(Error::Internal))?;
    context.deadline = Instant::now() + CONNECT;
    let client = connect::open(context).map_err(mapped)?;
    let result = carry(&client, &machine, waiting, packet, limit, context, outcome);
    connect::close(client);
    result
}

fn carry(
    client: &Arc<HubClient>,
    machine: &MachineId,
    waiting: &str,
    packet: &[u8],
    limit: usize,
    context: &mut Context,
    outcome: &mut Outcome,
) -> Result<Vec<u8>, Refusal> {
    // The hub learns a machine saved after it started; taking the catalog restarts nothing.
    connect::ask_reload(client, context).map_err(mapped)?;
    match attempt(client, machine, packet, limit, context, outcome) {
        // Nothing was dispatched: the Machine is not `online`, or the hub closed before the route
        // opened and its successor carries nothing yet.
        Err(Failed::Reset(ResetReason::NotOnline | ResetReason::Stopping)) => {}
        result => return result.map_err(Failed::refusal),
    }
    let mut report = Report {
        catalog: None,
        id: None,
        changed: false,
        committed: false,
        remote: Remote::NotContacted,
        remote_version: None,
        unforgotten: false,
    };
    connect::ask_connect(client, machine, waiting, context, &mut report).map_err(mapped)?;
    attempt(client, machine, packet, limit, context, outcome).map_err(Failed::refusal)
}

/// Opens one route channel and exchanges the packet for its reply, cancelling the channel when
/// the exchange ends otherwise, which ends the far command if it still runs.
fn attempt(
    client: &Arc<HubClient>,
    machine: &MachineId,
    packet: &[u8],
    limit: usize,
    context: &mut Context,
    outcome: &mut Outcome,
) -> Result<Vec<u8>, Failed> {
    let handle = open(
        client,
        &Channel::Route {
            machine: machine.clone(),
        },
        context,
    )
    .map_err(Failed::Refused)?;
    let result = exchange(client, handle, packet, limit, context, outcome);
    if result.is_err() {
        // The hub and the Machine stay.
        client.cancel(handle);
    }
    result
}

fn open(
    client: &HubClient,
    channel: &Channel,
    context: &mut Context,
) -> Result<ChannelHandle, Refusal> {
    loop {
        interrupted(context)?;
        if Instant::now() >= context.deadline {
            return Err(mapped(Error::Timeout));
        }
        match client.open_channel(channel, Instant::now() + POLL) {
            Ok(handle) => return Ok(handle),
            Err(ChannelFailure::Timeout) => {}
            Err(ChannelFailure::Ended(_) | ChannelFailure::Closed) => {
                return Err(mapped(Error::HubUnavailable));
            }
            Err(ChannelFailure::Unsendable) => return Err(mapped(Error::Internal)),
        }
    }
}

/// Writes the packet and its `end`, then reads the reply until the channel's result. The route
/// phase has no deadline of its own: the far side bounds the command, and a lost link or hub
/// ends the channel.
fn exchange(
    client: &HubClient,
    handle: ChannelHandle,
    packet: &[u8],
    limit: usize,
    context: &mut Context,
    outcome: &mut Outcome,
) -> Result<Vec<u8>, Failed> {
    let mut reply = Vec::new();
    let mut written = 0;
    let mut sending = true;
    loop {
        interrupted(context).map_err(Failed::Refused)?;
        if sending {
            match packet.get(written..).filter(|rest| !rest.is_empty()) {
                Some(rest) => match client.write(handle, rest, Instant::now() + POLL) {
                    Ok(committed) => {
                        written += committed;
                        continue;
                    }
                    // The channel is over; its deliveries say how.
                    Err(_) => sending = false,
                },
                None => {
                    sending = false;
                    if client.end(handle).is_ok() {
                        *outcome = Outcome::Unknown;
                    }
                }
            }
        }
        match client.receive(handle, Instant::now() + POLL) {
            None | Some(ChannelDelivery::Accepted | ChannelDelivery::Ended) => {}
            Some(ChannelDelivery::Data(bytes)) => {
                if reply.len() + bytes.len() > limit {
                    return Err(Failed::Refused(mapped(Error::RemoteProtocol)));
                }
                reply.extend_from_slice(&bytes);
            }
            Some(ChannelDelivery::Closed(ChannelOutcome::Replied)) => return Ok(reply),
            // The hub closes `lost` only once the packet's `end` was dispatched: the far command
            // may have run.
            Some(ChannelDelivery::Closed(_)) => {
                *outcome = Outcome::Unknown;
                return Err(Failed::Refused(mapped(Error::SshFailed)));
            }
            Some(ChannelDelivery::Reset(reason)) => {
                // The hub resets a route only while nothing was dispatched.
                *outcome = Outcome::NotSubmitted;
                return Err(Failed::Reset(reason));
            }
            // `outcome` already says whether the packet's `end` was sent.
            Some(ChannelDelivery::Lost) => {
                return Err(Failed::Refused(mapped(Error::HubUnavailable)));
            }
        }
    }
}

fn interrupted(context: &mut Context) -> Result<(), Refusal> {
    if context.interrupted().map_err(|_| mapped(Error::Internal))? {
        return Err(mapped(Error::Cancelled));
    }
    Ok(())
}

/// The refusal a hub `reset` of the route reads as.
fn refused(reason: ResetReason) -> Refusal {
    match reason {
        ResetReason::NotOnline | ResetReason::LinkLost => mapped(Error::SshFailed),
        ResetReason::NotFound => mapped(Error::Catalog(crate::machines::Error::NotFound)),
        ResetReason::TooLarge => INPUT_SOURCE,
        ResetReason::Stopping | ResetReason::Capacity | ResetReason::Unavailable => {
            mapped(Error::HubUnavailable)
        }
        _ => mapped(Error::RemoteProtocol),
    }
}
