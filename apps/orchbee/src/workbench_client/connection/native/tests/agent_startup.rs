//! Exact cross-machine Agent startup over the real native worker/mailbox protocol.
use super::{commands::drive_controlling_after_attach, setup::*};
use crate::{
    machine_agents,
    workbench_client::{ClientStartupTarget, state::CarrierFailure},
};
use orchbee_agent_observation_protocol::{AgentHostEpoch, AgentHostTerminalId};
use orchbee_client_server_protocol::*;
use orchbee_workbench_protocol::*;

fn target() -> TestResult<machine_agents::Target> {
    let mut target = machine_agents::tests::target();
    let TerminalViewerOutcome::Attached {
        host_epoch,
        host_terminal_id,
        ..
    } = attached_outcome()?
    else {
        return Err(test_error("Attached fixture"));
    };
    target.physical.host_epoch = AgentHostEpoch::parse(host_epoch.as_str())?;
    target.physical.host_terminal_id = AgentHostTerminalId::parse(host_terminal_id.as_str())?;
    Ok(target)
}
fn observe(harness: &Harness, target: &machine_agents::Target, changed: bool) -> TestResult {
    let request = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &request,
        WorkbenchSnapshotOutcome::Available {
            snapshot: machine_agents::tests::fixture(),
        },
    )?;
    let WorkbenchRequest::PanelAgent(request) = harness.carrier.take_workbench()? else {
        return Err(test_error("Agent verification must precede attachment"));
    };
    assert_eq!(
        request.action,
        PanelAgentAction::States {
            presentation_ids: vec![target.member.clone()]
        }
    );
    let mut binding = target.physical.clone();
    if changed {
        binding.generation = orchbee_agent_observation_protocol::AgentAttachmentGeneration::parse(
            &format!("{:032x}", 99),
        )?;
    }
    harness
        .carrier
        .respond_workbench(WorkbenchResponse::PanelAgent(PanelAgentResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome: PanelAgentOutcome::States {
                workbench_revision: WorkbenchRevision::new(1),
                report_revision: WorkbenchRevision::new(2),
                presentations: vec![TerminalAgentStateRow {
                    presentation_id: target.member.clone(),
                    snapshot: Some(TerminalAgentStateSnapshot {
                        binding,
                        sequence: WorkbenchRevision::new(8),
                        agent: Some(target.agent.clone()),
                        state: PanelAgentState::Working,
                    }),
                }],
            },
        }))
}
#[test]
fn agent_startup_draws_exact_second_tab_child_without_opening_a_terminal() -> TestResult {
    let geometry = fixture_geometry()?;
    let target = target()?;
    let mut steps = vec![WireKind::Workbench; 2];
    steps.extend([WireKind::Obcs; 5]);
    let mut harness = Harness::start_at(
        steps,
        geometry,
        ClientStartupTarget::Agent(Box::new(target.clone())),
    )?;
    observe(&harness, &target, false)?;
    let UiEvent::Bootstrap { key } = harness.ui.settle_next(FrameCut::None)? else {
        return Err(test_error("exact Bootstrap"));
    };
    assert_eq!(key.tab_id(), &target.tab);
    assert_eq!(harness.ui.state().focus_panel_id(), Some(&target.panel));
    let panel = harness.ui.state().cache().tabs()[1].panel(0).unwrap();
    assert_eq!(
        harness
            .ui
            .state()
            .selected_presentation(panel)
            .presentation_id(),
        &target.member
    );
    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("exact child Attach"));
    };
    let TerminalViewerAction::AttachBinding {
        session_id,
        worker_id,
        terminal_id,
    } = request.action()
    else {
        return Err(test_error("must attach canonical binding"));
    };
    assert_eq!(session_id.as_str(), target.context.session_id.as_str());
    assert_eq!(worker_id.as_str(), target.terminal.worker_id.as_str());
    assert_eq!(terminal_id.as_str(), target.terminal.terminal_id.as_str());
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(request.request_id().clone(), attached_outcome()?),
        ))?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Attached { .. }
    ));
    drive_controlling_after_attach(&mut harness, &key, geometry)?;
    assert!(harness.ui.state().ready().is_some());
    harness.finish()
}
#[test]
fn agent_startup_rejects_a_changed_launch_before_any_viewer_or_control_effect() -> TestResult {
    let target = target()?;
    let harness = Harness::start_at(
        vec![WireKind::Workbench; 2],
        fixture_geometry()?,
        ClientStartupTarget::Agent(Box::new(target.clone())),
    )?;
    observe(&harness, &target, true)?;
    harness.finish_after_worker_failure(CarrierFailure::AuthorityDivergence)
}
#[test]
fn agent_startup_detaches_a_replaced_host_before_acquiring_control() -> TestResult {
    let mut target = target()?;
    target.physical.host_epoch = AgentHostEpoch::parse(&format!("{:032x}", 99))?;
    let mut harness = Harness::start_at(
        vec![
            WireKind::Workbench,
            WireKind::Workbench,
            WireKind::Obcs,
            WireKind::Obcs,
        ],
        fixture_geometry()?,
        ClientStartupTarget::Agent(Box::new(target.clone())),
    )?;
    observe(&harness, &target, false)?;
    assert!(matches!(
        harness.ui.settle_next(FrameCut::None)?,
        UiEvent::Bootstrap { .. }
    ));
    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("Attach"));
    };
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(request.request_id().clone(), attached_outcome()?),
        ))?;
    let ClientMessage::TerminalViewer(detach) = harness.carrier.take_obcs()? else {
        return Err(test_error("must detach the mismatched host, never acquire"));
    };
    assert_eq!(
        detach.action(),
        &TerminalViewerAction::Detach {
            attachment_id: attached_viewer_id()?
        }
    );
    harness
        .carrier
        .respond_obcs(ServerMessage::TerminalViewerResult(
            TerminalViewerResult::new(
                detach.request_id().clone(),
                TerminalViewerOutcome::Detached {},
            ),
        ))?;
    harness.finish_after_worker_failure(CarrierFailure::AuthorityDivergence)
}

#[test]
fn agent_startup_missing_unbound_or_moved_member_never_falls_back_to_primary() -> TestResult {
    for case in 0..3 {
        let target = target()?;
        let harness = Harness::start_at(
            vec![WireKind::Workbench],
            fixture_geometry()?,
            ClientStartupTarget::Agent(Box::new(target)),
        )?;
        let mut snapshot = machine_agents::tests::fixture();
        let PanelNode::Panel {
            child_presentations,
            ..
        } = &mut snapshot.terminal_contexts[0].tabs[1].root
        else {
            return Err(test_error("fixture panel"));
        };
        match case {
            0 => child_presentations.clear(),
            1 => child_presentations[0].binding = None,
            _ => {
                let moved = child_presentations.pop().unwrap();
                let PanelNode::Panel {
                    child_presentations,
                    ..
                } = &mut snapshot.terminal_contexts[0].tabs[0].root
                else {
                    return Err(test_error("fixture destination panel"));
                };
                child_presentations.push(moved);
            }
        }
        let request = harness.carrier.take_read()?;
        harness
            .carrier
            .respond_read(&request, WorkbenchSnapshotOutcome::Available { snapshot })?;
        harness.finish_after_worker_failure(CarrierFailure::AuthorityDivergence)?;
    }
    Ok(())
}
