//! A replacement Worker's recovery on Windows (`proposals/platform/host-handover.md`, H2 design):
//! the Host ended while the Session's Keeper kept its programs. The replacement adopts that Keeper,
//! starts a recovery Host in the Host's slot that serves the lost epoch, and recovers each bound
//! terminal into it before the recovery Host's inventory is read as an adoption reads it. A Host
//! adopted while a Keeper runs recovers the orphans of its own epoch the same way, which completes
//! a recovery an earlier attempt left part done.
//!
//! Every step before a terminal's claim is undoable and leaves its program running in the Keeper.
//! A step that fails fails the replacement, which the Server retries under its cap; a terminal
//! already recovered stays recovered.

use std::{
    error::Error,
    io,
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    AdoptBinding, AdoptHostFailure, AdoptHostProcess, HostEpoch,
};
use orchbee_local_runtime::{Cohort, HoldingState, KeeperProcess, KeeperStatus, RetainedChild};
use orchbee_terminal_protocol::{
    CreationId, HandoverControl, RecoverTerminal, TerminalId, TerminalPixelGeometry,
};

use super::lazy::{HostStart, start_host};
use crate::{
    host_adoption as host,
    host_connection::HostBrokerConnection,
    session_keeper::{self, SessionKeeper},
    terminal_authorization::InventoryTerminal,
};

/// The size a recovered terminal starts at; a Client that reattaches resizes it, and the console's
/// resize after the claim makes a full-screen program redraw.
const RECOVERY_GEOMETRY: TerminalPixelGeometry = TerminalPixelGeometry {
    cell_pixel_height: 0,
    cell_pixel_width: 0,
    columns: 80,
    rows: 24,
};
/// How long the recovery Host may take from a `RecoverTerminal` to the restored terminal's claim.
const RECOVERY_DEADLINE_SECONDS: u8 = 10;
/// How long a recovery waits for the Keeper to see the lost Host's end, which orphans its holdings.
const ORPHAN_WAIT: Duration = Duration::from_secs(2);

/// A recovery Host with its terminals recovered and its inventory read.
pub(super) struct Recovered {
    pub(super) host: HostBrokerConnection,
    pub(super) child: RetainedChild,
    pub(super) keeper: SessionKeeper,
    pub(super) inventory: Vec<InventoryTerminal>,
    pub(super) restarts: u32,
}

/// Recovers the programs that `keeper`, the Session's recorded Keeper, kept after the Host that
/// served `epoch` ended, into a recovery Host this Worker starts from `cohort`.
pub(super) fn recover(
    cohort: &Cohort,
    runtime: &std::path::Path,
    keeper: AdoptHostProcess,
    epoch: Option<HostEpoch>,
    bindings: &[AdoptBinding],
    channel: host::Channel<'_>,
) -> Result<Recovered, host::Failed> {
    let failed = |error: Box<dyn Error>| host::Failed {
        failure: AdoptHostFailure::RecoveryFailed,
        restarts: 0,
        error,
    };
    let expected = (keeper.process_id.get(), keeper.creation_time.get());
    let executable = cohort.executable(orchbee_local_runtime::CohortArtifact::TerminalHost);
    let mut keeper = match SessionKeeper::adopt(runtime, &executable, Some(expected)) {
        Ok(Some(keeper)) => keeper,
        Ok(None) => return Err(failed("the Session's Keeper is gone".into())),
        Err(error) => return Err(failed(error)),
    };
    let (host, child) = start_host(
        cohort,
        runtime,
        Rc::clone(&channel.drain),
        HostStart::Recovery(epoch),
    )
    .map_err(|failure| {
        failure.record();
        failed(failure.into_error())
    })?;
    crate::diagnostic_events::host_started();
    let attached = (|| -> Result<(u32, u64), Box<dyn Error>> {
        host.stream().set_nonblocking(true)?;
        if epoch.is_some_and(|epoch| *host.host_epoch().as_bytes() != epoch.as_bytes()) {
            return Err(refused("the recovery Host serves another epoch"));
        }
        if !host.recovery() {
            return Err(refused("the recovery Host does not offer recovery"));
        }
        let process = child.exact_identity()?;
        keeper.attach(&host, channel.server, process)?;
        Ok(process)
    })();
    let process = attached.map_err(failed)?;
    recover_orphans(&mut keeper, &host, channel.server, process, bindings).map_err(failed)?;
    let (inventory, restarts) =
        host::read_inventory(&host, channel.server, channel.correlation, channel.deadline)?;
    Ok(Recovered {
        host,
        child,
        keeper,
        inventory,
        restarts,
    })
}

/// Recovers into `host`, the Host process `target`, each orphaned holding of that Host's epoch
/// that a binding names, and answers how many it recovered.
pub(super) fn recover_orphans(
    keeper: &mut SessionKeeper,
    host: &HostBrokerConnection,
    server: &orchbee_local_ipc::LocalStream,
    target: (u32, u64),
    bindings: &[AdoptBinding],
) -> Result<u32, Box<dyn Error>> {
    let epoch = *host.host_epoch().as_bytes();
    let target = KeeperProcess {
        pid: target.0,
        created: target.1,
    };
    let status = settled_status(keeper, target, epoch)?;
    let profile = super::launch_facts::recovery_profile();
    let mut recovered = 0;
    for binding in bindings {
        let identity = binding.identity;
        let Some(holding) = status.holdings.iter().find(|holding| {
            holding.state == HoldingState::Orphaned
                && holding.terminal.host_epoch == epoch
                && holding.terminal.creation_id == identity.creation_id().as_bytes()
                && holding.terminal.terminal_id == identity.terminal_id().as_bytes()
        }) else {
            continue;
        };
        let mark = derived_mark(holding.holding, target);
        keeper.assign(holding.holding, target, mark)?;
        let creation_id = CreationId::from_bytes(identity.creation_id().as_bytes())?;
        let terminal_id = TerminalId::from_bytes(identity.terminal_id().as_bytes())?;
        let recover = RecoverTerminal {
            creation_id,
            terminal_id,
            holding: holding.holding,
            mark,
            geometry: RECOVERY_GEOMETRY,
            terminal_profile: profile.clone(),
            configuration: None,
            deadline_seconds: RECOVERY_DEADLINE_SECONDS,
        };
        match session_keeper::request(
            host,
            server,
            &HandoverControl::RecoverTerminal(Box::new(recover)),
        )? {
            HandoverControl::TerminalRecovered(restored)
                if restored.creation_id == creation_id && restored.terminal_id == terminal_id => {}
            _ => return Err(refused("the recovery Host restored another terminal")),
        }
        match session_keeper::request(host, server, &HandoverControl::ClaimTerminal(terminal_id))? {
            HandoverControl::TerminalClaimed(claimed) if claimed == terminal_id => {}
            _ => return Err(refused("the recovery Host claimed another terminal")),
        }
        recovered += 1;
    }
    Ok(recovered)
}

/// Discards each orphan of `epoch` that no binding names, which ends its program: no person can
/// reach it, and the Keeper would otherwise keep it running until the Session stops. A discard
/// that fails is recorded and leaves the program running.
pub(super) fn discard_unbound(
    keeper: &mut SessionKeeper,
    epoch: [u8; 16],
    bindings: &[AdoptBinding],
) {
    let status = match keeper.status() {
        Ok(status) => status,
        Err(error) => return crate::diagnostic_events::keeper_failed("discard", &error),
    };
    for holding in &status.holdings {
        let bound = bindings.iter().any(|binding| {
            binding.identity.terminal_id().as_bytes() == holding.terminal.terminal_id
        });
        if holding.state != HoldingState::Orphaned || holding.terminal.host_epoch != epoch || bound
        {
            continue;
        }
        if let Err(error) = keeper.discard(holding.holding) {
            crate::diagnostic_events::keeper_failed("discard", error.as_ref());
        }
    }
}

/// The Keeper's status once no holding of `epoch` is still owned by a Host other than `target`:
/// the Keeper orphans a Host's holdings when it sees that Host end, which follows the end by at
/// most moments.
fn settled_status(
    keeper: &mut SessionKeeper,
    target: KeeperProcess,
    epoch: [u8; 16],
) -> io::Result<KeeperStatus> {
    let until = Instant::now() + ORPHAN_WAIT;
    loop {
        let status = keeper.status()?;
        let unsettled = status.holdings.iter().any(|holding| {
            holding.terminal.host_epoch == epoch
                && holding.owner != target
                && holding.state != HoldingState::Orphaned
        });
        if !unsettled || Instant::now() >= until {
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// The mark a recovery assigns `holding` to `target` under, derived from both, so that a retry
/// assigns it again under the same mark, which the Keeper answers without effect.
fn derived_mark(holding: [u8; 16], target: KeeperProcess) -> [u8; 16] {
    let salt = target
        .pid
        .to_le_bytes()
        .into_iter()
        .chain(target.created.to_le_bytes())
        .chain(*b"rcvr");
    let mut mark = [0_u8; 16];
    for ((byte, held), salt) in mark.iter_mut().zip(holding).zip(salt) {
        *byte = held ^ salt;
    }
    if mark == [0; 16] {
        mark[15] = 1;
    }
    mark
}

fn refused(text: &str) -> Box<dyn Error> {
    io::Error::new(io::ErrorKind::InvalidData, text.to_owned()).into()
}
