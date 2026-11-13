//! The relay table: each client channel paired with the link channel it is relayed to, a
//! `stop-session` request paired with its link `route`, and the bytes each direction holds until
//! the other side's credit admits them. A relay takes from one side only what the other side
//! committed, so a pair holds at most one window per direction. The rules for ending pairs when a
//! carrier ends and while the hub closes are pure functions of the pair and the time.
//! `docs/machines.md#channels` owns the rules.

use super::{
    super::routing::{self, StopReading},
    state::Hub,
};
use orchbee_machine_protocol::{
    Id, ResetReason, SendError,
    hub::{self, ChannelKind, Outcome, Reply, StopSessionOutcome},
    link,
};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

/// How long open `route` channels may still run once the hub is closing.
pub(super) const ALLOWANCE: Duration = Duration::from_secs(60);
/// The most reply bytes a `stop-session` keeps; the far `server stop` writes far fewer.
const STOP_REPLY_BYTES: usize = 1024 * 1024;

/// Who opened a pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Opener {
    /// A client's channel on its connection.
    Client { peer: u64, channel: u32 },
    /// A `stop-session` request, answered from the link `route`'s reply.
    StopSession { peer: u64, request: Id },
}

/// What becomes of the client's side of a pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Ending {
    /// The pair stays open.
    Keep,
    /// `close` with the outcome, after the bytes ahead of it.
    Close(Outcome),
    /// `reset` with the reason at once.
    Reset(ResetReason),
}

/// What waits behind the bytes toward the client.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Finish {
    End,
    Close(Outcome),
}

#[derive(Debug)]
struct Pair {
    opener: Opener,
    carrier: u64,
    link: u32,
    kind: ChannelKind,
    /// The link accepted, so the client's channel was accepted too.
    accepted: bool,
    /// The opener's `end` was queued toward the link: a routed command may have started.
    dispatched: bool,
    /// The opener sent `end`, which waits behind `toward_link`.
    end_toward_link: bool,
    toward_link: Vec<u8>,
    toward_client: Vec<u8>,
    finish: Option<Finish>,
    /// The link channel is over: closed, reset, or its carrier ended.
    link_done: bool,
}

/// Every pair, found by either end.
#[derive(Debug, Default)]
pub(super) struct Relays {
    pairs: BTreeMap<u64, Pair>,
    next: u64,
    by_client: BTreeMap<(u64, u32), u64>,
    by_link: BTreeMap<(u64, u32), u64>,
}

impl Relays {
    /// The pairs a closing hub still lets finish.
    pub(super) fn routes(&self) -> usize {
        self.pairs
            .values()
            .filter(|pair| pair.kind == ChannelKind::Route)
            .count()
    }

    fn insert(&mut self, pair: Pair) {
        self.next += 1;
        if let Opener::Client { peer, channel } = pair.opener {
            self.by_client.insert((peer, channel), self.next);
        }
        self.by_link.insert((pair.carrier, pair.link), self.next);
        self.pairs.insert(self.next, pair);
    }

    fn remove(&mut self, key: u64) -> Option<Pair> {
        let pair = self.pairs.remove(&key)?;
        if let Opener::Client { peer, channel } = pair.opener {
            self.by_client.remove(&(peer, channel));
        }
        self.by_link.remove(&(pair.carrier, pair.link));
        Some(pair)
    }

    fn of_client(&self, peer: u64, channel: u32) -> Option<u64> {
        self.by_client.get(&(peer, channel)).copied()
    }

    fn of_link(&self, carrier: u64, link: u32) -> Option<u64> {
        self.by_link.get(&(carrier, link)).copied()
    }

    fn keys(&self, mut pick: impl FnMut(&Pair) -> bool) -> Vec<u64> {
        self.pairs
            .iter()
            .filter(|(_, pair)| pick(pair))
            .map(|(key, _)| *key)
            .collect()
    }
}

/// How a pair ends when its carrier ends: `link-lost` only where no effect can have started.
pub(super) const fn carrier_end(kind: ChannelKind, accepted: bool, dispatched: bool) -> Ending {
    match kind {
        ChannelKind::Session if accepted => Ending::Close(Outcome::Lost),
        ChannelKind::Route if dispatched => Ending::Close(Outcome::Lost),
        ChannelKind::Session | ChannelKind::Route => Ending::Reset(ResetReason::LinkLost),
    }
}

/// What a closing hub does with a pair at `now`, closing having begun at `began`: a `session`
/// ends at once, a `route` may finish until the allowance and then ends as at its carrier's end.
pub(super) fn closing(
    kind: ChannelKind,
    accepted: bool,
    dispatched: bool,
    began: Instant,
    now: Instant,
) -> Ending {
    match kind {
        ChannelKind::Session if accepted => Ending::Close(Outcome::Lost),
        ChannelKind::Session => Ending::Reset(ResetReason::Stopping),
        ChannelKind::Route if now < began + ALLOWANCE => Ending::Keep,
        ChannelKind::Route => carrier_end(kind, accepted, dispatched),
    }
}

/// Whether a closing hub may say goodbye and end its carriers: no route and no catalog read is
/// left, or the allowance has passed.
pub(super) fn closing_done(routes: usize, reloads: usize, began: Instant, now: Instant) -> bool {
    (routes == 0 && reloads == 0) || now >= began + ALLOWANCE
}

/// The hub channel outcome a link `close` relays as.
const fn relayed(kind: ChannelKind, outcome: &link::Outcome) -> Outcome {
    match (kind, outcome) {
        (ChannelKind::Session, link::Outcome::Ended) => Outcome::Ended,
        (ChannelKind::Session, link::Outcome::Failed) => Outcome::Failed,
        (ChannelKind::Route, link::Outcome::Replied) => Outcome::Replied,
        _ => Outcome::Lost,
    }
}

/// The `stop-session` outcome of a link `route` that did not reply.
const fn unreplied(dispatched: bool) -> StopSessionOutcome {
    if dispatched {
        StopSessionOutcome::Unknown
    } else {
        StopSessionOutcome::NotOnline
    }
}

impl Hub {
    /// A client's `open` of `channel`: relayed to the Machine's link, or reset.
    pub(super) fn open_channel(&mut self, peer: u64, channel: u32, open: &hub::Channel) {
        let reason = if self.lifetime.is_closing() {
            ResetReason::Stopping
        } else if self.machines.state(open.machine()).is_none() {
            ResetReason::NotFound
        } else {
            let link = match open {
                hub::Channel::Session { .. } => None,
                hub::Channel::Route { .. } => Some(link::Channel::Route),
            };
            match self.open_link(open.machine(), link) {
                Ok((carrier, link)) => {
                    self.relays.insert(Pair {
                        opener: Opener::Client { peer, channel },
                        carrier,
                        link,
                        kind: open.kind(),
                        accepted: false,
                        dispatched: false,
                        end_toward_link: false,
                        toward_link: Vec::new(),
                        toward_client: Vec::new(),
                        finish: None,
                        link_done: false,
                    });
                    return;
                }
                Err(reason) => reason,
            }
        };
        self.reset_client(peer, channel, reason);
    }

    /// A `stop-session` for an `online` Machine: a link `route` carrying that Session's
    /// `server stop`, answered when it replies. `None` when the Machine is not `online`.
    pub(super) fn stop_session(
        &mut self,
        peer: u64,
        request: Id,
        machine: &orchbee_machine_protocol::MachineId,
    ) -> Option<StopSessionOutcome> {
        let Some((carrier, session)) = self.machines.online_link(machine) else {
            return Some(StopSessionOutcome::NotOnline);
        };
        let packet = routing::stop_packet(session.as_str());
        let Some(handle) = self.carriers.get(&carrier) else {
            return Some(StopSessionOutcome::NotOnline);
        };
        let line = handle.line().clone();
        let mut wire = line.lock();
        if !wire.open {
            return Some(StopSessionOutcome::NotOnline);
        }
        let Ok(link) = wire.connection.open(link::Channel::Route) else {
            return Some(StopSessionOutcome::Failed);
        };
        // The packet is far smaller than the initial credit, so it is committed whole.
        let written = wire.connection.write(link, &packet);
        let dispatched = matches!(written, Ok(count) if count == packet.len())
            && wire.connection.end(link).is_ok();
        if !dispatched {
            let _reset = wire.connection.reset(link, ResetReason::Cancelled);
            return Some(StopSessionOutcome::Failed);
        }
        drop(wire);
        line.wake();
        self.relays.insert(Pair {
            opener: Opener::StopSession { peer, request },
            carrier,
            link,
            kind: ChannelKind::Route,
            accepted: false,
            dispatched: true,
            end_toward_link: false,
            toward_link: Vec::new(),
            toward_client: Vec::new(),
            finish: None,
            link_done: false,
        });
        None
    }

    /// Opens a link channel toward the `online` Machine `machine`: `route` when `channel` names it,
    /// otherwise `session` for the record's Session.
    fn open_link(
        &self,
        machine: &orchbee_machine_protocol::MachineId,
        channel: Option<link::Channel>,
    ) -> Result<(u64, u32), ResetReason> {
        let (carrier, session) = self
            .machines
            .online_link(machine)
            .ok_or(ResetReason::NotOnline)?;
        let handle = self.carriers.get(&carrier).ok_or(ResetReason::NotOnline)?;
        let channel = channel.unwrap_or(link::Channel::Session { session });
        let mut wire = handle.line().lock();
        if !wire.open {
            return Err(ResetReason::NotOnline);
        }
        // The link holds 256 channels the hub opened, or used its last id.
        let link = wire
            .connection
            .open(channel)
            .map_err(|_| ResetReason::Unavailable)?;
        drop(wire);
        handle.line().wake();
        Ok((carrier, link))
    }

    pub(super) fn client_data(&mut self, peer: u64, channel: u32, bytes: &[u8]) {
        if let Some(key) = self.relays.of_client(peer, channel)
            && let Some(pair) = self.relays.pairs.get_mut(&key)
        {
            pair.toward_link.extend_from_slice(bytes);
            self.flush_link(key);
        }
    }

    pub(super) fn client_writable(&mut self, peer: u64, channel: u32) {
        if let Some(key) = self.relays.of_client(peer, channel) {
            self.flush_client(key);
        }
    }

    pub(super) fn client_end(&mut self, peer: u64, channel: u32) {
        if let Some(key) = self.relays.of_client(peer, channel)
            && let Some(pair) = self.relays.pairs.get_mut(&key)
        {
            pair.end_toward_link = true;
            self.flush_link(key);
        }
    }

    /// The client reset its channel, or the hub's state machine reset it `too-large`: the link
    /// channel is cancelled, which ends a routed command the helper runs.
    pub(super) fn client_reset(&mut self, peer: u64, channel: u32) {
        if let Some(key) = self.relays.of_client(peer, channel) {
            self.cancel_link(key, ResetReason::Cancelled);
            self.relays.remove(key);
        }
    }

    /// The connection `peer` is over: each of its channels is cancelled toward the far end. A
    /// `stop-session` it asked for runs on and its reply goes nowhere.
    pub(super) fn peer_left(&mut self, peer: u64) {
        for key in self
            .relays
            .keys(|pair| matches!(pair.opener, Opener::Client { peer: owner, .. } if owner == peer))
        {
            self.cancel_link(key, ResetReason::Cancelled);
            self.relays.remove(key);
        }
    }

    pub(super) fn link_accepted(&mut self, carrier: u64, link: u32) {
        let Some(key) = self.relays.of_link(carrier, link) else {
            return;
        };
        let Some(pair) = self.relays.pairs.get_mut(&key) else {
            return;
        };
        pair.accepted = true;
        if let Opener::Client { peer, channel } = pair.opener
            && let Some(peer) = self.peers.get(&peer)
        {
            let _accepted = peer.line.lock().connection.accept(channel);
            peer.line.wake();
        }
    }

    pub(super) fn link_data(&mut self, carrier: u64, link: u32, bytes: Vec<u8>) {
        let Some(key) = self.relays.of_link(carrier, link) else {
            return;
        };
        let Some(pair) = self.relays.pairs.get_mut(&key) else {
            return;
        };
        match pair.opener {
            Opener::Client { .. } => {
                pair.toward_client.extend_from_slice(&bytes);
                self.flush_client(key);
            }
            Opener::StopSession { peer, request } => {
                let over = pair.toward_client.len() + bytes.len() > STOP_REPLY_BYTES;
                if !over {
                    pair.toward_client.extend_from_slice(&bytes);
                }
                if over {
                    self.cancel_link(key, ResetReason::Cancelled);
                    self.relays.remove(key);
                    self.reply_stop(peer, request, StopSessionOutcome::Unknown);
                } else if let Some(handle) = self.carriers.get(&carrier) {
                    let _taken = handle.line().lock().connection.take(link, bytes.len());
                    handle.line().wake();
                }
            }
        }
    }

    pub(super) fn link_writable(&mut self, carrier: u64, link: u32) {
        if let Some(key) = self.relays.of_link(carrier, link) {
            self.flush_link(key);
        }
    }

    pub(super) fn link_end(&mut self, carrier: u64, link: u32) {
        if let Some(key) = self.relays.of_link(carrier, link)
            && let Some(pair) = self.relays.pairs.get_mut(&key)
            && pair.finish.is_none()
            && matches!(pair.opener, Opener::Client { .. })
        {
            pair.finish = Some(Finish::End);
            self.flush_client(key);
        }
    }

    pub(super) fn link_closed(&mut self, carrier: u64, link: u32, outcome: link::Outcome) {
        let Some(key) = self.relays.of_link(carrier, link) else {
            return;
        };
        let Some(pair) = self.relays.pairs.get_mut(&key) else {
            return;
        };
        pair.link_done = true;
        match pair.opener {
            Opener::Client { .. } => {
                // `close` implies `end`, so it replaces an `end` still waiting.
                pair.finish = Some(Finish::Close(relayed(pair.kind, &outcome)));
                self.flush_client(key);
            }
            Opener::StopSession { peer, request } => {
                let read = if outcome == link::Outcome::Replied {
                    routing::stop_reading(&pair.toward_client)
                } else {
                    StopReading::Unknown
                };
                self.relays.remove(key);
                self.reply_stop(peer, request, stop_outcome(read));
            }
        }
    }

    pub(super) fn link_reset(&mut self, carrier: u64, link: u32, reason: ResetReason) {
        let Some(key) = self.relays.of_link(carrier, link) else {
            return;
        };
        let Some(pair) = self.relays.remove(key) else {
            return;
        };
        match pair.opener {
            Opener::Client { peer, channel } => self.reset_client(peer, channel, reason),
            // A responder's reset asserts no effect: the Session was not stopped.
            Opener::StopSession { peer, request } => {
                self.reply_stop(peer, request, StopSessionOutcome::Failed);
            }
        }
    }

    /// The carrier `carrier` ended, or is being ended: every pair on it whose link channel was still
    /// open ends by the no-effect rule. A pair whose link already closed keeps its outcome.
    pub(super) fn carrier_ended(&mut self, carrier: u64) {
        for key in self
            .relays
            .keys(|pair| pair.carrier == carrier && !pair.link_done)
        {
            let Some(pair) = self.relays.pairs.get(&key) else {
                continue;
            };
            let ending = carrier_end(pair.kind, pair.accepted, pair.dispatched);
            self.end_pair(key, ending, None);
        }
    }

    /// Applies closing at `now`, closing having begun at `began`, to every pair.
    pub(super) fn close_relays(&mut self, began: Instant, now: Instant) {
        for key in self.relays.keys(|pair| !pair.link_done) {
            let Some(pair) = self.relays.pairs.get(&key) else {
                continue;
            };
            let ending = closing(pair.kind, pair.accepted, pair.dispatched, began, now);
            if ending != Ending::Keep {
                self.end_pair(key, ending, Some(ResetReason::Stopping));
            }
        }
    }

    /// Ends the pair `key` toward its client with `ending`, resetting its link channel with
    /// `cancel` first when the link is still there.
    fn end_pair(&mut self, key: u64, ending: Ending, cancel: Option<ResetReason>) {
        if let Some(reason) = cancel {
            self.cancel_link(key, reason);
        }
        let Some(pair) = self.relays.pairs.get_mut(&key) else {
            return;
        };
        pair.link_done = true;
        match (pair.opener, ending) {
            (_, Ending::Keep) => {}
            (Opener::StopSession { peer, request }, _) => {
                let outcome = unreplied(pair.dispatched);
                self.relays.remove(key);
                self.reply_stop(peer, request, outcome);
            }
            (Opener::Client { peer, channel }, Ending::Reset(reason)) => {
                self.relays.remove(key);
                self.reset_client(peer, channel, reason);
            }
            (Opener::Client { peer, channel }, Ending::Close(outcome)) => {
                // A `close` needs an accepted channel; a dispatched route may not have one yet.
                if !pair.accepted {
                    pair.accepted = true;
                    if let Some(peer) = self.peers.get(&peer) {
                        let _accepted = peer.line.lock().connection.accept(channel);
                    }
                }
                pair.finish = Some(Finish::Close(outcome));
                self.flush_client(key);
            }
        }
    }

    /// Commits what the pair holds toward the link, takes as much from the client, and sends the
    /// opener's `end` once nothing is ahead of it.
    fn flush_link(&mut self, key: u64) {
        let Self {
            relays,
            carriers,
            peers,
            ..
        } = self;
        let Some(pair) = relays.pairs.get_mut(&key) else {
            return;
        };
        if pair.link_done {
            pair.toward_link.clear();
            return;
        }
        let Some(handle) = carriers.get(&pair.carrier) else {
            return;
        };
        let (committed, ended) = {
            let mut wire = handle.line().lock();
            let committed = if pair.toward_link.is_empty() {
                0
            } else {
                wire.connection
                    .write(pair.link, &pair.toward_link)
                    .unwrap_or(0)
            };
            let ended = pair.end_toward_link
                && !pair.dispatched
                && committed == pair.toward_link.len()
                && wire.connection.end(pair.link).is_ok();
            (committed, ended)
        };
        handle.line().wake();
        pair.toward_link.drain(..committed);
        if ended {
            pair.dispatched = true;
        }
        if committed > 0
            && let Opener::Client { peer, channel } = pair.opener
            && let Some(peer) = peers.get(&peer)
        {
            let _taken = peer.line.lock().connection.take(channel, committed);
            peer.line.wake();
        }
    }

    /// Commits what the pair holds toward the client, takes as much from the link, and sends what
    /// waits behind the bytes; a pair over on both sides leaves the table.
    fn flush_client(&mut self, key: u64) {
        let Some(Opener::Client { peer, channel }) =
            self.relays.pairs.get(&key).map(|pair| pair.opener)
        else {
            return;
        };
        if !self.peers.contains_key(&peer) {
            self.cancel_link(key, ResetReason::Cancelled);
            self.relays.remove(key);
            return;
        }
        let Self {
            relays,
            carriers,
            peers,
            ..
        } = self;
        let (Some(pair), Some(client)) = (relays.pairs.get_mut(&key), peers.get(&peer)) else {
            return;
        };
        let (committed, finished) = {
            let mut wire = client.line.lock();
            let committed = if pair.toward_client.is_empty() {
                0
            } else {
                match wire.connection.write(channel, &pair.toward_client) {
                    Ok(committed) => committed,
                    // The channel or the connection is gone: nothing will take these bytes.
                    Err(SendError::UnknownChannel | SendError::Closed) => pair.toward_client.len(),
                    Err(_) => 0,
                }
            };
            let finished = if committed == pair.toward_client.len() {
                match pair.finish.take() {
                    Some(Finish::End) => {
                        let _ended = wire.connection.end(channel);
                        false
                    }
                    Some(Finish::Close(outcome)) => {
                        let _closed = wire.connection.close(channel, outcome);
                        true
                    }
                    None => false,
                }
            } else {
                false
            };
            (committed, finished)
        };
        client.line.wake();
        pair.toward_client.drain(..committed);
        if committed > 0
            && !pair.link_done
            && let Some(handle) = carriers.get(&pair.carrier)
        {
            let _taken = handle.line().lock().connection.take(pair.link, committed);
            handle.line().wake();
        }
        if finished {
            relays.remove(key);
        }
    }

    /// Resets the pair's link channel with `reason` unless the link is over already.
    fn cancel_link(&mut self, key: u64, reason: ResetReason) {
        let Some(pair) = self.relays.pairs.get_mut(&key) else {
            return;
        };
        if pair.link_done {
            return;
        }
        pair.link_done = true;
        if let Some(handle) = self.carriers.get(&pair.carrier) {
            let _reset = handle.line().lock().connection.reset(pair.link, reason);
            handle.line().wake();
        }
    }

    /// Resets the client's channel with `reason`, or with `unavailable` when its kind does not list
    /// `reason`: no effect, and the Machine cannot serve the channel now.
    fn reset_client(&self, peer: u64, channel: u32, reason: ResetReason) {
        let Some(peer) = self.peers.get(&peer) else {
            return;
        };
        let mut wire = peer.line.lock();
        if let Err(SendError::Reason) = wire.connection.reset(channel, reason) {
            let _reset = wire.connection.reset(channel, ResetReason::Unavailable);
        }
        drop(wire);
        peer.line.wake();
    }

    fn reply_stop(&self, peer: u64, request: Id, outcome: StopSessionOutcome) {
        if let Some(peer) = self.peers.get(&peer) {
            let mut wire = peer.line.lock();
            if wire.open {
                let _replied = wire.connection.reply(request, Reply::StopSession(outcome));
            }
            drop(wire);
            peer.line.wake();
        }
    }
}

/// The `stop-session` outcome of the far `server stop`'s reply.
const fn stop_outcome(read: StopReading) -> StopSessionOutcome {
    match read {
        StopReading::Stopped => StopSessionOutcome::Stopped,
        StopReading::NotRunning => StopSessionOutcome::NotRunning,
        StopReading::Unknown => StopSessionOutcome::Unknown,
        StopReading::Failed => StopSessionOutcome::Failed,
    }
}

#[cfg(test)]
mod tests;
