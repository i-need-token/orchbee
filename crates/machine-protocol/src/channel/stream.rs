//! One channel at one end: its kind, which end opened it, whether the responder accepted it, and
//! the ledgers of both directions — credit granted and `data` received for the peer's direction,
//! credit received and `data` committed for this end's — with the frames waiting behind its data.

use std::collections::VecDeque;

use super::INITIAL_CREDIT;
use crate::{Frame, protocol::KindRules};

/// One open channel, or one whose `close` this end queued behind pending data.
#[derive(Debug)]
pub(super) struct Stream<Kind> {
    pub(super) kind: Kind,
    /// This end opened it.
    pub(super) mine: bool,
    /// The responder sent `accept`.
    pub(super) accepted: bool,
    rules: KindRules,
    /// `data` bytes received from the peer.
    pub(super) received: u64,
    /// Credit granted to the peer: the initial credit and every `credit` sent.
    pub(super) granted: u64,
    /// Delivered bytes the consumer has not taken.
    pub(super) untaken: u64,
    /// Taken bytes not yet regranted.
    pub(super) unregranted: u64,
    /// The peer sent `end`.
    pub(super) peer_ended: bool,
    /// Credit the peer granted: the initial credit and every `credit` received.
    pub(super) peer_granted: u64,
    /// `data` bytes this end committed to send.
    pub(super) committed: u64,
    /// This end queued `end` or `close`.
    pub(super) ended: bool,
    /// This end queued `close`: closed for receiving, still flushing its data.
    pub(super) closing: bool,
    /// `data` frames committed and not yet written.
    pub(super) pending: VecDeque<Frame>,
    /// `end` and `close`, which wait behind the channel's pending data.
    pub(super) after_data: VecDeque<Frame>,
    /// The channel waits in the sender's round.
    pub(super) ready: bool,
}

impl<Kind> Stream<Kind> {
    pub(super) fn new(kind: Kind, mine: bool, rules: KindRules) -> Self {
        Self {
            kind,
            mine,
            accepted: false,
            rules,
            received: 0,
            granted: u64::from(INITIAL_CREDIT),
            untaken: 0,
            unregranted: 0,
            peer_ended: false,
            peer_granted: u64::from(INITIAL_CREDIT),
            committed: 0,
            ended: false,
            closing: false,
            pending: VecDeque::new(),
            after_data: VecDeque::new(),
            ready: false,
        }
    }

    /// The bound of the peer's `data`, `None` when the kind gives the peer's direction none.
    pub(super) const fn peer_bound(&self) -> Option<u64> {
        if self.mine {
            self.rules.responder_data
        } else {
            self.rules.opener_data
        }
    }

    /// The bound of this end's `data`, `None` when the kind gives this end's direction none.
    pub(super) const fn own_bound(&self) -> Option<u64> {
        if self.mine {
            self.rules.opener_data
        } else {
            self.rules.responder_data
        }
    }

    /// Whether the peer is the responder and has not accepted, so it may send only `accept` or
    /// `reset`, and this end, as the opener, receives nothing else yet.
    pub(super) const fn awaits_accept(&self) -> bool {
        self.mine && !self.accepted
    }

    /// Whether this end may send `credit`: a responder only after its `accept`.
    pub(super) const fn may_grant(&self) -> bool {
        self.mine || self.accepted
    }
}
