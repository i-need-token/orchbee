//! The helper threads that run filesystem calls the operating system cannot bound, such as on a
//! network volume. Every submitted job answers exactly once and within its deadline: with its
//! result, or past the deadline as timed out, after which its helper is abandoned and the result
//! it returns late is dropped. While 8 abandoned helpers are outstanding, new work is refused as a
//! stuck filesystem until one returns.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    thread,
    time::{Duration, Instant},
};

use super::timer::{Timer, TimerId};

/// How long a job may take, from its submission, before it answers as timed out.
const DEADLINE: Duration = Duration::from_secs(30);
/// Abandoned helpers beyond which new work is refused.
const MAXIMUM_ABANDONED: usize = 8;
/// Helpers that run jobs at once; more jobs wait in order.
const MAXIMUM_HELPERS: usize = 8;
/// Jobs that wait for a helper; more are refused as overloaded.
const MAXIMUM_WAITING: usize = 1024;

/// Why a job answered without its result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Interrupted {
    /// The job passed its deadline.
    Timeout,
    /// Too many helpers are abandoned to accept work.
    StuckFilesystem,
    /// Too many jobs wait, or no helper could start.
    Overloaded,
    /// The engine is stopping.
    Stopping,
}

type Work = Box<dyn FnOnce() + Send>;
type Expire = Box<dyn FnOnce(Interrupted) + Send>;

/// The helper pool. Dropping it stops admission; helpers finish their current job and end.
pub(crate) struct Blocking {
    shared: Arc<Shared>,
}

/// The lock guards the pool's counts, its waiting jobs and the jobs being run; it is never held
/// while a job or an answer runs. A panic ends the process, so a poisoned lock is never observed.
struct Shared {
    timer: Arc<Timer>,
    state: Mutex<State>,
    work: Condvar,
}

#[derive(Default)]
struct State {
    waiting: VecDeque<Job>,
    /// The jobs a helper runs, with the deadline that would abandon it.
    running: HashMap<u64, Running>,
    helpers: usize,
    idle: usize,
    abandoned: usize,
    next: u64,
    stopping: bool,
}

struct Job {
    id: u64,
    work: Work,
    expire: Expire,
    deadline: TimerId,
}

struct Running {
    expire: Expire,
    deadline: TimerId,
}

impl Blocking {
    /// A pool whose deadlines `timer` keeps.
    pub(crate) fn new(timer: Arc<Timer>) -> Self {
        Self {
            shared: Arc::new(Shared {
                timer,
                state: Mutex::new(State::default()),
                work: Condvar::new(),
            }),
        }
    }

    /// Runs `work` on a helper and passes its result to `done`, or passes why it has none.
    /// `done` runs exactly once: on the helper, on the timer thread at the deadline, or here
    /// when the job is refused.
    pub(crate) fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
        done: impl FnOnce(Result<T, Interrupted>) + Send + 'static,
    ) {
        self.run_within(DEADLINE, work, done);
    }

    fn run_within<T: Send + 'static>(
        &self,
        deadline: Duration,
        work: impl FnOnce() -> T + Send + 'static,
        done: impl FnOnce(Result<T, Interrupted>) + Send + 'static,
    ) {
        let answer = Arc::new(Mutex::new(Some(done)));
        let expire: Expire = Box::new({
            let answer = Arc::clone(&answer);
            move |why| take(&answer).map_or((), |done| done(Err(why)))
        });
        let work: Work = Box::new(move || {
            let result = work();
            if let Some(done) = take(&answer) {
                done(Ok(result));
            }
        });
        let mut state = self.shared.lock();
        let refusal = if state.stopping {
            Some(Interrupted::Stopping)
        } else if state.abandoned >= MAXIMUM_ABANDONED {
            Some(Interrupted::StuckFilesystem)
        } else if state.waiting.len() >= MAXIMUM_WAITING {
            Some(Interrupted::Overloaded)
        } else {
            None
        };
        if let Some(why) = refusal {
            drop(state);
            expire(why);
            return;
        }
        state.next += 1;
        let id = state.next;
        let deadline = self.shared.timer.schedule(Instant::now() + deadline, {
            let shared = Arc::downgrade(&self.shared);
            move || {
                if let Some(shared) = shared.upgrade() {
                    shared.expire(id);
                }
            }
        });
        state.waiting.push_back(Job {
            id,
            work,
            expire,
            deadline,
        });
        if state.idle > 0 {
            self.shared.work.notify_one();
        } else if state.helpers < MAXIMUM_HELPERS {
            Shared::spawn(&self.shared, &mut state);
        }
    }
}

impl Drop for Blocking {
    fn drop(&mut self) {
        let waiting = {
            let mut state = self.shared.lock();
            state.stopping = true;
            std::mem::take(&mut state.waiting)
        };
        self.shared.work.notify_all();
        for job in waiting {
            self.shared.timer.cancel(job.deadline);
            (job.expire)(Interrupted::Stopping);
        }
    }
}

impl Shared {
    /// Starts one more helper. A pool without a running helper refuses what waits, since nothing
    /// would take it before its deadline.
    fn spawn(shared: &Arc<Self>, state: &mut MutexGuard<'_, State>) {
        let started = thread::Builder::new()
            .name("orchbee-files-fs".into())
            .spawn({
                let shared = Arc::clone(shared);
                move || shared.serve()
            });
        match started {
            Ok(_) => state.helpers += 1,
            Err(_) if state.helpers == 0 => {
                let refused: Vec<Job> = state.waiting.drain(..).collect();
                for job in refused {
                    shared.timer.cancel(job.deadline);
                    let expire = job.expire;
                    // Answered off the caller's lock by the timer thread, so no answer runs
                    // while the pool is locked.
                    shared
                        .timer
                        .schedule(Instant::now(), move || expire(Interrupted::Overloaded));
                }
            }
            Err(_) => {}
        }
    }

    fn serve(self: Arc<Self>) {
        let mut state = self.lock();
        loop {
            let Some(job) = state.waiting.pop_front() else {
                if state.stopping {
                    state.helpers -= 1;
                    return;
                }
                state.idle += 1;
                state = self
                    .work
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
                state.idle -= 1;
                continue;
            };
            state.running.insert(
                job.id,
                Running {
                    expire: job.expire,
                    deadline: job.deadline,
                },
            );
            drop(state);
            (job.work)();
            state = self.lock();
            match state.running.remove(&job.id) {
                Some(running) => self.timer.cancel(running.deadline),
                None => {
                    // The deadline abandoned this helper and already counted it out.
                    state.abandoned -= 1;
                    return;
                }
            }
        }
    }

    /// Job `id` reached its deadline: a waiting job leaves the queue, and a running job's helper
    /// is abandoned, replaced while work waits.
    fn expire(self: &Arc<Self>, id: u64) {
        let mut state = self.lock();
        let expire = if let Some(position) = state.waiting.iter().position(|job| job.id == id) {
            state.waiting.remove(position).map(|job| job.expire)
        } else if let Some(running) = state.running.remove(&id) {
            state.helpers -= 1;
            state.abandoned += 1;
            if !state.waiting.is_empty() && state.idle == 0 && !state.stopping {
                Self::spawn(self, &mut state);
            }
            Some(running.expire)
        } else {
            None
        };
        drop(state);
        if let Some(expire) = expire {
            expire(Interrupted::Timeout);
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn take<T>(slot: &Mutex<Option<T>>) -> Option<T> {
    slot.lock().unwrap_or_else(PoisonError::into_inner).take()
}

#[cfg(test)]
mod tests;
