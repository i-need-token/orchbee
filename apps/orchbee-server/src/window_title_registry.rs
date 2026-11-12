//! Session title policy rendered for a connection-owned foreground display context.
use orchbee_local_runtime::WindowTitleTemplate;
use orchbee_workbench_protocol::{WindowTitle, WindowTitleContext};
use orchbee_workbench_protocol::{
    WindowTitleAction as Action, WindowTitleFailure as Failure, WindowTitleOutcome as Outcome,
    WindowTitleReason as Reason, WindowTitleSnapshot, WorkbenchRequestId, WorkbenchRevision,
};
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    time::{Duration, Instant},
};
mod host_name;
struct Listener {
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
    profile: Option<String>,
    template: Option<WindowTitleTemplate>,
    hostname: String,
    delivery: Option<(NonZeroU64, WindowTitleSnapshot)>,
    delivery_revision: u64,
    clear_pending: bool,
    clear_delivery: Option<NonZeroU64>,
}
impl Registry {
    pub(crate) fn configure(&mut self, profile: &str) {
        if self.profile.as_deref() == Some(profile) {
            return;
        }
        self.profile = Some(profile.into());
        self.clear_delivery = None;
        self.template = WindowTitleTemplate::parse(profile).ok().flatten();
        self.hostname = host_name::host_name().unwrap_or_default();
    }
    fn rendered(&mut self, generation: NonZeroU64, context: &WindowTitleContext) -> Outcome {
        if self.foreground() != Some(generation) {
            return Outcome::Listening { snapshot: None };
        }
        let title = if let Some(title) = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.title.as_ref())
        {
            Some(title.clone())
        } else if let Some(template) = &self.template {
            template
                .render([
                    &self.hostname,
                    &context.workspace,
                    &context.tab,
                    &context.panel,
                    &context.terminal_title,
                ])
                .and_then(|text| WindowTitle::normalize(&text).ok())
        } else if self.clear_pending || self.clear_delivery == Some(generation) {
            self.clear_delivery = Some(generation);
            None
        } else {
            return Outcome::Listening { snapshot: None };
        };
        if self
            .delivery
            .as_ref()
            .is_none_or(|(previous, snapshot)| *previous != generation || snapshot.title != title)
        {
            let Some(revision) = self.delivery_revision.checked_add(1) else {
                return Outcome::Failed {
                    reason: Failure::Unavailable,
                };
            };
            self.delivery_revision = revision;
            self.delivery = Some((
                generation,
                WindowTitleSnapshot {
                    revision: WorkbenchRevision::new(revision),
                    title,
                },
            ));
        }
        self.clear_pending = false;
        Outcome::Listening {
            snapshot: self.delivery.as_ref().map(|(_, snapshot)| snapshot.clone()),
        }
    }

    /// Forgets a connection's listening and says whether it was in front.
    pub(crate) fn disconnect(&mut self, generation: NonZeroU64) -> bool {
        let foreground = self.foreground() == Some(generation);
        self.listeners.remove(&generation);
        foreground
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
        self.receipts
            .retain(|_, item| now.saturating_duration_since(item.at) < Duration::from_secs(60));
        if let Action::Listen { focused, context } = &action {
            let focused = *focused;
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
            self.listeners
                .insert(generation, Listener { order, focused });
            return self.rendered(generation, context);
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
        self.clear_pending = snapshot.title.is_none();
        self.clear_delivery = None;
        self.delivery = None;
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
