//! Commit invalidation fanout. A listener owns its wait; publishers never wait for delivery.
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::Duration;

#[derive(Debug, Default)]
pub struct SessionChangeSource(Mutex<Vec<Weak<SessionChangeSignal>>>);
impl SessionChangeSource {
    pub fn watch(&self, signal: &Arc<SessionChangeSignal>) {
        let mut listeners = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        listeners.retain(|listener| listener.strong_count() != 0);
        listeners.push(Arc::downgrade(signal));
    }
    pub fn notify(&self) {
        let listeners = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for listener in listeners.iter().filter_map(Weak::upgrade) {
            listener.notify();
        }
    }
}

#[derive(Debug, Default)]
pub struct SessionChangeSignal {
    state: Mutex<SignalState>,
    changed: Condvar,
}
#[derive(Debug, Default)]
struct SignalState {
    revision: u64,
    wake: u64,
}
impl SessionChangeSignal {
    pub fn notify(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.revision = state.revision.wrapping_add(1);
        state.wake = state.wake.wrapping_add(1);
        self.changed.notify_all();
    }
    /// Carrier mailbox activity is a wakeup, never a semantic publication.
    pub fn wake(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.wake = state.wake.wrapping_add(1);
        self.changed.notify_all();
    }
    pub fn revision(&self) -> u64 {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .revision
    }
    pub fn wake_revision(&self) -> u64 {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .wake
    }
    pub fn wait(&self, observed: u64, sampling: Option<Duration>) {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(duration) = sampling {
            drop(
                self.changed
                    .wait_timeout_while(state, duration, |current| current.wake == observed),
            );
        } else {
            drop(
                self.changed
                    .wait_while(state, |current| current.wake == observed),
            );
        }
    }
}
