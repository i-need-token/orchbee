//! Retains an already-started byte relay through Client origins and derived lanes. Creation of
//! endpoints, processes and relay work belongs to the platform adapter that transfers the task.

use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

/// The final owner stops and joins the transferred relay task.
#[derive(Debug)]
pub(crate) struct RetainedCarrier {
    stop: Arc<AtomicBool>,
    task: Option<JoinHandle<io::Result<()>>>,
}

impl RetainedCarrier {
    /// Takes the real relay task and the stop flag already observed by that task.
    pub(crate) fn new(stop: Arc<AtomicBool>, task: JoinHandle<io::Result<()>>) -> Self {
        Self {
            stop,
            task: Some(task),
        }
    }
}

impl Drop for RetainedCarrier {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(task) = self.task.take() {
            let _settled = task.join();
        }
    }
}
