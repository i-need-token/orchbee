//! The file engine's filesystem work, run through the process's [`Helpers`] without waiting. Each
//! job waits for its helper on a thread of its own and hands the outcome on from there, so a
//! caller on the link's reader never blocks, while the deadline and the bound on abandoned helpers
//! stay the process's own.

use std::{sync::mpsc, thread};

use orchbee_files_protocol::{ErrorCode, FilesError, Operation};

use crate::blocking::Helpers;

/// Runs `work` for `operation` on a helper and hands `done` the outcome [`Helpers::run`] returns,
/// or `failed` when no thread can start to wait for it.
pub(crate) fn run<T, W, D>(helpers: &Helpers, operation: Operation, work: W, done: D)
where
    T: Send + 'static,
    W: FnOnce() -> T + Send + 'static,
    D: FnOnce(Result<T, FilesError>) + Send + 'static,
{
    let helpers = helpers.clone();
    let (handover, received) = mpsc::sync_channel::<(W, D)>(1);
    let started = thread::Builder::new()
        .name("orchbee-files-call".into())
        .spawn(move || {
            if let Ok((work, done)) = received.recv() {
                done(helpers.run(operation, work));
            }
        });
    let job = (work, done);
    let unsent = match started {
        Ok(_) => handover.send(job).err().map(|unsent| unsent.0),
        Err(_) => Some(job),
    };
    if let Some((_, done)) = unsent {
        done(Err(FilesError::new(ErrorCode::Failed)));
    }
}
