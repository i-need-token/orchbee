use super::*;
use orchbee_terminal_protocol::{COLOR_DEFAULT, COLOR_NONE, COLOR_PALETTE, TerminalTextStyle};

fn row(text: &str, foreground: u32, flags: u16) -> TerminalTextRow {
    TerminalTextRow {
        index: 0,
        flags: 0,
        prompt_class: 0,
        cells: text
            .chars()
            .map(|character| TerminalTextCell {
                text: character.to_string().into(),
                width: 0,
                style: TerminalTextStyle {
                    foreground,
                    background: COLOR_DEFAULT,
                    underline_color: COLOR_NONE,
                    flags,
                    underline: 0,
                },
            })
            .collect(),
    }
}

#[test]
fn timed_error_has_to_be_the_latest_entry_at_a_live_empty_composer() {
    let red = COLOR_PALETTE | 1;
    let mut history = vec![
        row("• Earlier progress", COLOR_DEFAULT, 0),
        row("", COLOR_DEFAULT, 0),
        row(
            "■ You’ve hit your usage limit. Visit https://chatgpt.com/settings/usage to",
            red,
            0,
        ),
        row("purchase more credits or try again", red, 0),
        row("at Nov 22nd, 2026 2:23 AM.", red, 0),
        row("", COLOR_DEFAULT, 0),
    ];
    let input = row("› Ask Codex to do anything", COLOR_DEFAULT, FAINT);
    let footer = [
        row("", COLOR_DEFAULT, 0),
        row(
            "  GPT-5.6-Sol default · Context 54% used · 5h 0% left",
            COLOR_DEFAULT,
            0,
        ),
        row("  weekly 22% left", COLOR_DEFAULT, 0),
        row("  ← for agents · ? for shortcuts", COLOR_DEFAULT, 0),
    ];
    assert!(matches!(
        composer_rows(&history, &input, &footer, 2),
        Some(Composer::Quota(RetryTime {
            date: Some((2026, 11, 22)),
            hour: 2,
            minute: 23
        }))
    ));
    let draft = row("› Ask Codex to do anything", COLOR_DEFAULT, 0);
    assert!(composer_rows(&history, &draft, &footer, 2).is_none());
    assert!(composer_rows(&history, &input, &footer, 12).is_none());
    let collapsed = [row("  ? for shortcuts", COLOR_DEFAULT, 0)];
    assert!(matches!(
        composer_rows(&history, &input, &collapsed, 2),
        Some(Composer::Quota(_))
    ));
    let with_warnings = [
        row(
            "  GPT-6.2-Sol max · Context 54% used · 5h 0% left · weekly 22% left · Standard",
            COLOR_DEFAULT,
            0,
        ),
        row(
            "  ← for agents · ? for shortcuts                ⚠ 7 warnings · f2 to view",
            COLOR_DEFAULT,
            0,
        ),
    ];
    assert!(matches!(
        composer_rows(&history, &input, &with_warnings, 2),
        Some(Composer::Quota(_))
    ));
    for overlay in [
        "  Enter to confirm",
        "  Enter to confirm · Esc to cancel",
        "  ? for shortcuts                Enter to confirm",
    ] {
        assert!(composer_rows(&history, &input, &[row(overlay, COLOR_DEFAULT, 0)], 2).is_none());
    }
    let separate_overlay = [
        row(
            "  GPT-5.6-Sol default · repo                                        ",
            COLOR_DEFAULT,
            0,
        ),
        row("  Enter to confirm · Esc", COLOR_DEFAULT, 0),
    ];
    assert!(composer_rows(&history, &input, &separate_overlay, 2).is_none());
    history[2].cells[0].style.foreground = COLOR_DEFAULT;
    assert!(composer_rows(&history, &input, &footer, 2).is_none());
    history[2].cells[0].style.foreground = red;
    assert!(composer_rows(&history[3..], &input, &footer, 2).is_none());
    history[4] = row("at 5:00 P", red, 0);
    assert!(composer_rows(&history, &input, &footer, 2).is_none());
    history.push(row("• Started a new turn", COLOR_DEFAULT, 0));
    assert!(matches!(
        composer_rows(&history, &input, &footer, 2),
        Some(Composer::Clear)
    ));
}

#[test]
fn reset_time_uses_the_host_date_and_waits_through_the_displayed_minute() {
    let at = |day, hour, minute| {
        unix_milliseconds_at_local(LocalClock {
            year: 2026,
            month: 11,
            day,
            hour,
            minute,
        })
        .expect("fixture uses unambiguous dates and times")
    };
    let now = at(23, 8, 0);
    let today = retry_time("■ You've hit your usage limit. Try again at 12:03 PM.").unwrap();
    assert_eq!(today.deadline(now, now), Some(at(23, 12, 4)));
    assert_eq!(today.deadline(now, at(22, 8, 0)), None);
    assert_eq!(today.deadline(at(23, 12, 4), now), None);
    let week = retry_time("■ You’ve hit your usage limit for Example. Switch to another model now, or try again at Nov 30th, 2026 12:01 AM.").unwrap();
    assert_eq!(week.deadline(now, now), Some(at(30, 0, 2)));
    for message in [
        "■ You've hit your usage limit. Try again later.",
        "■ Your workspace is out of credits. Try again at 10:30 AM.",
        "■ You've hit your usage limit. Try again at 00:00 AM.",
        "■ You've hit your usage limit. Try again at 10:62 PM.",
        "■ You've hit your usage limit. Try again at Nov 23rd, 2026 10:30 AM. Trailing text",
    ] {
        assert!(retry_time(message).is_none(), "{message}");
    }
}

use super::super::state::{Run, Screen};
use crate::session_coordinator::{HostAgentRun, identify_host_agent};
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity, SendScreenGate,
};
use orchbee_terminal_protocol::{
    AttachmentId, SCREEN_GATE_QUIET_MILLISECONDS, TerminalFrameRoute, TerminalId, TerminalProcess,
    TerminalProcessConsole, TerminalProcessInfo,
};
use std::{
    num::NonZeroU64,
    time::{Duration, Instant},
};

struct Timeline {
    run: Run,
    attachment_id: AttachmentId,
    epoch: [u8; 16],
    tick: Instant,
}

impl Timeline {
    fn new(born: Option<u64>) -> Self {
        let attachment_id = AttachmentId::from_bytes([1; 16]).unwrap();
        let terminal = [2; 16];
        let epoch = [3; 16];
        let process = TerminalProcessInfo {
            attachment_id,
            terminal_id: TerminalId::from_bytes(terminal).unwrap(),
            root_pid: 4,
            processes: vec![TerminalProcess {
                pid: 4,
                parent_pid: None,
                creation_time: 134_116_992_000_000_000,
                console: TerminalProcessConsole::Attached,
                image: Some("C:/fixtures/codex.exe".into()),
                command_line: Some("codex".into()),
                current_directory: None,
            }],
            launch_kind: None,
        };
        let agent: HostAgentRun = identify_host_agent(&process).unwrap();
        let identity = HostTerminalIdentity::new(
            HostCreationId::from_bytes([7; 16]).unwrap(),
            HostEpoch::from_bytes(epoch).unwrap(),
            HostTerminalId::from_bytes(terminal).unwrap(),
        );
        Self {
            run: Run::new(identity, agent, born),
            attachment_id,
            epoch,
            tick: Instant::now(),
        }
    }

    fn see(
        &mut self,
        generation: u64,
        revision: u64,
        screen: Screen,
        now: u64,
        enabled: bool,
    ) -> Option<SendScreenGate> {
        let frame = TerminalFrameRoute {
            attachment_id: self.attachment_id,
            host_epoch: orchbee_terminal_protocol::HostEpoch::from_bytes(self.epoch).unwrap(),
            sequence: NonZeroU64::MIN,
            generation: NonZeroU64::new(generation).unwrap(),
            revision,
            base_revision: None,
        };
        self.run.observe(
            frame,
            screen,
            now,
            self.tick + Duration::from_millis(now.saturating_sub(local_at(22, 0, 0))),
            || enabled,
        )
    }
}

fn local_at(day: u16, hour: u16, minute: u16) -> u64 {
    unix_milliseconds_at_local(LocalClock {
        year: 2026,
        month: 11,
        day,
        hour,
        minute,
    })
    .unwrap()
}

fn same_time_quota() -> Screen {
    Screen::Quota(retry_time("■ You've hit your usage limit. Try again at 5:00 PM.").unwrap())
}

#[test]
fn a_restart_does_not_arm_an_existing_notice() {
    let due = local_at(23, 17, 1);
    let quota = same_time_quota();
    let mut timeline = Timeline::new(None);
    assert!(timeline.see(1, 10, quota, due - MINUTE_MS, true).is_none());
    assert!(timeline.see(1, 12, quota, due, true).is_none());
    assert!(timeline.see(2, 13, quota, due + 1, true).is_none());

    let explicit = Screen::Quota(
        retry_time("■ You've hit your usage limit. Try again at Nov 23rd, 2026 5:00 PM.").unwrap(),
    );
    let mut timeline = Timeline::new(None);
    assert!(
        timeline
            .see(1, 10, explicit, due - MINUTE_MS, true)
            .is_none()
    );
    assert!(timeline.see(1, 11, explicit, due, true).is_none());
}

#[test]
fn an_old_time_only_notice_keeps_its_date_across_unreadable_frames() {
    let mut timeline = Timeline::new(None);
    let due = local_at(22, 17, 1);
    let quota = same_time_quota();
    assert!(
        timeline
            .see(1, 1, Screen::Clear, due - 2 * MINUTE_MS, true)
            .is_none()
    );
    assert!(timeline.see(1, 2, quota, due - MINUTE_MS, true).is_none());
    // Samples without a complete composer or readable notice do not enter the
    // state machine. A new generation cannot manufacture a new occurrence.
    let tomorrow = local_at(23, 17, 1);
    assert!(
        timeline
            .see(2, 5, Screen::Clear, tomorrow - MINUTE_MS, true)
            .is_none()
    );
    let gate = timeline
        .see(3, 6, quota, tomorrow - MINUTE_MS, true)
        .expect("the first deadline has passed");
    assert_eq!(gate.first_revision, 2);
    timeline.run.finish();
    assert!(timeline.see(3, 7, quota, tomorrow, true).is_none());
}

#[test]
fn resizing_and_switching_off_preserve_the_original_input_floor() {
    let due = local_at(23, 17, 1);
    let quota = same_time_quota();
    let mut timeline = Timeline::new(None);
    assert!(
        timeline
            .see(1, 10, Screen::Clear, due - 2 * MINUTE_MS, false)
            .is_none()
    );
    assert!(timeline.see(1, 11, quota, due - MINUTE_MS, false).is_none());
    assert!(timeline.see(2, 15, quota, due, false).is_none());
    let gate = timeline.see(3, 21, quota, due + 1, true).unwrap();
    assert_eq!(
        (gate.generation.get(), gate.revision, gate.first_revision),
        (3, 21, 11)
    );
    timeline.run.finish();
    assert!(timeline.see(3, 22, quota, due + 4_000, false).is_none());
    assert!(timeline.see(3, 23, quota, due + 5_000, true).is_none());
}

#[test]
fn a_new_observed_turn_can_hit_the_same_reset_time() {
    let due = local_at(23, 17, 1);
    let quota = same_time_quota();
    let mut timeline = Timeline::new(Some(due - 3 * MINUTE_MS));
    assert!(
        timeline
            .see(1, 10, quota, due - 2 * MINUTE_MS, true)
            .is_none()
    );
    assert!(
        timeline
            .see(1, 12, Screen::Working, due - MINUTE_MS, true)
            .is_none()
    );
    assert!(
        timeline
            .see(1, 13, Screen::Working, due - 5000, true)
            .is_none()
    );
    assert!(timeline.see(1, 14, quota, due - 1000, true).is_none());
    let gate = timeline.see(1, 15, quota, due, true).unwrap();
    assert_eq!(
        (gate.pid.get(), gate.first_revision, gate.revision),
        (4, 12, 15)
    );
}

#[test]
fn each_observed_turn_has_its_own_input_floor() {
    let due = local_at(23, 17, 1);
    let quota = same_time_quota();
    let mut timeline = Timeline::new(None);
    let start = due - 10 * MINUTE_MS;
    assert!(timeline.see(1, 10, Screen::Working, start, true).is_none());
    assert!(
        timeline
            .see(1, 20, Screen::Clear, start + 1000, true)
            .is_none()
    );
    // The person's key that submits the second turn lands between these frames.
    assert!(
        timeline
            .see(1, 30, Screen::Working, start + 2000, true)
            .is_none()
    );
    assert!(timeline.see(1, 31, quota, due - 1000, true).is_none());
    let gate = timeline.see(1, 31, quota, due, true).unwrap();
    assert_eq!(gate.first_revision, 30);
}

#[test]
fn a_machine_that_slept_through_the_deadline_resumes_on_waking() {
    let due = local_at(23, 17, 1);
    let quota = same_time_quota();
    let mut timeline = Timeline::new(None);
    assert!(
        timeline
            .see(1, 10, Screen::Clear, due - 2 * MINUTE_MS, true)
            .is_none()
    );
    assert!(timeline.see(1, 11, quota, due - MINUTE_MS, true).is_none());
    let wake = due + 6 * 60 * MINUTE_MS;
    assert!(timeline.see(1, 11, quota, wake, true).is_some());
    assert!(
        timeline
            .see(1, 12, quota, wake + 5 * MINUTE_MS, true)
            .is_none()
    );
}

#[test]
fn safe_retries_outlast_redraws_and_reconnections_but_expire() {
    let due = local_at(23, 17, 1);
    let quota = same_time_quota();
    let mut timeline = Timeline::new(Some(due - 2 * MINUTE_MS));
    assert!(timeline.see(1, 10, quota, due - MINUTE_MS, true).is_none());
    let gate = timeline.see(1, 10, quota, due, true).unwrap();
    assert_eq!(
        gate.first_revision, 0,
        "a launch without a prior observation requires a terminal untouched by a person"
    );
    for round in 1..=6 {
        let retry = due + round * SCREEN_GATE_QUIET_MILLISECONDS;
        assert!(
            timeline
                .see(2, 10 + round, quota, retry - 1, true)
                .is_none()
        );
        assert!(timeline.see(2, 10 + round, quota, retry, true).is_some());
    }
    assert!(
        timeline
            .see(3, 24, quota, due + 4 * MINUTE_MS, true)
            .is_some()
    );
    assert!(
        timeline
            .see(3, 25, quota, due + 5 * MINUTE_MS, true)
            .is_none()
    );
}

#[test]
fn a_delivery_failure_is_retryable_only_below_the_input_frontier() {
    use super::super::{Delivery, WorkerOperationResult, WorkerRequestError, delivery};
    use crate::worker_obcn_carrier::{WorkerResponseFailure, WorkerTransportFailure};
    use orchbee_capability_node_protocol::{
        OperationId, SendCleanupStatus, SendProgress, SendRejection, SendScreenState,
        SendUnavailable, TerminalSendOutcome, TerminalSendResult,
    };

    assert_eq!(
        delivery(Err(WorkerRequestError::NotDelivered(
            WorkerTransportFailure::Closed
        ))),
        Delivery::Retry
    );
    assert_eq!(
        delivery(Err(WorkerRequestError::PostDeliveryOutcomeUnknown(
            WorkerResponseFailure::Disconnected
        ))),
        Delivery::Unknown
    );
    let result = |outcome| {
        Ok(WorkerOperationResult::Send(
            TerminalSendResult::new(OperationId::from_bytes([4; 16]).unwrap(), outcome).unwrap(),
        ))
    };
    assert_eq!(
        delivery(result(TerminalSendOutcome::Unavailable {
            reason: SendUnavailable::HostUnavailable,
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::Confirmed,
        })),
        Delivery::Retry,
    );
    assert_eq!(
        delivery(result(TerminalSendOutcome::Rejected {
            reason: SendRejection::ScreenRefused(SendScreenState::PersonInput),
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::Confirmed,
        })),
        Delivery::Refused,
    );
}
