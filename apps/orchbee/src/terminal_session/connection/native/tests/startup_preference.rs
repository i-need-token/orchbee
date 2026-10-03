use super::{commands::drive_controlling_after_attach, setup::*};
use crate::terminal_session::{
    NativeMemberTarget, NativeStartupTarget, startup_preference::Position,
};
use orchbee_client_server_protocol::*;
use orchbee_workbench_protocol::*;

#[test]
fn startup_preference_actual_carrier_restores_second_tab_without_open_or_agent_verification()
-> TestResult {
    let target = crate::machine_agents::tests::target();
    let geometry = fixture_geometry()?;
    let fallback = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut steps = vec![WireKind::Workbench];
    steps.extend([WireKind::Obcs; 5]);
    let mut harness = Harness::start_at(
        steps,
        geometry,
        NativeStartupTarget::LocalDirectory {
            position: Some(Position {
                location: target.context.location.clone(),
                member: Some(NativeMemberTarget {
                    tab: target.tab.clone(),
                    panel: target.panel.clone(),
                    presentation: target.member.clone(),
                }),
            }),
            fallback: std::sync::Arc::clone(&fallback),
            directory: Box::new(crate::terminal_session::StartupDirectory::new(
                None,
                crate::localization::Locale::EnUs,
                true,
            )),
        },
    )?;
    let request = harness.carrier.take_read()?;
    harness.carrier.respond_read(
        &request,
        WorkbenchSnapshotOutcome::Available {
            snapshot: crate::machine_agents::tests::fixture(),
        },
    )?;
    let UiEvent::Bootstrap { key } = harness.ui.settle_next(FrameCut::None)? else {
        return Err(test_error("remembered Bootstrap"));
    };
    assert_eq!(key.tab_id(), &target.tab);
    assert_eq!(harness.ui.state().focus_panel_id(), Some(&target.panel));
    assert!(!fallback.load(std::sync::atomic::Ordering::Acquire));
    let ClientMessage::TerminalViewer(request) = harness.carrier.take_obcs()? else {
        return Err(test_error("canonical AttachBinding"));
    };
    assert!(
        matches!(request.action(), TerminalViewerAction::AttachBinding { site_id, worker_id, terminal_id }
        if site_id.as_str() == target.context.site_id.as_str() && worker_id.as_str() == target.terminal.worker_id.as_str()
            && terminal_id.as_str() == target.terminal.terminal_id.as_str())
    );
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
