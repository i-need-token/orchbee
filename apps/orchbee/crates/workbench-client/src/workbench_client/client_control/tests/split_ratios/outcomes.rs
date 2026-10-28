use super::*;
use crate::local_client::workbench::split_ratio::SplitRatioFailure;
use crate::workbench_client::ui::tests::publication_hint::split_snapshot;

#[test]
fn rejection_unknown_and_stale_completion_never_write_or_resubmit() {
    for case in [
        "rejected",
        "unknown",
        "stale-operation",
        "interrupt",
        "expire",
    ] {
        let (mut native, mailbox, clock, elapsed) = ready_split_ui();
        let poll = mailbox.try_recv().unwrap().unwrap();
        mailbox.finish_lease(poll).unwrap();
        let mut control = controller(clock);
        control.tick(&mut native, false);
        let before = native.control_observation(false).unwrap();
        let command = ratio_command(&control, 0.7);
        control
            .lane
            .as_ref()
            .unwrap()
            .inject_delivery_for_test(lane::Delivery {
                command: command.clone(),
                workbench_snapshot: Some(split_snapshot(1)),
            });
        assert!(!control.tick(&mut native, false));
        let lane = control.lane.as_ref().unwrap();
        assert!(lane.take_layout_for_test().is_some());
        match case {
            "rejected" | "unknown" => lane.finish_layout_for_test(lane::LayoutCompletion {
                operation_id: command.operation_id.clone(),
                reply: Err(SplitRatioFailure {
                    may_have_committed: case == "unknown",
                    reason: Reason::StaleView,
                }),
            }),
            "stale-operation" => lane.finish_layout_for_test(lane::LayoutCompletion {
                operation_id: WorkbenchRequestId::parse(&format!("{:032x}", 999)).unwrap(),
                reply: Ok(WorkbenchRevision::new(2)),
            }),
            _ => {}
        }
        if case == "stale-operation" {
            control.tick(&mut native, false);
            assert!(completion(&control).is_none());
            assert!(control.pending.is_some());
            control.interrupt();
        } else if case == "interrupt" {
            control.interrupt();
        } else if case == "expire" {
            elapsed.store(51_000, std::sync::atomic::Ordering::SeqCst);
        }
        control.tick(&mut native, false);
        let actual = completion(&control).unwrap().1.outcome;
        assert_eq!(
            actual,
            if case == "rejected" {
                ClientViewCompletionOutcome::Rejected {
                    reason: Reason::StaleView,
                }
            } else {
                ClientViewCompletionOutcome::Incomplete {
                    reason: match case {
                        "unknown" => Reason::StaleView,
                        "expire" => Reason::Expired,
                        _ => Reason::Interrupted,
                    },
                }
            }
        );
        assert_eq!(native.control_observation(false).unwrap(), before);
        for _ in 0..3 {
            control.tick(&mut native, false);
            control.after_draw(&mut native);
        }
        assert!(
            control
                .lane
                .as_ref()
                .unwrap()
                .take_layout_for_test()
                .is_none()
        );
        assert!(mailbox.try_recv().unwrap().is_none());
    }
}

#[test]
fn committed_ratio_replaced_before_draw_cannot_claim_success() {
    use crate::workbench_client::ui::tests::publication_hint::settle_queued_refresh;
    use orchbee_workbench_protocol::{PanelNode, WorkbenchSnapshotOutcome, WorkbenchSplitRatio};
    let (mut native, mailbox, clock, _) = ready_split_ui();
    let poll = mailbox.try_recv().unwrap().unwrap();
    mailbox.finish_lease(poll).unwrap();
    let mut control = controller(clock.clone());
    control.tick(&mut native, false);
    let command = ratio_command(&control, 0.7);
    control
        .lane
        .as_ref()
        .unwrap()
        .inject_delivery_for_test(lane::Delivery {
            command: command.clone(),
            workbench_snapshot: Some(split_snapshot(1)),
        });
    control.tick(&mut native, false);
    control
        .lane
        .as_ref()
        .unwrap()
        .take_layout_for_test()
        .unwrap();
    control
        .lane
        .as_ref()
        .unwrap()
        .finish_layout_for_test(lane::LayoutCompletion {
            operation_id: command.operation_id,
            reply: Ok(WorkbenchRevision::new(2)),
        });
    control.tick(&mut native, false);
    let mut changed = split_snapshot(3);
    let PanelNode::Split { ratio, .. } = &mut changed.terminal_contexts[0].tabs[0].root else {
        panic!()
    };
    *ratio = WorkbenchSplitRatio::new(0.3).unwrap();
    settle_queued_refresh(
        &mut native,
        &mailbox,
        &clock,
        WorkbenchSnapshotOutcome::Available { snapshot: changed },
    );
    control.tick(&mut native, false);
    control.after_draw(&mut native);
    assert_eq!(
        completion(&control).unwrap().1.outcome,
        ClientViewCompletionOutcome::Incomplete {
            reason: Reason::StaleView
        }
    );
}
