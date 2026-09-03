use std::{error::Error, io};

use orchbee_server::{ProjectionAdvanceError, WorkbenchProjectionStore, WorkbenchPublication};
use orchbee_workbench_protocol::{
    ContractError, Delta, DeltaChange, MAXIMUM_DELTA_CHANGES, MAXIMUM_ENCODED_PROJECTION_BYTES,
    MAXIMUM_LABEL_LENGTH, MAXIMUM_TERMINAL_CONTEXTS, MAXIMUM_WORKSPACES_PER_PROJECT, Navigation,
    Project, SiteId, Snapshot, TabGroup, TabGroupColor, TerminalContext, WorkbenchProjectId,
    WorkbenchProtocolDocument, WorkbenchRevision, WorkbenchTabGroupId, WorkbenchTerminalContextId,
    WorkbenchWorkspaceId, Workspace, apply_workbench_delta, parse_snapshot, serialize_document,
};
use serde_json::Value;

const CORPUS: &str = include_str!("../../../schemas/workbench/v1/workbench-conformance-v1.json");

#[test]
fn advances_the_canonical_projection_transitions() -> Result<(), Box<dyn Error>> {
    let intent_bytes = document_bytes("snapshot-read")?;
    let cases = [
        ("snapshot-empty-r1", "snapshot-empty-r2"),
        ("snapshot-empty-r1", "snapshot-updated-r2"),
        ("snapshot-empty-r1", "snapshot-workspace-r2"),
        ("snapshot-workspace-r2", "snapshot-empty-r3"),
    ];

    for (initial_name, candidate_name) in cases {
        let initial_bytes = document_bytes(initial_name)?;
        let candidate_bytes = document_bytes(candidate_name)?;
        let initial = parse_snapshot(&initial_bytes)?;
        let candidate = parse_snapshot(&candidate_bytes)?;
        let mut store = WorkbenchProjectionStore::load(&initial_bytes)?;

        let publication = store.advance_projection(&candidate_bytes)?;
        let WorkbenchPublication::Delta(delta) = publication else {
            return Err(io::Error::other(format!(
                "canonical transition {initial_name} -> {candidate_name} did not emit a delta"
            ))
            .into());
        };
        let applied = apply_workbench_delta(&initial, &delta)
            .map_err(|reason| io::Error::other(format!("delta rejected: {reason:?}")))?;

        assert_eq!(applied, candidate);
        assert_eq!(store.read_snapshot(&intent_bytes)?.snapshot, candidate);
    }
    Ok(())
}

#[test]
fn derives_deterministic_navigation_first_publications() -> Result<(), Box<dyn Error>> {
    let initial_bytes = document_bytes("snapshot-empty-r1")?;
    let candidate_bytes = document_bytes("snapshot-workspace-r2")?;
    let mut first_store = WorkbenchProjectionStore::load(&initial_bytes)?;
    let mut second_store = WorkbenchProjectionStore::load(&initial_bytes)?;

    let first = first_store.advance_projection(&candidate_bytes)?;
    let second = second_store.advance_projection(&candidate_bytes)?;
    assert_eq!(first, second);

    let WorkbenchPublication::Delta(delta) = first else {
        return Err(io::Error::other("expected a delta publication").into());
    };
    assert!(matches!(
        delta.changes.first(),
        Some(DeltaChange::NavigationReplaced { .. })
    ));
    assert!(matches!(
        delta.changes.get(1),
        Some(DeltaChange::TerminalContextUpserted { .. })
    ));
    Ok(())
}

#[test]
fn rejects_candidates_atomically() -> Result<(), Box<dyn Error>> {
    let initial_bytes = document_bytes("snapshot-empty-r1")?;
    let initial = parse_snapshot(&initial_bytes)?;
    let intent_bytes = document_bytes("snapshot-read")?;
    let mut store = WorkbenchProjectionStore::load(&initial_bytes)?;

    assert_eq!(
        store.advance_projection(b"{"),
        Err(ProjectionAdvanceError::Contract(
            ContractError::InvalidDocument
        ))
    );
    assert_eq!(store.read_snapshot(&intent_bytes)?.snapshot, initial);

    let read_intent = document_bytes("snapshot-read")?;
    assert_eq!(
        store.advance_projection(&read_intent),
        Err(ProjectionAdvanceError::Contract(
            ContractError::WrongDocumentKind
        ))
    );
    assert_eq!(store.read_snapshot(&intent_bytes)?.snapshot, initial);

    let invalid_candidate = invalid_document_bytes("missing-referenced-context")?;
    assert_eq!(
        store.advance_projection(&invalid_candidate),
        Err(ProjectionAdvanceError::Contract(
            ContractError::InvalidDocument
        ))
    );
    assert_eq!(store.read_snapshot(&intent_bytes)?.snapshot, initial);

    assert_eq!(
        store.advance_projection(&initial_bytes),
        Err(ProjectionAdvanceError::NonIncreasingRevision)
    );
    assert_eq!(store.read_snapshot(&intent_bytes)?.snapshot, initial);

    let mut other_site = parse_snapshot(&document_bytes("snapshot-empty-r2")?)?;
    other_site.site_id = SiteId::parse("22222222222222222222222222222222")?;
    assert_eq!(
        store.advance_projection(&encode_snapshot(&other_site)?),
        Err(ProjectionAdvanceError::SiteMismatch)
    );
    assert_eq!(store.read_snapshot(&intent_bytes)?.snapshot, initial);

    let candidate_bytes = document_bytes("snapshot-empty-r2")?;
    let padding = MAXIMUM_ENCODED_PROJECTION_BYTES
        .checked_sub(candidate_bytes.len())
        .ok_or_else(|| io::Error::other("candidate fixture exceeds the projection limit"))?;
    let mut oversized_candidate = vec![b' '; padding + 1];
    oversized_candidate.extend_from_slice(&candidate_bytes);
    assert!(matches!(
        store.advance_projection(&oversized_candidate),
        Err(ProjectionAdvanceError::Contract(
            ContractError::EncodedValueTooLarge {
                actual,
                maximum: MAXIMUM_ENCODED_PROJECTION_BYTES
            }
        )) if actual == MAXIMUM_ENCODED_PROJECTION_BYTES + 1
    ));
    assert_eq!(store.read_snapshot(&intent_bytes)?.snapshot, initial);
    Ok(())
}

#[test]
fn accepts_the_exact_candidate_size_boundary() -> Result<(), Box<dyn Error>> {
    let initial_bytes = document_bytes("snapshot-empty-r1")?;
    let candidate_bytes = document_bytes("snapshot-empty-r2")?;
    let padding = MAXIMUM_ENCODED_PROJECTION_BYTES
        .checked_sub(candidate_bytes.len())
        .ok_or_else(|| io::Error::other("candidate fixture exceeds the projection limit"))?;
    let mut bounded_candidate = vec![b' '; padding];
    bounded_candidate.extend_from_slice(&candidate_bytes);
    let mut store = WorkbenchProjectionStore::load(&initial_bytes)?;

    assert!(matches!(
        store.advance_projection(&bounded_candidate)?,
        WorkbenchPublication::Delta(_)
    ));
    Ok(())
}

#[test]
fn selects_delta_at_the_change_limit_and_snapshot_above_it() -> Result<(), Box<dyn Error>> {
    let initial = maximum_context_snapshot()?;
    let initial_bytes = encode_snapshot(&initial)?;
    let candidate = add_one_group_to_every_context(&initial, false)?;
    let candidate_bytes = encode_snapshot(&candidate)?;
    let mut delta_store = WorkbenchProjectionStore::load(&initial_bytes)?;

    let delta_publication = delta_store.advance_projection(&candidate_bytes)?;
    let WorkbenchPublication::Delta(delta) = delta_publication else {
        return Err(io::Error::other("exact change limit did not emit a delta").into());
    };
    assert_eq!(delta.changes.len(), MAXIMUM_DELTA_CHANGES);
    assert_eq!(
        apply_workbench_delta(&initial, &delta)
            .map_err(|reason| io::Error::other(format!("delta rejected: {reason:?}")))?,
        candidate
    );

    let snapshot_candidate = add_one_group_to_every_context(&initial, true)?;
    let snapshot_candidate_bytes = encode_snapshot(&snapshot_candidate)?;
    let mut snapshot_store = WorkbenchProjectionStore::load(&initial_bytes)?;
    assert_eq!(
        snapshot_store.advance_projection(&snapshot_candidate_bytes)?,
        WorkbenchPublication::Snapshot(snapshot_candidate)
    );
    Ok(())
}

#[test]
fn selects_snapshot_when_a_bounded_change_set_exceeds_the_encoded_limit()
-> Result<(), Box<dyn Error>> {
    let initial = maximum_context_snapshot()?;
    let initial_bytes = encode_snapshot(&initial)?;
    let mut candidate = initial.clone();
    candidate.revision = WorkbenchRevision::new(2);
    candidate
        .navigation
        .projects
        .first_mut()
        .ok_or_else(|| io::Error::other("maximum snapshot has no project"))?
        .label = "Changed project".to_owned();

    for (index, terminal_context) in candidate.terminal_contexts.iter_mut().enumerate().skip(1) {
        terminal_context.groups.push(TabGroup {
            color: TabGroupColor::Blue,
            group_id: WorkbenchTabGroupId::parse(&prefixed_identity('4', index))?,
            label: "x".to_owned(),
        });
    }
    fill_group_labels_to_projection_limit(&mut candidate)?;

    let changes = std::iter::once(DeltaChange::NavigationReplaced {
        navigation: candidate.navigation.clone(),
    })
    .chain(
        candidate
            .terminal_contexts
            .iter()
            .skip(1)
            .cloned()
            .map(|terminal_context| DeltaChange::TerminalContextUpserted { terminal_context }),
    )
    .collect::<Vec<_>>();
    assert_eq!(changes.len(), MAXIMUM_DELTA_CHANGES);
    let expected_delta = Delta {
        base_revision: initial.revision,
        changes,
        protocol_version: candidate.protocol_version,
        revision: candidate.revision,
        site_id: candidate.site_id.clone(),
    };
    let encoded_delta = serde_json::to_vec(&WorkbenchProtocolDocument::Delta(expected_delta))?;
    assert!(encoded_delta.len() > MAXIMUM_ENCODED_PROJECTION_BYTES);

    let candidate_bytes = encode_snapshot(&candidate)?;
    assert!(candidate_bytes.len() <= MAXIMUM_ENCODED_PROJECTION_BYTES);
    let mut store = WorkbenchProjectionStore::load(&initial_bytes)?;
    assert_eq!(
        store.advance_projection(&candidate_bytes)?,
        WorkbenchPublication::Snapshot(candidate)
    );
    Ok(())
}

fn maximum_context_snapshot() -> Result<Snapshot, Box<dyn Error>> {
    let template = parse_snapshot(&document_bytes("snapshot-empty-r1")?)?;
    let standalone_terminal_context_id =
        WorkbenchTerminalContextId::parse(&prefixed_identity('1', 0))?;
    let mut terminal_contexts = vec![TerminalContext {
        groups: Vec::new(),
        tabs: Vec::new(),
        terminal_context_id: standalone_terminal_context_id.clone(),
    }];
    let workspace_count = MAXIMUM_TERMINAL_CONTEXTS - 1;
    let project_count = workspace_count.div_ceil(MAXIMUM_WORKSPACES_PER_PROJECT);
    let mut projects = Vec::with_capacity(project_count);

    for project_index in 0..project_count {
        let first_workspace_index = project_index * MAXIMUM_WORKSPACES_PER_PROJECT;
        let last_workspace_index =
            (first_workspace_index + MAXIMUM_WORKSPACES_PER_PROJECT).min(workspace_count);
        let mut workspaces = Vec::with_capacity(last_workspace_index - first_workspace_index);
        for workspace_index in first_workspace_index..last_workspace_index {
            let identity_value = workspace_index + 1;
            let terminal_context_id =
                WorkbenchTerminalContextId::parse(&prefixed_identity('1', identity_value))?;
            terminal_contexts.push(TerminalContext {
                groups: Vec::new(),
                tabs: Vec::new(),
                terminal_context_id: terminal_context_id.clone(),
            });
            workspaces.push(Workspace {
                label: format!("Workspace {identity_value}"),
                terminal_context_id,
                workspace_id: WorkbenchWorkspaceId::parse(&prefixed_identity('3', identity_value))?,
            });
        }
        projects.push(Project {
            label: format!("Project {}", project_index + 1),
            project_id: WorkbenchProjectId::parse(&prefixed_identity('2', project_index + 1))?,
            workspaces,
        });
    }
    terminal_contexts
        .sort_by(|left, right| left.terminal_context_id.cmp(&right.terminal_context_id));

    Ok(Snapshot {
        navigation: Navigation {
            projects,
            standalone_terminal_context_id,
        },
        protocol_version: template.protocol_version,
        revision: WorkbenchRevision::new(1),
        site_id: template.site_id,
        terminal_contexts,
    })
}

fn add_one_group_to_every_context(
    initial: &Snapshot,
    change_navigation: bool,
) -> Result<Snapshot, Box<dyn Error>> {
    let mut candidate = initial.clone();
    candidate.revision = WorkbenchRevision::new(2);
    for (index, terminal_context) in candidate.terminal_contexts.iter_mut().enumerate() {
        terminal_context.groups.push(TabGroup {
            color: TabGroupColor::Blue,
            group_id: WorkbenchTabGroupId::parse(&prefixed_identity('4', index + 1))?,
            label: "Group".to_owned(),
        });
    }
    if change_navigation {
        let first_project = candidate
            .navigation
            .projects
            .first_mut()
            .ok_or_else(|| io::Error::other("maximum snapshot has no project"))?;
        first_project.label = "Changed project".to_owned();
    }
    Ok(candidate)
}

fn fill_group_labels_to_projection_limit(candidate: &mut Snapshot) -> Result<(), Box<dyn Error>> {
    let mut encoded_size =
        serde_json::to_vec(&WorkbenchProtocolDocument::Snapshot(candidate.clone()))?.len();
    if encoded_size > MAXIMUM_ENCODED_PROJECTION_BYTES {
        return Err(
            io::Error::other("minimum changed snapshot exceeds the projection limit").into(),
        );
    }

    for terminal_context in candidate.terminal_contexts.iter_mut().skip(1) {
        let remaining = MAXIMUM_ENCODED_PROJECTION_BYTES - encoded_size;
        let additional_label_bytes = remaining.min(MAXIMUM_LABEL_LENGTH - 1);
        terminal_context.groups[0]
            .label
            .push_str(&"x".repeat(additional_label_bytes));
        encoded_size += additional_label_bytes;
        if additional_label_bytes < MAXIMUM_LABEL_LENGTH - 1 {
            return Ok(());
        }
    }

    for (index, terminal_context) in candidate.terminal_contexts.iter_mut().enumerate().skip(1) {
        let group = TabGroup {
            color: TabGroupColor::Blue,
            group_id: WorkbenchTabGroupId::parse(&prefixed_identity('5', index))?,
            label: "y".to_owned(),
        };
        let minimum_group_bytes = serde_json::to_vec(&group)?.len() + 1;
        let remaining = MAXIMUM_ENCODED_PROJECTION_BYTES - encoded_size;
        if minimum_group_bytes > remaining {
            break;
        }
        terminal_context.groups.push(group);
        encoded_size += minimum_group_bytes;
        let remaining = MAXIMUM_ENCODED_PROJECTION_BYTES - encoded_size;
        let additional_label_bytes = remaining.min(MAXIMUM_LABEL_LENGTH - 1);
        terminal_context.groups[1]
            .label
            .push_str(&"y".repeat(additional_label_bytes));
        encoded_size += additional_label_bytes;
        if additional_label_bytes < MAXIMUM_LABEL_LENGTH - 1 {
            break;
        }
    }

    let actual_size =
        serde_json::to_vec(&WorkbenchProtocolDocument::Snapshot(candidate.clone()))?.len();
    if actual_size != encoded_size {
        return Err(io::Error::other(format!(
            "projection size accounting diverged: expected {encoded_size}, got {actual_size}"
        ))
        .into());
    }
    Ok(())
}

fn prefixed_identity(prefix: char, value: usize) -> String {
    format!("{prefix}{value:031x}")
}

fn encode_snapshot(snapshot: &Snapshot) -> Result<Vec<u8>, ContractError> {
    serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone()))
}

fn document_bytes(name: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let corpus = serde_json::from_str::<Value>(CORPUS)?;
    let document = corpus
        .get("documents")
        .and_then(Value::as_object)
        .and_then(|documents| documents.get(name))
        .ok_or_else(|| io::Error::other(format!("missing conformance document {name}")))?;
    Ok(serde_json::to_vec(document)?)
}

fn invalid_document_bytes(name: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let corpus = serde_json::from_str::<Value>(CORPUS)?;
    let fixture = corpus
        .get("invalidDocuments")
        .and_then(Value::as_array)
        .and_then(|fixtures| {
            fixtures
                .iter()
                .find(|fixture| fixture.get("name").and_then(Value::as_str) == Some(name))
        })
        .and_then(|fixture| fixture.get("value"))
        .ok_or_else(|| io::Error::other(format!("missing invalid conformance document {name}")))?;
    Ok(serde_json::to_vec(fixture)?)
}
