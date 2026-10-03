//! Dedicated hidden Win32 owner and pump; Unicode writes have no fallback or replay.
use super::super::terminal_effects::{Job, Outcome};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
};
use winsafe::{self as w, co, gui, prelude::*};

#[derive(Default)]
pub(in crate::terminal_session) struct Writer {
    sender: Option<mpsc::SyncSender<Job>>,
    stop: Arc<AtomicBool>,
    task: Option<JoinHandle<()>>,
}
impl Writer {
    pub(in crate::terminal_session) fn submit(&mut self, job: Job) {
        if self.stop.load(Ordering::Acquire) {
            return;
        }
        if self.sender.is_none() {
            let (sender, receiver) = mpsc::sync_channel(1);
            let stop = Arc::clone(&self.stop);
            let Ok(task) = thread::Builder::new()
                .name("orchbee-clipboard".into())
                .spawn(move || run(receiver, stop, deliver))
            else {
                return;
            };
            self.task = Some(task);
            self.sender = Some(sender);
        }
        let _ = self
            .sender
            .as_ref()
            .expect("initialized sender")
            .try_send(job);
    }
    pub(in crate::terminal_session) fn close(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.sender.take();
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        self.close();
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
        if let Ok(job) = receiver.try_recv()
            && let Some(text) = job.text()
            && job.begin()
        {
            job.settle(if deliver(ticking.hwnd(), text) {
                Outcome::Delivered
            } else {
                Outcome::Failed
            });
        }
        Ok(())
    });
    let _ = window.run_main(Some(co::SW::HIDE));
}

#[cfg(test)]
mod tests;
