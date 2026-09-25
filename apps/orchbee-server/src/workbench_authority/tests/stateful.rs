use super::*;
use orchbee_workbench_protocol::parse_snapshot;
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
enum Step {
    CreateProject(u8),
    CreateWorkspace(u8),
    RenameProject(u8),
    RenameWorkspace(u8),
    RenameTab(u8),
    Replay(u8),
    Conflict(u8),
    Stale,
    Denied,
    Open,
}

#[test]
fn generated_workbench_traces_preserve_membership_revision_identity_replay_and_atomicity() {
    for seed in 1_u64..=96 {
        let mut random = seed;
        let steps: Vec<_> = (0..80)
            .map(|_| {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                let label = random.to_le_bytes()[2] % 5;
                match random % 10 {
                    0 => Step::CreateProject(label),
                    1 => Step::CreateWorkspace(label),
                    2 => Step::RenameProject(label),
                    3 => Step::RenameWorkspace(label),
                    4 => Step::RenameTab(label),
                    5 => Step::Replay(label),
                    6 => Step::Conflict(label),
                    7 => Step::Stale,
                    8 => Step::Denied,
                    _ => Step::Open,
                }
            })
            .collect();
        if let Err(failure) = run(&steps) {
            let reduced = shrink(steps);
            panic!(
                "seed={seed}; first={failure}; reduced={reduced:?}; failure={:?}",
                run(&reduced)
            );
        }
    }
}

fn shrink(mut steps: Vec<Step>) -> Vec<Step> {
    let mut index = 0;
    while index < steps.len() {
        let mut candidate = steps.clone();
        candidate.remove(index);
        if run(&candidate).is_err() {
            steps = candidate;
            index = 0;
        } else {
            index += 1;
        }
    }
    steps
}

fn require(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.to_owned())
    }
}

fn run(steps: &[Step]) -> Result<(), String> {
    let (mut authority, grant) = authority();
    let reader = WorkbenchGrant::read_only(grant.site_id.clone());
    let mut history: Vec<(WorkbenchCommand, WorkbenchCommandResult)> = Vec::new();
    let mut seen_identities = BTreeSet::new();
    seen_identities.insert(identity(1));
    for (index, step) in steps.iter().enumerate() {
        let before = snapshot(&mut authority, &grant);
        let old_cursor = authority.last_identity;
        let old_ledger = authority.ledger.len();
        if matches!(step, Step::Open) {
            let binding = orchbee_workbench_protocol::PresentationBinding {
                terminal_id: orchbee_workbench_protocol::TerminalId::parse(&identity(700)).unwrap(),
                worker_id: orchbee_workbench_protocol::WorkerId::parse(&identity(800)).unwrap(),
            };
            let result = authority.commit_standalone_intent(binding.clone());
            let after = snapshot(&mut authority, &grant);
            require(
                authority.ledger.len() == old_ledger,
                "internal membership entered command ledger",
            )?;
            if before.terminal_contexts[0].tabs.is_empty() {
                require(result.is_ok(), "first internal membership refused")?;
                require(
                    after.revision.value() == before.revision.value() + 1
                        && authority.last_identity == old_cursor + 3,
                    "membership revision or cursor differs",
                )?;
                let tabs = &after.terminal_contexts[0].tabs;
                require(tabs.len() == 1, "membership did not create one Tab")?;
                let orchbee_workbench_protocol::PanelNode::Panel {
                    panel_id,
                    primary_presentation,
                    ..
                } = &tabs[0].root
                else {
                    return Err("membership did not create a Panel".to_owned());
                };
                require(
                    primary_presentation.binding == Some(binding),
                    "membership changed logical binding",
                )?;
                for id in [
                    tabs[0].tab_id.to_string(),
                    panel_id.to_string(),
                    primary_presentation.presentation_id.to_string(),
                ] {
                    require(
                        seen_identities.insert(id),
                        "membership reused aggregate identity",
                    )?;
                }
            } else {
                require(
                    result == Err(StandaloneIntentError::Occupied)
                        && after == before
                        && authority.last_identity == old_cursor,
                    "duplicate membership changed authority",
                )?;
            }
            let bytes = serialize_document(&WorkbenchProtocolDocument::Snapshot(after))
                .map_err(|e| e.to_string())?;
            parse_snapshot(&bytes).map_err(|e| e.to_string())?;
            continue;
        }
        let project = before.navigation.projects.first().map_or_else(
            || WorkbenchProjectId::parse(&identity(9999)).unwrap(),
            |p| p.project_id.clone(),
        );
        let workspace = before
            .navigation
            .projects
            .first()
            .and_then(|p| p.workspaces.first())
            .map_or_else(
                || WorkbenchWorkspaceId::parse(&identity(9998)).unwrap(),
                |w| w.workspace_id.clone(),
            );
        let label = |number| WorkbenchLabel::parse(&format!("label-{number}")).unwrap();
        let action = match step {
            Step::CreateProject(n) => WorkbenchAction::ProjectCreate { label: label(n) },
            Step::CreateWorkspace(n) => WorkbenchAction::WorkspaceCreate {
                project_id: project,
                label: label(n),
            },
            Step::RenameProject(n) => WorkbenchAction::ProjectRename {
                project_id: project,
                label: label(n),
            },
            Step::RenameWorkspace(n) => WorkbenchAction::WorkspaceRename {
                project_id: project,
                workspace_id: workspace,
                label: label(n),
            },
            Step::RenameTab(n) => {
                let terminal_context_id = before.navigation.standalone_terminal_context_id.clone();
                let tab_id = before
                    .terminal_contexts
                    .iter()
                    .find(|context| context.terminal_context_id == terminal_context_id)
                    .and_then(|context| context.tabs.first())
                    .map_or_else(
                        || WorkbenchTabId::parse(&identity(9_997)).unwrap(),
                        |tab| tab.tab_id.clone(),
                    );
                WorkbenchAction::TabRename {
                    terminal_context_id,
                    tab_id,
                    label: label(n),
                }
            }
            _ => create("Fallback"),
        };
        let mut request = command(
            u128::try_from(index).unwrap() + 1,
            before.revision.value(),
            action,
        );
        if matches!(step, Step::Stale) {
            request.expected_revision = WorkbenchRevision::new(u64::MAX);
        }
        let replay = match step {
            Step::Replay(n) | Step::Conflict(n) if !history.is_empty() => {
                history.get(usize::from(*n) % history.len()).cloned()
            }
            _ => None,
        };
        if let Some((original, _)) = &replay {
            request = original.clone();
            if matches!(step, Step::Conflict(_)) {
                request.expected_revision = WorkbenchRevision::new(u64::MAX);
                request.action = create("Conflict");
            }
        }
        let permission = if matches!(step, Step::Denied) {
            &reader
        } else {
            &grant
        };
        let result = submit(&mut authority, permission, request.clone());
        let after = snapshot(&mut authority, &grant);
        if !matches!(&request.action, WorkbenchAction::TabRename { .. }) {
            require(
                after.terminal_contexts[0] == before.terminal_contexts[0],
                "navigation changed standalone membership",
            )?;
        }
        let bytes = serialize_document(&WorkbenchProtocolDocument::Snapshot(after.clone()))
            .map_err(|e| e.to_string())?;
        parse_snapshot(&bytes).map_err(|e| e.to_string())?;
        if let Some((_, original_result)) = replay {
            require(
                after == before
                    && authority.last_identity == old_cursor
                    && authority.ledger.len() == old_ledger,
                "replay/conflict changed authority",
            )?;
            if matches!(step, Step::Replay(_)) {
                require(result == original_result, "replay changed original outcome")?;
            } else {
                require(
                    matches!(
                        result.outcome,
                        WorkbenchCommandOutcome::Rejected {
                            reason: WorkbenchRejectionReason::RequestIdentityConflict,
                            ..
                        }
                    ),
                    "changed request did not conflict",
                )?;
            }
            continue;
        }
        let changed = matches!(
            result.outcome,
            WorkbenchCommandOutcome::ProjectCreated { .. }
                | WorkbenchCommandOutcome::WorkspaceCreated { .. }
                | WorkbenchCommandOutcome::Renamed { .. }
        );
        if changed {
            require(
                after.revision.value() == before.revision.value() + 1,
                "changed revision is not exactly next",
            )?;
            match &request.action {
                WorkbenchAction::ProjectCreate { label } => {
                    require(
                        after.navigation.projects.len() == before.navigation.projects.len() + 1
                            && after.terminal_contexts == before.terminal_contexts,
                        "project create changed wrong aggregate",
                    )?;
                    let created = after
                        .navigation
                        .projects
                        .last()
                        .ok_or("missing created project")?;
                    require(
                        created.label == label.as_str() && created.workspaces.is_empty(),
                        "project create result differs",
                    )?;
                    require(
                        seen_identities.insert(created.project_id.to_string()),
                        "project identity was reused",
                    )?;
                }
                WorkbenchAction::WorkspaceCreate { label, .. } => {
                    require(
                        after.terminal_contexts.len() == before.terminal_contexts.len() + 1,
                        "workspace lacks one new context",
                    )?;
                    let workspace = after
                        .navigation
                        .projects
                        .first()
                        .and_then(|p| p.workspaces.last())
                        .ok_or("missing workspace")?;
                    require(workspace.label == label.as_str(), "workspace label differs")?;
                    require(
                        seen_identities.insert(workspace.workspace_id.to_string()),
                        "workspace identity reused",
                    )?;
                    require(
                        seen_identities.insert(workspace.terminal_context_id.to_string()),
                        "context identity reused",
                    )?;
                }
                WorkbenchAction::ProjectRename { label, .. } => {
                    let mut expected = before.clone();
                    expected.revision = after.revision;
                    expected
                        .navigation
                        .projects
                        .first_mut()
                        .ok_or("missing renamed project")?
                        .label = label.as_str().to_owned();
                    require(
                        after == expected && authority.last_identity == old_cursor,
                        "project rename changed unrelated state",
                    )?;
                }
                WorkbenchAction::WorkspaceRename { label, .. } => {
                    let mut expected = before.clone();
                    expected.revision = after.revision;
                    expected
                        .navigation
                        .projects
                        .first_mut()
                        .and_then(|p| p.workspaces.first_mut())
                        .ok_or("missing renamed workspace")?
                        .label = label.as_str().to_owned();
                    require(
                        after == expected && authority.last_identity == old_cursor,
                        "workspace rename changed unrelated state",
                    )?;
                }
                WorkbenchAction::TabRename {
                    terminal_context_id,
                    tab_id,
                    label,
                } => {
                    let mut expected = before.clone();
                    expected.revision = after.revision;
                    let renamed = expected
                        .terminal_contexts
                        .iter_mut()
                        .find(|context| context.terminal_context_id == *terminal_context_id)
                        .and_then(|context| {
                            context.tabs.iter_mut().find(|tab| tab.tab_id == *tab_id)
                        })
                        .ok_or("missing renamed tab")?;
                    renamed.label = label.as_str().to_owned();
                    renamed.automatic_label = false;
                    require(
                        after == expected && authority.last_identity == old_cursor,
                        "tab rename changed unrelated state",
                    )?;
                }
                WorkbenchAction::PanelRename {
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    label,
                } => {
                    let mut expected = before.clone();
                    expected.revision = after.revision;
                    let tab = expected
                        .terminal_contexts
                        .iter_mut()
                        .find(|context| context.terminal_context_id == *terminal_context_id)
                        .and_then(|context| {
                            context.tabs.iter_mut().find(|tab| tab.tab_id == *tab_id)
                        })
                        .ok_or("missing renamed panel tab")?;
                    super::super::panel_primary_mut(&mut tab.root, panel_id)
                        .ok_or("missing renamed panel")?
                        .custom_label = label.custom_label().cloned();
                    require(
                        after == expected && authority.last_identity == old_cursor,
                        "panel rename changed unrelated state",
                    )?;
                }
                WorkbenchAction::PanelInputSet {
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    right_click,
                } => {
                    let mut expected = before.clone();
                    expected.revision = after.revision;
                    let tab = expected
                        .terminal_contexts
                        .iter_mut()
                        .find(|context| context.terminal_context_id == *terminal_context_id)
                        .and_then(|context| {
                            context.tabs.iter_mut().find(|tab| tab.tab_id == *tab_id)
                        })
                        .ok_or("missing input panel tab")?;
                    let orchbee_workbench_protocol::PanelNode::Panel {
                        right_click: policy,
                        ..
                    } = super::super::panel_mut(&mut tab.root, panel_id)
                        .ok_or("missing input panel")?
                    else {
                        return Err("not a panel".to_owned());
                    };
                    *policy = *right_click;
                    require(
                        after == expected && authority.last_identity == old_cursor,
                        "panel input changed unrelated state",
                    )?;
                }
                WorkbenchAction::WorkspaceLaunch { .. } | WorkbenchAction::TabCreate { .. } => {
                    return Err("pure trace generated tab.create".to_owned());
                }
                WorkbenchAction::PanelSplit { .. } => {
                    return Err("pure trace generated panel.split".to_owned());
                }
                WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. } => {
                    return Err("pure trace generated directory association".to_owned());
                }
                WorkbenchAction::TabClose { .. }
                | WorkbenchAction::PanelClose { .. }
                | WorkbenchAction::WorkspaceClose { .. } => {
                    return Err("pure trace generated lifecycle close".to_owned());
                }
                WorkbenchAction::TabMove { .. }
                | WorkbenchAction::TabMoveIndex { .. }
                | WorkbenchAction::PanelMove { .. }
                | WorkbenchAction::PanelSwap { .. }
                | WorkbenchAction::PanelRelocate { .. } => {
                    return Err("direct navigation trace generated Site-composed move".to_owned());
                }
                WorkbenchAction::LaunchProfileCreate { .. }
                | WorkbenchAction::LaunchProfileDelete { .. } => {
                    return Err("navigation trace generated a launch-profile mutation".to_owned());
                }
            }
        } else {
            require(
                after == before && authority.last_identity == old_cursor,
                "rejection/no-op consumed state or identity",
            )?;
        }
        if matches!(step, Step::Denied) {
            require(
                matches!(result.outcome, WorkbenchCommandOutcome::Denied {})
                    && authority.ledger.len() == old_ledger,
                "denied call settled",
            )?;
        } else {
            history.push((request, result));
        }
        let encoded_bytes: usize = authority
            .ledger
            .values()
            .map(|entry| ledger_size(entry.command(), entry.replay_result()).unwrap())
            .sum();
        require(
            encoded_bytes == authority.ledger_bytes,
            "canonical ledger accounting diverged",
        )?;
    }
    Ok(())
}
