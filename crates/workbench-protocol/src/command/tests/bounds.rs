use super::*;
use crate::{Navigation, Project, TerminalContext, Workspace};

fn near_limit_snapshot() -> Snapshot {
    let id = |value: u128| format!("{value:032x}");
    let standalone = WorkbenchTerminalContextId::parse(&id(1)).unwrap();
    let mut snapshot = Snapshot {
        protocol_version: ProtocolVersion,
        site_id: SiteId::parse(&id(10000)).unwrap(),
        revision: WorkbenchRevision::new(1),
        navigation: Navigation {
            projects: Vec::new(),
            standalone_terminal_context_id: standalone.clone(),
        },
        terminal_contexts: vec![TerminalContext {
            terminal_context_id: standalone,
            groups: Vec::new(),
            tabs: Vec::new(),
        }],
    };
    let mut next = 2;
    for _project in 0..2 {
        let mut project = Project {
            directories: Vec::new(),
            project_id: WorkbenchProjectId::parse(&id(next)).unwrap(),
            label: "P".to_owned(),
            workspaces: Vec::new(),
        };
        next += 1;
        for _workspace in 0..256 {
            let workspace_id = WorkbenchWorkspaceId::parse(&id(next)).unwrap();
            next += 1;
            let terminal_context_id = WorkbenchTerminalContextId::parse(&id(next)).unwrap();
            next += 1;
            project.workspaces.push(Workspace {
                workspace_id,
                terminal_context_id: terminal_context_id.clone(),
                label: "\u{1f41d}".repeat(512),
                primary_directory_id: None,
            });
            snapshot.terminal_contexts.push(TerminalContext {
                terminal_context_id,
                groups: Vec::new(),
                tabs: Vec::new(),
            });
        }
        snapshot.navigation.projects.push(project);
    }
    let encoded =
        serde_json::to_vec(&WorkbenchProtocolDocument::Snapshot(snapshot.clone())).unwrap();
    let mut excess = encoded.len() - MAXIMUM_ENCODED_PROJECTION_BYTES;
    for workspace in snapshot
        .navigation
        .projects
        .iter_mut()
        .flat_map(|project| &mut project.workspaces)
    {
        let removed = (excess / 4).min(511);
        workspace
            .label
            .truncate(workspace.label.len() - removed * 4);
        excess -= removed * 4;
        if excess > 0 && excess < 4 {
            workspace.label.pop();
            workspace.label.push(match excess {
                1 => '\u{20ac}',
                2 => '\u{a2}',
                3 => 'x',
                _ => unreachable!(),
            });
            excess = 0;
        }
        if excess == 0 {
            break;
        }
    }
    assert_eq!(
        serde_json::to_vec(&WorkbenchProtocolDocument::Snapshot(snapshot.clone()))
            .unwrap()
            .len(),
        MAXIMUM_ENCODED_PROJECTION_BYTES
    );
    snapshot
}

#[test]
fn nested_canonical_snapshot_and_raw_outer_result_enforce_distinct_exact_bounds() {
    let snapshot = near_limit_snapshot();
    let result = |snapshot| {
        WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse("11111111111111111111111111111111").unwrap(),
            outcome: WorkbenchSnapshotOutcome::Available { snapshot },
        })
    };
    let response = result(snapshot.clone());
    let mut encoded = serialize_workbench_response(&response).unwrap();
    encoded.resize(MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES, b' ');
    assert_eq!(parse_workbench_response(&encoded).unwrap(), response);
    encoded.push(b' ');
    assert!(matches!(
        parse_workbench_response(&encoded),
        Err(ContractError::EncodedValueTooLarge {
            maximum: MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES,
            ..
        })
    ));
    let mut too_large = snapshot;
    too_large.navigation.projects[0].label.push('x');
    let over_limit = result(too_large);
    let raw = serde_json::to_vec(&over_limit).unwrap();
    assert!(raw.len() < MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES);
    assert!(matches!(
        parse_workbench_response(&raw),
        Err(ContractError::EncodedValueTooLarge {
            maximum: MAXIMUM_ENCODED_PROJECTION_BYTES,
            ..
        })
    ));
    assert!(serialize_workbench_response(&over_limit).is_err());
}
