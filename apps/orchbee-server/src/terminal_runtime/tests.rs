use super::{TerminalRuntime, TerminalRuntimeState};
use orchbee_server::SessionChangeSignal;
use std::sync::Arc;

/// A State stream resends only when the revision moved, so every change must move it and a repeat
/// must not, and a watching stream must be woken by the change.
#[test]
fn each_change_advances_the_revision_and_wakes_watchers_and_a_repeat_does_neither() {
    let runtime = TerminalRuntime::default();
    let signal = Arc::new(SessionChangeSignal::default());
    runtime.watch(&signal);
    let first = runtime.current();
    assert_eq!(first.state, TerminalRuntimeState::Serving);
    assert_eq!(first.revision.get(), 1);

    runtime.set(TerminalRuntimeState::Serving);
    assert_eq!(runtime.current(), first);
    assert_eq!(signal.terminal_runtime_revision(), 0);

    let mut revision = first.revision;
    for state in [
        TerminalRuntimeState::Reconnecting,
        TerminalRuntimeState::Serving,
        TerminalRuntimeState::Reconnecting,
        TerminalRuntimeState::GivenUp,
    ] {
        let woken = signal.terminal_runtime_revision();
        runtime.set(state);
        let current = runtime.current();
        assert_eq!(current.state, state);
        assert!(current.revision > revision);
        assert_ne!(signal.terminal_runtime_revision(), woken);
        revision = current.revision;
    }
}
