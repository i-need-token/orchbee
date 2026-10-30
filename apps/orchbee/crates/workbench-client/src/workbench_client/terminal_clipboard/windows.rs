//! Dedicated hidden Win32 owner and pump; Unicode writes have no fallback or replay.
use super::super::terminal_effects::{Job, Outcome};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Instant,
};
use winsafe::{self as w, co, gui, prelude::*};

use crate::background_thread::BackgroundThread;

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

    pub(in crate::workbench_client) fn submit(&mut self, job: Job) {
        if self.stop.load(Ordering::Acquire) {
            return;
        }
        if self.sender.is_none() {
            let (sender, receiver) = mpsc::sync_channel(1);
            let stop = Arc::clone(&self.stop);
            let Ok(task) =
                BackgroundThread::spawn("orchbee-clipboard", move || run(receiver, stop, deliver))
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

fn deliver(window: &w::HWND, text: &str) -> bool {
    if window.ptr().is_null() {
        return false;
    }
    let Ok(_opened) = clipboard_win::Clipboard::new_for(window.ptr()) else {
        return false;
    };
    if clipboard_win::raw::empty().is_err() {
        return false;
    }
    clipboard_win::raw::set_string_with(text, clipboard_win::options::NoClear).is_ok()
}

fn run(
    receiver: mpsc::Receiver<Job>,
    stop: Arc<AtomicBool>,
    deliver: impl Fn(&w::HWND, &str) -> bool + 'static,
) {
    let window = gui::WindowMain::new(gui::WindowMainOpts {
        title: "Orchbee clipboard",
        size: (1, 1),
        style: co::WS::POPUP,
        ex_style: co::WS_EX::TOOLWINDOW | co::WS_EX::NOACTIVATE,
        ..Default::default()
    });
    let created = window.clone();
    window.on().wm_create(move |_| {
        if created.hwnd().SetTimer(1, 25, None).is_err() {
            created.close();
        }
        Ok(0)
    });
    let closing = window.clone();
    window.on().wm_close(move || {
        closing.hwnd().DestroyWindow()?;
        Ok(())
    });
    let ticking = window.clone();
    window.on().wm_timer(1, move || {
        if stop.load(Ordering::Acquire) {
            let _ = ticking.hwnd().KillTimer(1);
            ticking.close();
            return Ok(());
        }
        if let Ok(job) = receiver.try_recv() {
            job.dispatch_clipboard(|text| {
                if deliver(ticking.hwnd(), text) {
                    Outcome::Delivered
                } else {
                    Outcome::Failed
                }
            });
        }
        Ok(())
    });
    let _ = window.run_main(Some(co::SW::HIDE));
}

#[cfg(test)]
mod tests;
