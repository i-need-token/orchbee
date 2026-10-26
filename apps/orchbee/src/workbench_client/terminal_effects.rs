//! Bounded external-effect FIFO, cancelled before dispatch and never replayed afterwards.
use super::state::PaneRef;
use orchbee_terminal_protocol::{AttachmentId, HostEpoch};
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

mod record;
pub(super) use record::Stream;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Kind {
    Clipboard,
    Bell,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Scope {
    #[cfg(windows)]
    Diagnostic,
    #[cfg(windows)]
    PickerIdentifier,
    Selection {
        generation: NonZeroU64,
        export: Option<NonZeroU64>,
        episode: Option<NonZeroU64>,
    },
    Program {
        attachment: AttachmentId,
        epoch: HostEpoch,
    },
}

pub(super) enum Payload {
    Clipboard(String),
    Bell,
}
impl Payload {
    fn bytes(&self) -> usize {
        match self {
            Self::Clipboard(text) => text.len(),
            Self::Bell => 0,
        }
    }
    const fn kind(&self) -> Kind {
        match self {
            Self::Clipboard(_) => Kind::Clipboard,
            Self::Bell => Kind::Bell,
        }
    }
}

fn valid_program_text(text: &str) -> bool {
    text.len() <= orchbee_terminal_protocol::MAXIMUM_CLIPBOARD_TEXT_BYTES
        && text
            .bytes()
            .all(|byte| byte != 0x7f && (byte >= 0x20 || matches!(byte, b'\t' | b'\n' | b'\r')))
}

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
    payload: Payload,
    receipt: Arc<Receipt>,
}
impl Job {
    pub(super) fn cancelled(&self) -> bool {
        self.receipt.0.load(Ordering::Acquire) == CANCEL_REQUESTED
    }
    pub(super) fn begin(&self) -> bool {
        self.receipt
            .0
            .compare_exchange(QUEUED, DISPATCHED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    pub(super) const fn kind(&self) -> Kind {
        self.payload.kind()
    }
    pub(super) fn text(&self) -> Option<&str> {
        match &self.payload {
            Payload::Clipboard(text) => Some(text),
            Payload::Bell => None,
        }
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
    scope: Scope,
    payload: Payload,
}
struct Active {
    key: PaneRef,
    scope: Scope,
    kind: Kind,
    bytes: usize,
    receipt: Arc<Receipt>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Completion {
    pub(super) key: PaneRef,
    pub(super) scope: Scope,
    pub(super) kind: Kind,
    pub(super) outcome: Outcome,
}

#[derive(Default)]
pub(super) struct Queue {
    pending: VecDeque<Pending>,
    active: Option<Active>,
    bytes: usize,
}
impl Queue {
    #[cfg(windows)]
    pub(super) fn enqueue_identifier(&mut self, key: PaneRef, text: String) -> bool {
        self.enqueue(key, Scope::PickerIdentifier, Payload::Clipboard(text))
    }

    #[cfg(windows)]
    pub(super) fn enqueue_diagnostic(&mut self, key: PaneRef, text: String) -> bool {
        self.enqueue(key, Scope::Diagnostic, Payload::Clipboard(text))
    }
    pub(super) fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }
    pub(super) fn enqueue_selection(
        &mut self,
        key: PaneRef,
        generation: NonZeroU64,
        text: String,
    ) -> bool {
        self.enqueue(
            key,
            Scope::Selection {
                generation,
                export: None,
                episode: None,
            },
            Payload::Clipboard(text),
        )
    }
    pub(super) fn enqueue_copy_selection(
        &mut self,
        key: PaneRef,
        generation: NonZeroU64,
        export: NonZeroU64,
        episode: NonZeroU64,
        text: String,
    ) -> bool {
        self.enqueue(
            key,
            Scope::Selection {
                generation,
                export: Some(export),
                episode: Some(episode),
            },
            Payload::Clipboard(text),
        )
    }
    pub(super) fn enqueue_program(
        &mut self,
        key: PaneRef,
        attachment: AttachmentId,
        epoch: HostEpoch,
        payload: Payload,
    ) -> bool {
        if let Payload::Clipboard(text) = &payload
            && !valid_program_text(text)
        {
            return false;
        }
        self.enqueue(key, Scope::Program { attachment, epoch }, payload)
    }
    fn enqueue(&mut self, key: PaneRef, scope: Scope, payload: Payload) -> bool {
        if matches!(&payload, Payload::Clipboard(text) if text.contains('\0'))
            || self.pending.len() >= CAPACITY
            || payload.bytes() > BYTE_CAPACITY.saturating_sub(self.bytes)
        {
            return false;
        }
        self.bytes += payload.bytes();
        self.pending.push_back(Pending {
            key,
            scope,
            payload,
        });
        true
    }
    pub(super) fn reconcile(&mut self, current: impl Fn(&PaneRef, Scope) -> bool) {
        self.pending.retain(|pending| {
            if current(&pending.key, pending.scope) {
                true
            } else {
                self.bytes -= pending.payload.bytes();
                false
            }
        });
        if let Some(active) = &self.active
            && !current(&active.key, active.scope)
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
            scope: pending.scope,
            kind: pending.payload.kind(),
            bytes: pending.payload.bytes(),
            receipt: Arc::clone(&receipt),
        });
        Some(Job {
            payload: pending.payload,
            receipt,
        })
    }
    #[cfg(test)]
    pub(super) fn take_outcome(&mut self) -> Option<(Kind, Outcome)> {
        let completion = self.take_completion()?;
        Some((completion.kind, completion.outcome))
    }
    pub(super) fn take_completion(&mut self) -> Option<Completion> {
        let active = self.active.as_ref()?;
        let outcome = active.receipt.outcome()?;
        let active = self.active.take()?;
        self.bytes -= active.bytes;
        Some(Completion {
            key: active.key,
            scope: active.scope,
            kind: active.kind,
            outcome,
        })
    }
    pub(super) fn close(&mut self) {
        self.reconcile(|_, _| false);
    }
    pub(super) fn cancel_pane(&mut self, key: &PaneRef) {
        self.reconcile(|candidate, _| candidate != key);
    }
    pub(super) fn cancel_selection(&mut self, key: &PaneRef) {
        self.reconcile(|candidate, scope| {
            candidate != key || !matches!(scope, Scope::Selection { .. })
        });
    }
}

#[cfg(test)]
mod tests;
