//! The TUI's credential-store calls, off the interface thread: asking once whether answers can be
//! remembered here, writing an answer kept for a login that succeeded, and forgetting a Machine's
//! remembered answers on the person's request. The answer is moved into the writing thread and
//! wiped when that thread drops it, written or not; a write that fails is reported, without the
//! answer. `apps/orchbee/docs/machines.md#remembered-answers` owns the rules.

use crate::machine_credentials::{
    Deleted, Failure, Item, Remembering, SecretKind, Store, readable_label,
};
use crate::machine_login::Answer;
use crate::machine_ssh;
use crate::ui_wakeup::UiWakeup;
use orchbee_machine_protocol::MachineId;
use std::{
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread,
};

/// How a forget ended, as the picker reports it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Forgotten {
    Removed,
    NothingSaved,
    Failed,
}

/// This TUI's store, what it answered about remembering, its forget in flight, and the writes
/// that failed.
pub(super) struct Credentials {
    store: Store,
    /// Whether answers can be remembered here, once the probe answered.
    remembering: Option<Remembering>,
    probing: Option<Receiver<Remembering>>,
    forgetting: Option<Receiver<Forgotten>>,
    unsaved: (Sender<()>, Receiver<()>),
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
            remembering: None,
            probing: None,
            forgetting: None,
            unsaved: mpsc::channel(),
        }
    }

    /// Whether answers can be remembered here, once known. The first call starts the probe, which
    /// reads one store name no item has and runs `ssh -V`, on its own thread; `wakeup` is woken
    /// when it answers. A probe that cannot start answers unavailable.
    pub(super) fn remembering(&mut self, wakeup: Option<&UiWakeup>) -> Option<Remembering> {
        if self.remembering.is_none() && self.probing.is_none() {
            let (sender, receiver) = mpsc::channel();
            let (store, wakeup) = (self.store, wakeup.cloned());
            let started = thread::Builder::new()
                .name("orchbee-machine-remembering".into())
                .spawn(move || {
                    let _sent = sender.send(machine_ssh::remembering(&store));
                    if let Some(wakeup) = wakeup {
                        wakeup.signal();
                    }
                });
            match started {
                Ok(_) => self.probing = Some(receiver),
                Err(_) => self.remembering = Some(Remembering::Unavailable),
            }
        }
        if let Some(probing) = &self.probing {
            match probing.try_recv() {
                Ok(remembering) => self.remembering = Some(remembering),
                Err(TryRecvError::Disconnected) => {
                    self.remembering = Some(Remembering::Unavailable)
                }
                Err(TryRecvError::Empty) => {}
            }
            if self.remembering.is_some() {
                self.probing = None;
            }
        }
        self.remembering
    }

    /// Whether the probe runs and has not answered yet.
    pub(super) const fn probing(&self) -> bool {
        self.probing.is_some()
    }

    /// Writes `answer` as `machine`'s `kind` answer, labelled for the Machine's `label`; a write
    /// that fails, or cannot start, is reported by [`Self::unsaved`] and wakes `wakeup`.
    pub(super) fn remember(
        &self,
        machine: &str,
        kind: SecretKind,
        answer: Answer,
        label: &str,
        wakeup: Option<UiWakeup>,
    ) {
        let unsaved = self.unsaved.0.clone();
        let Ok(machine) = MachineId::new(machine.to_owned()) else {
            let _sent = unsaved.send(());
            return;
        };
        let store = self.store;
        let label = readable_label(label);
        let failed = unsaved.clone();
        // A thread that cannot start drops the closure, and the answer with it.
        let started = thread::Builder::new()
            .name("orchbee-machine-remember".into())
            .spawn(move || {
                let written = store.write(&Item::new(machine, kind), answer.as_bytes(), &label);
                drop(answer);
                if written.is_err() {
                    let _sent = failed.send(());
                    if let Some(wakeup) = wakeup {
                        wakeup.signal();
                    }
                }
            });
        if started.is_err() {
            let _sent = unsaved.send(());
        }
    }

    /// Whether a write failed since the last call.
    pub(super) fn unsaved(&self) -> bool {
        let mut any = false;
        while self.unsaved.1.try_recv().is_ok() {
            any = true;
        }
        any
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
