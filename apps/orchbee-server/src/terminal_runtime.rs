//! The Session's terminal-runtime state: whether a Worker serves its terminals, the Server is
//! replacing a lost one, or it gave up. The Worker replacement sets it; the
//! `workbench.terminal-runtime` read answers from it and its State streams follow its revisions.
//!
//! The state starts `Serving` at revision 1, and the revision advances by one on each change, so a
//! reader that saw a revision knows whether anything happened since. Setting the state it already
//! holds changes nothing. Watchers are woken after the lock is released and are held weakly, so a
//! closed Client connection leaves nothing behind and a change never waits on one.
use orchbee_server::{SessionChangeSignal, SessionChangeSource};
use std::{
    num::NonZeroU64,
    sync::{Arc, Mutex},
};

/// What the Session's terminals can do now.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalRuntimeState {
    /// A Worker serves the terminals.
    Serving,
    /// The Worker was lost and the Server is replacing it; the programs keep running in the Host.
    Reconnecting,
    /// The Server gave up replacing the Worker until the Session stops; the programs keep running
    /// but no Client can reach them.
    GivenUp,
}

/// The state and the revision that names it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerminalRuntimeSnapshot {
    pub(crate) state: TerminalRuntimeState,
    pub(crate) revision: NonZeroU64,
}

/// Lock order: a leaf; no other lock is taken while it is held, and watchers are woken after it is
/// released.
#[derive(Debug)]
pub(crate) struct TerminalRuntime {
    current: Mutex<TerminalRuntimeSnapshot>,
    watchers: SessionChangeSource,
}

impl Default for TerminalRuntime {
    fn default() -> Self {
        Self {
            current: Mutex::new(TerminalRuntimeSnapshot {
                state: TerminalRuntimeState::Serving,
                revision: NonZeroU64::MIN,
            }),
            watchers: SessionChangeSource::default(),
        }
    }
}

impl TerminalRuntime {
    /// Records `state`; when it differs from the current one, the revision advances and every
    /// watcher wakes.
    pub(crate) fn set(&self, state: TerminalRuntimeState) {
        let mut current = self.lock();
        if current.state == state {
            return;
        }
        current.state = state;
        current.revision = current.revision.saturating_add(1);
        drop(current);
        self.watchers.notify_terminal_runtime();
    }

    /// The state as it stands.
    pub(crate) fn current(&self) -> TerminalRuntimeSnapshot {
        *self.lock()
    }

    /// Wakes `signal` after every later revision, so a State stream rereads the state then.
    pub(crate) fn watch(&self, signal: &Arc<SessionChangeSignal>) {
        self.watchers.watch(signal);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, TerminalRuntimeSnapshot> {
        self.current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests;
