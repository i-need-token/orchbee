//! Linux and macOS system notifications. An outer terminal with a known notification sequence
//! shows them itself, which also reaches a remote login's own desktop; otherwise one lazily
//! started thread runs the desktop notification program for each message under a deadline.
use crate::{
    background_thread::BackgroundThread,
    host_capabilities::unix::{self as host, Notifier, Overrun},
};
use orchbee_workbench_protocol::Notification;
use std::{
    ffi::OsStr,
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

/// Deadline on one notification program, after which it is killed.
const PROGRAM_BUDGET: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct SystemNotifications {
    sender: Option<mpsc::SyncSender<Notification>>,
    stop: Arc<AtomicBool>,
    task: BackgroundThread,
}

impl SystemNotifications {
    /// Construct the lazy owner without starting its notification thread.
    pub fn new() -> Self {
        Self::default()
    }

    /// Process exit: drops pending notifications without waiting.
    pub fn request_stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.sender.take();
    }

    /// Process exit: observes the notification thread against the shared deadline.
    ///
    /// # Errors
    /// TimedOut when the thread is still running at the deadline; it is detached on drop.
    pub fn stop_before(&mut self, deadline: Instant) -> io::Result<()> {
        self.request_stop();
        self.task.join_before(deadline)
    }

    /// Whether system delivery is written as the outer terminal's notification sequence.
    pub fn through_terminal(&self) -> bool {
        host::facts().terminal_notifications
    }

    pub fn submit(&mut self, message: Notification) -> io::Result<()> {
        crate::tui_capabilities::Capability::SystemNotification.require()?;
        let Some(notifier) = host::facts().notifier.as_ref() else {
            return Err(crate::host_capabilities::Unsupported(
                crate::host_capabilities::Capability::SystemNotification,
            )
            .into());
        };
        if self.sender.is_none() {
            let (sender, receiver) = mpsc::sync_channel(4);
            let stop = Arc::clone(&self.stop);
            self.task = BackgroundThread::spawn("orchbee-system-notifications", move || {
                run(&receiver, &stop, notifier);
            })?;
            self.sender = Some(sender);
        }
        self.sender
            .as_ref()
            .expect("initialized sender")
            .try_send(message)
            .map_err(|_| io::ErrorKind::WouldBlock.into())
    }
}
impl Drop for SystemNotifications {
    fn drop(&mut self) {
        // Only stop_before waits; an unfinished thread is detached here.
        self.request_stop();
    }
}

fn run(receiver: &mpsc::Receiver<Notification>, stop: &AtomicBool, notifier: &Notifier) {
    while let Ok(message) = receiver.recv() {
        if stop.load(Ordering::Acquire) {
            return;
        }
        let body = message.body.as_deref().unwrap_or_default();
        let (program, arguments): (_, Vec<&OsStr>) = match notifier {
            // The script reads the title and body as arguments; neither is ever parsed as script.
            Notifier::AppleScript(path) => (
                path,
                [
                    "-e",
                    "on run argv",
                    "-e",
                    "display notification (item 2 of argv) with title (item 1 of argv)",
                    "-e",
                    "end run",
                    message.title.as_str(),
                    body,
                ]
                .into_iter()
                .map(OsStr::new)
                .collect(),
            ),
            Notifier::NotifySend(path) => (
                path,
                ["--app-name=Orchbee", "--", message.title.as_str(), body]
                    .into_iter()
                    .filter(|argument| !argument.is_empty())
                    .map(OsStr::new)
                    .collect(),
            ),
        };
        // Like the Windows balloon, a shown or failed notification reports nothing further.
        let _ = host::run(program, &arguments, None, PROGRAM_BUDGET, Overrun::Kill);
    }
}

/// Ring the terminal bell for one audible notification sound.
pub fn sound(kind: orchbee_workbench_protocol::NotificationSound) {
    use std::io::Write;
    if kind != orchbee_workbench_protocol::NotificationSound::None {
        let mut output = std::io::stdout().lock();
        let _ = output.write_all(b"\x07").and_then(|()| output.flush());
    }
}
