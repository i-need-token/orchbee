use super::*;
use orchbee_workbench_protocol::{
    ClientPopupSource, ClientPresentationSelection, ClientViewAvailability, ClientViewPhase,
    ClientViewRecord, ClientViewSelection, ClientViewportObservation, ClientViewportUnavailable,
    ConfiguredCommandFailure, PresentationBinding, TerminalId, TerminalPresentationId,
    WorkbenchClientId, WorkbenchPanelId, WorkbenchRevision, WorkbenchTabId,
    WorkbenchTerminalContextId, WorkerId,
};

fn record(sequence: u64, phase: ClientViewPhase) -> ClientViewRecord {
    ClientViewRecord {
        client_id: WorkbenchClientId::parse(&format!("{:032x}", 1)).unwrap(),
        heartbeat: ClientViewSequence::new(sequence).unwrap(),
        view_revision: ClientViewSequence::new(sequence).unwrap(),
        availability: ClientViewAvailability::Current,
        view: ClientViewObservation {
            workbench_revision: WorkbenchRevision::new(7),
            split_revision: None,
            phase,
            selection: Some(ClientViewSelection {
                terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{:032x}", 2))
                    .unwrap(),
                tab_id: Some(WorkbenchTabId::parse(&format!("{:032x}", 3)).unwrap()),
                panel_id: Some(WorkbenchPanelId::parse(&format!("{:032x}", 4)).unwrap()),
                zoomed: false,
                panel_count: 1,
            }),
        },
    }
}

fn source(record: &ClientViewRecord) -> ClientPopupSource {
    let selected = record.view.selection.as_ref().unwrap();
    ClientPopupSource {
        terminal_context_id: selected.terminal_context_id.clone(),
        tab_id: selected.tab_id.clone().unwrap(),
        panel_id: selected.panel_id.clone().unwrap(),
        member: ClientPresentationSelection::Bound {
            presentation_id: TerminalPresentationId::parse(&format!("{:032x}", 5)).unwrap(),
            binding: PresentationBinding {
                worker_id: WorkerId::parse(&format!("{:032x}", 6)).unwrap(),
                terminal_id: TerminalId::parse(&format!("{:032x}", 7)).unwrap(),
            },
            attachment_generation: WorkbenchRevision::new(1),
            incarnation: 1,
            viewport: ClientViewportObservation::Unavailable {
                reason: ClientViewportUnavailable::FrameUnavailable,
            },
        },
    }
}

fn sample(record: &ClientViewRecord) -> Sample {
    Sample {
        heartbeat: record.heartbeat,
        revision: record.view_revision,
        view: record.view.clone(),
        graphics: None,
    }
}

#[test]
fn prefix_busy_registration_waits_for_the_exact_ready_publication_without_catalog_activation() {
    for command_id in ["cmd_startup_0", "cmd_reloaded_0"] {
        let lane = ClientControlLane::for_test();
        let ready = record(2, ClientViewPhase::Ready);
        let source = source(&ready);
        assert_eq!(
            lane.invoke_configured(command_id.into(), Some(source.clone()), sample(&ready)),
            Ok(())
        );
        assert_eq!(
            lane.invoke_configured("duplicate".into(), None, sample(&ready)),
            Err(ConfiguredCommandFailure::Busy)
        );
        let mut handoff = lane.shared.0.lock().unwrap();
        assert!(
            handoff
                .take_ready_invocation(&record(1, ClientViewPhase::Busy))
                .is_none()
        );
        let invocation = handoff
            .take_ready_invocation(&ready)
            .unwrap()
            .bind(&ready)
            .unwrap();
        assert_eq!(invocation.command_id, command_id);
        assert!(invocation.validate().is_ok());
        assert_eq!(invocation.expected_revision, ready.view.workbench_revision);
        let bound = invocation.source.unwrap();
        assert_eq!(bound.client_id, ready.client_id);
        assert_eq!(bound.view_revision, ready.view_revision);
        assert_eq!(bound.presentation, source);
        assert!(handoff.take_ready_invocation(&ready).is_none());
    }
}

#[test]
fn source_or_revision_changes_after_enqueue_refuse_without_retargeting() {
    for change in 0..4 {
        let lane = ClientControlLane::for_test();
        let ready = record(2, ClientViewPhase::Ready);
        lane.invoke_configured("cmd_test_0".into(), Some(source(&ready)), sample(&ready))
            .unwrap();
        let mut changed = ready.clone();
        match change {
            0 => changed.view.phase = ClientViewPhase::Busy,
            1 => changed.view_revision = ClientViewSequence::new(3).unwrap(),
            2 => changed.availability = ClientViewAvailability::Stale,
            _ => changed.view.workbench_revision = WorkbenchRevision::new(8),
        }
        let queued = lane
            .shared
            .0
            .lock()
            .unwrap()
            .take_ready_invocation(&changed)
            .unwrap();
        assert_eq!(
            queued.bind(&changed),
            Err(ConfiguredCommandFailure::StaleSource)
        );
    }
}
