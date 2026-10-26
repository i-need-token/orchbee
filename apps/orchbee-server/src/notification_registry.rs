//! Connection-owned bounded notification inboxes; no UI, transport, or clock reads here.
use orchbee_workbench_protocol::{
    Notification, NotificationAction, NotificationFailure, NotificationOutcome,
    NotificationReason as Reason, WorkbenchRequestId,
};
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    time::{Duration, Instant},
};
const LIVE: Duration = Duration::from_secs(5);
const RETAIN: Duration = Duration::from_secs(60);
struct Listener {
    accepting: bool,
    pending: Option<(Notification, Instant)>,
}
struct Receipt {
    message: Notification,
    outcome: NotificationOutcome,
    at: Instant,
}
#[derive(Default)]
pub(crate) struct Registry {
    listeners: BTreeMap<NonZeroU64, Listener>,
    receipts: BTreeMap<WorkbenchRequestId, Receipt>,
    last: Option<Instant>,
    pub(crate) disabled: bool,
}
fn outcome(reason: Reason) -> NotificationOutcome {
    NotificationOutcome::Show {
        shown: reason == Reason::Shown,
        reason,
    }
}
impl Registry {
    pub(crate) fn disconnect(&mut self, generation: NonZeroU64) {
        self.listeners.remove(&generation);
    }
    pub(crate) fn execute(
        &mut self,
        generation: NonZeroU64,
        id: WorkbenchRequestId,
        action: NotificationAction,
        now: Instant,
    ) -> NotificationOutcome {
        self.receipts
            .retain(|_, receipt| now.saturating_duration_since(receipt.at) < RETAIN);
        for listener in self.listeners.values_mut() {
            if listener
                .pending
                .as_ref()
                .is_some_and(|(_, at)| now.saturating_duration_since(*at) >= LIVE)
            {
                listener.pending = None;
            }
        }
        match action {
            NotificationAction::Listen { accepting } => {
                if !self.listeners.contains_key(&generation) && self.listeners.len() >= 32 {
                    return NotificationOutcome::Failed {
                        reason: NotificationFailure::Unavailable,
                    };
                }
                let listener = self.listeners.entry(generation).or_insert(Listener {
                    accepting,
                    pending: None,
                });
                listener.accepting = accepting;
                NotificationOutcome::Listening {
                    notification: if accepting {
                        listener.pending.take().map(|(message, _)| message)
                    } else {
                        None
                    },
                }
            }
            NotificationAction::Show { notification } => {
                if let Some(receipt) = self.receipts.get(&id) {
                    return if receipt.message == notification {
                        receipt.outcome.clone()
                    } else {
                        NotificationOutcome::Failed {
                            reason: NotificationFailure::Conflict,
                        }
                    };
                }
                if self.receipts.len() >= 256 {
                    return outcome(Reason::Busy);
                }
                let reason = if self.listeners.is_empty() {
                    if self.disabled {
                        Reason::Disabled
                    } else {
                        Reason::NoForegroundClient
                    }
                } else if self.last.is_some_and(|last| {
                    now.saturating_duration_since(last) < Duration::from_secs(1)
                }) {
                    Reason::RateLimited
                } else if self
                    .listeners
                    .values()
                    .all(|l| !l.accepting || l.pending.is_some())
                {
                    Reason::Busy
                } else {
                    for listener in self
                        .listeners
                        .values_mut()
                        .filter(|l| l.accepting && l.pending.is_none())
                    {
                        listener.pending = Some((notification.clone(), now));
                    }
                    self.last = Some(now);
                    Reason::Shown
                };
                let outcome = outcome(reason);
                self.receipts.insert(
                    id,
                    Receipt {
                        message: notification,
                        outcome: outcome.clone(),
                        at: now,
                    },
                );
                outcome
            }
        }
    }
}
#[cfg(test)]
mod tests;
