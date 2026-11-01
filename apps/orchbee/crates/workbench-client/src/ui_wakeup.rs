//! Coalesce worker notifications for the single UI waiter without owning input,
//! payloads, or acknowledgements. Mailbox state remains the delivery authority.

#[cfg(unix)]
use orchbee_local_ipc::LocalStream;
#[cfg(windows)]
use orchbee_windows_console_host::InputWake;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, Thread};
#[cfg(unix)]
use std::{
    io::{Read, Write},
    os::fd::{AsFd, BorrowedFd},
};

/// A poll of the terminal input cannot observe `unpark`.
#[cfg(unix)]
struct WaitChannel {
    reader: LocalStream,
    writer: LocalStream,
}

#[cfg(unix)]
impl WaitChannel {
    fn new() -> std::io::Result<Self> {
        let (reader, writer) = LocalStream::pair()?;
        reader.set_nonblocking(true)?;
        writer.set_nonblocking(true)?;
        Ok(Self { reader, writer })
    }
}

/// A notification for the thread that created it. A signal survives the race
/// between checking for work and parking; repeated signals may coalesce.
#[derive(Clone)]
pub struct UiWakeup {
    pending: Arc<AtomicBool>,
    waiter: Thread,
    /// A console wait cannot observe `unpark`. Absent only when the system refuses the event.
    #[cfg(windows)]
    input_wake: Option<Arc<InputWake>>,
    /// Absent only when the system refuses the channel; the waiter's bound still holds.
    #[cfg(unix)]
    wait_channel: Option<Arc<WaitChannel>>,
}

impl UiWakeup {
    /// Bind this wakeup to the current UI thread, never to a connection worker.
    pub fn current() -> Self {
        Self {
            pending: Arc::new(AtomicBool::new(false)),
            waiter: thread::current(),
            #[cfg(windows)]
            input_wake: InputWake::new().ok().map(Arc::new),
            #[cfg(unix)]
            wait_channel: WaitChannel::new().ok().map(Arc::new),
        }
    }

    /// Notify the owning UI loop that an already-derived local redraw is pending.
    pub fn signal(&self) {
        self.pending.store(true, Ordering::Release);
        self.waiter.unpark();
        #[cfg(windows)]
        if let Some(wake) = &self.input_wake {
            // A failed signal leaves `pending` set for the waiter's next bounded check.
            let _ = wake.signal();
        }
        #[cfg(unix)]
        if let Some(channel) = &self.wait_channel {
            // A full channel already holds an undrained wake.
            let _ = (&channel.writer).write(&[1]);
        }
    }

    /// Consume only the notification, before inspecting the mailbox. Work
    /// published after this exchange leaves another notification pending.
    pub fn take(&self) -> bool {
        self.pending.swap(false, Ordering::AcqRel)
    }

    /// Readable while a signal is undrained, for a waiter that polls other descriptors too.
    #[cfg(unix)]
    pub fn wait_descriptor(&self) -> Option<BorrowedFd<'_>> {
        self.wait_channel
            .as_ref()
            .map(|channel| channel.reader.as_fd())
    }

    /// Call before [`Self::take`], so a signal after that inspection still ends the wait.
    #[cfg(unix)]
    pub fn drain_wait(&self) {
        if let Some(channel) = &self.wait_channel {
            let mut bytes = [0_u8; 64];
            while matches!((&channel.reader).read(&mut bytes), Ok(count) if count == bytes.len()) {}
        }
    }

    #[cfg(windows)]
    pub(crate) fn input_wake(&self) -> Option<&Arc<InputWake>> {
        self.input_wake.as_ref()
    }
}

#[cfg(test)]
mod tests;
