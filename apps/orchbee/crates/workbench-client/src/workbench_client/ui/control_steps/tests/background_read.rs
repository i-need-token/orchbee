//! A background publication read keeps the view Ready but holds the refresh slot. Remote focus
//! waits it out under the original deadline instead of refusing as busy, then acts on what it
//! installed.
use super::*;
use crate::workbench_client::mailbox::{CommandMailbox, QueuedCommand};
use crate::workbench_client::ui::tests::{
    membership::snapshot,
    publication_hint::{attach_split_peer, split_survivor_snapshot},
    refresh_gating::{background_read, settle_background_read},
};
use orchbee_workbench_protocol::{Snapshot, TerminalPresentationId, WorkbenchRevision};

/// Finishes every queued lease and reports whether one of them was another refresh.
fn drain_reads(mailbox: &CommandMailbox) -> bool {
    let mut read = false;
    while let Some(lease) = mailbox.try_recv().unwrap() {
        read |= matches!(lease.record(), QueuedCommand::Refresh(_));
        mailbox.finish_lease(lease).unwrap();
    }
    read
}

/// The current canonical Workbench after an unrelated commit.
fn unrelated_commit(native: &WorkbenchClientUi) -> Snapshot {
    let mut snapshot = native
        .state
        .as_ref()
        .unwrap()
        .cache()
        .canonical_snapshot()
        .clone();
    snapshot.revision = WorkbenchRevision::new(snapshot.revision.value() + 1);
    snapshot
}

#[test]
fn focus_waits_out_a_background_read_then_revalidates_its_target() {
    for kind in ["tab", "panel", "agent"] {
        for ending in ["landed", "removed", "expired"] {
            if kind == "agent" && ending == "removed" {
                continue;
            }
            let (mut native, mailbox, clock, agent) = match kind {
                "tab" => {
                    let (native, mailbox, clock, _) = ready_ui();
                    (native, mailbox, clock, None)
                }
                "panel" => {
                    let (mut native, mailbox, clock, _) = ready_split_ui();
                    attach_split_peer(&mut native, &mailbox, &clock);
                    (native, mailbox, clock, None)
                }
                _ => {
                    let (native, agent, clock) =
                        crate::workbench_client::ui::focused_physical_agent_fixture();
                    let mailbox = native.mailbox.clone().unwrap();
                    (native, mailbox, clock, Some(agent))
                }
            };
            drain_reads(&mailbox);
            let mut control = controller(clock.clone());
            control.tick(&mut native, false);
            let read = background_read(&mut native, &mailbox);
            control.tick(&mut native, false);
            let mut command = command(&control, None);
            let ClientViewIntent::Panel {
                terminal_context_id,
                tab_id,
                panel_id,
            } = command.intent
            else {
                unreachable!()
            };
            let tabs = native.state.as_ref().unwrap().cache().tabs();
            command.intent = match &agent {
                None if kind == "tab" => ClientViewIntent::Tab {
                    terminal_context_id,
                    tab_id: tabs[1].tab_id().clone(),
                },
                None => ClientViewIntent::Panel {
                    terminal_context_id,
                    tab_id,
                    panel_id: tabs[0].panel(1).unwrap().panel_id().clone(),
                },
                Some(agent) => ClientViewIntent::AgentFocus {
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    presentation_id: TerminalPresentationId::parse(
                        agent.binding.presentation_id.as_str(),
                    )
                    .unwrap(),
                    target: orchbee_agent_observation_protocol::AgentNameTarget::Terminal {
                        binding: agent.binding.clone(),
                    },
                },
            };
            let requested = command.intent.clone();
            let before = native.control_observation(false).unwrap();
            inject(&control, command);
            control.tick(&mut native, false);
            assert!(
                completion(&control).is_none(),
                "{kind}/{ending}: a background read is no refusal"
            );
            assert!(
                native.background_reads_yield(),
                "{kind}/{ending}: no further background read starts ahead of the waiting step"
            );
            assert_eq!(
                native.control_observation(false).unwrap(),
                before,
                "{kind}/{ending}: nothing moves while the step waits"
            );
            if ending == "expired" {
                control.pending.as_mut().unwrap().deadline = control.clock.now();
                control.tick(&mut native, false);
                assert_eq!(
                    completion(&control).unwrap().1.outcome,
                    ClientViewCompletionOutcome::Rejected {
                        reason: Reason::Expired
                    },
                    "{kind}: only expiry ends the wait"
                );
                assert_eq!(native.control_observation(false).unwrap(), before);
                continue;
            }
            let installed = match (kind, ending) {
                (_, "landed") => unrelated_commit(&native),
                ("tab", _) => snapshot(1, 2),
                _ => split_survivor_snapshot(2, 0),
            };
            // Output keeps arriving, so another read is due the moment this one applies.
            settle_background_read(&mut native, &mailbox, &clock, read, installed, true);
            assert!(
                !drain_reads(&mailbox),
                "{kind}/{ending}: no background read takes the slot from the waiting step"
            );
            control.tick(&mut native, false);
            if ending == "removed" {
                assert_eq!(
                    completion(&control).unwrap().1.outcome,
                    ClientViewCompletionOutcome::Rejected {
                        reason: Reason::TargetUnavailable
                    },
                    "{kind}: the step revalidates against what the read installed"
                );
                continue;
            }
            assert!(completion(&control).is_none(), "{kind}");
            assert!(
                !native.background_reads_yield(),
                "{kind}: the pause ends once the step acts"
            );
            if kind == "tab" {
                let lease = mailbox.try_recv().unwrap().expect("the Tab switch");
                assert!(matches!(lease.record(), QueuedCommand::Switch(_)));
                continue;
            }
            control.tick(&mut native, false);
            control.after_draw(&mut native);
            let (sample, done) = completion(&control).unwrap();
            assert_eq!(
                done.outcome,
                ClientViewCompletionOutcome::Applied {},
                "{kind}"
            );
            assert!(requested.is_observed(&sample.view, sample.view.workbench_revision));
        }
    }
}
