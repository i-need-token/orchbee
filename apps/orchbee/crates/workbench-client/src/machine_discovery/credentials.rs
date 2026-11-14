//! The TUI's credential-store calls, off the interface thread: writing an answer kept for a login
//! that succeeded, and forgetting a Machine's remembered answers on the person's request. The
//! answer is moved into the writing thread and wiped when that thread drops it, written or not.
//! `apps/orchbee/docs/machines.md#remembered-answers` owns the rules.

use crate::machine_credentials::{
    Deleted, Failure, Item, Remembering, SecretKind, Store, readable_label,
};
use crate::machine_login::Answer;
use crate::ui_wakeup::UiWakeup;
use orchbee_machine_protocol::MachineId;
use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
};

/// How a forget ended, as the picker reports it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Forgotten {
    Removed,
    NothingSaved,
    Failed,
}

/// This TUI's store and its forget in flight.
pub(super) struct Credentials {
    store: Store,
    forgetting: Option<Receiver<Forgotten>>,
}

impl Default for Credentials {
    fn default() -> Self {
        Self::new()
    }
}

impl Credentials {
    pub(super) fn new() -> Self {
        // A unit test never reaches the person's real store.
        #[cfg(not(test))]
        let store = Store::select();
        #[cfg(test)]
        let store = Store::unavailable();
        Self {
            store,
            forgetting: None,
        }
    }

    /// How long the store keeps an answer in this login; it reads one name no item has.
    pub(super) fn remembering(&self) -> Remembering {
        self.store.remembering()
    }

    /// Writes `answer` as `machine`'s `kind` answer, labelled for the Machine's `label`.
    pub(super) fn remember(&self, machine: &str, kind: SecretKind, answer: Answer, label: &str) {
        let Ok(machine) = MachineId::new(machine.to_owned()) else {
            return;
        };
        let store = self.store;
        let label = readable_label(label);
        // A thread that cannot start drops the closure, and the answer with it.
        let _started = thread::Builder::new()
            .name("orchbee-machine-remember".into())
            .spawn(move || {
                let _written = store.write(&Item::new(machine, kind), answer.as_bytes(), &label);
                drop(answer);
            });
    }

    /// Deletes both of `machine`'s remembered answers; [`Self::forgotten`] reports how it went.
    /// A forget already in flight lets this one go.
    pub(super) fn forget(&mut self, machine: &str, wakeup: Option<UiWakeup>) -> bool {
        let Ok(machine) = MachineId::new(machine.to_owned()) else {
            return false;
        };
        if self.forgetting.is_some() {
            return false;
        }
        let (sender, receiver) = mpsc::channel();
        let store = self.store;
        let started = thread::Builder::new()
            .name("orchbee-machine-forget".into())
            .spawn(move || {
                let outcome = match store.forget(&machine) {
                    Ok(Deleted::Removed) => Forgotten::Removed,
                    Ok(Deleted::Absent) => Forgotten::NothingSaved,
                    Err(Failure::Unavailable | Failure::Failed) => Forgotten::Failed,
                };
                let _sent = sender.send(outcome);
                if let Some(wakeup) = wakeup {
                    wakeup.signal();
                }
            });
        if started.is_ok() {
            self.forgetting = Some(receiver);
        }
        started.is_ok()
    }

    /// How the forget in flight ended, once it has.
    pub(super) fn forgotten(&mut self) -> Option<Forgotten> {
        let outcome = match self.forgetting.as_ref()?.try_recv() {
            Ok(outcome) => outcome,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Forgotten::Failed,
        };
        self.forgetting = None;
        Some(outcome)
    }
}
