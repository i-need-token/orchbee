//! Shared Agent checkpoint fixtures for unit tests and the owning application's tests.

use super::{
    AgentLaunchBinding, AgentViewSnapshot, Arc, Checkpoint, ClientContextLocation,
    ClientContextTarget, MetadataTokens, PanelAgentState, PanelMetadataEffective, PanelNode,
    PlacementOrder, PresentationBinding, ProtocolVersion, Row, SessionId, Snapshot,
    StateCheckpoint, Target, TerminalAgentStateSnapshot, TerminalPresentationId, WorkbenchPanelId,
    WorkbenchRevision, WorkbenchTabId, placements,
};
use orchbee_workbench_protocol::{
    Navigation, Presentation, Tab, TerminalContext, TerminalId, WorkbenchTerminalContextId,
    WorkerId,
};

pub(crate) fn id(n: u64) -> String {
    format!("{n:032x}")
}

pub fn state_checkpoint(states: Vec<TerminalAgentStateSnapshot>) -> StateCheckpoint {
    let placements = placements(&fixture()).unwrap();
    let mut observed = placements
        .iter()
        .map(|p| (p.member.clone(), None))
        .collect::<std::collections::BTreeMap<_, _>>();
    for state in states {
        observed.insert(
            TerminalPresentationId::parse(state.binding.presentation_id.as_str()).unwrap(),
            Some(state),
        );
    }
    StateCheckpoint {
        placements,
        observed,
        revision: WorkbenchRevision::new(7),
        bytes: 0,
    }
}

pub fn fixture() -> Snapshot {
    Snapshot {
        protocol_version: ProtocolVersion,
        session_id: SessionId::parse(&id(1)).unwrap(),
        revision: WorkbenchRevision::new(1),
        navigation: {
            let fixture_context_id = WorkbenchTerminalContextId::parse(&id(2)).unwrap();
            let fixture_workspace_id = orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                fixture_context_id.as_str(),
            )
            .unwrap();
            Navigation {
                project_order: Vec::new(),
                directories: Vec::new(),
                workspaces: std::iter::once(orchbee_workbench_protocol::Workspace {
                    workspace_id: fixture_workspace_id.clone(),
                    terminal_context_id: fixture_context_id.clone(),
                    project_id: None,
                    primary_directory_id: None,
                    label: "Workspace".into(),
                })
                .chain(Vec::new())
                .collect(),
                projects: Vec::new(),
                workspace_order: Vec::new(),
            }
        },
        terminal_contexts: vec![TerminalContext {
            terminal_context_id: WorkbenchTerminalContextId::parse(&id(2)).unwrap(),
            groups: Vec::new(),
            tabs: [10, 20]
                .into_iter()
                .map(|n| Tab {
                    automatic_label: false,
                    group_id: None,
                    label: "tab".into(),
                    pinned: false,
                    tab_id: WorkbenchTabId::parse(&id(n)).unwrap(),
                    root: PanelNode::Panel {
                        right_click: Default::default(),
                        panel_id: WorkbenchPanelId::parse(&id(n + 1)).unwrap(),
                        primary_presentation: presentation(n + 2),
                        child_presentations: vec![presentation(n + 3)],
                    },
                })
                .collect(),
        }],
    }
}

pub(crate) fn presentation(n: u64) -> Presentation {
    Presentation {
        custom_label: None,
        label: "terminal\u{202e}label".into(),
        presentation_id: TerminalPresentationId::parse(&id(n)).unwrap(),
        binding: Some(PresentationBinding {
            worker_id: WorkerId::parse(&id(99)).unwrap(),
            terminal_id: TerminalId::parse(&id(n + 100)).unwrap(),
        }),
        launch_failure: None,
        launch_program: None,
    }
}

pub fn physical(n: u64) -> AgentLaunchBinding {
    use orchbee_agent_observation_protocol::{
        AgentAttachmentGeneration, AgentHostEpoch, AgentHostTerminalId, AgentLaunchBinding,
        AgentPresentationId,
    };
    AgentLaunchBinding {
        presentation_id: AgentPresentationId::parse(&id(n)).unwrap(),
        generation: AgentAttachmentGeneration::parse(&id(1)).unwrap(),
        host_epoch: AgentHostEpoch::parse(&id(2)).unwrap(),
        host_terminal_id: AgentHostTerminalId::parse(&id(n + 100)).unwrap(),
    }
}

pub fn row(n: u64) -> Row {
    let mut target = target();
    target.member = TerminalPresentationId::parse(&id(n)).unwrap();
    target.terminal = presentation(n).binding.unwrap();
    target.physical = physical(n);
    Row {
        seen: true,
        name: None,
        order: PlacementOrder::default(),
        target,
        label: "build".into(),
        workspace: "space".into(),
        tab_label: Some("Task".into()),
        terminal_title: orchbee_workbench_protocol::TerminalTitleObservation::Unavailable {},
        state: PanelAgentState::Working,
        sequence: WorkbenchRevision::new(1),
        metadata: PanelMetadataEffective::default(),
        tokens: MetadataTokens::default(),
    }
}

pub(crate) fn target() -> Target {
    Target {
        context: ClientContextTarget {
            session_id: SessionId::parse(&id(1)).unwrap(),
            location: {
                let fixture_context_id = WorkbenchTerminalContextId::parse(&id(2)).unwrap();
                ClientContextLocation::Workspace {
                    workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                        fixture_context_id.as_str(),
                    )
                    .unwrap(),
                    terminal_context_id: fixture_context_id,
                }
            },
        },
        tab: WorkbenchTabId::parse(&id(20)).unwrap(),
        panel: WorkbenchPanelId::parse(&id(21)).unwrap(),
        member: TerminalPresentationId::parse(&id(23)).unwrap(),
        terminal: presentation(23).binding.unwrap(),
        physical: physical(23),
        agent: "codex".into(),
    }
}

pub fn checkpoint(rows: Vec<Row>) -> Arc<Checkpoint> {
    Arc::new(Checkpoint {
        session_id: SessionId::parse(&id(1)).unwrap(),
        report_revision: WorkbenchRevision::new(2),
        contexts: std::iter::once(target().context)
            .chain(rows.iter().map(|r| r.target.context.clone()))
            .collect::<Vec<_>>()
            .into(),
        view: AgentViewSnapshot {
            revision: WorkbenchRevision::new(0),
            spec: None,
        },
        rows: rows.into(),
    })
}
