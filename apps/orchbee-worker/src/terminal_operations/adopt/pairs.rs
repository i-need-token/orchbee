//! The two copies of one terminal that a move left half done in two adopted Hosts
//! (`proposals/platform/host-handover.md`, A terminal in two Hosts). A move keeps the creation id,
//! so the copies pair by it: the copy that runs owns the terminal, or, when none runs, the quiesced
//! one it was leaving. A copy a move left behind is dropped, an import never claimed is discarded,
//! and a quiesced owner resumes. Beside a running owner, a quiesced copy whose Keeper answers is
//! dropped only once its Host saw `claimed`, so a drop it refuses as moving resumes it through that
//! Keeper first, which answers `claimed-elsewhere`. A binding that names another copy than the owner is rebound to it,
//! which the Server does as for a move turn. A copy no binding names stays as any other the import
//! quarantines.

use std::{
    collections::BTreeMap,
    error::Error,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    AdoptBinding, AdoptHostFailure, AdoptMovedBinding, HostTerminalIdentity,
    MAXIMUM_ADOPT_HOST_MOVED,
};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_terminal_protocol::{
    DiscardOutcome, DiscardTarget, HandoverControl, HandoverState, ResumeOutcome, TerminalId,
};

use super::Found;
use crate::{
    host_adoption as host,
    host_connection::{HostBrokerConnection, HostFailure},
    session_keeper::{HostRefusal, Serving, request},
    terminal_authorization::InventoryTerminal,
};

/// Each drop, discard, or resume, within the adoption's own deadline.
const STEP_BUDGET: Duration = Duration::from_secs(3);

/// A copy in one of the adopted Hosts: the index of its Host in the adoption, and the copy.
type HostCopy = (usize, InventoryTerminal);

/// What the import takes after the pairing: the bindings, each naming its owner, and those that
/// named another copy, with the identity they name now.
pub(super) struct Paired {
    pub(super) bindings: Vec<AdoptBinding>,
    pub(super) moved: Vec<AdoptMovedBinding>,
}

/// Pairs the copies the inventories of `found` hold of each terminal a binding names, ends every
/// copy but the owner, and takes the ended copies out of those inventories. A lost Host exchange
/// fails the adoption, and so does an answer that shows a claim the inventory did not: the next
/// attempt reads the settled states.
pub(super) fn pair(
    found: &mut [Found],
    bindings: &[AdoptBinding],
    server: &UnixStream,
    deadline: Instant,
) -> Result<Paired, host::Failed> {
    let mut copies: BTreeMap<[u8; 16], Vec<HostCopy>> = BTreeMap::new();
    for (index, found) in found.iter().enumerate() {
        for terminal in &found.inventory {
            copies
                .entry(terminal.identity.creation_id().as_bytes())
                .or_default()
                .push((index, *terminal));
        }
    }
    let mut named: BTreeMap<[u8; 16], usize> = BTreeMap::new();
    for binding in bindings {
        *named
            .entry(binding.identity.creation_id().as_bytes())
            .or_default() += 1;
    }
    let mut paired = Paired {
        bindings: bindings.to_vec(),
        moved: Vec::new(),
    };
    let mut ended = Vec::new();
    for binding in &mut paired.bindings {
        let creation = binding.identity.creation_id().as_bytes();
        // Two bindings of one creation leave the import to refuse them.
        let (Some(group), Some(1)) = (copies.get(&creation), named.get(&creation).copied()) else {
            continue;
        };
        let Some((owner_host, owner)) = owner(group) else {
            continue;
        };
        let rebinds = owner.identity != binding.identity;
        if rebinds && paired.moved.len() >= MAXIMUM_ADOPT_HOST_MOVED {
            continue;
        }
        for (index, copy) in group
            .iter()
            .filter(|(_, copy)| copy.identity != owner.identity)
        {
            let host = &found[*index].host;
            let gone = match copy.handover {
                Some(HandoverState::Restored) => discard(host, server, copy.identity, deadline)?,
                _ => drop_copy(
                    host,
                    server,
                    copy.identity,
                    owner.handover.is_none(),
                    deadline,
                )?,
            };
            if gone {
                ended.push(copy.identity);
            }
        }
        if owner.handover == Some(HandoverState::Quiesced) {
            resume(&found[owner_host].host, server, owner.identity, deadline)?;
        }
        if rebinds {
            binding.identity = owner.identity;
            paired.moved.push(AdoptMovedBinding {
                terminal_id: binding.terminal_id,
                identity: owner.identity,
            });
        }
    }
    for found in found.iter_mut() {
        found
            .inventory
            .retain(|terminal| !ended.contains(&terminal.identity));
    }
    Ok(paired)
}

/// The copy that owns the terminal: the one that runs, or, when none does, the one quiesced copy.
/// Two that run, or none that runs and no single quiesced one, leave the copies as they are.
fn owner(group: &[HostCopy]) -> Option<HostCopy> {
    let one = |state: Option<HandoverState>| {
        let mut matching = group.iter().filter(|(_, copy)| copy.handover == state);
        match (matching.next(), matching.next()) {
            (Some(copy), None) => Some(*copy),
            _ => None,
        }
    };
    if group.iter().any(|(_, copy)| copy.handover.is_none()) {
        one(None)
    } else {
        one(Some(HandoverState::Quiesced))
    }
}

/// Drops a copy a move left behind, which ends no program; whether it is gone. Beside an owner that
/// runs, which the Keeper names, a drop refused as moving resumes the copy through its Keeper,
/// which answers `claimed-elsewhere`, and drops it again.
fn drop_copy(
    host: &HostBrokerConnection,
    server: &UnixStream,
    copy: HostTerminalIdentity,
    owner_runs: bool,
    deadline: Instant,
) -> Result<bool, host::Failed> {
    let terminal = terminal(copy)?;
    let drop = HandoverControl::DropTerminal(terminal);
    match ask(host, server, &drop, deadline)? {
        Answer::Given(answer) => return Ok(answer == HandoverControl::TerminalDropped(terminal)),
        Answer::Moving if owner_runs => {}
        Answer::Moving | Answer::Refused => return Ok(false),
    }
    let resume = HandoverControl::ResumeTerminal(terminal);
    match exchange(host, server, &resume, deadline)? {
        Some(HandoverControl::TerminalResumed(resumed)) if resumed.terminal_id == terminal => {
            if resumed.outcome != ResumeOutcome::ClaimedElsewhere {
                return Err(unsettled(
                    "a copy a move left resumed beside the running one",
                ));
            }
        }
        _ => return Ok(false),
    }
    Ok(
        exchange(host, server, &drop, deadline)?
            == Some(HandoverControl::TerminalDropped(terminal)),
    )
}

/// Discards an import never claimed; whether it is gone.
fn discard(
    host: &HostBrokerConnection,
    server: &UnixStream,
    copy: HostTerminalIdentity,
    deadline: Instant,
) -> Result<bool, host::Failed> {
    let discard = HandoverControl::DiscardImport(DiscardTarget::Terminal(terminal(copy)?));
    match exchange(host, server, &discard, deadline)? {
        Some(HandoverControl::ImportDiscarded(DiscardOutcome::Discarded)) => Ok(true),
        Some(HandoverControl::ImportDiscarded(DiscardOutcome::Claimed)) => {
            Err(unsettled("an import was claimed during the adoption"))
        }
        _ => Ok(false),
    }
}

/// Resumes a quiesced owner; one left quiesced resumes at its own deadline.
fn resume(
    host: &HostBrokerConnection,
    server: &UnixStream,
    owner: HostTerminalIdentity,
    deadline: Instant,
) -> Result<(), host::Failed> {
    let resume = HandoverControl::ResumeTerminal(terminal(owner)?);
    match exchange(host, server, &resume, deadline)? {
        Some(HandoverControl::TerminalResumed(resumed))
            if resumed.outcome == ResumeOutcome::ClaimedElsewhere =>
        {
            Err(unsettled("a quiesced terminal was claimed elsewhere"))
        }
        _ => Ok(()),
    }
}

/// The answer to `control`, or `None` when it was refused or the adoption has no room left.
fn exchange(
    host: &HostBrokerConnection,
    server: &UnixStream,
    control: &HandoverControl,
    deadline: Instant,
) -> Result<Option<HandoverControl>, host::Failed> {
    Ok(match ask(host, server, control, deadline)? {
        Answer::Given(answer) => Some(answer),
        Answer::Moving | Answer::Refused => None,
    })
}

/// What a Host made of one request of the pairing.
enum Answer {
    Given(HandoverControl),
    /// Refused because the terminal is still moving.
    Moving,
    /// Refused otherwise, or the adoption had no room left.
    Refused,
}

fn ask(
    host: &HostBrokerConnection,
    server: &UnixStream,
    control: &HandoverControl,
    deadline: Instant,
) -> Result<Answer, host::Failed> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Ok(Answer::Refused);
    }
    match request(
        host,
        Serving::Within(server, left.min(STEP_BUDGET)),
        control,
    ) {
        Ok(answer) => Ok(Answer::Given(answer)),
        Err(error) if HostFailure::find(error.as_ref()).is_some() => Err(host::Failed {
            failure: AdoptHostFailure::HostUnreachable,
            restarts: 0,
            error,
        }),
        Err(error) if HostRefusal::moving(error.as_ref()) => Ok(Answer::Moving),
        Err(_) => Ok(Answer::Refused),
    }
}

fn terminal(identity: HostTerminalIdentity) -> Result<TerminalId, host::Failed> {
    TerminalId::from_bytes(identity.terminal_id().as_bytes()).map_err(|error| host::Failed {
        failure: AdoptHostFailure::InventoryFailed,
        restarts: 0,
        error: Box::new(error) as Box<dyn Error>,
    })
}

fn unsettled(error: &'static str) -> host::Failed {
    host::Failed {
        failure: AdoptHostFailure::InventoryFailed,
        restarts: 0,
        error: error.into(),
    }
}
