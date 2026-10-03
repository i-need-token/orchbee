use super::*;
use crate::terminal_session::state::{AttachmentKey, PaneKey};
use orchbee_workbench_protocol::{WorkbenchPanelId, WorkbenchTabId};

fn pane(incarnation: u32) -> PaneRef {
    PaneRef::new(
        PaneKey::new(
            AttachmentKey::new(
                1,
                WorkbenchTabId::parse("00000000000000000000000000000001").unwrap(),
            ),
            WorkbenchPanelId::parse("00000000000000000000000000000002").unwrap(),
        ),
        incarnation,
    )
}

#[test]
fn cancellation_wins_before_os_dispatch_but_not_after_it_begins() {
    let mut queue = Queue::default();
    assert!(queue.enqueue(pane(1), NonZeroU64::MIN, "old".into()));
    let job = queue.dispatch().unwrap();
    queue.cancel_pane(&pane(1));
    assert!(!job.begin());
    assert_eq!(queue.take_outcome(), None);
    drop(job);
    assert_eq!(queue.take_outcome(), Some(Outcome::Cancelled));
    assert!(queue.enqueue(pane(2), NonZeroU64::MIN, "new".into()));
    let job = queue.dispatch().unwrap();
    assert!(job.begin());
    queue.close();
    assert_eq!(queue.take_outcome(), None);
    job.settle(Outcome::Delivered);
    assert_eq!(queue.take_outcome(), Some(Outcome::Delivered));
    assert!(queue.dispatch().is_none());
}

#[test]
fn adapter_loss_is_definite_before_dispatch_and_unknown_after_dispatch_without_replay() {
    for (begin, expected) in [(false, Outcome::Failed), (true, Outcome::Unknown)] {
        let mut queue = Queue::default();
        assert!(queue.enqueue(pane(1), NonZeroU64::MIN, "once".into()));
        let job = queue.dispatch().unwrap();
        if begin {
            assert!(job.begin());
        }
        drop(job);
        assert_eq!(queue.take_outcome(), Some(expected));
        assert_eq!(queue.take_outcome(), None);
        assert!(queue.dispatch().is_none());
    }
}

#[test]
fn replacing_one_incarnation_preserves_other_fifo_entries_and_rejects_truncated_unicode() {
    let mut queue = Queue::default();
    assert!(queue.enqueue(pane(1), NonZeroU64::MIN, "stale".into()));
    assert!(queue.enqueue(pane(2), NonZeroU64::MIN, "中文e\u{301}\nnext".into()));
    assert!(!queue.enqueue(pane(2), NonZeroU64::MIN, "cut\0off".into()));
    queue.cancel_pane(&pane(1));
    let job = queue.dispatch().unwrap();
    assert_eq!(job.text(), "中文e\u{301}\nnext");
    assert!(job.begin());
    assert!(!job.begin());
    job.settle(Outcome::Delivered);
    assert_eq!(queue.take_outcome(), Some(Outcome::Delivered));
    assert_eq!(queue.bytes, 0);
}
