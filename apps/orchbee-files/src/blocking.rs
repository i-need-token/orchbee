//! Filesystem calls that can hang, such as on an unreachable network volume. Each runs on a helper
//! thread that its operation waits on for at most 30 seconds; past that the operation answers
//! `timeout` and abandons the helper, whose late result is dropped. While 8 abandoned helpers are
//! still blocked, new filesystem work answers `unavailable` with reason `stuck-filesystem` instead
//! of piling up threads behind the same stuck volume.

use std::{
    sync::{
        Arc, Condvar, Mutex, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

use orchbee_files_protocol::{ErrorCode, FilesError, Operation, Token};
use orchbee_local_runtime::{LogLevel, LogValue, log_event};

const DEADLINE: Duration = Duration::from_secs(30);
const MAXIMUM_ABANDONED: usize = 8;

/// The helpers of every operation in the process.
pub(crate) struct Helpers {
    deadline: Duration,
    /// Helpers whose operation stopped waiting and that have not returned yet.
    abandoned: Arc<AtomicUsize>,
}

/// One helper's result, as its operation and the helper hand it over.
enum Slot<T> {
    Waiting,
    Done(T),
    Abandoned,
}

impl Helpers {
    pub(crate) fn new() -> Self {
        Self {
            deadline: DEADLINE,
            abandoned: Arc::new(AtomicUsize::new(0)),
        }
    }

    #[cfg(test)]
    fn with_deadline(deadline: Duration) -> Self {
        Self {
            deadline,
            abandoned: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Runs `work` for `operation` on a helper thread and waits for its result until the deadline.
    ///
    /// # Errors
    ///
    /// `timeout` past the deadline, `unavailable` with reason `stuck-filesystem` while too many
    /// helpers are abandoned, and `failed` when no helper thread can start.
    pub(crate) fn run<T: Send + 'static>(
        &self,
        operation: Operation,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, FilesError> {
        if self.abandoned.load(Ordering::Acquire) >= MAXIMUM_ABANDONED {
            let reason = Token::new("stuck-filesystem").expect("the reason is a token");
            return Err(FilesError::unavailable(reason));
        }
        let handover = Arc::new((Mutex::new(Slot::Waiting), Condvar::new()));
        let helper = Arc::clone(&handover);
        let abandoned = Arc::clone(&self.abandoned);
        thread::Builder::new()
            .name("orchbee-files-helper".into())
            .spawn(move || {
                let result = work();
                let (slot, done) = &*helper;
                let mut slot = slot.lock().unwrap_or_else(PoisonError::into_inner);
                if matches!(*slot, Slot::Abandoned) {
                    abandoned.fetch_sub(1, Ordering::AcqRel);
                } else {
                    *slot = Slot::Done(result);
                    done.notify_one();
                }
            })
            .map_err(|_| FilesError::new(ErrorCode::Failed))?;
        let (slot, done) = &*handover;
        let (mut slot, _) = done
            .wait_timeout_while(
                slot.lock().unwrap_or_else(PoisonError::into_inner),
                self.deadline,
                |slot| matches!(slot, Slot::Waiting),
            )
            .unwrap_or_else(PoisonError::into_inner);
        match std::mem::replace(&mut *slot, Slot::Abandoned) {
            Slot::Done(result) => Ok(result),
            Slot::Waiting | Slot::Abandoned => {
                self.abandoned.fetch_add(1, Ordering::AcqRel);
                log_event(
                    LogLevel::Warning,
                    "filesystem-timeout",
                    &[("operation", LogValue::Token(operation.name()))],
                );
                Err(FilesError::new(ErrorCode::Timeout))
            }
        }
    }
}

#[cfg(test)]
mod tests;
