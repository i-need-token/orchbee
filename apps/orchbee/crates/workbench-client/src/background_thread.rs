//! Ownership of one Client background thread under the exit contract that the Worker's background
//! jobs follow: a stop request never waits, a deadline wait joins only a thread that has already
//! ended, and dropping an owner never blocks.

use std::{
    io,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Process-exit bound on Client background cleanup. It is the longest deadline that a background
/// owner can still be inside after it observes a stop request: the plugin overlay read, the
/// configured-command run, and the Windows system-notification delivery each allow ten seconds.
/// Longer operation deadlines, such as Agent start, machine discovery, and the independent popup,
/// end at the stop request instead of running to their deadline.
pub const EXIT_BUDGET: Duration = Duration::from_secs(10);

/// Interval at which a deadline wait observes an unfinished thread again.
const OBSERVE_INTERVAL: Duration = Duration::from_millis(10);

/// One owned background thread. Dropping it joins a thread that has already ended and detaches a
/// running one, which then ends on its own.
#[derive(Debug)]
pub struct BackgroundThread<T = ()> {
    handle: Option<JoinHandle<T>>,
}

impl<T: Send + 'static> BackgroundThread<T> {
    /// Starts a named thread.
    ///
    /// # Errors
    /// Returns the operating system's thread-creation failure.
    pub fn spawn(name: &str, body: impl FnOnce() -> T + Send + 'static) -> io::Result<Self> {
        thread::Builder::new()
            .name(name.to_owned())
            .spawn(body)
            .map(|handle| Self {
                handle: Some(handle),
            })
    }
}

impl<T> BackgroundThread<T> {
    /// An owner whose thread has already been joined or never started.
    #[must_use]
    pub const fn ended() -> Self {
        Self { handle: None }
    }

    /// Adopts a thread that a test fixture started.
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) const fn adopt(handle: JoinHandle<T>) -> Self {
        Self {
            handle: Some(handle),
        }
    }

    /// Whether no thread remains running. Never waits.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.handle.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Joins the thread without a bound. Only reconnection and retirement, which must recover a
    /// result the thread returns, and owner closes outside process exit use it. `None` means no
    /// thread remained.
    pub fn join(&mut self) -> Option<thread::Result<T>> {
        self.handle.take().map(JoinHandle::join)
    }

    /// Observes the thread's end before `deadline` and joins it only after it has ended.
    ///
    /// # Errors
    /// `TimedOut` when the thread is still running at the deadline; the thread stays owned and is
    /// detached when the owner drops. A thread that panicked reports a failure.
    pub fn join_before(&mut self, deadline: Instant) -> io::Result<()> {
        loop {
            if self.is_finished() {
                return match self.join() {
                    Some(Err(_)) => Err(io::Error::other("Client background thread panicked")),
                    Some(Ok(_)) | None => Ok(()),
                };
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            thread::sleep(remaining.min(OBSERVE_INTERVAL));
        }
    }
}

impl<T> Default for BackgroundThread<T> {
    fn default() -> Self {
        Self::ended()
    }
}

impl<T> Drop for BackgroundThread<T> {
    fn drop(&mut self) {
        if self.is_finished() {
            let _ended = self.join();
        }
    }
}

#[cfg(test)]
mod tests;
