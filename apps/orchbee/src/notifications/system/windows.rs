//! One lazily created, hidden Win32 notification owner, joined with the TUI lifetime.
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
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use winsafe::{self as w, co, gui, prelude::*};
#[derive(Default)]
pub(crate) struct SystemNotifications {
    sender: Option<mpsc::SyncSender<Notification>>,
    stop: Arc<AtomicBool>,
    task: Option<JoinHandle<()>>,
}
impl SystemNotifications {
    pub(crate) fn submit(&mut self, message: Notification) -> io::Result<()> {
        if self.sender.is_none() {
            let (sender, receiver) = mpsc::sync_channel(4);
            let stop = Arc::clone(&self.stop);
            self.task = Some(
                thread::Builder::new()
                    .name("orchbee-system-notifications".into())
                    .spawn(move || run(receiver, stop))?,
            );
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
        self.stop.store(true, Ordering::Release);
        self.sender.take();
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
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
