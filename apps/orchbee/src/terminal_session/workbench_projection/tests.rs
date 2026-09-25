use super::*;
use orchbee_workbench_protocol::{
    MAXIMUM_LABEL_LENGTH, Navigation, PanelOrientation, PanelSlot, Presentation,
    PresentationBinding, Project, ProtocolVersion, Snapshot, Tab, TabGroup, TabGroupColor,
    TerminalContext, WorkbenchPanelSlotId, WorkbenchProjectId, WorkbenchSplitId,
    WorkbenchWorkspaceId, Workspace,
};

mod append_only;
mod catch_up;
mod closed;
mod created_tab;
mod label_evolution;
mod membership;
mod moved_tab;
mod panel_close;
mod panel_expectation;
mod panel_move;
mod projector;
mod relocated_member;
mod renamed_tab;
mod reorder;

const SITE_VALUE: u128 = 1;
const STANDALONE_CONTEXT_VALUE: u128 = 2;

fn projected(tab_count: usize, revision: u64) -> NativeContextTabs {
    project_native_standalone_tabs(&snapshot(tab_count, revision)).unwrap()
}

fn snapshot(tab_count: usize, revision: u64) -> Snapshot {
    Snapshot {
        navigation: Navigation {
            projects: Vec::new(),
            standalone_terminal_context_id: context_id(STANDALONE_CONTEXT_VALUE),
        },
        protocol_version: ProtocolVersion,
        revision: WorkbenchRevision::new(revision),
        site_id: site_id(SITE_VALUE),
        terminal_contexts: vec![terminal_context(STANDALONE_CONTEXT_VALUE, tab_count)],
    }
}

fn terminal_context(value: u128, tab_count: usize) -> TerminalContext {
    TerminalContext {
        groups: Vec::new(),
        tabs: (0..tab_count).map(tab).collect(),
        terminal_context_id: context_id(value),
    }
}

fn tab(index: usize) -> Tab {
    let seed = tab_seed(index);
    let label = (index + 1).to_string();
    Tab {
        automatic_label: false,
        group_id: None,
        label: label.clone(),
        pinned: false,
        root: panel(seed + 1, seed + 2, seed + 3, seed + 4, &label),
        tab_id: tab_id(seed),
    }
}

fn panel(panel: u128, presentation: u128, worker: u128, terminal: u128, label: &str) -> PanelNode {
    PanelNode::Panel {
        right_click: Default::default(),
        child_presentations: Vec::new(),
        panel_id: panel_id(panel),
        primary_presentation: Presentation {
            custom_label: None,
            binding: Some(PresentationBinding {
                terminal_id: terminal_id(terminal),
                worker_id: worker_id(worker),
            }),
            label: label.into(),
            presentation_id: presentation_id(presentation),
        },
    }
}

fn created_candidate(
    current_tab_count: usize,
    candidate_tab_count: usize,
    revision: u64,
    requested_label: &str,
) -> NativeContextTabs {
    let mut candidate = snapshot(candidate_tab_count, revision);
    let created = &mut candidate.terminal_contexts[0].tabs[current_tab_count];
    created.label = requested_label.into();
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut created.root
    else {
        unreachable!("fixture tab uses one Panel")
    };
    primary_presentation.label = requested_label.into();
    project_native_standalone_tabs(&candidate).unwrap()
}

fn expectation_for(
    candidate: &NativeContextTabs,
    index: usize,
    committed_revision: u64,
    requested_label: &str,
) -> CreatedTabExpectation {
    let tab = &candidate.tabs[index];
    CreatedTabExpectation::new(
        WorkbenchRevision::new(committed_revision),
        candidate.terminal_context_id().clone(),
        tab.tab_id.clone(),
        tab.panel_id.clone(),
        tab.presentation_id.clone(),
        label(requested_label),
        tab.automatic_label,
    )
}

fn tab_seed(index: usize) -> u128 {
    1_000 + u128::try_from(index).unwrap() * 10
}

fn label(value: &str) -> WorkbenchLabel {
    WorkbenchLabel::parse(value).unwrap()
}

fn site_id(value: u128) -> SiteId {
    SiteId::parse(&id(value)).unwrap()
}

fn context_id(value: u128) -> WorkbenchTerminalContextId {
    WorkbenchTerminalContextId::parse(&id(value)).unwrap()
}

fn tab_id(value: u128) -> WorkbenchTabId {
    WorkbenchTabId::parse(&id(value)).unwrap()
}

fn panel_id(value: u128) -> WorkbenchPanelId {
    WorkbenchPanelId::parse(&id(value)).unwrap()
}

fn presentation_id(value: u128) -> TerminalPresentationId {
    TerminalPresentationId::parse(&id(value)).unwrap()
}

fn worker_id(value: u128) -> WorkerId {
    WorkerId::parse(&id(value)).unwrap()
}

fn terminal_id(value: u128) -> TerminalId {
    TerminalId::parse(&id(value)).unwrap()
}

fn group_id(value: u128) -> orchbee_workbench_protocol::WorkbenchTabGroupId {
    orchbee_workbench_protocol::WorkbenchTabGroupId::parse(&id(value)).unwrap()
}

fn split_id(value: u128) -> WorkbenchSplitId {
    WorkbenchSplitId::parse(&id(value)).unwrap()
}

fn slot_id(value: u128) -> WorkbenchPanelSlotId {
    WorkbenchPanelSlotId::parse(&id(value)).unwrap()
}

fn project_id(value: u128) -> WorkbenchProjectId {
    WorkbenchProjectId::parse(&id(value)).unwrap()
}

fn workspace_id(value: u128) -> WorkbenchWorkspaceId {
    WorkbenchWorkspaceId::parse(&id(value)).unwrap()
}

fn id(value: u128) -> String {
    format!("{value:032x}")
}
