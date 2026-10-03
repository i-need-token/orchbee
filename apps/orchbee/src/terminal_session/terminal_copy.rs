//! Bounded complete-text delivery, cancelled before dispatch and never replayed afterwards.
use super::state::PaneRef;
use std::{
    collections::VecDeque,
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};

const QUEUED: u8 = 0;
const DISPATCHED: u8 = 1;
const DELIVERED: u8 = 2;
const FAILED: u8 = 3;
const UNKNOWN: u8 = 4;
const CANCELLED: u8 = 5;
const CANCEL_REQUESTED: u8 = 6;
const CAPACITY: usize = 8;
const BYTE_CAPACITY: usize = orchbee_terminal_protocol::MAXIMUM_SELECTION_EXPORT_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Outcome {
    Delivered,
    Failed,
    Unknown,
    Cancelled,
}

#[derive(Default)]
pub(super) struct Receipt(AtomicU8);
impl Receipt {
    fn cancel(&self) {
        let _ = self.0.compare_exchange(
            QUEUED,
            CANCEL_REQUESTED,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }
    fn outcome(&self) -> Option<Outcome> {
        match self.0.load(Ordering::Acquire) {
            DELIVERED => Some(Outcome::Delivered),
            FAILED => Some(Outcome::Failed),
            UNKNOWN => Some(Outcome::Unknown),
            CANCELLED => Some(Outcome::Cancelled),
            _ => None,
        }
    }
}

pub(super) struct Job {
    text: String,
    receipt: Arc<Receipt>,
}
impl Job {
    pub(super) fn begin(&self) -> bool {
        self.receipt
            .0
            .compare_exchange(QUEUED, DISPATCHED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    pub(super) fn text(&self) -> &str {
        &self.text
    }
    pub(super) fn settle(&self, outcome: Outcome) {
        let state = match outcome {
            Outcome::Delivered => DELIVERED,
            Outcome::Failed => FAILED,
            Outcome::Unknown => UNKNOWN,
            Outcome::Cancelled => CANCELLED,
        };
        let _ =
            self.receipt
                .0
                .compare_exchange(DISPATCHED, state, Ordering::AcqRel, Ordering::Acquire);
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        // Keep the active payload charged until its adapter releases it.
        let _ = self.receipt.0.compare_exchange(
            CANCEL_REQUESTED,
            CANCELLED,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        // A lost adapter before its first OS call is a definite failure. A
        // disappearing adapter after that call began cannot prove delivery.
        let _ =
            self.receipt
                .0
                .compare_exchange(QUEUED, FAILED, Ordering::AcqRel, Ordering::Acquire);
        let _ = self.receipt.0.compare_exchange(
            DISPATCHED,
            UNKNOWN,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }
}

struct Pending {
    key: PaneRef,
    generation: NonZeroU64,
    text: String,
}
struct Active {
    key: PaneRef,
    generation: NonZeroU64,
    bytes: usize,
    receipt: Arc<Receipt>,
}

#[derive(Default)]
pub(super) struct Queue {
    pending: VecDeque<Pending>,
    active: Option<Active>,
    bytes: usize,
}
impl Queue {
    pub(super) fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }
    pub(super) fn enqueue(&mut self, key: PaneRef, generation: NonZeroU64, text: String) -> bool {
        if text.contains('\0')
            || self.pending.len() >= CAPACITY
            || text.len() > BYTE_CAPACITY.saturating_sub(self.bytes)
        {
            return false;
        }
        self.bytes += text.len();
        self.pending.push_back(Pending {
            key,
            generation,
            text,
        });
        true
    }
    pub(super) fn reconcile(&mut self, current: impl Fn(&PaneRef, NonZeroU64) -> bool) {
        self.pending.retain(|pending| {
            if current(&pending.key, pending.generation) {
                true
            } else {
                self.bytes -= pending.text.len();
                false
            }
        });
        if let Some(active) = &self.active
            && !current(&active.key, active.generation)
        {
            active.receipt.cancel();
        }
    }
    pub(super) fn dispatch(&mut self) -> Option<Job> {
        if self.active.is_some() {
            return None;
        }
        let pending = self.pending.pop_front()?;
        let receipt = Arc::new(Receipt::default());
        self.active = Some(Active {
            key: pending.key,
            generation: pending.generation,
            bytes: pending.text.len(),
            receipt: Arc::clone(&receipt),
        });
        Some(Job {
            text: pending.text,
            receipt,
        })
    }
    pub(super) fn take_outcome(&mut self) -> Option<Outcome> {
        let outcome = self.active.as_ref()?.receipt.outcome()?;
        self.bytes -= self.active.take()?.bytes;
        Some(outcome)
    }
    pub(super) fn close(&mut self) {
        self.reconcile(|_, _| false);
    }
    pub(super) fn cancel_pane(&mut self, key: &PaneRef) {
        self.reconcile(|candidate, _| candidate != key);
    }
}

#[cfg(test)]
mod tests;
