//! One lazily created, hidden Win32 notification owner, stopped with the TUI lifetime.
use orchbee_workbench_protocol::Notification;
use std::{
    cell::RefCell,
    io,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use winsafe::{self as w, co, gui, prelude::*};

use crate::background_thread::BackgroundThread;
#[derive(Default)]
pub struct SystemNotifications {
    sender: Option<mpsc::SyncSender<Notification>>,
    stop: Arc<AtomicBool>,
    task: BackgroundThread,
}
impl SystemNotifications {
    /// Construct the same lazy native owner without starting its notification thread.
    pub fn new() -> Self {
        Self::default()
    }

    /// Process exit: removes the native owner's pending notifications without waiting.
    pub fn request_stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.sender.take();
    }

    /// Process exit: observes the native owner's thread against the shared deadline.
    ///
    /// # Errors
    /// TimedOut when the thread is still running at the deadline; it is detached on drop.
    pub fn stop_before(&mut self, deadline: Instant) -> io::Result<()> {
        self.request_stop();
        self.task.join_before(deadline)
    }

    /// System delivery always uses the native owner here, never an outer-terminal sequence.
    pub const fn through_terminal(&self) -> bool {
        false
    }

    pub fn submit(&mut self, message: Notification) -> io::Result<()> {
        crate::tui_capabilities::Capability::SystemNotification.require()?;
        if self.sender.is_none() {
            let (sender, receiver) = mpsc::sync_channel(4);
            let stop = Arc::clone(&self.stop);
            self.task = BackgroundThread::spawn("orchbee-system-notifications", move || {
                run(receiver, stop)
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
struct Icon {
    data: w::NOTIFYICONDATA,
    until: Instant,
}
impl Drop for Icon {
    fn drop(&mut self) {
        let _ = w::Shell_NotifyIcon(co::NIM::DELETE, &self.data);
    }
}
fn bounded_utf16(value: &str, capacity: usize) -> String {
    let mut units = 0;
    value
        .chars()
        .take_while(|ch| {
            units += ch.len_utf16();
            units < capacity
        })
        .collect()
}
fn show(window: &w::HWND, message: Notification) -> Option<Icon> {
    let mut data = w::NOTIFYICONDATA::default();
    // Query the root of this parentless owned popup through the safe OS binding.
    data.hWnd = window
        .GetAncestor(co::GA::ROOT)
        .filter(|root| root == window)?;
    data.uID = 1;
    data.uFlags = co::NIF::TIP;
    data.set_szTip("Orchbee");
    w::Shell_NotifyIcon(co::NIM::ADD, &data).ok()?;
    let mut icon = Icon {
        data,
        until: Instant::now() + Duration::from_secs(10),
    };
    icon.data.uFlags = co::NIF::INFO;
    icon.data.dwInfoFlags = co::NIIF::INFO | co::NIIF::NOSOUND;
    icon.data
        .set_szInfoTitle(&bounded_utf16(&message.title, 64));
    icon.data.set_szInfo(&bounded_utf16(
        message.body.as_deref().unwrap_or(&message.title),
        256,
    ));
    w::Shell_NotifyIcon(co::NIM::MODIFY, &icon.data).ok()?;
    Some(icon)
}
fn run(receiver: mpsc::Receiver<Notification>, stop: Arc<AtomicBool>) {
    let window = gui::WindowMain::new(gui::WindowMainOpts {
        title: "Orchbee",
        size: (1, 1),
        style: co::WS::POPUP,
        ex_style: co::WS_EX::TOOLWINDOW | co::WS_EX::NOACTIVATE,
        ..Default::default()
    });
    let icon: Rc<RefCell<Option<Icon>>> = Rc::new(RefCell::new(None));
    let created = window.clone();
    window.on().wm_create(move |_| {
        if created.hwnd().SetTimer(1, 100, None).is_err() {
            created.close();
        }
        Ok(0)
    });
    let ticking = window.clone();
    let closing = window.clone();
    let retiring = Rc::clone(&icon);
    window.on().wm_close(move || {
        retiring.borrow_mut().take();
        closing.hwnd().DestroyWindow()?;
        Ok(())
    });
    let current = Rc::clone(&icon);
    window.on().wm_timer(1, move || {
        if stop.load(Ordering::Acquire) {
            current.borrow_mut().take();
            let _ = ticking.hwnd().KillTimer(1);
            ticking.close();
            return Ok(());
        }
        if current
            .borrow()
            .as_ref()
            .is_some_and(|icon| Instant::now() >= icon.until)
        {
            current.borrow_mut().take();
        }
        if let Ok(message) = receiver.try_recv() {
            current.borrow_mut().take();
            *current.borrow_mut() = show(ticking.hwnd(), message);
        }
        Ok(())
    });
    // HIDE plus absence of WS_VISIBLE/activation flags prevents foreground stealing.
    let _ = window.run_main(Some(co::SW::HIDE));
    icon.borrow_mut().take();
}
#[cfg(test)]
#[path = "tests.rs"]
mod tests;

/// Play the host alert tone for one notification sound.
pub fn sound(kind: orchbee_workbench_protocol::NotificationSound) {
    use orchbee_workbench_protocol::NotificationSound;
    let tone = match kind {
        NotificationSound::None => return,
        NotificationSound::Done => winsafe::co::MBP::INFORMATION,
        NotificationSound::Request => winsafe::co::MBP::EXCLAMATION,
    };
    let _ = winsafe::MessageBeep(tone);
}
