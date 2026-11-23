//! The Worker's moves of terminals from older Terminal Hosts into its own on Windows
//! (`proposals/platform/host-handover.md`, H3 design). While a replacement serves, the Server sends
//! a move turn every 2 seconds; the Worker picks the terminal, runs the move inside the turn in
//! its serialized request loop, so every request the Server writes meanwhile waits for it, rebinds
//! the kernel and the attachments, and answers what moved. The first failed move stops moves for
//! the rest of the Worker's life; an older Host left without a terminal drains as P1c's do.

mod attachments;
mod steps;
mod translate;

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    num::NonZeroU32,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    HostEpoch, HostTerminalId, HostTerminalIdentity, TerminalMoveTurnOutcome,
};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_terminal_protocol::HandoverProcess;

use super::{host_set::HostSet, keeper_care::KeeperCare};
use crate::terminal_authorization::AuthorizedHostOperationKernel;
use steps::{Ended, Move, Refused};
pub(super) use translate::{lifecycle_answer, viewer_answer};

/// A terminal is due this long after the Worker last relayed input, a paste, a resize, or a send
/// to it.
const QUIET: Duration = Duration::from_secs(2);
/// A terminal that keeps getting input is due this long after a turn first found it waiting.
const PATIENCE: Duration = Duration::from_secs(600);
/// The turns an old copy whose end keeps being refused is tried for before it is left.
const OLD_COPY_TURNS: u32 = 30;

/// A Host terminal as a map key: its Host's epoch and its id there.
type IdentityKey = [u8; 32];

fn key_of(epoch: HostEpoch, terminal: HostTerminalId) -> IdentityKey {
    let mut key = [0; 32];
    key[..16].copy_from_slice(&epoch.as_bytes());
    key[16..].copy_from_slice(&terminal.as_bytes());
    key
}

fn key(identity: HostTerminalIdentity) -> IdentityKey {
    key_of(identity.host_epoch(), identity.terminal_id())
}

/// The old copy of the last move, which ends once the Server recorded the rebinding: at the next
/// turn.
struct OldCopy {
    identity: HostTerminalIdentity,
    settled: bool,
    turns: u32,
}

/// What the Worker keeps of its moves for its whole life.
#[derive(Default)]
pub(super) struct Moves {
    /// A move failed, so no other runs.
    stopped: bool,
    /// The turns ended: the Server sends no other to this Worker.
    finished: bool,
    /// Host terminals an old Host refused as ineligible, which stay where they run.
    skipped: BTreeSet<IdentityKey>,
    /// When the Worker last relayed input, a paste, a resize, or a send to a Host terminal.
    relayed: BTreeMap<[u8; 16], Instant>,
    /// When a turn first found a Host terminal waiting.
    waiting: BTreeMap<[u8; 16], Instant>,
    old_copy: Option<OldCopy>,
    /// Each Host terminal that moved and the identity it runs as now, which a request the Server
    /// wrote before it rebound still names.
    aliases: BTreeMap<IdentityKey, HostTerminalIdentity>,
}

impl Moves {
    /// Notes that the Worker relayed input, a paste, a resize, or a send to `terminal`.
    fn note_relayed(&mut self, terminal: HostTerminalId) {
        if !self.finished {
            self.relayed.insert(terminal.as_bytes(), Instant::now());
        }
    }

    /// The identity a Host terminal runs as now, after any move.
    fn translate(&self, identity: HostTerminalIdentity) -> HostTerminalIdentity {
        self.alias(identity.host_epoch(), identity.terminal_id())
            .filter(|to| to.creation_id() == identity.creation_id())
            .unwrap_or(identity)
    }

    /// The identity the Host terminal `terminal` of the Host of `epoch` moved to, if it moved.
    fn alias(&self, epoch: HostEpoch, terminal: HostTerminalId) -> Option<HostTerminalIdentity> {
        self.aliases.get(&key_of(epoch, terminal)).copied()
    }

    /// One move turn of the request read at `readable`. Only a lost Host exchange is an error,
    /// which ends the Worker as any lost exchange does.
    pub(super) fn turn(
        &mut self,
        hosts: &mut HostSet<'_>,
        kernel: &mut AuthorizedHostOperationKernel,
        care: &KeeperCare,
        server: &UnixStream,
        readable: Instant,
    ) -> Result<TerminalMoveTurnOutcome, Box<dyn Error>> {
        self.end_old_copy(hosts, server)?;
        let outcome = self.next(hosts, kernel, care, server, readable)?;
        // The Server sends no turn after `Done`, so an old copy still to end keeps them coming.
        Ok(match outcome {
            TerminalMoveTurnOutcome::Done if self.old_copy.is_some() => {
                TerminalMoveTurnOutcome::Waiting
            }
            TerminalMoveTurnOutcome::Done => {
                self.finished = true;
                self.relayed.clear();
                self.waiting.clear();
                outcome
            }
            outcome => outcome,
        })
    }

    /// Ends the last move's old copy, which this turn says the Server recorded as rebound.
    fn end_old_copy(
        &mut self,
        hosts: &HostSet<'_>,
        server: &UnixStream,
    ) -> Result<(), Box<dyn Error>> {
        let Some(copy) = self.old_copy.as_mut() else {
            return Ok(());
        };
        let epoch = copy.identity.host_epoch().as_bytes();
        let Some(index) = hosts
            .older()
            .into_iter()
            .find(|index| *hosts.connection(*index).host_epoch().as_bytes() == epoch)
        else {
            // Its Host ended, and the copy with it.
            self.old_copy = None;
            return Ok(());
        };
        if attachments::end_old_copy(hosts.connection(index), server, copy.identity, copy.settled)?
        {
            self.old_copy = None;
        } else {
            copy.turns += 1;
            if copy.turns >= OLD_COPY_TURNS {
                self.old_copy = None;
            }
        }
        Ok(())
    }

    fn next(
        &mut self,
        hosts: &mut HostSet<'_>,
        kernel: &mut AuthorizedHostOperationKernel,
        care: &KeeperCare,
        server: &UnixStream,
        readable: Instant,
    ) -> Result<TerminalMoveTurnOutcome, Box<dyn Error>> {
        if !orchbee_local_runtime::moves_live() || self.stopped {
            return Ok(TerminalMoveTurnOutcome::Done);
        }
        let mut candidates = Vec::new();
        for index in hosts.older() {
            let host = hosts.connection(index);
            if !host.handover() {
                continue;
            }
            let epoch = HostEpoch::from_bytes(*host.host_epoch().as_bytes())?;
            candidates.extend(
                kernel
                    .movable_on(epoch)
                    .into_iter()
                    .filter(|(_, identity)| !self.skipped.contains(&key(*identity)))
                    .map(|(terminal, identity)| (index, terminal, identity)),
            );
        }
        if candidates.is_empty() {
            return Ok(TerminalMoveTurnOutcome::Done);
        }
        let now = Instant::now();
        let listed = |terminal: &[u8; 16]| {
            candidates
                .iter()
                .any(|(_, _, identity)| identity.terminal_id().as_bytes() == *terminal)
        };
        self.relayed.retain(|terminal, _| listed(terminal));
        self.waiting.retain(|terminal, _| listed(terminal));
        let due = candidates
            .iter()
            .copied()
            .find(|(_, _, identity)| self.due(identity.terminal_id(), now));
        for (_, _, identity) in &candidates {
            self.waiting
                .entry(identity.terminal_id().as_bytes())
                .or_insert(now);
        }
        let Some((old, terminal, from)) = due else {
            return Ok(TerminalMoveTurnOutcome::Waiting);
        };
        // Both Hosts hold their terminals in the Session's Keeper, which no replacement is under
        // way for.
        if care.replacing()
            || !hosts.keeper_attached(old)
            || hosts.connection(old).keeper_troubled()
        {
            return Ok(TerminalMoveTurnOutcome::Waiting);
        }
        let new = match hosts.own() {
            Some(index) => index,
            None => match hosts.own_or_start(readable) {
                // The Keeper's care has a new Host attach the Keeper between requests.
                Ok(_) => return Ok(TerminalMoveTurnOutcome::Waiting),
                Err(_) => return Ok(self.stop("host-start", Refused::Failed("other"))),
            },
        };
        if !hosts.connection(new).handover() {
            return Ok(self.stop("import", Refused::Failed("other")));
        }
        if !hosts.keeper_attached(new) || hosts.connection(new).keeper_troubled() {
            return Ok(TerminalMoveTurnOutcome::Waiting);
        }
        let Some(target) = hosts.host_process(new).and_then(|(pid, created)| {
            Some(HandoverProcess {
                pid: NonZeroU32::new(pid)?,
                creation_time: created,
            })
        }) else {
            return Ok(TerminalMoveTurnOutcome::Waiting);
        };
        let began = Instant::now();
        let ended = Move {
            server,
            old: hosts.connection(old),
            new: hosts.connection(new),
            target,
            from,
        }
        .run()?;
        let moved = match ended {
            Ended::Moved(moved) => moved,
            Ended::Refused { step, refused } => {
                return Ok(match refused {
                    Refused::Passing(_) => TerminalMoveTurnOutcome::Waiting,
                    Refused::Ineligible(reason) => {
                        self.skipped.insert(key(from));
                        crate::diagnostic_events::terminal_move_failed(
                            step,
                            refused.class(),
                            reason,
                            false,
                        );
                        TerminalMoveTurnOutcome::Waiting
                    }
                    Refused::Failed(_) => self.stop(step, refused),
                });
            }
        };
        let milliseconds = u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX);
        kernel.rebind(terminal, from, moved.to);
        self.aliases.insert(key(from), moved.to);
        self.relayed.remove(&from.terminal_id().as_bytes());
        self.waiting.remove(&from.terminal_id().as_bytes());
        self.old_copy = Some(OldCopy {
            identity: from,
            settled: moved.settled,
            turns: 0,
        });
        crate::diagnostic_events::terminal_moved(
            hosts.slot_index(old),
            hosts.slot_index(new),
            moved.record_bytes,
            moved.settled,
            milliseconds,
        );
        attachments::transfer(
            hosts.connection(old),
            hosts.connection(new),
            server,
            from.terminal_id(),
            moved.to.terminal_id(),
        )?;
        Ok(TerminalMoveTurnOutcome::Moved {
            terminal_id: terminal,
            from,
            to: moved.to,
        })
    }

    fn due(&self, terminal: HostTerminalId, now: Instant) -> bool {
        let terminal = terminal.as_bytes();
        self.relayed
            .get(&terminal)
            .is_none_or(|last| now.duration_since(*last) >= QUIET)
            || self
                .waiting
                .get(&terminal)
                .is_some_and(|first| now.duration_since(*first) >= PATIENCE)
    }

    /// Stops moves for the rest of the Worker's life after `step` failed.
    fn stop(&mut self, step: &'static str, refused: Refused) -> TerminalMoveTurnOutcome {
        self.stopped = true;
        crate::diagnostic_events::terminal_move_failed(
            step,
            refused.class(),
            refused.reason(),
            true,
        );
        TerminalMoveTurnOutcome::Done
    }
}
