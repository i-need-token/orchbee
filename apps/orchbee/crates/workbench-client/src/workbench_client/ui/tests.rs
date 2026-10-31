pub(in crate::workbench_client::ui) mod agent_board;
pub(in crate::workbench_client::ui) mod close_impact;
pub(in crate::workbench_client::ui) mod composition;
pub(in crate::workbench_client::ui) mod context_selection;
pub(in crate::workbench_client::ui) mod control_takeover;
pub(in crate::workbench_client::ui) mod create_membership;
pub(in crate::workbench_client::ui) mod input_provenance;
pub(in crate::workbench_client) mod membership;
pub(in crate::workbench_client::ui) mod modal_pointer_roundtrip;
pub(in crate::workbench_client) mod move_membership;
pub(in crate::workbench_client::ui) mod normal_exit_hold;
pub(in crate::workbench_client::ui) mod panel_input;
pub(in crate::workbench_client::ui) mod panel_reports;
pub(in crate::workbench_client::ui) mod program_effect;
pub(in crate::workbench_client) mod publication_hint;
pub(in crate::workbench_client::ui) mod quick_workspace;
pub(in crate::workbench_client::ui) mod rename_membership;
pub(in crate::workbench_client::ui) mod retired_membership;
pub(in crate::workbench_client::ui) mod selection;
pub(in crate::workbench_client::ui) mod startup_queue;
pub(in crate::workbench_client::ui) mod viewport;
pub(in crate::workbench_client::ui) mod window_title;
pub(in crate::workbench_client) mod workspace_close;
pub(in crate::workbench_client::ui) mod workspace_setup;
pub(in crate::workbench_client::ui) mod worktree_launch;

use super::*;
use std::{sync::atomic::Ordering, time::Duration};

#[test]
fn worker_completion_interrupts_the_ui_wait() {
    let (mut ui, mailbox, clock, _) = super::tests::move_membership::ready_ui();
    super::tests::viewport::install_projection(&mut ui, Some((0, false, 24)), 0);
    ui.schedule_poll(&mailbox);
    let lease = mailbox.try_recv().unwrap().unwrap();
    mailbox
        .publish_poll_result_before(
            lease.poll_key().unwrap().clone(),
            None,
            false,
            clock.now() + Duration::from_secs(1),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(matches!(
        mailbox
            .take_causal_before(
                &lease.poll_key().unwrap(),
                clock.now() + Duration::from_secs(1),
                &clock
            )
            .unwrap(),
        Some(CausalSettlement::NoFeedback)
    ));
    let wakeup = super::super::UiWakeup::current();
    ui.set_ui_wakeup(Some(wakeup.clone()));
    assert!(wakeup.take());
    std::thread::park_timeout(Duration::ZERO); // Consume registration's park token.
    let start = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let started = Arc::clone(&start);
    let worker = std::thread::spawn(move || {
        while !started.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        mailbox.finish_lease(lease).unwrap();
    });
    let began = std::time::Instant::now();
    // No channel wait between triggering completion and parking: channel
    // synchronization itself may consume std::thread's unpark token.
    start.store(true, Ordering::Release);
    std::thread::park_timeout(Duration::from_millis(50));
    assert!(wakeup.take());
    println!(
        "real worker completion interrupted 50ms UI wait after {:?}",
        began.elapsed()
    );
    worker.join().unwrap();
}

#[test]
fn one_second_of_empty_replies_is_bounded_even_with_repeated_same_instant_ticks() {
    let (mut ui, mailbox, clock, elapsed) = super::tests::move_membership::ready_ui();
    super::tests::viewport::install_projection(&mut ui, Some((0, false, 24)), 0);
    let wakeup = super::super::UiWakeup::current();
    ui.set_ui_wakeup(Some(wakeup.clone()));
    let mut polls = 0;
    for milliseconds in 0..=1000 {
        elapsed.store(milliseconds, Ordering::SeqCst);
        for _ in 0..20 {
            wakeup.take();
            ui.tick();
            if let Some(lease) = mailbox.try_recv().unwrap() {
                assert!(matches!(lease.record(), QueuedCommand::Poll(_)));
                polls += 1;
                assert!(polls <= 1 + milliseconds / 8);
                mailbox
                    .publish_poll_result_before(
                        lease.poll_key().unwrap().clone(),
                        None,
                        false,
                        clock.now() + Duration::from_secs(1),
                        &clock,
                    )
                    .unwrap();
                ui.tick();
                assert!(matches!(
                    mailbox
                        .take_causal_before(
                            &lease.poll_key().unwrap(),
                            clock.now() + Duration::from_secs(1),
                            &clock
                        )
                        .unwrap(),
                    Some(CausalSettlement::NoFeedback)
                ));
                mailbox.finish_lease(lease).unwrap();
                ui.tick();
                assert_eq!(ui.next_poll_delay(), Some(Duration::from_millis(8)));
            }
        }
    }
    assert_eq!(polls, 126); // t=0, 8, ... 1000; 125 intervals per second.
    println!(
        "controlled idle trace: 20,020 UI ticks over 1000ms admit 126 empty polls (including t=0)"
    );
}

#[test]
fn worker_publication_and_lease_completion_wake_ui_while_empty_polls_are_paced() {
    let (mut ui, mailbox, clock, elapsed) = super::tests::viewport::controlling_ui();
    super::tests::viewport::install_projection(&mut ui, Some((0, false, 24)), 0);
    let wakeup = super::super::UiWakeup::current();
    ui.set_ui_wakeup(Some(wakeup.clone()));
    assert!(
        wakeup.take(),
        "registration observes already published work"
    );
    ui.schedule_poll(&mailbox);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let poll = lease.poll_key().unwrap().clone();
    mailbox
        .publish_poll_result_before(
            poll,
            None,
            false,
            clock.now() + Duration::from_secs(1),
            &clock,
        )
        .unwrap();
    assert!(
        wakeup.take(),
        "an empty result must release its causal lease promptly too"
    );
    ui.tick();
    assert_eq!(ui.next_poll_delay(), Some(Duration::from_millis(8)));
    assert!(matches!(
        mailbox
            .take_causal_before(
                &lease.poll_key().unwrap(),
                clock.now() + Duration::from_secs(1),
                &clock
            )
            .unwrap(),
        Some(CausalSettlement::NoFeedback)
    ));
    mailbox.finish_lease(lease).unwrap();
    assert!(
        wakeup.take(),
        "the scheduler need not wait 50 ms for the released lease"
    );
    assert!(mailbox.try_recv().unwrap().is_none());
    elapsed.store(7, Ordering::SeqCst);
    ui.tick();
    assert_eq!(ui.next_poll_delay(), Some(Duration::from_millis(1)));
    assert!(mailbox.try_recv().unwrap().is_none());
    elapsed.store(8, Ordering::SeqCst);
    ui.tick();
    let next = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(next.record(), QueuedCommand::Poll(_)));
    assert_eq!(ui.next_poll_delay(), None);
    mailbox
        .publish_poll_result_before(
            next.poll_key().unwrap(),
            None,
            false,
            clock.now() + Duration::from_secs(1),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox
        .take_causal_before(
            &next.poll_key().unwrap(),
            clock.now() + Duration::from_secs(1),
            &clock,
        )
        .unwrap();
    mailbox.finish_lease(next).unwrap();
    assert_eq!(ui.next_poll_delay(), Some(Duration::from_millis(8)));
    assert!(ui.forward(b"a"));
    assert!(ui.forward(b"b"));
    assert_eq!(
        ui.next_poll_delay(),
        None,
        "admitted input cancels idle pacing"
    );
    ui.tick();
    let input = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Input(tagged) = input.record() else {
        panic!("ordered input before poll")
    };
    assert_eq!(tagged.value().as_bytes(), b"ab");
    mailbox.finish_lease(input).unwrap();
    let poll = mailbox.try_recv().unwrap().unwrap();
    assert!(
        matches!(poll.record(), QueuedCommand::Poll(_)),
        "echo poll is admitted without advancing the idle deadline"
    );
    mailbox.finish_lease(poll).unwrap();
    ui.set_ui_wakeup(None);
    wakeup.take();
    mailbox.close(CarrierFailure::Protocol);
    assert!(
        !wakeup.take(),
        "a released surface cannot wake its former waiter"
    );
}

#[test]
fn mailbox_retirement_interrupts_the_bound_ui_waiter() {
    let (ui, mailbox, _clock, _elapsed) = super::tests::move_membership::ready_ui();
    let wakeup = super::super::UiWakeup::current();
    ui.set_ui_wakeup(Some(wakeup.clone()));
    assert!(wakeup.take());
    mailbox.close(CarrierFailure::OutcomeUnknown);
    assert!(wakeup.take());
}

#[test]
fn one_shot_notices_expire_without_discarding_unknown_outcomes() {
    use crate::workbench_state::ClientNotice;
    use std::sync::atomic::Ordering;
    let (mut ui, _mailbox, _clock, elapsed) = move_membership::ready_ui();
    ui.state
        .as_mut()
        .unwrap()
        .set_notice(ClientNotice::HistoryRecovering);
    assert!(!ui.expire_status_notices());
    elapsed.store(4000, Ordering::SeqCst);
    assert!(ui.expire_status_notices());
    assert!(ui.view().notice().is_none());
    ui.state
        .as_mut()
        .unwrap()
        .set_notice(ClientNotice::DuplicateOutcomeUnknown);
    assert!(!ui.expire_status_notices());
    elapsed.store(12000, Ordering::SeqCst);
    assert!(!ui.expire_status_notices());
    assert!(ui.view().notice().is_some());
}
