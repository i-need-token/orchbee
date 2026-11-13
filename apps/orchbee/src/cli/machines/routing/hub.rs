//! The near side of a routed command: the hub of this data root carries the Machine, `machine
//! connect` rules bring it `online`, and one hub `route` channel carries the packet and its reply.
//! An interrupt resets the channel and leaves the Machine's connection to the hub.

use super::{
    super::{Context, Error, Remote, Report, connect},
    Outcome, mapped,
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

type Refusal = (&'static str, u8);

/// Sends `packet` to the Machine `machine` and returns the reply packet's bytes. `outcome` turns
/// `unknown` once the packet's `end` is sent and stays so unless the hub states no effect.
pub(super) fn route(
    machine: &str,
    packet: &[u8],
    limit: usize,
    context: &mut Context,
    outcome: &mut Outcome,
) -> Result<Vec<u8>, Refusal> {
    let machine = MachineId::new(machine).map_err(|_| ("internal", 1))?;
    context.deadline = Instant::now() + CONNECT;
    let client = connect::open(context).map_err(mapped)?;
    let result = carry(&client, &machine, packet, limit, context, outcome);
    connect::close(client);
    result
}

fn carry(
    client: &Arc<HubClient>,
    machine: &MachineId,
    packet: &[u8],
    limit: usize,
    context: &mut Context,
    outcome: &mut Outcome,
) -> Result<Vec<u8>, Refusal> {
    let mut report = Report {
        catalog: None,
        id: None,
        changed: false,
        committed: false,
        remote: Remote::NotContacted,
        remote_version: None,
    };
    connect::ask_connect(client, machine, context, &mut report).map_err(mapped)?;
    let handle = open(
        client,
        &Channel::Route {
            machine: machine.clone(),
        },
        context,
    )?;
    let result = exchange(client, handle, packet, limit, context, outcome);
    if result.is_err() {
        // Ends the far command when it still runs; the hub and the Machine stay.
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
            Err(ChannelFailure::Unsendable) => return Err(("internal", 1)),
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
) -> Result<Vec<u8>, Refusal> {
    let mut reply = Vec::new();
    let mut written = 0;
    let mut sending = true;
    loop {
        interrupted(context)?;
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
                    return Err(("remote-protocol", 5));
                }
                reply.extend_from_slice(&bytes);
            }
            Some(ChannelDelivery::Closed(ChannelOutcome::Replied)) => return Ok(reply),
            // The link or the hub was lost after the packet may have reached the far side.
            Some(ChannelDelivery::Closed(_)) => return Err(("ssh-failed", 5)),
            Some(ChannelDelivery::Reset(reason)) => {
                // A reset states that the command did not run.
                *outcome = Outcome::NotSubmitted;
                return Err(refused(reason));
            }
            // `outcome` already says whether the packet's `end` was sent.
            Some(ChannelDelivery::Lost) => return Err(mapped(Error::HubUnavailable)),
        }
    }
}

fn interrupted(context: &mut Context) -> Result<(), Refusal> {
    if context.interrupted().map_err(|_| ("internal", 1))? {
        return Err(mapped(Error::Cancelled));
    }
    Ok(())
}

/// The refusal a hub `reset` of the route reads as.
fn refused(reason: ResetReason) -> Refusal {
    match reason {
        ResetReason::NotOnline | ResetReason::LinkLost => mapped(Error::SshFailed),
        ResetReason::NotFound => ("not-found", 3),
        ResetReason::TooLarge => ("input-source", 3),
        ResetReason::Stopping | ResetReason::Capacity | ResetReason::Unavailable => {
            mapped(Error::HubUnavailable)
        }
        _ => mapped(Error::RemoteProtocol),
    }
}
