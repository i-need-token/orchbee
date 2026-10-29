//! Validates bounded canonical Workbench documents and applies deltas or publications
//! atomically after checking revision and identity relationships. It performs pure value
//! transitions; authoritative commit, persistence, and publication remain with the Server.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
};

use crate::{
    identity::{
        TerminalId, TerminalPresentationId, WorkbenchPanelId, WorkbenchPanelSlotId,
        WorkbenchSplitId, WorkbenchTabGroupId, WorkbenchTabId, WorkbenchTerminalContextId,
    },
    model::{
        Delta, DeltaChange, DeltaRejectionReason, Navigation, PanelNode, Presentation, Snapshot,
        SnapshotReadIntent, TerminalContext, WorkbenchProtocolDocument,
        WorkbenchPublicationDocument,
    },
};

pub const MAXIMUM_ENCODED_INTENT_BYTES: usize = 16 * 1024;
pub const MAXIMUM_ENCODED_PROJECTION_BYTES: usize = 1024 * 1024;
pub const MAXIMUM_LABEL_LENGTH: usize = 512;
pub const MAXIMUM_PROJECTS: usize = 256;
pub const MAXIMUM_DIRECTORIES_PER_PROJECT: usize = 256;
/// Session capacity preserves the aggregate admitted by historical Project-owned directories.
pub const MAXIMUM_DIRECTORIES: usize = MAXIMUM_PROJECTS * MAXIMUM_DIRECTORIES_PER_PROJECT;
pub const MAXIMUM_WORKSPACES_PER_PROJECT: usize = 256;
pub const MAXIMUM_TERMINAL_CONTEXTS: usize = 1024;
pub const MAXIMUM_TAB_GROUPS_PER_CONTEXT: usize = 64;
pub const MAXIMUM_TABS_PER_CONTEXT: usize = 256;
pub const MAXIMUM_CHILD_PRESENTATIONS_PER_PANEL: usize = 16;
pub const MAXIMUM_PANEL_NODE_DEPTH: usize = 16;
pub const MAXIMUM_PANEL_NODES_PER_CONTEXT: usize = 4096;
pub const MAXIMUM_DELTA_CHANGES: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractError {
    EncodedValueTooLarge { actual: usize, maximum: usize },
    InvalidDocument,
    WrongDocumentKind,
}

impl fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EncodedValueTooLarge { actual, maximum } => {
                write!(
                    formatter,
                    "encoded value uses {actual} bytes; maximum is {maximum}"
                )
            }
            Self::InvalidDocument => formatter.write_str("invalid Workbench contract document"),
            Self::WrongDocumentKind => formatter.write_str("unexpected Workbench document kind"),
        }
    }
}

impl std::error::Error for ContractError {}

pub fn parse_document(encoded: &[u8]) -> Result<WorkbenchProtocolDocument, ContractError> {
    enforce_encoded_limit(encoded, MAXIMUM_ENCODED_PROJECTION_BYTES)?;
    let document = serde_json::from_slice::<WorkbenchProtocolDocument>(encoded)
        .map_err(|_error| ContractError::InvalidDocument)?;
    match &document {
        WorkbenchProtocolDocument::SnapshotRead(_intent) => {
            enforce_encoded_limit(encoded, MAXIMUM_ENCODED_INTENT_BYTES)?;
        }
        WorkbenchProtocolDocument::Snapshot(snapshot) => validate_snapshot(snapshot)?,
        WorkbenchProtocolDocument::Delta(delta) => validate_delta_structure(delta)?,
    }
    Ok(document)
}

pub fn parse_snapshot_read_intent(encoded: &[u8]) -> Result<SnapshotReadIntent, ContractError> {
    match parse_document(encoded)? {
        WorkbenchProtocolDocument::SnapshotRead(intent) => Ok(intent),
        WorkbenchProtocolDocument::Snapshot(_) | WorkbenchProtocolDocument::Delta(_) => {
            Err(ContractError::WrongDocumentKind)
        }
    }
}

pub fn parse_snapshot(encoded: &[u8]) -> Result<Snapshot, ContractError> {
    match parse_document(encoded)? {
        WorkbenchProtocolDocument::Snapshot(snapshot) => Ok(snapshot),
        WorkbenchProtocolDocument::SnapshotRead(_) | WorkbenchProtocolDocument::Delta(_) => {
            Err(ContractError::WrongDocumentKind)
        }
    }
}

pub fn parse_delta(encoded: &[u8]) -> Result<Delta, ContractError> {
    match parse_document(encoded)? {
        WorkbenchProtocolDocument::Delta(delta) => Ok(delta),
        WorkbenchProtocolDocument::SnapshotRead(_) | WorkbenchProtocolDocument::Snapshot(_) => {
            Err(ContractError::WrongDocumentKind)
        }
    }
}

pub fn serialize_document(document: &WorkbenchProtocolDocument) -> Result<Vec<u8>, ContractError> {
    let encoded = serde_json::to_vec(document).map_err(|_error| ContractError::InvalidDocument)?;
    let maximum = match document {
        WorkbenchProtocolDocument::SnapshotRead(_intent) => MAXIMUM_ENCODED_INTENT_BYTES,
        WorkbenchProtocolDocument::Snapshot(_) | WorkbenchProtocolDocument::Delta(_) => {
            MAXIMUM_ENCODED_PROJECTION_BYTES
        }
    };
    enforce_encoded_limit(&encoded, maximum)?;
    Ok(encoded)
}

pub fn apply_workbench_delta(
    snapshot: &Snapshot,
    delta: &Delta,
) -> Result<Snapshot, DeltaRejectionReason> {
    if snapshot.session_id != delta.session_id {
        return Err(DeltaRejectionReason::SessionMismatch);
    }
    if snapshot.revision != delta.base_revision {
        return Err(DeltaRejectionReason::BaseRevisionMismatch);
    }
    if delta.revision <= delta.base_revision {
        return Err(DeltaRejectionReason::NonIncreasingRevision);
    }
    if validate_snapshot(snapshot).is_err() || validate_delta_structure(delta).is_err() {
        return Err(DeltaRejectionReason::InvalidProjection);
    }

    let mut navigation = snapshot.navigation.clone();
    let mut navigation_replaced = false;
    let mut changed_context_ids = HashSet::new();
    let mut terminal_contexts = snapshot
        .terminal_contexts
        .iter()
        .cloned()
        .map(|terminal_context| {
            (
                terminal_context.terminal_context_id.clone(),
                terminal_context,
            )
        })
        .collect::<BTreeMap<_, _>>();

    for change in &delta.changes {
        match change {
            DeltaChange::NavigationReplaced {
                navigation: replacement,
            } => {
                if navigation_replaced {
                    return Err(DeltaRejectionReason::DuplicateChangeTarget);
                }
                navigation_replaced = true;
                navigation = replacement.clone();
            }
            DeltaChange::TerminalContextUpserted { terminal_context } => {
                let terminal_context_id = terminal_context.terminal_context_id.clone();
                if !changed_context_ids.insert(terminal_context_id.clone()) {
                    return Err(DeltaRejectionReason::DuplicateChangeTarget);
                }
                terminal_contexts.insert(terminal_context_id, terminal_context.clone());
            }
            DeltaChange::TerminalContextRemoved {
                terminal_context_id,
            } => {
                if !changed_context_ids.insert(terminal_context_id.clone()) {
                    return Err(DeltaRejectionReason::DuplicateChangeTarget);
                }
                terminal_contexts.remove(terminal_context_id);
            }
        }
    }

    let candidate = Snapshot {
        navigation,
        protocol_version: delta.protocol_version,
        revision: delta.revision,
        session_id: snapshot.session_id.clone(),
        terminal_contexts: terminal_contexts.into_values().collect(),
    };
    validate_snapshot(&candidate).map_err(|_error| DeltaRejectionReason::InvalidProjection)?;
    Ok(candidate)
}

/// Folds one ordered committed-publication batch over `base`. Every Delta must name the running
/// revision as its base; a Snapshot document requires a strictly increasing revision and replaces
/// the running projection outright. An empty batch is an accepted no-op.
pub fn apply_workbench_publication(
    base: &Snapshot,
    publications: &[WorkbenchPublicationDocument],
) -> Result<Snapshot, DeltaRejectionReason> {
    let mut current = base.clone();
    for publication in publications {
        current = match publication {
            WorkbenchPublicationDocument::Snapshot(snapshot) => {
                if snapshot.session_id != current.session_id {
                    return Err(DeltaRejectionReason::SessionMismatch);
                }
                if snapshot.revision <= current.revision {
                    return Err(DeltaRejectionReason::NonIncreasingRevision);
                }
                if validate_snapshot(snapshot).is_err() {
                    return Err(DeltaRejectionReason::InvalidProjection);
                }
                snapshot.clone()
            }
            WorkbenchPublicationDocument::Delta(delta) => apply_workbench_delta(&current, delta)?,
        };
    }
    Ok(current)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ValidationError;

#[derive(Default)]
struct SnapshotIdentities {
    bound_terminal_ids: HashSet<TerminalId>,
    group_ids: HashSet<WorkbenchTabGroupId>,
    panel_ids: HashSet<WorkbenchPanelId>,
    presentation_ids: HashSet<TerminalPresentationId>,
    slot_ids: HashSet<WorkbenchPanelSlotId>,
    split_ids: HashSet<WorkbenchSplitId>,
    tab_ids: HashSet<WorkbenchTabId>,
}

#[derive(Default)]
struct ContextTreeState {
    node_count: usize,
}

pub(crate) fn validate_snapshot(snapshot: &Snapshot) -> Result<(), ContractError> {
    validate_navigation_structure(&snapshot.navigation)
        .map_err(|_error| ContractError::InvalidDocument)?;
    if !strictly_ordered_by(&snapshot.navigation.projects, |project| &project.project_id)
        || snapshot.terminal_contexts.len() > MAXIMUM_TERMINAL_CONTEXTS
        || !strictly_ordered_by(&snapshot.terminal_contexts, |context| {
            &context.terminal_context_id
        })
    {
        return Err(ContractError::InvalidDocument);
    }

    let mut referenced_context_counts = HashMap::<WorkbenchTerminalContextId, usize>::new();
    for workspace in &snapshot.navigation.workspaces {
        increment_reference(
            &mut referenced_context_counts,
            workspace.terminal_context_id.clone(),
        );
    }

    let context_ids = snapshot
        .terminal_contexts
        .iter()
        .map(|context| context.terminal_context_id.clone())
        .collect::<HashSet<_>>();
    if referenced_context_counts
        .iter()
        .any(|(identity, count)| *count != 1 || !context_ids.contains(identity))
        || context_ids
            .iter()
            .any(|identity| !referenced_context_counts.contains_key(identity))
    {
        return Err(ContractError::InvalidDocument);
    }

    let mut identities = SnapshotIdentities::default();
    for terminal_context in &snapshot.terminal_contexts {
        validate_terminal_context(terminal_context, &mut identities)
            .map_err(|_error| ContractError::InvalidDocument)?;
    }
    Ok(())
}

pub(crate) fn validate_delta_structure(delta: &Delta) -> Result<(), ContractError> {
    if delta.changes.len() > MAXIMUM_DELTA_CHANGES {
        return Err(ContractError::InvalidDocument);
    }
    for change in &delta.changes {
        match change {
            DeltaChange::NavigationReplaced { navigation } => {
                validate_navigation_structure(navigation)
                    .map_err(|_error| ContractError::InvalidDocument)?;
            }
            DeltaChange::TerminalContextUpserted { terminal_context } => {
                validate_terminal_context_structure(terminal_context, false)
                    .map_err(|_error| ContractError::InvalidDocument)?;
            }
            DeltaChange::TerminalContextRemoved {
                terminal_context_id: _terminal_context_id,
            } => {}
        }
    }
    Ok(())
}

pub(crate) fn validate_navigation_structure(
    navigation: &Navigation,
) -> Result<(), ValidationError> {
    if navigation.projects.len() > MAXIMUM_PROJECTS
        || navigation.directories.len() > MAXIMUM_DIRECTORIES
        || navigation.workspaces.len() > MAXIMUM_TERMINAL_CONTEXTS
        || !strictly_ordered_by(&navigation.projects, |project| &project.project_id)
        || !strictly_ordered_by(&navigation.directories, |directory| &directory.directory_id)
        || !strictly_ordered_by(&navigation.workspaces, |workspace| &workspace.workspace_id)
    {
        return Err(ValidationError);
    }
    if !navigation.workspace_order.is_empty() {
        navigation
            .ordered_workspaces()
            .map_err(|_| ValidationError)?;
    }
    navigation.ordered_projects().map_err(|_| ValidationError)?;
    for project in &navigation.projects {
        validate_label(&project.label)?;
    }
    let project_ids: HashSet<_> = navigation
        .projects
        .iter()
        .map(|project| &project.project_id)
        .collect();
    let directory_ids: HashSet<_> = navigation
        .directories
        .iter()
        .map(|directory| &directory.directory_id)
        .collect();
    let mut group_sizes = HashMap::new();
    let mut context_ids = HashSet::new();
    for workspace in &navigation.workspaces {
        validate_label(&workspace.label)?;
        if !context_ids.insert(&workspace.terminal_context_id) {
            return Err(ValidationError);
        }
        if let Some(project_id) = &workspace.project_id {
            if !project_ids.contains(project_id) {
                return Err(ValidationError);
            }
            let count = group_sizes.entry(project_id).or_insert(0);
            *count += 1;
            if *count > MAXIMUM_WORKSPACES_PER_PROJECT {
                return Err(ValidationError);
            }
        }
        if workspace
            .primary_directory_id
            .as_ref()
            .is_some_and(|id| !directory_ids.contains(id))
        {
            return Err(ValidationError);
        }
    }
    Ok(())
}

fn validate_terminal_context(
    terminal_context: &TerminalContext,
    identities: &mut SnapshotIdentities,
) -> Result<(), ValidationError> {
    validate_terminal_context_structure(terminal_context, true)?;
    let mut local_group_ids = HashSet::new();
    for group in &terminal_context.groups {
        if !local_group_ids.insert(group.group_id.clone())
            || !identities.group_ids.insert(group.group_id.clone())
        {
            return Err(ValidationError);
        }
    }

    let mut found_unpinned_tab = false;
    let mut group_positions = HashMap::<WorkbenchTabGroupId, (usize, usize, usize)>::new();
    let mut tree_state = ContextTreeState::default();
    for (index, tab) in terminal_context.tabs.iter().enumerate() {
        if !identities.tab_ids.insert(tab.tab_id.clone()) {
            return Err(ValidationError);
        }
        if tab.pinned {
            if found_unpinned_tab || tab.group_id.is_some() {
                return Err(ValidationError);
            }
        } else {
            found_unpinned_tab = true;
        }
        if let Some(group_id) = &tab.group_id {
            if !local_group_ids.contains(group_id) {
                return Err(ValidationError);
            }
            group_positions
                .entry(group_id.clone())
                .and_modify(|position| {
                    position.1 = index;
                    position.2 += 1;
                })
                .or_insert((index, index, 1));
        }
        validate_panel_node(&tab.root, 1, &mut tree_state, identities)?;
    }
    if group_positions
        .values()
        .any(|(first, last, count)| last - first + 1 != *count)
        || tree_state.node_count > MAXIMUM_PANEL_NODES_PER_CONTEXT
    {
        return Err(ValidationError);
    }
    Ok(())
}

fn validate_terminal_context_structure(
    terminal_context: &TerminalContext,
    enforce_tree_limits: bool,
) -> Result<(), ValidationError> {
    if terminal_context.groups.len() > MAXIMUM_TAB_GROUPS_PER_CONTEXT
        || terminal_context.tabs.len() > MAXIMUM_TABS_PER_CONTEXT
    {
        return Err(ValidationError);
    }
    for group in &terminal_context.groups {
        crate::WorkbenchTabGroupLabel::parse(&group.label).map_err(|_| ValidationError)?;
    }
    let mut tree_state = ContextTreeState::default();
    for tab in &terminal_context.tabs {
        validate_label(&tab.label)?;
        validate_panel_node_structure(&tab.root, 1, &mut tree_state, enforce_tree_limits)?;
    }
    if enforce_tree_limits && tree_state.node_count > MAXIMUM_PANEL_NODES_PER_CONTEXT {
        return Err(ValidationError);
    }
    Ok(())
}

fn validate_panel_node(
    node: &PanelNode,
    depth: usize,
    state: &mut ContextTreeState,
    identities: &mut SnapshotIdentities,
) -> Result<(), ValidationError> {
    state.node_count += 1;
    if depth > MAXIMUM_PANEL_NODE_DEPTH {
        return Err(ValidationError);
    }
    match node {
        PanelNode::Panel {
            right_click: _,
            child_presentations,
            panel_id,
            primary_presentation,
        } => {
            if !identities.panel_ids.insert(panel_id.clone()) {
                return Err(ValidationError);
            }
            validate_presentation(primary_presentation, identities)?;
            for presentation in child_presentations {
                validate_presentation(presentation, identities)?;
            }
        }
        PanelNode::Split {
            children,
            orientation: _orientation,
            split_id,
            ..
        } => {
            if !identities.split_ids.insert(split_id.clone()) {
                return Err(ValidationError);
            }
            for child in children.iter() {
                if !identities.slot_ids.insert(child.slot_id.clone()) {
                    return Err(ValidationError);
                }
                validate_panel_node(&child.node, depth + 1, state, identities)?;
            }
        }
    }
    Ok(())
}

fn validate_panel_node_structure(
    node: &PanelNode,
    depth: usize,
    state: &mut ContextTreeState,
    enforce_tree_limits: bool,
) -> Result<(), ValidationError> {
    state.node_count += 1;
    if enforce_tree_limits && depth > MAXIMUM_PANEL_NODE_DEPTH {
        return Err(ValidationError);
    }
    match node {
        PanelNode::Panel {
            right_click: _,
            child_presentations,
            panel_id: _panel_id,
            primary_presentation,
        } => {
            if child_presentations.len() > MAXIMUM_CHILD_PRESENTATIONS_PER_PANEL {
                return Err(ValidationError);
            }
            validate_presentation_structure(primary_presentation)?;
            for presentation in child_presentations {
                validate_presentation_structure(presentation)?;
            }
        }
        PanelNode::Split {
            children,
            orientation: _orientation,
            split_id: _split_id,
            ..
        } => {
            for child in children.iter() {
                validate_panel_node_structure(&child.node, depth + 1, state, enforce_tree_limits)?;
            }
        }
    }
    Ok(())
}

fn validate_presentation(
    presentation: &Presentation,
    identities: &mut SnapshotIdentities,
) -> Result<(), ValidationError> {
    validate_presentation_structure(presentation)?;
    if !identities
        .presentation_ids
        .insert(presentation.presentation_id.clone())
    {
        return Err(ValidationError);
    }
    if let Some(binding) = &presentation.binding
        && !identities
            .bound_terminal_ids
            .insert(binding.terminal_id.clone())
    {
        return Err(ValidationError);
    }
    Ok(())
}

fn validate_presentation_structure(presentation: &Presentation) -> Result<(), ValidationError> {
    validate_label(&presentation.label)?;
    // The launched program names the current binding's launch and appears only beside it.
    if presentation.launch_program.is_some() && presentation.binding.is_none() {
        return Err(ValidationError);
    }
    match &presentation.launch_failure {
        None => Ok(()),
        // A failure describes the current binding, and its executable meets the launch contract.
        Some(_) if presentation.binding.is_none() => Err(ValidationError),
        Some(failure) => failure.executable.as_ref().map_or(Ok(()), |executable| {
            crate::LaunchSelection::new(executable.clone(), Vec::new())
                .map(drop)
                .map_err(|_| ValidationError)
        }),
    }
}

fn validate_label(label: &str) -> Result<(), ValidationError> {
    let character_count = label.chars().count();
    if label.trim().is_empty() || character_count > MAXIMUM_LABEL_LENGTH {
        return Err(ValidationError);
    }
    Ok(())
}

fn strictly_ordered_by<Value, Key: Ord>(values: &[Value], key: impl Fn(&Value) -> &Key) -> bool {
    values
        .iter()
        .zip(values.iter().skip(1))
        .all(|(left, right)| key(left) < key(right))
}

fn increment_reference(
    counts: &mut HashMap<WorkbenchTerminalContextId, usize>,
    identity: WorkbenchTerminalContextId,
) {
    counts
        .entry(identity)
        .and_modify(|count| *count += 1)
        .or_insert(1);
}

pub(crate) fn enforce_encoded_limit(encoded: &[u8], maximum: usize) -> Result<(), ContractError> {
    if encoded.len() > maximum {
        return Err(ContractError::EncodedValueTooLarge {
            actual: encoded.len(),
            maximum,
        });
    }
    Ok(())
}
