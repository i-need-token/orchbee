//! One thread that runs short callbacks at their deadlines: the file engine's call deadlines,
//! idle expiries and watch windows. A callback runs on the timer thread, so it hands work on and
//! never blocks; it may schedule or cancel other callbacks.

use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
    io,
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    thread::{self, JoinHandle},
    time::Instant,
};

type Callback = Box<dyn FnOnce() + Send>;

/// A scheduled callback, which [`Timer::cancel`] withdraws before it runs.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct TimerId(u64);

/// The timer thread and its schedule. Dropping it stops the thread without running what remains.
pub(crate) struct Timer {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

/// The lock guards the schedule; it is never held while a callback runs. A panic ends the
/// process, so a poisoned lock is never observed.
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}

#[derive(Default)]
struct State {
    due: BinaryHeap<Reverse<(Instant, u64)>>,
    callbacks: HashMap<u64, Callback>,
    next: u64,
    stopping: bool,
}

impl Timer {
    /// Starts the timer thread.
    ///
    /// # Errors
    ///
    /// The thread could not start.
    pub(crate) fn start() -> io::Result<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        });
        let thread = thread::Builder::new()
            .name("orchbee-files-timer".into())
            .spawn({
                let shared = Arc::clone(&shared);
                move || shared.run()
            })?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// Runs `callback` on the timer thread at `at`, or at once when `at` has passed.
    pub(crate) fn schedule(
        &self,
        at: Instant,
        callback: impl FnOnce() + Send + 'static,
    ) -> TimerId {
        let mut state = self.shared.lock();
        state.next += 1;
        let id = state.next;
        state.callbacks.insert(id, Box::new(callback));
        state.due.push(Reverse((at, id)));
        self.shared.changed.notify_all();
        TimerId(id)
    }

    /// Withdraws `id`; a callback that already ran or is running is not affected.
    pub(crate) fn cancel(&self, id: TimerId) {
        self.shared.lock().callbacks.remove(&id.0);
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        self.shared.lock().stopping = true;
        self.shared.changed.notify_all();
        // The thread only waits on the condition variable between short callbacks. A callback
        // that held the last handle drops it on the timer thread, which then ends by itself.
        if let Some(thread) = self.thread.take()
            && thread.thread().id() != thread::current().id()
        {
            let _ = thread.join();
        }
    }
}

impl Shared {
    fn run(&self) {
        let mut state = self.lock();
        loop {
            if state.stopping {
                return;
            }
            let now = Instant::now();
            match state.due.peek().copied() {
                Some(Reverse((at, id))) if at <= now => {
                    state.due.pop();
                    if let Some(callback) = state.callbacks.remove(&id) {
                        drop(state);
                        callback();
                        state = self.lock();
                    }
                }
                Some(Reverse((at, _))) => {
                    state = self
                        .changed
                        .wait_timeout(state, at - now)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0;
                }
                None => {
                    state = self
                        .changed
                        .wait(state)
                        .unwrap_or_else(PoisonError::into_inner);
                }
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
