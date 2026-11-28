//! One thread that runs short callbacks at their deadlines, such as the idle expiries of the file
//! engine and of Quick Open's file lists. It starts with the first callback scheduled. A callback
//! runs on the timer thread, so it hands work on and never blocks; it may schedule other callbacks.

use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
    io,
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    thread::{self, JoinHandle},
    time::Instant,
};

type Callback = Box<dyn FnOnce() + Send>;

/// The timer thread and its schedule. Dropping it stops the thread without running what remains.
pub(crate) struct Timer {
    shared: Arc<Shared>,
    /// Absent until the first callback is scheduled.
    thread: Mutex<Option<JoinHandle<()>>>,
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
    /// A timer whose thread has not started.
    pub(crate) fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State::default()),
                changed: Condvar::new(),
            }),
            thread: Mutex::new(None),
        }
    }

    /// Runs `callback` on the timer thread at `at`, or at once when `at` has passed.
    ///
    /// # Errors
    ///
    /// The timer thread could not start, so `callback` never runs.
    pub(crate) fn schedule(
        &self,
        at: Instant,
        callback: impl FnOnce() + Send + 'static,
    ) -> io::Result<()> {
        self.start()?;
        let mut state = self.shared.lock();
        state.next += 1;
        let id = state.next;
        state.callbacks.insert(id, Box::new(callback));
        state.due.push(Reverse((at, id)));
        self.shared.changed.notify_all();
        Ok(())
    }

    fn start(&self) -> io::Result<()> {
        let mut thread = self.thread.lock().unwrap_or_else(PoisonError::into_inner);
        if thread.is_none() {
            let shared = Arc::clone(&self.shared);
            *thread = Some(
                thread::Builder::new()
                    .name("orchbee-files-timer".into())
                    .spawn(move || shared.run())?,
            );
        }
        Ok(())
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        self.shared.lock().stopping = true;
        self.shared.changed.notify_all();
        // The thread only waits on the condition variable between short callbacks. A callback
        // that held the last handle drops it on the timer thread, which then ends by itself.
        let thread = self
            .thread
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(thread) = thread.take()
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
