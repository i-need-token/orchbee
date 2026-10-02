//! Bounded owned image obligations survive backend drop and partial frame output.
use super::ModeLease;
use std::{collections::BTreeSet, sync::Mutex};

const MAXIMUM_OWNED_IMAGES: usize = 128;

#[derive(Debug)]
pub(crate) struct ImageLease {
    pub(super) obligation: ModeLease,
    state: Mutex<State>,
}
#[derive(Debug)]
struct State {
    ids: BTreeSet<u32>,
    unconfirmed_control: bool,
}
impl ImageLease {
    pub(crate) const fn new() -> Self {
        Self {
            obligation: ModeLease::new(),
            state: Mutex::new(State {
                ids: BTreeSet::new(),
                unconfirmed_control: false,
            }),
        }
    }
    pub(crate) fn capacity(&self) -> usize {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        MAXIMUM_OWNED_IMAGES.saturating_sub(state.ids.len())
    }
    pub(crate) fn register(&self, id: u32) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if id == 0 || state.ids.len() >= MAXIMUM_OWNED_IMAGES || state.ids.contains(&id) {
            return false;
        }
        // The obligation precedes the first possible output, including buffered output.
        self.obligation.mark_operation_started();
        state.ids.insert(id);
        true
    }
    pub(crate) fn control_started(&self) {
        self.obligation.mark_operation_started();
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .unconfirmed_control = true;
    }
    pub(crate) fn flushed(&self, deleted: &[u32]) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        for id in deleted {
            state.ids.remove(id);
        }
        state.unconfirmed_control = false;
        if state.ids.is_empty() {
            self.obligation.clear_after_success();
        }
    }
    /// Snapshot before writing: a panic-restoration writer never holds this mutex.
    pub(super) fn cleanup_sequence(&self) -> String {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let mut sequence = String::new();
        if state.unconfirmed_control || !state.ids.is_empty() {
            sequence.push_str("\x18\x1b\\");
        }
        for id in &state.ids {
            sequence.push_str(&delete(*id));
        }
        sequence
    }
    pub(super) fn restored(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.ids.clear();
        state.unconfirmed_control = false;
    }
}
pub(crate) fn delete(id: u32) -> String {
    format!("\x1b_Ga=d,d=I,i={id},q=2;\x1b\\")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deletion_is_owned_bounded_and_unsettled_until_the_flush() {
        let lease = ImageLease::new();
        assert!(!lease.register(0));
        for id in 1..=128 {
            assert!(lease.register(id));
        }
        assert!(!lease.register(129));
        assert!(!lease.register(1));
        lease.control_started();
        let before = lease.cleanup_sequence();
        assert!(before.starts_with("\x18\x1b\\"));
        assert!(before.contains(&delete(1)));
        assert!(!before.contains("d=A"));
        assert_eq!(lease.capacity(), 0);
        lease.flushed(&[1]);
        assert_eq!(lease.capacity(), 1);
        assert!(!lease.cleanup_sequence().contains(&delete(1)));
        assert!(lease.obligation.restoration_required());
        lease.flushed(&(2..=128).collect::<Vec<_>>());
        assert_eq!(lease.cleanup_sequence(), "");
        assert!(!lease.obligation.restoration_required());
    }
}
