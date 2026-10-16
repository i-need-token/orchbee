use super::*;
use crate::terminal_session::NativeContextLocation;
use orchbee_workbench_protocol::{
    SiteId, WorkbenchProjectId, WorkbenchTerminalContextId, WorkbenchWorkspaceId,
};

fn workspace(project: u8, number: u8) -> Context {
    Context {
        target: NativeContextTarget {
            site_id: SiteId::parse(&format!("{:032x}", 9)).unwrap(),
            location: NativeContextLocation::Workspace {
                workspace_id: WorkbenchWorkspaceId::parse(&format!("{number:032x}")).unwrap(),
                terminal_context_id: WorkbenchTerminalContextId::parse(&format!(
                    "{:032x}",
                    100 + u16::from(number)
                ))
                .unwrap(),
            },
        },
        project_id: Some(WorkbenchProjectId::parse(&format!("{project:032x}")).unwrap()),
        project: Some("same Project".into()),
        workspace: Some("same 工作区".into()),
    }
}

fn picker() -> Picker {
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.observe_agent_sidebar(true);
    picker.observe_local(vec![
        context(11),
        workspace(1, 21),
        workspace(1, 22),
        workspace(2, 23),
    ]);
    picker
}

#[test]
fn sorting_bindings_emit_the_focused_identity_without_navigation_or_repeat() {
    let mut picker = picker();
    assert!(picker.enter_sidebar_navigation());
    let target = picker
        .sidebar_navigation_rows()
        .into_iter()
        .find(|row| matches!(row, SidebarRow::Project { .. }))
        .unwrap()
        .target();
    assert!(picker.focus_sidebar_target(&target));
    for (code, up) in [(KeyCode::Up, true), (KeyCode::Down, false)] {
        assert_eq!(
            picker.sidebar_key(KeyEvent::press(code, Modifiers::CTRL)),
            SidebarIntent::Reorder {
                target: target.clone(),
                up
            }
        );
        assert_eq!(picker.sidebar_focused_target(), Some(&target));
        let mut repeat = KeyEvent::press(code, Modifiers::CTRL);
        repeat.kind = KeyKind::Repeat;
        assert_eq!(picker.sidebar_key(repeat), SidebarIntent::None);
        assert!(picker.request.is_none());
    }
}

fn key(picker: &mut Picker, code: KeyCode) -> SidebarIntent {
    picker.sidebar_key(KeyEvent::press(code, Modifiers::NONE))
}

fn window(picker: &Picker, heights: &[u16], cells: u16, gap: u16) -> SidebarWindow {
    let rows = picker.sidebar_navigation_rows();
    assert_eq!(rows.len(), heights.len());
    picker.space_window_for_row_gaps(&rows, heights, &vec![gap; rows.len()], cells)
}

fn checkpoint(site: u8) -> crate::machine_spaces::Observation {
    use crate::machine_spaces::{Checkpoint, ProjectSummary, Row as SpaceRow};
    let site_id = SiteId::parse(&format!("{site:032x}")).unwrap();
    let mut target = workspace(1, 21).target;
    target.site_id = site_id.clone();
    crate::machine_spaces::Observation::Ready(std::sync::Arc::new(Checkpoint {
        site_id,
        revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
        metadata_revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
        git_requested: false,
        projects: [1, 2]
            .into_iter()
            .map(|id| ProjectSummary {
                project_id: WorkbenchProjectId::parse(&format!("{id:032x}")).unwrap(),
                label: "same Project".into(),
            })
            .collect(),
        rows: vec![SpaceRow {
            target,
            project_id: Some(WorkbenchProjectId::parse(&format!("{:032x}", 1)).unwrap()),
            label: Some("same 工作区".into()),
            tokens: Default::default(),
            git: None,
        }]
        .into(),
    }))
}

fn agent(context: NativeContextTarget, number: u64) -> crate::machine_agents::Row {
    let mut row = crate::machine_agents::tests::row(number);
    row.target.context = context;
    row
}

fn observe_agents(picker: &mut Picker, rows: Vec<crate::machine_agents::Row>) {
    observe_agents_at(picker, rows, 1);
}

fn observe_agents_at(
    picker: &mut Picker,
    rows: Vec<crate::machine_agents::Row>,
    report_revision: u64,
) {
    let mut checkpoint = crate::machine_agents::tests::checkpoint(rows)
        .as_ref()
        .clone();
    checkpoint.report_revision =
        orchbee_workbench_protocol::WorkbenchRevision::new(report_revision);
    checkpoint.site_id = SiteId::parse(&format!("{:032x}", 9)).unwrap();
    checkpoint.contexts = picker
        .local_contexts
        .iter()
        .map(|context| context.target.clone())
        .collect::<Vec<_>>()
        .into();
    picker.observe_agents(vec![(
        Choice::Local,
        crate::machine_agents::Observation::Ready(std::sync::Arc::new(checkpoint)),
    )]);
}

#[test]
fn sidebar_source_tracks_release_semantics_without_focus_scroll_or_copy() {
    let mut picker = picker();
    let target = SidebarTarget::Context {
        machine: Choice::Local,
        target: workspace(1, 21).target,
    };
    let rows = picker.sidebar_navigation_rows();
    let window =
        picker.space_window_for_row_gaps(&rows, &vec![1; rows.len()], &vec![0; rows.len()], 3);
    let source = picker.sidebar_source();
    let drawn = source
        .drawn(target.clone(), SidebarPart::Primary, window)
        .unwrap();
    assert_eq!(
        picker.sidebar_activation(&drawn),
        Some(SidebarActivation::Activate(
            target.selection().expect("Context selection")
        ))
    );

    assert!(picker.enter_sidebar_navigation());
    assert!(picker.focus_sidebar_target(&target));
    picker.queue_space_scroll(window, false);
    assert_eq!(picker.sidebar_source(), source);
    assert_eq!(
        picker.sidebar_activation(&drawn),
        Some(SidebarActivation::Activate(
            target.selection().expect("Context selection")
        ))
    );

    let mut relabeled = vec![
        context(11),
        workspace(1, 21),
        workspace(1, 22),
        workspace(2, 23),
    ];
    relabeled[1].workspace = Some("renamed visible copy".into());
    relabeled[1].project = Some("renamed project copy".into());
    picker.observe_local(relabeled);
    assert_eq!(picker.sidebar_source(), source);
    assert_eq!(
        picker.sidebar_activation(&drawn),
        Some(SidebarActivation::Activate(
            target.selection().expect("Context selection")
        ))
    );
}

#[test]
fn sidebar_source_mints_only_live_parts_and_rejects_structurally_stale_release() {
    let mut picker = picker();
    let selected_context = workspace(1, 21).target;
    let agent = agent(selected_context.clone(), 23);
    observe_agents(&mut picker, vec![agent.clone()]);
    let context_target = SidebarTarget::Context {
        machine: Choice::Local,
        target: selected_context.clone(),
    };
    let agent_target = SidebarTarget::Agent {
        machine: Choice::Local,
        target: Box::new(agent.target.clone()),
    };
    picker.observe_focus(Choice::Local, Some(selected_context), None);
    let project = picker.sidebar_parent(&context_target).unwrap();
    let rows = picker.sidebar_navigation_rows();
    let window =
        picker.space_window_for_row_gaps(&rows, &vec![1; rows.len()], &vec![0; rows.len()], 10);
    let source = picker.sidebar_source();

    assert!(
        source
            .drawn(project.clone(), SidebarPart::Menu, window)
            .is_some()
    );
    assert!(
        source
            .drawn(project.clone(), SidebarPart::Create, window)
            .is_some()
    );
    assert!(
        source
            .drawn(context_target.clone(), SidebarPart::Menu, window)
            .is_some()
    );
    assert!(
        source
            .drawn(context_target.clone(), SidebarPart::Create, window)
            .is_none()
    );
    assert!(
        source
            .drawn(agent_target.clone(), SidebarPart::Menu, window)
            .is_none()
    );
    assert!(source.has_part(&project, SidebarPart::Menu));
    assert!(source.has_part(&project, SidebarPart::Create));
    assert!(source.has_part(&context_target, SidebarPart::Menu));
    assert!(!source.has_part(&context_target, SidebarPart::Create));
    assert!(!source.has_part(&agent_target, SidebarPart::Menu));
    let standalone = SidebarTarget::Context {
        machine: Choice::Local,
        target: context(11).target,
    };
    assert!(
        source
            .drawn(standalone.clone(), SidebarPart::Menu, window)
            .is_some()
    );
    assert!(source.has_part(&standalone, SidebarPart::Menu));
    assert!(!source.has_part(&standalone, SidebarPart::Create));

    picker.open = true;
    let covered = picker.sidebar_source();
    assert!(covered.has_part(&project, SidebarPart::Menu));
    assert!(covered.has_part(&project, SidebarPart::Create));
    assert!(
        covered
            .drawn(project.clone(), SidebarPart::Menu, window)
            .is_none(),
        "surface admission is independent from the semantic control footprint"
    );
    picker.open = false;

    let project_primary = source
        .drawn(project.clone(), SidebarPart::Primary, window)
        .unwrap();
    let context_disclosure = source
        .drawn(context_target.clone(), SidebarPart::Disclosure, window)
        .unwrap();
    let agent_primary = source
        .drawn(agent_target.clone(), SidebarPart::Primary, window)
        .unwrap();
    assert!(
        source
            .drawn(agent_target, SidebarPart::Disclosure, window)
            .is_none()
    );
    assert_eq!(
        picker.sidebar_activation(&project_primary),
        Some(SidebarActivation::ToggleProject(project.clone()))
    );
    assert_eq!(
        picker.sidebar_activation(&context_disclosure),
        Some(SidebarActivation::ToggleContext(context_target))
    );
    assert!(matches!(
        picker.sidebar_activation(&agent_primary),
        Some(SidebarActivation::Activate(_))
    ));

    let mut refreshed = agent;
    refreshed.sequence = orchbee_workbench_protocol::WorkbenchRevision::new(2);
    refreshed.state = orchbee_workbench_protocol::PanelAgentState::Blocked;
    observe_agents_at(&mut picker, vec![refreshed], 2);
    assert_eq!(picker.sidebar_source(), source);
    assert!(matches!(
        picker.sidebar_activation(&agent_primary),
        Some(SidebarActivation::Activate(_))
    ));

    assert!(picker.toggle_sidebar_project(&project));
    assert!(picker.sidebar_activation(&project_primary).is_none());
    assert!(picker.sidebar_activation(&context_disclosure).is_none());
    assert!(picker.sidebar_activation(&agent_primary).is_none());
}

#[test]
fn sidebar_fixed_actions_revalidate_their_effect_scope_and_availability() {
    let mut picker = picker();
    picker.observe_focus(Choice::Local, Some(workspace(1, 21).target), None);
    let source = picker.sidebar_source();
    let new_workspace = picker.drawn_sidebar_fixed(&source, SidebarFixedAction::NewWorkspace);
    let search = picker.drawn_sidebar_fixed(&source, SidebarFixedAction::Search);
    let settings = picker.drawn_sidebar_fixed(&source, SidebarFixedAction::Settings);
    let browse = picker.drawn_sidebar_fixed(&source, SidebarFixedAction::Browse);
    assert_eq!(
        picker.sidebar_fixed_activation(&new_workspace),
        Some(SidebarFixedActivation::NewWorkspace)
    );
    assert_eq!(
        picker.sidebar_fixed_activation(&search),
        Some(SidebarFixedActivation::Search)
    );
    assert_eq!(
        picker.sidebar_fixed_activation(&settings),
        Some(SidebarFixedActivation::Settings)
    );
    assert_eq!(
        picker.sidebar_fixed_activation(&browse),
        Some(SidebarFixedActivation::Browse)
    );

    picker.observe_focus(Choice::Local, Some(workspace(1, 22).target), None);
    assert_eq!(
        picker.sidebar_source(),
        source,
        "same-site current movement is not a tree-source change"
    );
    assert!(picker.sidebar_fixed_activation(&new_workspace).is_none());
    assert!(picker.sidebar_fixed_activation(&browse).is_none());
    assert_eq!(
        picker.sidebar_fixed_activation(&search),
        Some(SidebarFixedActivation::Search)
    );
    assert_eq!(
        picker.sidebar_fixed_activation(&settings),
        Some(SidebarFixedActivation::Settings)
    );

    let source = picker.sidebar_source();
    let settings = picker.drawn_sidebar_fixed(&source, SidebarFixedAction::Settings);
    picker.available = false;
    assert!(picker.sidebar_fixed_activation(&settings).is_none());
    let unavailable = picker.sidebar_source();
    let settings = picker.drawn_sidebar_fixed(&unavailable, SidebarFixedAction::Settings);
    let search = picker.drawn_sidebar_fixed(&unavailable, SidebarFixedAction::Search);
    assert_eq!(
        picker.sidebar_fixed_activation(&settings),
        Some(SidebarFixedActivation::Settings)
    );
    assert!(picker.sidebar_fixed_activation(&search).is_none());
}

#[test]
fn agent_children_default_expanded_keep_global_ordinals_and_independent_disclosure() {
    let mut picker = picker();
    let first = workspace(1, 21).target;
    let second = workspace(1, 22).target;
    let standalone = context(11).target;
    observe_agents(
        &mut picker,
        vec![
            agent(first.clone(), 23),
            agent(second.clone(), 24),
            agent(first.clone(), 25),
            agent(standalone.clone(), 26),
        ],
    );
    let ordinals = |picker: &Picker| {
        picker
            .sidebar_navigation_rows()
            .iter()
            .filter_map(|row| match row {
                SidebarRow::Agent { ordinal, .. } => Some(*ordinal),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(ordinals(&picker), [3, 0, 2, 1]);
    let first = SidebarTarget::Context {
        machine: Choice::Local,
        target: first,
    };
    let standalone = SidebarTarget::Context {
        machine: Choice::Local,
        target: standalone,
    };
    assert!(picker.sidebar_context_agents_expanded(&first));
    assert!(picker.sidebar_context_agents_expanded(&standalone));
    assert!(picker.toggle_sidebar_context_agents(&first));
    assert_eq!(ordinals(&picker), [3, 1]);
    assert!(picker.sidebar_context_agents_expanded(&standalone));
    assert!(picker.toggle_sidebar_context_agents(&standalone));
    assert_eq!(ordinals(&picker), [1]);
    assert!(picker.toggle_sidebar_context_agents(&first));
    assert_eq!(ordinals(&picker), [0, 2, 1]);
    picker.observe_focus(Choice::Local, Some(second), None);
    assert!(!picker.sidebar_context_agents_expanded(&standalone));
    assert!(picker.sidebar_context_agents_expanded(&first));
    assert!(picker.request.is_none());
}

#[test]
fn context_keyboard_disclosure_enters_and_leaves_exact_agent_without_activating_early() {
    let mut picker = picker();
    let context = workspace(1, 21).target;
    let row = agent(context.clone(), 23);
    let agent_target = row.target.clone();
    observe_agents(&mut picker, vec![row]);
    picker.observe_focus(Choice::Local, Some(context.clone()), None);
    assert!(picker.enter_sidebar_navigation());
    let owner = SidebarTarget::Context {
        machine: Choice::Local,
        target: context.clone(),
    };
    assert_eq!(picker.sidebar_focused_target(), Some(&owner));

    assert!(picker.sidebar_context_agents_expanded(&owner));
    assert_eq!(key(&mut picker, KeyCode::Right), SidebarIntent::None);
    assert!(matches!(
        picker.sidebar_focused_target(),
        Some(SidebarTarget::Agent { target, .. }) if target.as_ref() == &agent_target
    ));
    assert_eq!(
        key(&mut picker, KeyCode::Enter),
        SidebarIntent::Activate(Selection {
            machine: Choice::Local,
            context: Some(crate::terminal_session::NativeStartupTarget::Agent(
                Box::new(agent_target)
            ))
        })
    );
    assert!(picker.request.is_none());

    assert_eq!(key(&mut picker, KeyCode::Left), SidebarIntent::None);
    assert_eq!(picker.sidebar_focused_target(), Some(&owner));
    assert_eq!(key(&mut picker, KeyCode::Left), SidebarIntent::None);
    assert!(!picker.sidebar_context_agents_expanded(&owner));
    assert_eq!(picker.sidebar_focused_target(), Some(&owner));
    assert_eq!(
        key(&mut picker, KeyCode::Enter),
        SidebarIntent::Activate(Selection {
            machine: Choice::Local,
            context: Some(context.into())
        })
    );
    assert!(picker.request.is_none());
}

#[test]
fn unavailable_agent_source_preserves_explicit_collapse_until_context_disappears() {
    let mut picker = picker();
    let context = workspace(1, 21).target;
    let row = agent(context.clone(), 23);
    observe_agents(&mut picker, vec![row.clone()]);
    let owner = SidebarTarget::Context {
        machine: Choice::Local,
        target: context,
    };
    assert!(picker.toggle_sidebar_context_agents(&owner));
    picker.observe_agents(vec![(
        Choice::Local,
        crate::machine_agents::Observation::Unavailable,
    )]);
    assert!(!picker.sidebar_context_agents_expanded(&owner));
    observe_agents(&mut picker, vec![row]);
    assert!(picker.sidebar_context_has_agents(&owner));
    assert!(!picker.sidebar_context_agents_expanded(&owner));
}

#[test]
fn pointer_disclosure_preserves_manual_viewport_and_focus_while_canceling_old_wheel_geometry() {
    for active in [false, true] {
        let mut picker = picker();
        let owner_context = context(11).target;
        observe_agents(&mut picker, vec![agent(owner_context.clone(), 23)]);
        let owner = SidebarTarget::Context {
            machine: Choice::Local,
            target: owner_context,
        };
        let retained = SidebarTarget::Context {
            machine: Choice::Local,
            target: workspace(2, 23).target,
        };
        assert!(picker.focus_sidebar_target(&retained));
        let before = {
            let rows = picker.sidebar_navigation_rows();
            let heights = vec![1; rows.len()];
            let gaps = vec![0; rows.len()];
            picker.space_window_for_row_gaps(&rows, &heights, &gaps, 2)
        };
        picker.queue_space_scroll(before, true);
        assert!(picker.settle_sidebar_scroll());
        let manual_top = {
            let rows = picker.sidebar_navigation_rows();
            picker
                .space_window_for_row_gaps(&rows, &vec![1; rows.len()], &vec![0; rows.len()], 2)
                .top
        };
        assert!(manual_top < before.top);
        if !active {
            picker.leave_sidebar_navigation();
        }

        let old_geometry = {
            let rows = picker.sidebar_navigation_rows();
            picker.space_window_for_row_gaps(&rows, &vec![1; rows.len()], &vec![0; rows.len()], 2)
        };
        picker.queue_space_scroll(old_geometry, false);
        assert!(picker.toggle_sidebar_context_agents(&owner));
        assert!(!picker.settle_sidebar_scroll());
        let expanded_top = {
            let rows = picker.sidebar_navigation_rows();
            picker
                .space_window_for_row_gaps(&rows, &vec![1; rows.len()], &vec![0; rows.len()], 2)
                .top
        };
        assert_eq!(expanded_top, manual_top);
        if active {
            assert_eq!(picker.sidebar_focused_target(), Some(&retained));
        } else {
            assert!(picker.enter_sidebar_navigation());
            assert_eq!(picker.sidebar_focused_target(), Some(&retained));
        }
        assert!(picker.request.is_none());
    }
}

#[test]
fn agent_join_uses_choice_site_and_full_context_location_across_colliding_ids() {
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.observe_agent_sidebar(true);
    let local_workspace = workspace(1, 21).target;
    let mut remote_site_workspace = local_workspace.clone();
    remote_site_workspace.site_id = SiteId::parse(&format!("{:032x}", 10)).unwrap();
    let standalone = Context {
        target: NativeContextTarget {
            site_id: local_workspace.site_id.clone(),
            location: NativeContextLocation::Standalone(
                WorkbenchTerminalContextId::parse(&format!("{:032x}", 21)).unwrap(),
            ),
        },
        project_id: None,
        project: None,
        workspace: Some("same 工作区".into()),
    };
    picker.observe_local(vec![
        Context {
            target: local_workspace.clone(),
            project_id: workspace(1, 21).project_id,
            project: Some("same Project".into()),
            workspace: Some("same 工作区".into()),
        },
        standalone.clone(),
    ]);
    let remote_a = profile(1, "same machine", true);
    let remote_b = profile(2, "same machine", true);
    let choice_a = Choice::Remote(remote_a.id.clone());
    let choice_b = Choice::Remote(remote_b.id.clone());
    picker.replace(&[remote_a, remote_b]);
    picker.observe_spaces(vec![
        (choice_a.clone(), checkpoint(9)),
        (choice_b.clone(), checkpoint(10)),
    ]);

    let checkpoint = |site: &SiteId,
                      contexts: Vec<NativeContextTarget>,
                      rows: Vec<crate::machine_agents::Row>| {
        let mut checkpoint = crate::machine_agents::tests::checkpoint(rows)
            .as_ref()
            .clone();
        checkpoint.site_id = site.clone();
        checkpoint.contexts = contexts.into();
        crate::machine_agents::Observation::Ready(std::sync::Arc::new(checkpoint))
    };
    picker.observe_agents(vec![
        (
            Choice::Local,
            checkpoint(
                &local_workspace.site_id,
                vec![local_workspace.clone(), standalone.target.clone()],
                vec![
                    agent(local_workspace.clone(), 23),
                    agent(standalone.target.clone(), 24),
                ],
            ),
        ),
        (
            choice_a.clone(),
            checkpoint(
                &local_workspace.site_id,
                vec![local_workspace.clone()],
                vec![agent(local_workspace.clone(), 25)],
            ),
        ),
        (
            choice_b.clone(),
            checkpoint(
                &remote_site_workspace.site_id,
                vec![remote_site_workspace.clone()],
                vec![agent(remote_site_workspace.clone(), 26)],
            ),
        ),
    ]);

    let owners = picker
        .sidebar_navigation_rows()
        .iter()
        .filter_map(|row| match row {
            SidebarRow::Context {
                has_agents: true, ..
            } => Some(row.target()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for owner in &owners {
        assert!(picker.toggle_sidebar_context_agents(owner));
    }
    for (choice, context, member) in [
        (Choice::Local, local_workspace, 23_u64),
        (Choice::Local, standalone.target, 24),
        (choice_a, workspace(1, 21).target, 25),
        (choice_b, remote_site_workspace, 26),
    ] {
        let owner = SidebarTarget::Context {
            machine: choice.clone(),
            target: context,
        };
        assert!(picker.sidebar_context_has_agents(&owner));
        assert!(picker.toggle_sidebar_context_agents(&owner));
        let visible = picker
            .sidebar_navigation_rows()
            .iter()
            .filter_map(|row| match row {
                SidebarRow::Agent { row, .. } => Some((
                    row.choice.clone(),
                    row.sidebar_agent().unwrap().target.member.clone(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].0, choice);
        assert_eq!(
            visible[0].1.as_str(),
            format!("{member:032x}"),
            "an Agent crossed its endpoint, Site, or Context owner"
        );
        assert!(picker.toggle_sidebar_context_agents(&owner));
    }
}

#[test]
fn complete_checkpoints_preserve_empty_projects_and_machine_site_identity() {
    let mut picker = picker();
    let a = profile(1, "same machine", true);
    let b = profile(2, "same machine", true);
    picker.replace(&[a.clone(), b.clone()]);
    picker.observe_spaces(vec![
        (Choice::Local, checkpoint(9)),
        (Choice::Remote(a.id), checkpoint(9)),
        (Choice::Remote(b.id), checkpoint(10)),
    ]);
    let targets: Vec<_> = picker
        .sidebar_navigation_rows()
        .iter()
        .map(SidebarRow::target)
        .collect();
    assert_eq!(
        targets.len(),
        9,
        "each Machine has two Projects and one Workspace"
    );
    assert_eq!(
        targets
            .iter()
            .filter(|target| matches!(target, SidebarTarget::Project { .. }))
            .count(),
        6
    );
    assert!(
        targets
            .iter()
            .enumerate()
            .all(|(i, target)| !targets[..i].contains(target))
    );
    let empty = targets[2].clone();
    assert!(picker.focus_sidebar_target(&empty));
    assert_eq!(key(&mut picker, KeyCode::Enter), SidebarIntent::None);
    assert_eq!(
        key(&mut picker, KeyCode::F(2)),
        SidebarIntent::OpenMenu(empty)
    );
    assert!(picker.request.is_none());
}

#[test]
fn regroup_and_dissolve_preserve_focused_agent_and_exact_activation() {
    let mut picker = picker();
    let active = workspace(1, 21).target;
    let first = agent(active.clone(), 23);
    let member = NativeMemberTarget {
        tab: first.target.tab.clone(),
        panel: first.target.panel.clone(),
        presentation: first.target.member.clone(),
    };
    let selected = SidebarTarget::Agent {
        machine: Choice::Local,
        target: Box::new(first.target.clone()),
    };
    let expected = Selection {
        machine: Choice::Local,
        context: Some(NativeStartupTarget::Agent(Box::new(first.target.clone()))),
    };
    observe_agents(&mut picker, vec![first, agent(workspace(2, 23).target, 24)]);
    picker.observe_focus(Choice::Local, Some(active.clone()), Some(member.clone()));
    assert!(picker.focus_sidebar_target(&selected));
    let context = picker.sidebar_parent(&selected).unwrap();
    for group in [Some(2), None, Some(1)] {
        let mut rows = picker.local_contexts.to_vec();
        let row = rows.iter_mut().find(|row| row.target == active).unwrap();
        row.project_id = group.map(|id| WorkbenchProjectId::parse(&format!("{id:032x}")).unwrap());
        row.project = group.map(|_| "renamed group".into());
        picker.observe_local(rows);
        assert_eq!(picker.sidebar_focused_target(), Some(&selected));
        assert_eq!(picker.current_context.as_ref(), Some(&active));
        assert_eq!(picker.current_member.as_ref(), Some(&member));
        assert_eq!(picker.sidebar_parent(&context).is_some(), group.is_some());
        assert_eq!(
            key(&mut picker, KeyCode::Enter),
            SidebarIntent::Activate(expected.clone())
        );
        assert!(picker.request.is_none());
        assert_eq!(
            picker
                .sidebar_navigation_rows()
                .iter()
                .filter(|row| matches!(row, SidebarRow::Agent { .. }))
                .count(),
            2
        );
    }
    // Mouse activation resolves the same existing Tab / Panel / presentation as Enter.
    assert!(picker.activate_sidebar_agent(&expected));
    assert!(
        picker.request.is_none(),
        "the already current Agent remains a no-op"
    );
}

#[test]
fn project_collapse_keeps_independent_latent_agent_disclosure_and_counts() {
    use orchbee_workbench_protocol::PanelAgentState;
    let mut picker = picker();
    let first = SidebarTarget::Context {
        machine: Choice::Local,
        target: workspace(1, 21).target,
    };
    let second = SidebarTarget::Context {
        machine: Choice::Local,
        target: workspace(1, 22).target,
    };
    let mut waiting = agent(workspace(1, 21).target, 23);
    waiting.state = PanelAgentState::Blocked;
    observe_agents(
        &mut picker,
        vec![waiting, agent(workspace(1, 22).target, 24)],
    );
    let project = picker.sidebar_parent(&first).unwrap();
    picker.observe_focus(Choice::Local, Some(workspace(1, 21).target), None);
    assert!(picker.toggle_sidebar_context_agents(&second));
    assert!(picker.focus_sidebar_target(&first));
    assert!(picker.toggle_sidebar_project(&project));
    assert_eq!(picker.sidebar_focused_target(), Some(&project));
    assert!(picker.sidebar_context_agents_expanded(&first));
    assert!(!picker.sidebar_context_agents_expanded(&second));
    let counts = picker.sidebar_agent_counts(&project).unwrap();
    assert_eq!((counts.total, counts.waiting), (2, 1));
    assert_eq!(picker.sidebar_workspace_count(), 4);
    assert!(!picker.activate_sidebar_workspace(&Selection {
        machine: Choice::Local,
        context: Some(workspace(1, 21).target.into())
    }));
    assert!(picker.toggle_sidebar_project(&project));
    assert!(picker.sidebar_context_agents_expanded(&first));
    assert!(!picker.sidebar_context_agents_expanded(&second));
    assert_eq!(picker.sidebar_agent_counts(&project), Some(counts));
    assert_eq!(picker.current_context, Some(workspace(1, 21).target));
    assert!(picker.request.is_none());
}

#[test]
fn regroup_into_collapsed_project_uses_new_parent_then_dissolve_returns_root_entry() {
    let mut picker = picker();
    let owner = SidebarTarget::Context {
        machine: Choice::Local,
        target: workspace(1, 21).target,
    };
    let destination = picker
        .sidebar_parent(&SidebarTarget::Context {
            machine: Choice::Local,
            target: workspace(2, 23).target,
        })
        .unwrap();
    assert!(picker.toggle_sidebar_project(&destination));
    assert!(picker.focus_sidebar_target(&owner));
    let mut rows = picker.local_contexts.to_vec();
    rows[1].project_id = workspace(2, 23).project_id;
    picker.observe_local(rows.clone());
    assert_eq!(picker.sidebar_focused_target(), Some(&destination));
    assert_eq!(picker.sidebar_parent(&owner), Some(destination));
    for row in &mut rows {
        row.project_id = None;
        row.project = None;
    }
    picker.observe_local(rows);
    assert!(picker.focus_sidebar_target(&owner));
    assert!(picker.sidebar_parent(&owner).is_none());
    assert_eq!(picker.sidebar_navigation_rows().len(), 4);
    assert_eq!(key(&mut picker, KeyCode::Left), SidebarIntent::None);
    assert_eq!(picker.sidebar_focused_target(), Some(&owner));
    assert!(picker.request.is_none());
}

#[test]
fn sidebar_focus_disclosure_and_menu_preserve_active_context_until_explicit_activation() {
    let mut picker = picker();
    let active = workspace(1, 22).target;
    picker.observe_focus(Choice::Local, Some(active.clone()), None);
    assert!(picker.enter_sidebar_navigation());
    assert_eq!(
        picker.sidebar_focused_target(),
        Some(&SidebarTarget::Context {
            machine: Choice::Local,
            target: active.clone()
        })
    );
    assert_eq!(key(&mut picker, KeyCode::Left), SidebarIntent::None);
    let project = picker.sidebar_focused_target().unwrap().clone();
    assert!(matches!(project, SidebarTarget::Project { .. }));
    assert_eq!(key(&mut picker, KeyCode::Enter), SidebarIntent::None);
    assert!(!picker.sidebar_project_expanded(&project));
    assert_eq!(
        key(&mut picker, KeyCode::F(2)),
        SidebarIntent::OpenMenu(project.clone())
    );
    assert_eq!(picker.current_context, Some(active.clone()));
    assert!(picker.request.is_none());
    assert_eq!(key(&mut picker, KeyCode::Right), SidebarIntent::None);
    assert_eq!(key(&mut picker, KeyCode::Right), SidebarIntent::None);
    let expected = Selection {
        machine: Choice::Local,
        context: Some(workspace(1, 21).target.into()),
    };
    assert_eq!(
        key(&mut picker, KeyCode::Enter),
        SidebarIntent::Activate(expected)
    );
    assert_eq!(picker.current_context, Some(active));
    assert!(
        picker.request.is_none(),
        "the dispatcher alone admits the returned activation"
    );
}

#[test]
fn sidebar_reconciles_removed_children_to_parent_and_removed_projects_to_neighbor() {
    let mut picker = picker();
    picker.enter_sidebar_navigation();
    let removed = SidebarTarget::Context {
        machine: Choice::Local,
        target: workspace(1, 22).target,
    };
    assert!(picker.focus_sidebar_target(&removed));
    picker.observe_local(vec![context(11), workspace(1, 21), workspace(2, 23)]);
    assert!(
        matches!(picker.sidebar_focused_target(), Some(SidebarTarget::Project { project_id, .. }) if project_id.as_str() == format!("{:032x}", 1))
    );
    assert!(!picker.sidebar_target_exists(&removed));
    picker.observe_local(vec![context(11), workspace(2, 23)]);
    assert!(
        matches!(picker.sidebar_focused_target(), Some(SidebarTarget::Project { project_id, .. }) if project_id.as_str() == format!("{:032x}", 2))
    );
    picker.observe_local(Vec::new());
    assert!(picker.sidebar_navigation_active());
    assert!(picker.sidebar_focused_target().is_none());
    assert_eq!(key(&mut picker, KeyCode::Enter), SidebarIntent::None);
    picker.observe_agent_sidebar(false);
    assert!(!picker.sidebar_navigation_active());
    assert_eq!(key(&mut picker, KeyCode::F(2)), SidebarIntent::None);
    assert!(
        picker.enter_sidebar_navigation(),
        "entry can precede the revealed frame"
    );
    assert_eq!(key(&mut picker, KeyCode::F(1)), SidebarIntent::Help);
    assert_eq!(key(&mut picker, KeyCode::Esc), SidebarIntent::None);
    assert!(!picker.sidebar_navigation_active());
}

#[test]
fn removed_workspace_focus_uses_last_project_even_with_a_stale_agent_observation() {
    let mut picker = picker();
    let row = agent(workspace(1, 21).target, 23);
    let focused = SidebarTarget::Agent {
        machine: Choice::Local,
        target: Box::new(row.target.clone()),
    };
    observe_agents(&mut picker, vec![row]);
    assert!(picker.focus_sidebar_target(&focused));
    let owner = picker.sidebar_parent(&focused).unwrap();
    let project = picker.sidebar_parent(&owner).unwrap();
    picker.observe_local(vec![context(11), workspace(1, 22), workspace(2, 23)]);
    assert!(!picker.sidebar_target_exists(&focused));
    assert_eq!(picker.sidebar_focused_target(), Some(&project));
    assert!(picker.request.is_none());
}

#[test]
fn sidebar_navigation_reveal_uses_visible_tree_heights_and_resumes_after_wheel() {
    let mut picker = picker();
    picker.enter_sidebar_navigation();
    key(&mut picker, KeyCode::End);
    let heights = [1, 1, 2, 3, 1, 2];
    let last = window(&picker, &heights, 3, 0);
    assert_eq!((last.top, last.end()), (4, 6));
    picker.queue_space_scroll(last, true);
    assert!(picker.settle_sidebar_scroll());
    assert!(window(&picker, &heights, 3, 0).end() < 6);
    key(&mut picker, KeyCode::End);
    assert_eq!(window(&picker, &heights, 3, 0).end(), 6);
    assert!(picker.request.is_none());
}

#[test]
fn entering_on_a_collapsed_active_workspace_focuses_its_parent_without_disclosure() {
    let mut picker = picker();
    let active = workspace(1, 22).target;
    picker.observe_focus(Choice::Local, Some(active.clone()), None);
    let parent = picker
        .sidebar_parent(&SidebarTarget::Context {
            machine: Choice::Local,
            target: active.clone(),
        })
        .unwrap();
    assert!(picker.toggle_sidebar_project(&parent));
    assert!(!picker.sidebar_navigation_active());
    assert!(picker.sidebar_focused_target().is_none());
    let before: Vec<_> = picker
        .sidebar_navigation_rows()
        .iter()
        .map(SidebarRow::target)
        .collect();
    assert!(picker.enter_sidebar_navigation());
    assert_eq!(picker.sidebar_focused_target(), Some(&parent));
    assert!(!picker.sidebar_project_expanded(&parent));
    assert_eq!(
        picker
            .sidebar_navigation_rows()
            .iter()
            .map(SidebarRow::target)
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(picker.current_context, Some(active));
    assert!(picker.request.is_none());
}

#[test]
fn width_keys_only_emit_intents_including_empty_owned_navigation() {
    use crate::tui_layout::SidebarWidthChange;
    let mut picker = picker();
    picker.observe_local(Vec::new());
    picker.enter_sidebar_navigation();
    for (code, modifiers, expected) in [
        (
            KeyCode::Left,
            Modifiers::NONE.with_shift(),
            SidebarWidthChange::Narrower,
        ),
        (
            KeyCode::Right,
            Modifiers::NONE.with_shift(),
            SidebarWidthChange::Wider,
        ),
        (
            KeyCode::Char('0'),
            Modifiers::NONE,
            SidebarWidthChange::Reset,
        ),
    ] {
        let before = picker.clone();
        assert_eq!(
            picker.sidebar_key(KeyEvent::press(code, modifiers)),
            SidebarIntent::Resize(expected)
        );
        assert_eq!(
            picker, before,
            "the dispatcher alone admits a painted width intent"
        );
        assert_eq!(
            picker.sidebar_key(KeyEvent::press(code, Modifiers::CTRL)),
            SidebarIntent::None
        );
    }
    picker.leave_sidebar_navigation();
    assert_eq!(
        picker.sidebar_key(KeyEvent::press(
            KeyCode::Right,
            Modifiers::NONE.with_shift()
        )),
        SidebarIntent::None
    );
}

#[test]
fn width_compare_and_set_rejects_equal_rect_staleness_and_preserves_navigation() {
    use crate::{
        client_view::SidebarSurface,
        tui_layout::{DrawnSidebarWidth, SidebarWidth},
    };
    let width = |columns| SidebarWidth::new(columns).unwrap();
    let mut picker = picker();
    let active = workspace(1, 21).target;
    picker.observe_focus(Choice::Local, Some(active.clone()), None);
    picker.enter_sidebar_navigation();
    let parent = picker
        .sidebar_parent(&SidebarTarget::Context {
            machine: Choice::Local,
            target: active.clone(),
        })
        .unwrap();
    picker.toggle_sidebar_project(&parent);
    key(&mut picker, KeyCode::End);
    let focused = picker.sidebar_focused_target().cloned();
    let queued = window(&picker, &[1; 4], 2, 0);
    picker.queue_space_scroll(queued, true);
    assert!(picker.set_sidebar_width(SidebarWidth::DEFAULT, width(60)));
    assert!(picker.spaces.scroll.is_none());
    assert!(picker.sidebar_navigation_active());
    assert_eq!(picker.sidebar_focused_target(), focused.as_ref());
    assert!(!picker.sidebar_project_expanded(&parent));
    assert_eq!(picker.current_context, Some(active));
    assert!(picker.request.is_none());
    assert_eq!(window(&picker, &[1; 4], 2, 0).end(), 4);
    let viewport = Rect::new(0, 0, 60, 12);
    let old = DrawnSidebarWidth::new(viewport, SidebarSurface::Overlay(width(60))).unwrap();
    assert!(picker.set_sidebar_width(old.preferred(), width(40)));
    let current =
        DrawnSidebarWidth::new(viewport, SidebarSurface::Overlay(picker.sidebar_width())).unwrap();
    assert_eq!(old.sidebar, current.sidebar);
    let before = picker.clone();
    assert!(!picker.set_sidebar_width(old.preferred(), width(28)));
    assert!(!picker.set_sidebar_width(width(40), width(40)));
    assert_eq!(picker, before);
    picker.observe_agent_sidebar(false);
    assert!(!picker.set_sidebar_width(width(40), width(30)));
    picker.observe_agent_sidebar(true);
    picker.show();
    assert!(!picker.set_sidebar_width(width(40), width(30)));
}

#[derive(Clone, Copy, Debug)]
enum Event {
    Key(u8),
    Retain(u8),
    Reverse,
    Agents(u8),
    Toggle(u8),
    Filter,
    Hide,
    Enter,
    Regroup(u8),
}

fn replay(events: &[Event]) -> Result<(), &'static str> {
    let mut picker = picker();
    let original = [workspace(1, 21), workspace(1, 22), workspace(2, 23)];
    let active = original[0].target.clone();
    picker.observe_focus(Choice::Local, Some(active.clone()), None);
    picker.enter_sidebar_navigation();
    for event in events {
        match *event {
            Event::Key(index) => {
                key(
                    &mut picker,
                    [
                        KeyCode::Up,
                        KeyCode::Down,
                        KeyCode::Home,
                        KeyCode::End,
                        KeyCode::Left,
                        KeyCode::Right,
                        KeyCode::Enter,
                        KeyCode::F(2),
                        KeyCode::Esc,
                    ][usize::from(index)],
                );
            }
            Event::Retain(mask) => {
                picker.observe_local(
                    original
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| mask & (1 << i) != 0)
                        .map(|(_, row)| row.clone())
                        .collect(),
                );
            }
            Event::Reverse => {
                let mut rows = picker.local_contexts.to_vec();
                rows.reverse();
                picker.observe_local(rows);
            }
            Event::Regroup(group) => {
                let mut rows = picker.local_contexts.to_vec();
                for row in &mut rows {
                    if !matches!(row.target.location, NativeContextLocation::Workspace { .. }) {
                        continue;
                    }
                    row.project_id = (group > 0)
                        .then(|| WorkbenchProjectId::parse(&format!("{group:032x}")).unwrap());
                    row.project = row.project_id.as_ref().map(|_| "same Project".into());
                }
                picker.observe_local(rows);
            }
            Event::Agents(mask) => {
                observe_agents(
                    &mut picker,
                    original
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| mask & (1 << index) != 0)
                        .map(|(index, context)| agent(context.target.clone(), 23 + index as u64))
                        .collect(),
                );
            }
            Event::Toggle(index) => {
                let owners = picker
                    .sidebar_navigation_rows()
                    .iter()
                    .filter_map(|row| match row {
                        SidebarRow::Context {
                            has_agents: true, ..
                        } => Some(row.target()),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if let Some(owner) = owners.get(usize::from(index) % owners.len().max(1)) {
                    picker.toggle_sidebar_context_agents(owner);
                }
            }
            Event::Filter => picker.agent_control(crate::machine_picker::agents::Control::State),
            Event::Hide => picker.observe_agent_sidebar(false),
            Event::Enter => {
                picker.observe_agent_sidebar(true);
                picker.enter_sidebar_navigation();
            }
        }
        if picker.request.is_some() || picker.current_context.as_ref() != Some(&active) {
            return Err("browsing activated a context");
        }
        let targets: Vec<_> = picker
            .sidebar_navigation_rows()
            .iter()
            .map(SidebarRow::target)
            .collect();
        if targets
            .iter()
            .enumerate()
            .any(|(i, target)| targets[..i].contains(target))
        {
            return Err("duplicate stable row identity");
        }
        let rows = picker.sidebar_navigation_rows();
        for row in rows
            .iter()
            .filter(|row| matches!(row, SidebarRow::Agent { .. }))
        {
            let target = row.target();
            let Some(parent) = picker.sidebar_parent(&target) else {
                return Err("visible Agent has no Context parent");
            };
            if !targets.contains(&parent)
                || !picker.sidebar_context_agents_expanded(&parent)
                || !picker.sidebar_target_exists(&target)
            {
                return Err("visible Agent escaped its exact current expanded owner");
            }
        }
        if let Some(target) = picker.sidebar_focused_target() {
            if !targets.contains(target) || !picker.sidebar_target_exists(target) {
                return Err("focused target is not visible and current");
            }
        } else if picker.sidebar_navigation_active() && !targets.is_empty() {
            return Err("nonempty keyboard owner lost focus");
        }
    }
    Ok(())
}

#[test]
fn bounded_sidebar_traces_preserve_identity_focus_and_activation_separation() {
    for seed in [1_u64, 7, 29, 101, 733, 4093] {
        let mut random = seed;
        let events: Vec<_> = (0..128)
            .map(|_| {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                match random % 9 {
                    0 => Event::Retain(((random >> 8) % 8) as u8),
                    1 => Event::Reverse,
                    2 => Event::Agents(((random >> 8) % 8) as u8),
                    3 => Event::Toggle(((random >> 8) % 4) as u8),
                    4 => Event::Filter,
                    5 => Event::Hide,
                    6 => Event::Enter,
                    7 => Event::Regroup(((random >> 8) % 3) as u8),
                    _ => Event::Key(((random >> 8) % 9) as u8),
                }
            })
            .collect();
        if replay(&events).is_err() {
            let mut reduced = events;
            let mut index = 0;
            while index < reduced.len() {
                let mut candidate = reduced.clone();
                candidate.remove(index);
                if replay(&candidate).is_err() {
                    reduced = candidate;
                    index = 0;
                } else {
                    index += 1;
                }
            }
            panic!("seed {seed}, replay {reduced:?}: {:?}", replay(&reduced));
        }
    }
}
