use super::*;
use crate::workbench_projection::project_preferred_workspace;
use crate::workbench_state::tests::snapshot;
use orchbee_workbench_protocol::{
    Directory, PanelNode, Presentation, PresentationBinding, TerminalId, TerminalPresentationId,
    WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchTerminalContextId, WorkerId,
};

fn id(n: u128) -> String {
    format!("{n:032x}")
}
fn project(n: u128) -> WorkbenchProjectId {
    WorkbenchProjectId::parse(&id(n)).unwrap()
}
fn workspace(n: u128) -> WorkbenchWorkspaceId {
    WorkbenchWorkspaceId::parse(&id(n)).unwrap()
}
fn path(value: &str) -> WorkbenchDirectoryPath {
    WorkbenchDirectoryPath::parse(value).unwrap()
}

pub(crate) fn fixture() -> Snapshot {
    let mut value = snapshot(4, 1, None);
    let mut tabs = std::mem::take(&mut value.terminal_contexts[0].tabs);
    value.terminal_contexts[0].tabs.push(tabs.remove(0));
    for n in [70, 71] {
        value.navigation.projects.push(Project {
            project_id: project(n),
            label: "same label".into(),
        });
    }
    for n in 0..2 {
        let directory = WorkbenchDirectoryId::parse(&id(90 + n)).unwrap();
        let context = WorkbenchTerminalContextId::parse(&id(82 + n)).unwrap();
        let mut members = vec![tabs.remove(0)];
        if n == 0 {
            members.push(tabs.remove(0));
        }
        let PanelNode::Panel {
            primary_presentation,
            ..
        } = &members[0].root
        else {
            unreachable!()
        };
        value.navigation.directories.push(Directory {
            directory_id: directory.clone(),
            worker_id: primary_presentation
                .binding
                .as_ref()
                .unwrap()
                .worker_id
                .clone(),
            path: path(if n == 0 {
                "C:/first-repository"
            } else {
                "C:/second-repository"
            }),
        });
        value.navigation.workspaces.push(Workspace {
            terminal_directory: None,
            automatic_label: false,
            workspace_id: workspace(80 + n),
            terminal_context_id: context.clone(),
            project_id: Some(project(70)),
            primary_directory_id: Some(directory),
            label: "duplicate Workspace label".into(),
        });
        value.terminal_contexts.push(TerminalContext {
            terminal_context_id: context,
            tabs: members,
            groups: Vec::new(),
        });
    }
    value.navigation.workspace_order = value
        .navigation
        .workspaces
        .iter()
        .map(|workspace| workspace.workspace_id.clone())
        .collect();
    value
}

#[test]
fn project_source_is_first_displayed_member_independent_of_current_context_and_labels() {
    let mut value = fixture();
    let current = project_preferred_workspace(&value).unwrap();
    let target = SidebarWorktreeTarget::Project(project(70));
    let first = AddressedWorktreeSource::capture(current.canonical_snapshot(), &target).unwrap();
    assert_eq!(
        first.cache.location(),
        &ClientContextLocation::Workspace {
            workspace_id: workspace(80),
            terminal_context_id: WorkbenchTerminalContextId::parse(&id(82)).unwrap(),
        }
    );
    assert_eq!(
        first.primary.as_ref().unwrap().terminal_id().as_str(),
        id(1014)
    );
    assert_ne!(first.cache.location(), current.location());
    assert!(
        AddressedWorktreeSource::capture(&value, &SidebarWorktreeTarget::Project(project(71)))
            .is_none(),
        "empty Project has no borrowed source"
    );
    value.navigation.workspace_order.reverse();
    let reordered = AddressedWorktreeSource::capture(&value, &target).unwrap();
    assert_eq!(
        reordered.primary.as_ref().unwrap().terminal_id().as_str(),
        id(1034)
    );
    assert!(
        !first.unchanged(&value),
        "reordering must retire discovery from the previous first member"
    );
}

#[test]
fn workspace_uses_first_terminal_primary_instead_of_later_tab_or_child_agent() {
    let mut value = fixture();
    let PanelNode::Panel {
        child_presentations,
        ..
    } = &mut value.terminal_contexts[1].tabs[0].root
    else {
        unreachable!()
    };
    child_presentations.push(Presentation {
        custom_label: None,
        presentation_id: TerminalPresentationId::parse(&id(9001)).unwrap(),
        binding: Some(PresentationBinding {
            terminal_id: TerminalId::parse(&id(9002)).unwrap(),
            worker_id: WorkerId::parse(&id(1013)).unwrap(),
        }),
        label: "focused Agent".into(),
        launch_failure: None,
        launch_program: None,
    });
    let source =
        AddressedWorktreeSource::capture(&value, &SidebarWorktreeTarget::Workspace(workspace(80)))
            .unwrap();
    assert_eq!(source.member.as_ref().unwrap().tab.as_str(), id(1010));
    assert_eq!(
        source.member.as_ref().unwrap().presentation.as_str(),
        id(1012)
    );
    assert_eq!(
        source.primary.as_ref().unwrap().terminal_id().as_str(),
        id(1014)
    );
    assert_ne!(
        source.primary.as_ref().unwrap().terminal_id().as_str(),
        id(1024)
    );
    assert_ne!(
        source.primary.as_ref().unwrap().terminal_id().as_str(),
        id(9002)
    );
}

#[test]
fn admitted_cwd_precedes_same_workspace_fallback_and_unavailable_never_becomes_absent() {
    let value = fixture();
    let source =
        AddressedWorktreeSource::capture(&value, &SidebarWorktreeTarget::Project(project(70)))
            .unwrap();
    assert_eq!(
        source.resolve_sample(RootDirectory::Available(Some(path(
            "C:/first-terminal/cwd"
        )))),
        Some(WorktreeSource::Directory {
            path: path("C:/first-terminal/cwd")
        })
    );
    assert_eq!(
        source.resolve_sample(RootDirectory::Available(None)),
        Some(WorktreeSource::Workspace {
            workspace_id: workspace(80)
        })
    );
    assert_eq!(source.resolve_sample(RootDirectory::Unavailable), None);
    // This model returns one source only. A failed/non-Git query of that
    // source cannot switch to the other member's potentially different repo.
    assert_eq!(
        source.cache.directory().unwrap().path,
        path("C:/first-repository")
    );
    assert_ne!(
        source.cache.directory().unwrap().path,
        path("C:/second-repository")
    );
}

#[test]
fn cwd_naming_the_workspace_directory_is_that_workspace_so_the_server_can_group_alongside_it() {
    let mut value = fixture();
    // Ungrouped: the Server's Alongside placement then creates the Project at commit, which a
    // Directory source without a Project would instead leave ungrouped.
    for member in &mut value.navigation.workspaces {
        if member.workspace_id == workspace(80) {
            member.project_id = None;
        }
    }
    let source =
        AddressedWorktreeSource::capture(&value, &SidebarWorktreeTarget::Workspace(workspace(80)))
            .unwrap();
    for cwd in ["C:/first-repository", "c:\\First-Repository\\"] {
        assert_eq!(
            source.resolve_sample(RootDirectory::Available(Some(path(cwd)))),
            Some(WorktreeSource::Workspace {
                workspace_id: workspace(80)
            }),
            "{cwd}"
        );
    }
    // Elsewhere the admitted cwd keeps its precedence as its own Directory source.
    assert_eq!(
        source.resolve_sample(RootDirectory::Available(Some(path(
            "C:/first-repository/nested"
        )))),
        Some(WorktreeSource::Directory {
            path: path("C:/first-repository/nested")
        })
    );
}

#[test]
fn source_scope_drift_retires_project_membership_directory_and_terminal_proof() {
    let value = fixture();
    let source =
        AddressedWorktreeSource::capture(&value, &SidebarWorktreeTarget::Project(project(70)))
            .unwrap();
    for change in 0..5 {
        let mut next = value.clone();
        match change {
            0 => next.navigation.workspaces[1].project_id = Some(project(71)),
            1 => next.navigation.workspace_order.reverse(),
            2 => next.navigation.directories[0].path = path("C:/changed-source"),
            3 => {
                let PanelNode::Panel {
                    primary_presentation,
                    ..
                } = &mut next.terminal_contexts[1].tabs[0].root
                else {
                    unreachable!()
                };
                primary_presentation.binding.as_mut().unwrap().terminal_id =
                    TerminalId::parse(&id(9900)).unwrap();
            }
            4 => next.terminal_contexts[1].tabs.reverse(),
            _ => unreachable!(),
        }
        assert!(
            !source.unchanged(&next),
            "source change {change} must retire old choices"
        );
    }
    let mut unrelated = value;
    unrelated.revision = orchbee_workbench_protocol::WorkbenchRevision::new(2);
    unrelated.navigation.projects[1].label = "unrelated group renamed".into();
    assert!(
        source.unchanged(&unrelated),
        "unrelated canonical change is not source drift"
    );
}

#[test]
fn explicitly_empty_workspace_can_use_only_its_own_directory_without_inventing_a_process_sample() {
    let mut value = fixture();
    value.terminal_contexts[1].tabs.clear();
    let source =
        AddressedWorktreeSource::capture(&value, &SidebarWorktreeTarget::Workspace(workspace(80)))
            .unwrap();
    assert!(source.primary.is_none());
    assert!(source.member.is_none());
    assert_eq!(
        source.resolve_sample(RootDirectory::Available(None)),
        Some(WorktreeSource::Workspace {
            workspace_id: workspace(80)
        })
    );
    assert_eq!(
        source.resolve_sample(RootDirectory::Available(Some(path("C:/unrelated-cwd")))),
        None
    );
    assert_eq!(source.resolve_sample(RootDirectory::Unavailable), None);
}

#[test]
fn unbound_first_terminal_is_unavailable_rather_than_an_absent_process_cwd() {
    let mut value = fixture();
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut value.terminal_contexts[1].tabs[0].root
    else {
        unreachable!()
    };
    primary_presentation.binding = None;
    let source =
        AddressedWorktreeSource::capture(&value, &SidebarWorktreeTarget::Project(project(70)))
            .unwrap();
    assert!(source.member.is_some());
    assert!(source.primary.is_none());
    assert_eq!(source.resolve_sample(RootDirectory::Available(None)), None);
    assert_eq!(source.resolve_sample(RootDirectory::Unavailable), None);
}
