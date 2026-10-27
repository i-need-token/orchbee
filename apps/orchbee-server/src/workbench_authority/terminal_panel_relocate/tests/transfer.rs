use super::*;
use orchbee_workbench_protocol::{
    MAXIMUM_TABS_PER_CONTEXT, Project, Snapshot, Tab, TerminalContext, TerminalPresentationId,
    WorkbenchDirectoryId, WorkbenchProjectId, WorkbenchWorkspaceId, Workspace,
};
use std::collections::BTreeMap;

fn leaf(n: u128) -> PanelNode {
    PanelNode::Panel {
        right_click: Default::default(),
        child_presentations: Vec::new(),
        panel_id: WorkbenchPanelId::parse(&identity(n)).unwrap(),
        primary_presentation: Presentation {
            binding: Some(binding(n + 100_000)),
            custom_label: None,
            label: format!("shell {n}"),
            presentation_id: TerminalPresentationId::parse(&identity(n + 1)).unwrap(),
            launch_failure: None,
        },
    }
}

fn tab(n: u128) -> Tab {
    Tab {
        automatic_label: false,
        tab_id: WorkbenchTabId::parse(&identity(n)).unwrap(),
        label: format!("tab {n}"),
        group_id: None,
        pinned: false,
        root: leaf(n + 1),
    }
}

fn install(owner: &mut WorkbenchAuthority, snapshot: Snapshot) {
    owner.projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap(),
    )
    .unwrap();
}

fn populated() -> (WorkbenchAuthority, WorkbenchGrant) {
    let (mut owner, grant) = authority();
    let mut snapshot = owner.snapshot().clone();
    for n in [1000, 2000] {
        let workspace_id = WorkbenchWorkspaceId::parse(&identity(n + 1)).unwrap();
        let context_id = WorkbenchTerminalContextId::parse(&identity(n + 2)).unwrap();
        snapshot.navigation.projects.push(Project {
            project_id: WorkbenchProjectId::parse(&identity(n)).unwrap(),
            label: format!("project {n}"),
        });
        snapshot.navigation.directories.extend(Vec::new());
        snapshot.navigation.workspaces.extend(vec![Workspace {
            project_id: Some(WorkbenchProjectId::parse(&identity(n)).unwrap()),
            workspace_id: workspace_id.clone(),
            terminal_context_id: context_id.clone(),
            label: format!("workspace {n}"),
            primary_directory_id: None,
        }]);
        snapshot.navigation.workspace_order = snapshot
            .navigation
            .workspaces
            .iter()
            .map(|workspace| workspace.workspace_id.clone())
            .collect();
        snapshot.terminal_contexts.push(TerminalContext {
            terminal_context_id: context_id,
            groups: Vec::new(),
            tabs: vec![tab(n + 3)],
        });
    }
    owner.last_identity = 3000;
    install(&mut owner, snapshot);
    (owner, grant)
}

fn retained(snapshot: &Snapshot) -> BTreeMap<String, PanelNode> {
    snapshot
        .terminal_contexts
        .iter()
        .flat_map(|context| context.tabs.iter())
        .flat_map(|tab| leaves(&tab.root))
        .map(|leaf| (leaf_panel_id(&leaf).as_str().to_owned(), leaf))
        .collect()
}

fn transfer_command(
    owner: &WorkbenchAuthority,
    id: u128,
    source_context: usize,
    destination: WorkbenchPanelRelocateDestination,
) -> WorkbenchCommand {
    let source = &owner.snapshot().terminal_contexts[source_context];
    command(
        owner,
        id,
        WorkbenchAction::PanelRelocate {
            terminal_context_id: source.terminal_context_id.clone(),
            tab_id: source.tabs[0].tab_id.clone(),
            panel_id: leaf_panel_id(&leaves(&source.tabs[0].root)[0]),
            destination,
        },
    )
}

fn relocate_with_directory(
    owner: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
    worker: Option<WorkerId>,
) -> WorkbenchCommandResult {
    let admission = match owner.begin_terminal_panel_relocate(Some(grant), command) {
        WorkbenchPanelRelocateBegin::Ready(result) => return result,
        WorkbenchPanelRelocateBegin::Admitted(admission) => admission,
    };
    let prepared = match worker {
        Some(worker) => admission.finish_with_directory(worker),
        None => admission.finish(),
    };
    match prepared {
        WorkbenchPanelRelocatePreparation::Ready(result) => result,
        WorkbenchPanelRelocatePreparation::Prepared(prepared) => prepared.commit(),
    }
}

#[test]
fn relocation_directory_is_worker_proven_atomic_and_replayable() {
    use orchbee_workbench_protocol::WorkbenchDirectoryPath;
    for source in [0, 1] {
        for case in 0..5 {
            let (mut owner, base_grant) = populated();
            let grant = if case == 3 {
                base_grant
            } else {
                base_grant.with_directory_association()
            };
            let mut before = owner.snapshot().clone();
            if case == 4 {
                // Fill the reachable encoded projection limit; the global directory count
                // limit is larger than any serializable projection. The fixture Workspace keeps
                // its primary Directory; only the added Directories grow.
                let existing = before.navigation.directories.len();
                before
                    .navigation
                    .directories
                    .extend(
                        (existing..256).map(|index| orchbee_workbench_protocol::Directory {
                            directory_id: WorkbenchDirectoryId::parse(&identity(
                                4000 + index as u128,
                            ))
                            .unwrap(),
                            worker_id: binding(1).worker_id,
                            path: WorkbenchDirectoryPath::parse("C:/").unwrap(),
                        }),
                    );
                let size = serialize_document(&WorkbenchProtocolDocument::Snapshot(before.clone()))
                    .unwrap()
                    .len();
                let mut remaining =
                    orchbee_workbench_protocol::MAXIMUM_ENCODED_PROJECTION_BYTES - size;
                for directory in before.navigation.directories.iter_mut().skip(existing) {
                    let added = remaining.min(4093);
                    directory.path =
                        WorkbenchDirectoryPath::parse(&format!("C:/{}", "x".repeat(added)))
                            .unwrap();
                    remaining -= added;
                }
                assert_eq!(remaining, 0);
                install(&mut owner, before.clone());
            }
            let path = WorkbenchDirectoryPath::parse("C:/retained/source").unwrap();
            let command = transfer_command(
                &owner,
                90_000,
                source,
                WorkbenchPanelRelocateDestination::NewWorkspace {
                    project_id: Some(before.navigation.projects[1].project_id.clone()),
                    primary_directory_id: None,
                    primary_directory_path: Some(path.clone()),
                    label: WorkbenchLabel::parse("moved").unwrap(),
                    tab_label: None,
                },
            );
            let high = owner.last_identity;
            let worker = match case {
                0 => None,
                1 => Some(WorkerId::parse(&identity(9999)).unwrap()),
                _ => Some(binding(1).worker_id),
            };
            let response = relocate_with_directory(&mut owner, &grant, command.clone(), worker);
            if case != 2 {
                assert_eq!(owner.snapshot(), &before, "case {case}");
                assert_eq!(owner.last_identity, high);
                match case {
                    0 => assert!(matches!(
                        response.outcome,
                        WorkbenchCommandOutcome::Unavailable {
                            reason: WorkbenchCommandUnavailableReason::DirectoryUnavailable
                        }
                    )),
                    1 => assert!(matches!(
                        response.outcome,
                        WorkbenchCommandOutcome::Rejected {
                            reason: WorkbenchRejectionReason::WorkspaceDirectoryUnavailable,
                            ..
                        }
                    )),
                    3 => assert!(matches!(
                        response.outcome,
                        WorkbenchCommandOutcome::Denied {}
                    )),
                    _ => assert!(matches!(
                        response.outcome,
                        WorkbenchCommandOutcome::Rejected {
                            reason: WorkbenchRejectionReason::ResourceLimit,
                            ..
                        }
                    )),
                }
                continue;
            }
            let WorkbenchCommandOutcome::PanelRelocatedToNewTab {
                transfer: Some(transfer),
                ..
            } = &response.outcome
            else {
                panic!("{response:?}")
            };
            let directory_id = transfer.created_directory_id.as_ref().unwrap();
            let navigation = &owner.snapshot().navigation;
            let directory = navigation
                .directories
                .iter()
                .find(|directory| &directory.directory_id == directory_id)
                .unwrap();
            assert_eq!(directory.path, path);
            assert_eq!(directory.worker_id, binding(1).worker_id);
            let workspace = navigation
                .workspaces
                .iter()
                .find(|workspace| {
                    Some(&workspace.workspace_id) == transfer.created_workspace_id.as_ref()
                })
                .unwrap();
            assert_eq!(workspace.primary_directory_id.as_ref(), Some(directory_id));
            assert_eq!(retained(owner.snapshot()), retained(&before));
            assert_eq!(owner.last_identity, high + 4);
            assert_eq!(
                owner.snapshot().revision.value(),
                before.revision.value() + 1
            );
            assert_eq!(
                navigation.directories.len(),
                before.navigation.directories.len() + 1
            );
            // Retained result is authoritative even without another Worker probe.
            assert_eq!(
                relocate_with_directory(&mut owner, &grant, command, None),
                response
            );
        }
    }
}

#[test]
fn cross_context_reference_matrix_conserves_leaves_and_reports_source_disposal() {
    // Independent reference partitions: fixture/project Workspace source, one/many
    // source leaves, existing/new Tab/new Workspace, and four graft directions.
    for source_index in [0, 1] {
        for source_split in [false, true] {
            for destination_kind in 0..3 {
                for direction in [
                    WorkbenchPanelSplitDirection::Above,
                    WorkbenchPanelSplitDirection::Below,
                    WorkbenchPanelSplitDirection::Left,
                    WorkbenchPanelSplitDirection::Right,
                ] {
                    let (mut owner, grant) = populated();
                    let mut initial = owner.snapshot().clone();
                    if source_split {
                        let root = &mut initial.terminal_contexts[source_index].tabs[0].root;
                        insert_at_root(
                            root,
                            leaf(2700),
                            WorkbenchPanelSplitDirection::Right,
                            &Minted::Split {
                                split_id: WorkbenchSplitId::parse(&identity(2710)).unwrap(),
                                first_slot_id: WorkbenchPanelSlotId::parse(&identity(2711))
                                    .unwrap(),
                                second_slot_id: WorkbenchPanelSlotId::parse(&identity(2712))
                                    .unwrap(),
                            },
                        );
                    }
                    // A non-root graft must preserve the target's unrelated branch.
                    let target = &mut initial.terminal_contexts[2].tabs[0].root;
                    let target_panel = leaf_panel_id(target);
                    insert_at_root(
                        target,
                        leaf(2800),
                        WorkbenchPanelSplitDirection::Right,
                        &Minted::Split {
                            split_id: WorkbenchSplitId::parse(&identity(2810)).unwrap(),
                            first_slot_id: WorkbenchPanelSlotId::parse(&identity(2811)).unwrap(),
                            second_slot_id: WorkbenchPanelSlotId::parse(&identity(2812)).unwrap(),
                        },
                    );
                    let mut next = 500_000;
                    for (index, context) in initial.terminal_contexts.iter_mut().enumerate() {
                        let group_id = orchbee_workbench_protocol::WorkbenchTabGroupId::parse(
                            &identity(499_990 + index as u128),
                        )
                        .unwrap();
                        context.groups.push(orchbee_workbench_protocol::TabGroup {
                            group_id: group_id.clone(),
                            label: "retained group".to_owned(),
                            color: orchbee_workbench_protocol::TabGroupColor::Blue,
                        });
                        for tab in &mut context.tabs {
                            tab.pinned = index < 2;
                            tab.group_id = (index == 2).then(|| group_id.clone());
                            super::presentations::add_children(&mut tab.root, &mut next);
                        }
                    }
                    owner.last_identity = next;
                    install(&mut owner, initial.clone());
                    let target_context = initial.terminal_contexts[2].terminal_context_id.clone();
                    let destination = match destination_kind {
                        0 => WorkbenchPanelRelocateDestination::Tab {
                            terminal_context_id: Some(target_context.clone()),
                            tab_id: initial.terminal_contexts[2].tabs[0].tab_id.clone(),
                            target_panel_id: Some(target_panel),
                            direction,
                        },
                        1 => WorkbenchPanelRelocateDestination::NewTab {
                            terminal_context_id: Some(target_context),
                            label: None,
                        },
                        _ => WorkbenchPanelRelocateDestination::NewWorkspace {
                            project_id: Some(initial.navigation.projects[1].project_id.clone()),
                            primary_directory_id: None,
                            primary_directory_path: None,
                            label: WorkbenchLabel::parse("moved").unwrap(),
                            tab_label: None,
                        },
                    };
                    let request = transfer_command(&owner, 10_000, source_index, destination);
                    let result = submit(&mut owner, &grant, request.clone());
                    let transfer = match &result.outcome {
                        WorkbenchCommandOutcome::PanelRelocated {
                            transfer: Some(transfer),
                            ..
                        }
                        | WorkbenchCommandOutcome::PanelRelocatedToNewTab {
                            transfer: Some(transfer),
                            ..
                        } => transfer,
                        other => panic!("relocation expected: {other:?}"),
                    };
                    // Each source Workspace owns one Tab, so moving its only leaf closes it.
                    let removed_workspace = !source_split;
                    assert_eq!(transfer.closed_source_tab, !source_split);
                    assert_eq!(
                        transfer.closed_source_workspace_id.is_some(),
                        removed_workspace
                    );
                    assert_eq!(
                        transfer.created_workspace_id.is_some(),
                        destination_kind == 2
                    );
                    let after = owner.snapshot().clone();
                    assert_eq!(retained(&after), retained(&initial));
                    for previous in &initial.terminal_contexts {
                        if let Some(context) = after.terminal_contexts.iter().find(|context| {
                            context.terminal_context_id == previous.terminal_context_id
                        }) {
                            assert_eq!(context.groups, previous.groups);
                            for tab in &context.tabs {
                                if let Some(old) =
                                    previous.tabs.iter().find(|old| old.tab_id == tab.tab_id)
                                {
                                    assert_eq!(
                                        (&tab.group_id, tab.pinned),
                                        (&old.group_id, old.pinned)
                                    );
                                } else {
                                    assert_eq!((&tab.group_id, tab.pinned), (&None, false));
                                }
                            }
                        }
                    }
                    assert_eq!(after.revision.value(), initial.revision.value() + 1);
                    assert_eq!(
                        after.terminal_contexts.len(),
                        3 - usize::from(removed_workspace) + usize::from(destination_kind == 2)
                    );
                    assert_eq!(
                        after.navigation.ordered_workspaces().unwrap().len(),
                        3 - usize::from(removed_workspace) + usize::from(destination_kind == 2)
                    );
                    assert_eq!(submit(&mut owner, &grant, request), result);
                    assert_eq!(owner.snapshot(), &after);
                    assert!(!owner.retired);
                }
            }
        }
    }
}

#[test]
fn final_leaf_new_tab_exchange_works_at_capacity_and_nonempty_growth_refuses() {
    for split_source in [false, true] {
        let (mut owner, grant) = populated();
        let mut initial = owner.snapshot().clone();
        let context = &mut initial.terminal_contexts[0];
        for n in 1..MAXIMUM_TABS_PER_CONTEXT {
            context.tabs.push(tab(4000 + 10 * n as u128));
        }
        if split_source {
            insert_at_root(
                &mut context.tabs[0].root,
                leaf(9000),
                WorkbenchPanelSplitDirection::Right,
                &Minted::Split {
                    split_id: WorkbenchSplitId::parse(&identity(9010)).unwrap(),
                    first_slot_id: WorkbenchPanelSlotId::parse(&identity(9011)).unwrap(),
                    second_slot_id: WorkbenchPanelSlotId::parse(&identity(9012)).unwrap(),
                },
            );
        }
        owner.last_identity = 10_000;
        install(&mut owner, initial.clone());
        let request = transfer_command(
            &owner,
            20_000,
            0,
            WorkbenchPanelRelocateDestination::NewTab {
                terminal_context_id: None,
                label: None,
            },
        );
        let result = submit(&mut owner, &grant, request);
        if split_source {
            assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::Rejected {
                    reason: WorkbenchRejectionReason::ResourceLimit,
                    ..
                }
            ));
            assert_eq!(owner.snapshot(), &initial);
            assert_eq!(owner.last_identity, 10_000);
        } else {
            assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::PanelRelocatedToNewTab { .. }
            ));
            assert_eq!(
                owner.snapshot().terminal_contexts[0].tabs.len(),
                MAXIMUM_TABS_PER_CONTEXT
            );
            assert_eq!(retained(owner.snapshot()), retained(&initial));
        }
    }
}

#[test]
fn transfer_resolution_refuses_without_allocating_or_removing_source() {
    let (mut owner, grant) = populated();
    let initial = owner.snapshot().clone();
    let project = initial.navigation.projects[1].project_id.clone();
    for (destination, reason) in [
        (
            WorkbenchPanelRelocateDestination::NewTab {
                terminal_context_id: Some(
                    WorkbenchTerminalContextId::parse(&identity(99_000)).unwrap(),
                ),
                label: None,
            },
            WorkbenchRejectionReason::TerminalContextUnavailable,
        ),
        (
            WorkbenchPanelRelocateDestination::NewWorkspace {
                project_id: Some(WorkbenchProjectId::parse(&identity(99_000)).unwrap()),
                primary_directory_id: None,
                primary_directory_path: None,
                label: WorkbenchLabel::parse("x").unwrap(),
                tab_label: None,
            },
            WorkbenchRejectionReason::ProjectUnavailable,
        ),
        (
            WorkbenchPanelRelocateDestination::NewWorkspace {
                project_id: Some(project),
                primary_directory_id: Some(WorkbenchDirectoryId::parse(&identity(99_000)).unwrap()),
                primary_directory_path: None,
                label: WorkbenchLabel::parse("x").unwrap(),
                tab_label: None,
            },
            WorkbenchRejectionReason::WorkspaceDirectoryUnavailable,
        ),
        (
            WorkbenchPanelRelocateDestination::Tab {
                terminal_context_id: Some(initial.terminal_contexts[2].terminal_context_id.clone()),
                tab_id: initial.terminal_contexts[2].tabs[0].tab_id.clone(),
                target_panel_id: Some(WorkbenchPanelId::parse(&identity(99_000)).unwrap()),
                direction: WorkbenchPanelSplitDirection::Below,
            },
            WorkbenchRejectionReason::PanelUnavailable,
        ),
    ] {
        let request = transfer_command(&owner, 30_000 + owner.ledger.len() as u128, 1, destination);
        let result = submit(&mut owner, &grant, request.clone());
        assert_eq!(
            result.outcome,
            WorkbenchCommandOutcome::Rejected {
                revision: initial.revision,
                reason
            }
        );
        assert_eq!(owner.snapshot(), &initial);
        assert_eq!(owner.last_identity, 3000);
        assert_eq!(submit(&mut owner, &grant, request), result);
    }
    assert!(!owner.retired);
}

#[test]
fn repeated_cross_context_transfers_match_a_panel_location_model() {
    for seed in 1..=24_u64 {
        let mut random = seed;
        let steps: Vec<_> = (0..64)
            .map(|_| {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                (random.to_le_bytes()[2], random % 3 == 0)
            })
            .collect();
        if let Err(first) = run_transfers(&steps) {
            let mut reduced = steps;
            let mut index = 0;
            while index < reduced.len() {
                let mut candidate = reduced.clone();
                candidate.remove(index);
                if run_transfers(&candidate).is_err() {
                    reduced = candidate;
                    index = 0;
                } else {
                    index += 1;
                }
            }
            panic!(
                "seed={seed}; first={first}; reduced={reduced:?}; failure={:?}",
                run_transfers(&reduced)
            );
        }
    }
}

fn run_transfers(steps: &[(u8, bool)]) -> Result<(), String> {
    let (mut owner, grant) = populated();
    let original = retained(owner.snapshot());
    for (step, (source_hint, new_workspace)) in steps.iter().enumerate() {
        let before = owner.snapshot().clone();
        let available: Vec<_> = before
            .terminal_contexts
            .iter()
            .enumerate()
            .filter(|(_, context)| !context.tabs.is_empty())
            .map(|(i, _)| i)
            .collect();
        let source_index = available[usize::from(*source_hint) % available.len()];
        let panel = leaf_panel_id(&leaves(&before.terminal_contexts[source_index].tabs[0].root)[0]);
        let destination = if *new_workspace {
            WorkbenchPanelRelocateDestination::NewWorkspace {
                project_id: Some(before.navigation.projects[0].project_id.clone()),
                primary_directory_id: None,
                primary_directory_path: None,
                label: WorkbenchLabel::parse("generated").unwrap(),
                tab_label: None,
            }
        } else {
            let target_index = (source_index + 1) % before.terminal_contexts.len();
            WorkbenchPanelRelocateDestination::NewTab {
                terminal_context_id: Some(
                    before.terminal_contexts[target_index]
                        .terminal_context_id
                        .clone(),
                ),
                label: None,
            }
        };
        // The fixture Tab retains request 100_000; generated requests start above it.
        let request = transfer_command(&owner, 200_000 + step as u128, source_index, destination);
        let result = submit(&mut owner, &grant, request.clone());
        let WorkbenchCommandOutcome::PanelRelocatedToNewTab {
            new_tab_id,
            transfer: Some(transfer),
            ..
        } = &result.outcome
        else {
            return Err(format!("step {step}: {result:?}"));
        };
        let after = owner.snapshot();
        if retained(after) != original || after.revision.value() != before.revision.value() + 1 {
            return Err(format!("step {step}: conservation/revision"));
        }
        let destination = after
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == transfer.target_terminal_context_id)
            .and_then(|context| context.tabs.iter().find(|tab| &tab.tab_id == new_tab_id));
        if destination.is_none_or(|tab| leaf_panel_id(&tab.root) != panel) {
            return Err(format!("step {step}: destination"));
        }
        for context in &before.terminal_contexts {
            for tab in &context.tabs {
                for other in leaves(&tab.root)
                    .iter()
                    .map(leaf_panel_id)
                    .filter(|id| id != &panel)
                {
                    if !after
                        .terminal_contexts
                        .iter()
                        .find(|next| next.terminal_context_id == context.terminal_context_id)
                        .and_then(|context| {
                            context.tabs.iter().find(|next| next.tab_id == tab.tab_id)
                        })
                        .is_some_and(|tab| find_panel(&tab.root, &other).is_some())
                    {
                        return Err(format!("step {step}: unrelated membership"));
                    }
                }
            }
        }
        if submit(&mut owner, &grant, request) != result || owner.retired {
            return Err(format!("step {step}: replay/retirement"));
        }
    }
    Ok(())
}

#[test]
fn either_cross_context_close_fences_transfer_without_retaining_or_allocating() {
    for endpoint in [1, 2] {
        let (mut owner, grant) = populated();
        let initial = owner.snapshot().clone();
        open_aggregate_close(
            &mut owner,
            &grant,
            50_000,
            &initial.terminal_contexts[endpoint].tabs[0].tab_id,
        );
        let request = transfer_command(
            &owner,
            50_001,
            1,
            WorkbenchPanelRelocateDestination::Tab {
                terminal_context_id: Some(initial.terminal_contexts[2].terminal_context_id.clone()),
                tab_id: initial.terminal_contexts[2].tabs[0].tab_id.clone(),
                target_panel_id: None,
                direction: WorkbenchPanelSplitDirection::Below,
            },
        );
        let bytes = owner.ledger_bytes;
        let before = owner.snapshot().clone();
        let result = submit(&mut owner, &grant, request);
        assert!(matches!(
            result.outcome,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::TerminalBusy
            }
        ));
        assert_eq!(owner.snapshot(), &before);
        assert_eq!(owner.ledger_bytes, bytes);
        assert_eq!(owner.last_identity, 3000);
    }
}
