//! Linux and macOS clipboard writes through the session's frozen route: a desktop clipboard
//! program run on one owned thread, or an OSC 52 sequence handed back for the lifecycle writer.
//! A session without a route settles every admitted job as unsupported.
use super::super::terminal_effects::{Job, Outcome};
use crate::{
    background_thread::BackgroundThread,
    host_capabilities::unix::{self as host, ClipboardRoute, Exit, Overrun, Program},
};
use std::{
    ffi::OsStr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

/// Copied text above this size is refused before any byte reaches the outer terminal; terminals
/// cap the OSC 52 payloads they accept.
const TERMINAL_TEXT_LIMIT: usize = 1024 * 1024;
/// Deadline on one clipboard program, after which it is killed and delivery is unconfirmed.
const PROGRAM_BUDGET: Duration = Duration::from_secs(3);

#[derive(Default)]
pub(in crate::workbench_client) struct Writer {
    sender: Option<mpsc::SyncSender<Job>>,
    stop: Arc<AtomicBool>,
    task: BackgroundThread,
}

impl Writer {
    pub(in crate::workbench_client) fn new() -> Self {
        Self::default()
    }

    /// Returns the job when the outer terminal's writer must deliver it as an OSC 52 sequence.
    pub(in crate::workbench_client) fn submit(&mut self, job: Job) -> Option<Job> {
        match &host::facts().clipboard {
            None => job.dispatch_clipboard(|_text| Outcome::Unsupported),
            Some(ClipboardRoute::Terminal) => {
                if job
                    .text()
                    .is_none_or(|text| text.len() <= TERMINAL_TEXT_LIMIT)
                {
                    return Some(job);
                }
                job.dispatch_clipboard(|_text| Outcome::Failed);
            }
            Some(ClipboardRoute::Program(program)) => self.send(program, job),
        }
        None
    }

    fn send(&mut self, program: &'static Program, job: Job) {
        if self.stop.load(Ordering::Acquire) {
            return;
        }
        if self.sender.is_none() {
            let (sender, receiver) = mpsc::sync_channel(1);
            let stop = Arc::clone(&self.stop);
            let Ok(task) =
                BackgroundThread::spawn("orchbee-clipboard", move || run(receiver, stop, program))
            else {
                return;
            };
            self.task = task;
            self.sender = Some(sender);
        }
        let _ = self
            .sender
            .as_ref()
            .expect("initialized sender")
            .try_send(job);
    }

    pub(in crate::workbench_client) fn close(&mut self) {
        self.request_stop();
        let _ = self.task.join();
    }
    /// Abandons undelivered clipboard writes without waiting.
    pub(in crate::workbench_client) fn request_stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.sender.take();
    }
    /// Process exit: a delivery still running at the deadline is abandoned.
    pub(in crate::workbench_client) fn stop_before(
        &mut self,
        deadline: Instant,
    ) -> std::io::Result<()> {
        self.request_stop();
        self.task.join_before(deadline)
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        self.request_stop();
    }
}

fn run(receiver: mpsc::Receiver<Job>, stop: Arc<AtomicBool>, program: &Program) {
    let arguments: Vec<&OsStr> = program.arguments.iter().map(OsStr::new).collect();
    while let Ok(job) = receiver.recv() {
        if stop.load(Ordering::Acquire) {
            return;
        }
        job.dispatch_clipboard(|text| {
            match host::run(
                &program.path,
                &arguments,
                Some(text.as_bytes()),
                PROGRAM_BUDGET,
                Overrun::Kill,
            ) {
                Exit::Succeeded => Outcome::Delivered,
                Exit::Failed => Outcome::Failed,
                Exit::Overran => Outcome::Unknown,
            }
        });
    }
}
