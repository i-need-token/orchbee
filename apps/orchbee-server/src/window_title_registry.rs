//! Latest Site title with connection-owned foreground routing and bounded replay.
use orchbee_workbench_protocol::{
    WindowTitleAction as Action, WindowTitleFailure as Failure, WindowTitleOutcome as Outcome,
    WindowTitleReason as Reason, WindowTitleSnapshot, WorkbenchRequestId, WorkbenchRevision,
};
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    time::{Duration, Instant},
};
struct Listener {
    seen: Instant,
    order: u64,
    focused: bool,
}
struct Receipt {
    at: Instant,
    action: Action,
    outcome: Outcome,
}
#[derive(Default)]
pub(crate) struct Registry {
    listeners: BTreeMap<NonZeroU64, Listener>,
    receipts: BTreeMap<WorkbenchRequestId, Receipt>,
    snapshot: Option<WindowTitleSnapshot>,
    order: u64,
}
impl Registry {
    pub(crate) fn disconnect(&mut self, generation: NonZeroU64) {
        self.listeners.remove(&generation);
    }
    fn foreground(&self) -> Option<NonZeroU64> {
        self.listeners
            .iter()
            .max_by_key(|(_, item)| item.order)
            .map(|(id, _)| *id)
    }
    pub(crate) fn execute(
        &mut self,
        generation: NonZeroU64,
        id: WorkbenchRequestId,
        action: Action,
        now: Instant,
    ) -> Outcome {
        self.listeners
            .retain(|_, item| now.saturating_duration_since(item.seen) < Duration::from_secs(5));
        self.receipts
            .retain(|_, item| now.saturating_duration_since(item.at) < Duration::from_secs(60));
        if let Action::Listen { focused } = action {
            let previous = self.listeners.get(&generation);
            if previous.is_none() && self.listeners.len() >= 32 {
                return Outcome::Failed {
                    reason: Failure::Busy,
                };
            }
            let promote =
                previous.is_none() || (focused && previous.is_some_and(|item| !item.focused));
            let order = if promote {
                let Some(next) = self.order.checked_add(1) else {
                    return Outcome::Failed {
                        reason: Failure::Unavailable,
                    };
                };
                self.order = next;
                next
            } else {
                previous.expect("existing listener").order
            };
            self.listeners.insert(
                generation,
                Listener {
                    seen: now,
                    order,
                    focused,
                },
            );
            return Outcome::Listening {
                snapshot: if self.foreground() == Some(generation) {
                    self.snapshot.clone()
                } else {
                    None
                },
            };
        }
        if let Some(receipt) = self.receipts.get(&id) {
            return if receipt.action == action {
                receipt.outcome.clone()
            } else {
                Outcome::Failed {
                    reason: Failure::Conflict,
                }
            };
        }
        if self.receipts.len() >= 256 {
            return Outcome::Failed {
                reason: Failure::Busy,
            };
        }
        let Some(revision) = self
            .snapshot
            .as_ref()
            .map_or(0, |snapshot| snapshot.revision.value())
            .checked_add(1)
        else {
            return Outcome::Failed {
                reason: Failure::Unavailable,
            };
        };
        let title = match &action {
            Action::Set { title } => Some(title.clone()),
            Action::Clear {} => None,
            Action::Listen { .. } => unreachable!(),
        };
        let changed = self.foreground().is_some();
        let reason = if !changed {
            Reason::NoForegroundClient
        } else if title.is_some() {
            Reason::Set
        } else {
            Reason::Cleared
        };
        let snapshot = WindowTitleSnapshot {
            revision: WorkbenchRevision::new(revision),
            title,
        };
        let outcome = Outcome::Published {
            snapshot: snapshot.clone(),
            changed,
            reason,
        };
        self.snapshot = Some(snapshot);
        self.receipts.insert(
            id,
            Receipt {
                at: now,
                action,
                outcome: outcome.clone(),
            },
        );
        outcome
    }
}
#[cfg(test)]
mod tests;
