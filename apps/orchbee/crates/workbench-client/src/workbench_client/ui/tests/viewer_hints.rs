//! How a Server's viewer hints replace the ordinary poll timer.
use super::super::*;
use super::publication_hint::{attach_split_peer, ready_split_ui};
use crate::workbench_client::mailbox::CausalSettlement;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

struct Split {
    ui: WorkbenchClientUi,
    mailbox: Arc<CommandMailbox>,
    clock: LocalOperationClock,
    elapsed: Arc<AtomicU64>,
    focus: PaneRef,
    sibling: PaneRef,
}

/// Two attached panes holding frames, on a connection that has proved it sends hints, with the
/// fallback too far off to explain any poll.
fn split() -> Split {
    let (mut ui, mailbox, clock, elapsed) = ready_split_ui();
    attach_split_peer(&mut ui, &mailbox, &clock);
    // Setup can leave its own poll queued, and a hint must not be credited with serving it.
    while let Some(lease) = mailbox.try_recv().unwrap() {
        mailbox.finish_lease(lease).unwrap();
    }
    ui.ordinary_poll_serial = None;
    ui.next_fallback_poll = clock.now().checked_add(Duration::from_secs(3_600));
    for projection in &mut ui.projections {
        projection.status = ClientProjectionStatus::FrameAvailable;
        // A new projection starts marked, because its first record can precede its hint route.
        projection.record_ready = false;
    }
    let state = ui.state.as_ref().unwrap();
    let focus = state.focus_pane_ref().unwrap();
    let sibling = state
        .pane_by_panel(state.cache().tabs()[0].panel(1).unwrap().panel_id())
        .unwrap()
        .pane_ref();
    let mut split = Split {
        ui,
        mailbox,
        clock,
        elapsed,
        focus,
        sibling,
    };
    let focus = split.focus.clone();
    split.mailbox.note_viewer_ready(split.viewer(&focus));
    assert_eq!(split.poll_now(), Some(focus));
    split
}

impl Split {
    fn viewer(&self, key: &PaneRef) -> u128 {
        let projection = self
            .ui
            .projections
            .iter()
            .find(|projection| projection.key == *key)
            .unwrap();
        u128::from_be_bytes(*projection.value.attachment().as_bytes())
    }

    /// Asks the scheduler for a poll and settles it empty, returning the pane it served.
    fn poll_now(&mut self) -> Option<PaneRef> {
        self.ui.schedule_poll(&self.mailbox);
        let lease = self.mailbox.try_recv().unwrap()?;
        let poll = lease.poll_key().unwrap();
        let cutoff = self.clock.now() + Duration::from_secs(1);
        self.mailbox
            .publish_poll_result_before(poll.clone(), None, false, cutoff, &self.clock)
            .unwrap();
        self.ui.tick();
        assert!(matches!(
            self.mailbox
                .take_causal_before(&poll, cutoff, &self.clock)
                .unwrap(),
            Some(CausalSettlement::NoFeedback)
        ));
        self.mailbox.finish_lease(lease).unwrap();
        Some(poll.pane_ref().clone())
    }

    fn advance(&self, milliseconds: u64) {
        self.elapsed.fetch_add(milliseconds, Ordering::SeqCst);
    }
}

#[test]
fn a_hint_needs_no_timer_and_the_focused_pane_polls_first() {
    let mut split = split();
    let (focus, sibling) = (split.focus.clone(), split.sibling.clone());

    assert_eq!(split.poll_now(), None, "nothing is due without a hint");
    split.mailbox.note_viewer_ready(split.viewer(&sibling));
    assert_eq!(split.poll_now(), Some(sibling.clone()));

    // Arrival order does not outrank the pane the user is watching.
    split.mailbox.note_viewer_ready(split.viewer(&sibling));
    split.mailbox.note_viewer_ready(split.viewer(&focus));
    assert_eq!(split.poll_now(), Some(focus));
    assert_eq!(split.poll_now(), Some(sibling));
    assert_eq!(split.poll_now(), None);
}

#[test]
fn a_lost_hint_recovers_at_the_fallback_shared_among_the_panes() {
    let mut split = split();
    let round = FALLBACK_POLL_INTERVAL / 2;
    split.ui.next_fallback_poll = None;
    let first = split.poll_now().expect("no fallback is armed");
    assert_eq!(split.ui.next_poll_delay(), Some(round));
    assert_eq!(split.poll_now(), None);

    split.advance(u64::try_from(round.as_millis()).unwrap() - 1);
    assert_eq!(split.ui.next_poll_delay(), Some(Duration::from_millis(1)));
    assert_eq!(split.poll_now(), None);

    split.advance(1);
    let second = split.poll_now().expect("the fallback round elapsed");
    assert_ne!(first, second, "the fallback visits every pane in turn");
}

#[test]
fn a_hinted_poll_leaves_the_fallback_armed_for_its_sibling() {
    let mut split = split();
    let focus = split.focus.clone();
    let fallback = split.ui.next_fallback_poll;

    // However busy the focused pane is, it cannot keep a sibling's lost hint from being noticed.
    split.mailbox.note_viewer_ready(split.viewer(&focus));
    assert_eq!(split.poll_now(), Some(focus));
    assert_eq!(split.ui.next_fallback_poll, fallback);
}

#[test]
fn a_pane_owed_a_full_frame_keeps_the_short_deadline() {
    let mut split = split();
    let sibling = split.sibling.clone();
    split
        .ui
        .projections
        .iter_mut()
        .find(|projection| projection.key == sibling)
        .unwrap()
        .status = ClientProjectionStatus::WaitingForFullFrame;
    assert_eq!(split.ui.next_poll_delay(), Some(UNHINTED_POLL_INTERVAL));
    assert_eq!(split.poll_now(), None);
    split.advance(u64::try_from(UNHINTED_POLL_INTERVAL.as_millis()).unwrap());
    assert!(split.poll_now().is_some());
}
