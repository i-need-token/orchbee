//! The relay table: each client channel paired with the link channel it is relayed to, the link
//! `session` channel of a `stop-session` run's current exchange paired with that run, and the bytes
//! each direction holds until the other side's credit admits them. A relay takes from one side
//! only what the other side committed, so a pair holds at most one window per direction. The rules
//! for ending pairs when a carrier ends, when the far end resets a link channel, and while the hub
//! closes are pure functions of the pair and the time.
//! `docs/machines.md#channels` owns the rules.

use super::state::Hub;
use orchbee_machine_protocol::{
    MachineId, ResetReason, SendError,
    hub::{self, ChannelKind, Outcome},
    link,
};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

/// How long open `route` channels may still run once the hub is closing.
pub(super) const ALLOWANCE: Duration = Duration::from_secs(60);

/// Who opened a pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Opener {
    /// A client's channel on its connection.
    Client { peer: u64, channel: u32 },
    /// The `stop-session` run `run`, whose thread writes and reads the link channel through the
    /// pair.
    Stop { run: u64 },
}

/// What a run's channel holds for its thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Held {
    /// This many bytes were copied out, and their credit returned to the link.
    Bytes(usize),
    /// The link sends nothing more: it ended, closed or reset the channel, or its carrier ended.
    Over,
    /// Nothing yet.
    Waiting,
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
    /// The far end's reset reason, which a run reads.
    reset: Option<ResetReason>,
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

    fn insert(&mut self, pair: Pair) -> u64 {
        self.next += 1;
        if let Opener::Client { peer, channel } = pair.opener {
            self.by_client.insert((peer, channel), self.next);
        }
        self.by_link.insert((pair.carrier, pair.link), self.next);
        self.pairs.insert(self.next, pair);
        self.next
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

/// How a pair ends when the far end resets its link channel with `reason`: passed on only where no
/// effect can have started. Past that point the far end's reset cannot state that nothing ran, so
/// the pair ends `lost`, as at its carrier's end.
pub(super) const fn link_reset_end(
    kind: ChannelKind,
    accepted: bool,
    dispatched: bool,
    reason: ResetReason,
) -> Ending {
    match carrier_end(kind, accepted, dispatched) {
        Ending::Reset(_) => Ending::Reset(reason),
        ending => ending,
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

impl Pair {
    const fn new(opener: Opener, carrier: u64, link: u32, kind: ChannelKind) -> Self {
        Self {
            opener,
            carrier,
            link,
            kind,
            accepted: false,
            dispatched: false,
            end_toward_link: false,
            toward_link: Vec::new(),
            toward_client: Vec::new(),
            finish: None,
            link_done: false,
            reset: None,
        }
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
                    let opener = Opener::Client { peer, channel };
                    self.relays
                        .insert(Pair::new(opener, carrier, link, open.kind()));
                    return;
                }
                Err(reason) => reason,
            }
        };
        self.reset_client(peer, channel, reason);
    }

    /// Opens a link `session` channel for the run `run` while its carrier `carrier` still carries
    /// the `online` Machine `machine`, and returns its pair's key; `None` when there is no room or
    /// the Machine has moved on, and then nothing was sent.
    pub(super) fn open_stop_channel(
        &mut self,
        run: u64,
        machine: &MachineId,
        carrier: u64,
    ) -> Option<u64> {
        if self.machines.online_link(machine).map(|(on, _)| on) != Some(carrier) {
            return None;
        }
        let (carrier, link) = self.open_link(machine, None).ok()?;
        let opener = Opener::Stop { run };
        Some(
            self.relays
                .insert(Pair::new(opener, carrier, link, ChannelKind::Session)),
        )
    }

    /// Commits as much of `bytes` to the run's channel `key` as the link's credit admits and
    /// returns the count, 0 until the credit returns; `None` once the channel is over.
    pub(super) fn write_stop_channel(&self, key: u64, bytes: &[u8]) -> Option<usize> {
        let pair = self.relays.pairs.get(&key).filter(|pair| !pair.link_done)?;
        let handle = self.carriers.get(&pair.carrier)?;
        let committed = handle.line().lock().connection.write(pair.link, bytes).ok();
        handle.line().wake();
        committed
    }

    /// Copies what the run's channel `key` holds into `buffer`, returning the link its credit for
    /// the bytes copied.
    pub(super) fn read_stop_channel(&mut self, key: u64, buffer: &mut [u8]) -> Held {
        let Some(pair) = self.relays.pairs.get_mut(&key) else {
            return Held::Over;
        };
        if pair.toward_client.is_empty() {
            return if pair.link_done || pair.finish.is_some() {
                Held::Over
            } else {
                Held::Waiting
            };
        }
        let count = buffer.len().min(pair.toward_client.len());
        for (slot, byte) in buffer.iter_mut().zip(pair.toward_client.drain(..count)) {
            *slot = byte;
        }
        if !pair.link_done
            && let Some(handle) = self.carriers.get(&pair.carrier)
        {
            let _taken = handle.line().lock().connection.take(pair.link, count);
            handle.line().wake();
        }
        Held::Bytes(count)
    }

    /// Ends the run's channel `key`: resets the link channel `cancelled` unless it is over, and
    /// returns the far end's reset reason, if the far end reset it.
    pub(super) fn close_stop_channel(&mut self, key: u64) -> Option<ResetReason> {
        self.cancel_link(key, ResetReason::Cancelled);
        self.relays.remove(key).and_then(|pair| pair.reset)
    }

    /// Opens a link channel toward the `online` Machine `machine`: `route` when `channel` names it,
    /// otherwise `session` for the record's Session.
    fn open_link(
        &self,
        machine: &MachineId,
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

    /// The client's credit returned; true when the pair then finished and left the table.
    pub(super) fn client_writable(&mut self, peer: u64, channel: u32) -> bool {
        self.relays
            .of_client(peer, channel)
            .is_some_and(|key| self.flush_client(key))
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
        self.forget_stop_waiters(peer);
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
        pair.toward_client.extend_from_slice(&bytes);
        // A run's thread takes the bytes, and returns their credit, as it reads them.
        if matches!(pair.opener, Opener::Client { .. }) {
            self.flush_client(key);
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
        // `close` implies `end`, so it replaces an `end` still waiting; a run reads the end itself.
        if matches!(pair.opener, Opener::Client { .. }) {
            pair.finish = Some(Finish::Close(relayed(pair.kind, &outcome)));
            self.flush_client(key);
        }
    }

    /// The far end reset the link channel: its reason reaches the client only while no effect can
    /// have started. A run keeps the reason and judges it by its own submission.
    pub(super) fn link_reset(&mut self, carrier: u64, link: u32, reason: ResetReason) {
        let Some(key) = self.relays.of_link(carrier, link) else {
            return;
        };
        let Some(pair) = self.relays.pairs.get_mut(&key) else {
            return;
        };
        if matches!(pair.opener, Opener::Stop { .. }) {
            pair.link_done = true;
            pair.reset = Some(reason);
            return;
        }
        let ending = link_reset_end(pair.kind, pair.accepted, pair.dispatched, reason);
        self.end_pair(key, ending, None);
    }

    /// The carrier `carrier` ended, or is being ended: every pair on it whose link channel was still
    /// open ends by the no-effect rule, and every run on it ends by whether its request was
    /// submitted. A pair whose link already closed keeps its outcome.
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
        self.end_stops_on(carrier);
    }

    /// Applies closing at `now`, closing having begun at `began`, to every pair and every run. A
    /// run's channel may finish within the allowance, as a route may.
    pub(super) fn close_relays(&mut self, began: Instant, now: Instant) {
        for key in self.relays.keys(|pair| !pair.link_done) {
            let Some(pair) = self.relays.pairs.get(&key) else {
                continue;
            };
            let kind = match pair.opener {
                Opener::Client { .. } => pair.kind,
                Opener::Stop { .. } => ChannelKind::Route,
            };
            let ending = closing(kind, pair.accepted, pair.dispatched, began, now);
            if ending != Ending::Keep {
                self.end_pair(key, ending, Some(ResetReason::Stopping));
            }
        }
        if now >= began + ALLOWANCE {
            self.end_stops_closing();
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
            // A run's thread finds its channel over and removes the pair.
            (_, Ending::Keep) | (Opener::Stop { .. }, _) => {}
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
    /// waits behind the bytes; a pair over on both sides leaves the table, and then this is true.
    fn flush_client(&mut self, key: u64) -> bool {
        let Some(Opener::Client { peer, channel }) =
            self.relays.pairs.get(&key).map(|pair| pair.opener)
        else {
            return false;
        };
        if !self.peers.contains_key(&peer) {
            self.cancel_link(key, ResetReason::Cancelled);
            self.relays.remove(key);
            return true;
        }
        let Self {
            relays,
            carriers,
            peers,
            ..
        } = self;
        let (Some(pair), Some(client)) = (relays.pairs.get_mut(&key), peers.get(&peer)) else {
            return false;
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
        finished
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
}

#[cfg(test)]
mod tests;
