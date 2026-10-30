use super::*;
use crate::workbench_client::{
    state::{AttachmentKey, PaneKey, PaneRef},
    terminal_effects::Queue,
};
use orchbee_workbench_protocol::{WorkbenchPanelId, WorkbenchTabId};
use std::thread;
use std::{
    num::NonZeroU64,
    time::{Duration, Instant},
};

#[test]
fn hidden_owner_pump_dispatches_once_and_is_joined_without_using_the_shared_clipboard() {
    let mut queue = Queue::default();
    let pane = PaneRef::new(
        PaneKey::new(
            AttachmentKey::new(
                1,
                WorkbenchTabId::parse("00000000000000000000000000000001").unwrap(),
            ),
            WorkbenchPanelId::parse("00000000000000000000000000000002").unwrap(),
        ),
        1,
    );
    let text = "中文e\u{301}\nnext";
    assert!(queue.enqueue_selection(pane, NonZeroU64::MIN, text.into()));
    let (sender, receiver) = mpsc::sync_channel(1);
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let (observed, received) = mpsc::channel();
    let task = thread::Builder::new()
        .name("orchbee-clipboard-test".into())
        .spawn(move || {
            run(receiver, thread_stop, move |window, value| {
                assert!(!window.ptr().is_null());
                assert!(window.IsWindow());
                assert!(!window.IsWindowVisible());
                observed.send(value.to_owned()).unwrap();
                true
            });
        })
        .unwrap();
    let mut writer = Writer {
        sender: Some(sender),
        stop,
        task: crate::background_thread::BackgroundThread::adopt(task),
    };
    writer.submit(queue.dispatch().unwrap());
    assert_eq!(received.recv_timeout(Duration::from_secs(5)).unwrap(), text);
    let cutoff = Instant::now() + Duration::from_secs(5);
    let outcome = loop {
        if let Some(outcome) = queue.take_outcome() {
            break outcome;
        }
        assert!(Instant::now() < cutoff, "clipboard attempt did not settle");
        thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(
        outcome,
        (
            super::super::super::terminal_effects::Kind::Clipboard,
            Outcome::Delivered
        )
    );
    assert!(queue.dispatch().is_none());
    writer.close();
    assert!(writer.task.is_finished());
    assert!(matches!(
        received.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
}
