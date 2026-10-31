use super::*;
use crate::machine_discovery::Context;
use crate::workbench_projection::ClientContextLocation;
use orchbee_workbench_protocol::*;
mod anchoring;
fn contexts(count: u64) -> Vec<Context> {
    let session = SessionId::parse(&format!("{:032x}", 1)).unwrap();
    (0..count)
        .map(|n| Context {
            target: ClientContextTarget {
                session_id: session.clone(),
                location: ClientContextLocation::Workspace {
                    workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", n + 100))
                        .unwrap(),
                    terminal_context_id: WorkbenchTerminalContextId::parse(&format!(
                        "{:032x}",
                        n + 200
                    ))
                    .unwrap(),
                },
            },
            project_id: Some(WorkbenchProjectId::parse(&format!("{:032x}", 2)).unwrap()),
            project: Some("Repo".into()),
            workspace: Some("same 工作区".into()),
        })
        .collect()
}
fn picker(count: u64) -> Picker {
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.observe_agent_sidebar(true);
    picker.observe_local(contexts(count));
    picker
}

fn compact_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

#[test]
fn workspace_search_owns_unicode_spaces_parentheses_and_navigation_letters_until_acceptance() {
    let mut picker = picker(3);
    let mut source = contexts(3);
    for (context, label) in source
        .iter_mut()
        .zip(["project", "project (2) 中文", "project (3)"])
    {
        context.workspace = Some(label.into());
    }
    let expected = source[1].target.clone();
    picker.observe_local(source.clone());
    picker.show_workspaces();
    let old = paint(&picker, Locale::EnUs, 120, 40).target(PickerAction::Row(0));
    for character in "project (2) 中文".chars() {
        picker.key(KeyEvent::press(KeyCode::Char(character), Modifiers::NONE));
    }
    assert_eq!(picker.workspace_query.text(), "project (2) 中文");
    assert_eq!(picker.visible_rows().len(), 1);
    assert_eq!(
        picker.visible_rows()[0].context.as_ref().unwrap().target,
        expected
    );
    assert!(!picker.activate_drawn_picker(&old));
    assert!(picker.request.is_none());
    picker.key(KeyEvent::press(KeyCode::Enter, Modifiers::NONE));
    assert_eq!(
        picker.request.as_ref().unwrap().context,
        Some(expected.into())
    );

    picker.show_workspaces();
    picker.paste_workspace_query("project (3)");
    assert_eq!(picker.visible_rows().len(), 1);
    let query = picker.workspace_query.clone();
    picker.paste_workspace_query(&"界".repeat(129));
    assert_eq!(picker.workspace_query, query, "over-limit paste is atomic");
    picker.paste_workspace_query("\n");
    assert_eq!(
        picker.workspace_query, query,
        "controls cannot enter the query"
    );
    picker.key(KeyEvent::press(KeyCode::Char('u'), Modifiers::CTRL));
    picker.paste_workspace_query("jkj");
    assert!(picker.visible_rows().is_empty(), "j/k are query characters");
    picker.key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(!picker.open);
    assert!(picker.request.is_none());
    assert_eq!(picker.local_contexts.as_ref(), source);
}

#[test]
fn workspace_search_keeps_its_field_and_selection_identity_across_query_and_checkpoint_changes() {
    let mut picker = picker(3);
    picker.show_workspaces();
    picker.key(KeyEvent::press(KeyCode::Down, Modifiers::NONE));
    let selected = picker.selection();
    picker.paste_workspace_query("same 工作区");
    for (code, cursor) in [(KeyCode::Home, 0), (KeyCode::End, "same 工作区".len())] {
        picker.key(KeyEvent::press(code, Modifiers::NONE));
        assert_eq!(picker.workspace_query.cursor(), cursor);
        assert_eq!(picker.selection(), selected, "Home/End belong to the query");
        assert!(picker.request.is_none());
    }
    let mut source = contexts(3);
    source.reverse();
    picker.observe_local(source);
    assert_eq!(picker.selection(), selected);
    for (width, height) in [(120, 40), (80, 24), (60, 12), (40, 10), (19, 4)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let before = paint(&picker, locale, width, height);
            picker.paste_workspace_query("missing");
            let after = paint(&picker, locale, width, height);
            if width >= 34 && height >= 10 {
                let heading = locale.machine_picker_text("workspace-search");
                let before_field = before
                    .rows
                    .iter()
                    .position(|row| compact_text(row).contains(&compact_text(heading)));
                assert!(before_field.is_some());
                assert_eq!(
                    before_field,
                    after
                        .rows
                        .iter()
                        .position(|row| compact_text(row).contains(&compact_text(heading)))
                );
                assert!(compact_text(&after).contains(&compact_text(
                    locale.machine_picker_text("workspaces-no-matches")
                )));
            }
            assert!(after.contains("Esc"));
            picker.key(KeyEvent::press(KeyCode::Char('u'), Modifiers::CTRL));
            picker.paste_workspace_query("same 工作区");
        }
    }
}
fn window(picker: &Picker, heights: &[u16], cells: u16, gap: u16) -> SidebarWindow {
    let rows = picker.sidebar_navigation_rows();
    assert_eq!(rows.len(), heights.len());
    picker.space_window_for_row_gaps(&rows, heights, &vec![gap; rows.len()], cells)
}

fn git_checkpoint(revision: u64) -> std::sync::Arc<crate::machine_spaces::Checkpoint> {
    use crate::machine_spaces::{Checkpoint, Git, ProjectSummary, Row};
    let contexts = contexts(3);
    std::sync::Arc::new(Checkpoint {
        session_id: contexts[0].target.session_id.clone(),
        revision: WorkbenchRevision::new(revision),
        metadata_revision: WorkbenchRevision::new(revision),
        git_requested: true,
        projects: vec![ProjectSummary {
            project_id: contexts[0].project_id.clone().unwrap(),
            label: "Repo".into(),
        }]
        .into(),
        rows: contexts
            .into_iter()
            .map(|context| Row {
                target: context.target,
                project_id: context.project_id,
                label: context.workspace,
                tokens: Default::default(),
                directory: Some(Directory {
                    directory_id: WorkbenchDirectoryId::parse(&format!("{:032x}", 10)).unwrap(),
                    worker_id: WorkerId::parse(&format!("{:032x}", 11)).unwrap(),
                    path: WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
                }),
                git: Some(Git {
                    branch: Some("main".into()),
                    divergence: None,
                }),
                git_stale: false,
            })
            .collect(),
    })
}

#[test]
fn failed_and_missing_workspace_rounds_retain_git_groups_scroll_and_current_identity() {
    use crate::machine_spaces::Observation as Spaces;
    for collapsed in [false, true] {
        let mut picker = picker(3);
        picker.observe_spaces(vec![(Choice::Local, Spaces::Ready(git_checkpoint(2)))]);
        let current = picker.workspace_rows[1]
            .context
            .as_ref()
            .unwrap()
            .target
            .clone();
        picker.observe_focus(Choice::Local, Some(current.clone()), None);
        let project = picker.sidebar_navigation_rows()[0].target();
        if collapsed {
            assert!(picker.toggle_sidebar_group(&project));
        } else {
            picker.queue_space_scroll(window(&picker, &[1; 4], 2, 0), false);
            assert!(picker.settle_sidebar_scroll());
            // Finish the coalesced notch at the end of the list. The stable-window
            // contract retains the viewport, not work owned by an older painted source.
            assert!(!picker.settle_sidebar_scroll());
        }
        let before = picker
            .sidebar_navigation_rows()
            .iter()
            .map(|row| row.target())
            .collect::<Vec<_>>();
        let top = picker.spaces.clone();
        for observation in [Some(Spaces::Loading), Some(Spaces::Unavailable), None] {
            picker.observe_spaces(
                observation
                    .map(|observation| (Choice::Local, observation))
                    .into_iter()
                    .collect(),
            );
            assert_eq!(
                picker
                    .sidebar_navigation_rows()
                    .iter()
                    .map(|row| row.target())
                    .collect::<Vec<_>>(),
                before
            );
            assert_eq!(picker.spaces, top);
            assert_eq!(picker.current_context, Some(current.clone()));
            assert!(picker.space_checkpoint_stale(&picker.workspace_rows[0]));
            assert_eq!(
                picker
                    .space_checkpoint_row(&picker.workspace_rows[0])
                    .unwrap()
                    .git
                    .as_ref()
                    .unwrap()
                    .branch
                    .as_deref(),
                Some("main")
            );
        }
        picker.observe_spaces(vec![(Choice::Local, Spaces::Ready(git_checkpoint(1)))]);
        assert!(picker.workspace_checkpoint_covers(&current.session_id, WorkbenchRevision::new(2)));
        picker.observe_spaces(vec![(Choice::Local, Spaces::Ready(git_checkpoint(3)))]);
        assert!(!picker.space_checkpoint_stale(&picker.workspace_rows[0]));
        picker.forget_space_observations(&Choice::Local);
        assert!(
            picker
                .space_checkpoint_row(&picker.workspace_rows[0])
                .is_none()
        );
    }
}

#[test]
fn only_authoritative_same_source_git_results_can_clear_or_replace_a_retained_branch() {
    let previous = git_checkpoint(1);
    let mut next = (*git_checkpoint(2)).clone();
    let mut rows = next.rows.to_vec();
    rows[0].git = None; // Failed read, same directory.
    rows[1].git = Some(crate::machine_spaces::Git {
        branch: None,
        divergence: None,
    }); // Confirmed non-Git.
    rows[2].git = None;
    rows[2].directory.as_mut().unwrap().path = WorkbenchDirectoryPath::parse("C:/other").unwrap();
    next.rows = rows.into();
    let retained = next.retaining_git(&previous);
    assert_eq!(retained.rows[0].git, previous.rows[0].git);
    assert!(retained.rows[0].git_stale);
    assert_eq!(retained.rows[1].git.as_ref().unwrap().branch, None);
    assert!(!retained.rows[1].git_stale);
    assert!(
        retained.rows[2].git.is_none(),
        "changed directories cannot inherit old Git facts"
    );
    next.session_id = SessionId::parse(&format!("{:032x}", 9)).unwrap();
    assert!(next.retaining_git(&previous).rows[0].git.is_none());
}
#[test]
fn unified_tree_window_scrolls_and_focus_clips_oversized_rows() {
    let mut picker = picker(7);
    let heights = [1, 1, 3, 2, 1, 4, 1, 2];
    let spaces = window(&picker, &heights, 6, 1);
    assert_eq!((spaces.top, spaces.end(), spaces.maximum), (0, 2, 6));
    picker.queue_space_scroll(spaces, false);
    assert!(picker.settle_sidebar_scroll());
    assert_eq!(window(&picker, &heights, 6, 1).top, 2);
    for _ in 0..4 {
        let w = window(&picker, &heights, 6, 1);
        picker.queue_space_scroll(w, false);
        picker.settle_sidebar_scroll();
    }
    assert_eq!(window(&picker, &heights, 6, 1).top, 6);
    let context = picker.workspace_rows[4]
        .context
        .as_ref()
        .unwrap()
        .target
        .clone();
    picker.observe_focus(Choice::Local, Some(context), None);
    let clipped = window(&picker, &[1, 1, 1, 1, 1, 16, 1, 1], 2, 0);
    assert_eq!((clipped.top, clipped.capacity), (5, 1));
}
#[test]
fn workspace_geometry_changes_discard_queued_wheel_and_rows_require_exact_current_identity() {
    let mut picker = picker(8);
    let old = picker.workspace_rows[1].selection();
    let w = window(&picker, &[1; 9], 3, 0);
    picker.queue_space_scroll(w, false);
    let mut changed = contexts(8);
    changed.reverse();
    picker.observe_local(changed);
    assert!(!picker.settle_sidebar_scroll());
    assert!(picker.activate_sidebar_workspace(&old));
    assert_eq!(picker.request, Some(old.clone()));
    picker.open = false;
    picker.request = None;
    picker.observe_local(contexts(1));
    assert!(!picker.activate_sidebar_workspace(&old));
    let valid = picker.workspace_rows[0].selection();
    picker.observe_focus(
        Choice::Local,
        picker.workspace_rows[0]
            .context
            .as_ref()
            .map(|c| c.target.clone()),
        None,
    );
    assert!(picker.activate_sidebar_workspace(&valid));
    assert!(!picker.open);
    assert_eq!(picker.request, Some(valid.clone()));
    picker.observe_agent_sidebar(false);
    assert!(!picker.activate_sidebar_workspace(&valid));
}
#[test]
fn workspace_keyboard_picker_and_sidebar_request_the_same_duplicate_label_target() {
    let mut picker = picker(3);
    let exact = picker.workspace_rows[1].selection();
    picker.show_workspaces();
    picker.key(KeyEvent::press(KeyCode::Down, Modifiers::NONE));
    picker.key(KeyEvent::press(KeyCode::Enter, Modifiers::NONE));
    assert_eq!(picker.request, Some(exact.clone()));
    picker.open = false;
    picker.request = None;
    assert!(picker.activate_sidebar_workspace(&exact));
    assert!(!picker.open);
    assert_eq!(picker.request, Some(exact));
}

#[test]
fn sidebar_selection_scopes_supersede_old_targets_without_opening_a_picker() {
    let mut picker = picker(3);
    let first = picker.workspace_rows[0].selection();
    let last = picker.workspace_rows[2].selection();
    assert!(picker.activate_sidebar_workspace(&first));
    let old = picker.selection_scope().unwrap();
    assert!(picker.selection_current(old));
    assert!(picker.activate_sidebar_workspace(&last));
    assert!(!picker.selection_current(old));
    let direct = picker.selection_scope().unwrap();
    assert_eq!(picker.request, Some(last));
    assert!(!picker.open);
    picker.show_workspaces();
    assert!(!picker.selection_current(direct));
    assert!(!picker.direct_selection_pending());
    assert!(picker.open);
    assert!(picker.request.is_none());
    picker.key(KeyEvent::press(KeyCode::Enter, Modifiers::NONE));
    let explicit = picker.selection_scope().unwrap();
    picker.key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(!picker.selection_current(explicit));
    assert!(!picker.open);
    assert!(picker.request.is_none());
}
#[test]
fn workspace_state_uses_complete_post_attention_rows_before_agent_view_filters() {
    let mut picker = picker(2);
    let context = picker.workspace_rows[0]
        .context
        .as_ref()
        .unwrap()
        .target
        .clone();
    let mut working = crate::machine_agents::test_support::row(23);
    working.target.context = context.clone();
    let mut blocked = crate::machine_agents::test_support::row(24);
    blocked.target.context = context.clone();
    blocked.state = PanelAgentState::Blocked;
    let mut cp = crate::machine_agents::test_support::checkpoint(vec![working, blocked])
        .as_ref()
        .clone();
    cp.view.spec=Some(parse_agent_view_spec(br#"{"source":"test.filter","filter":{"op":"eq","field":"status","value":"idle"},"sort":[]}"#).unwrap());
    picker.observe_agents(vec![(
        Choice::Local,
        crate::machine_agents::Observation::Ready(std::sync::Arc::new(cp)),
    )]);
    assert!(picker.agent_rows.iter().all(|r| r.agent.is_none()));
    assert_eq!(
        picker.workspace_state(&picker.workspace_rows[0]),
        Some(MetadataState::Blocked)
    );
    assert_eq!(
        picker.workspace_state(&picker.workspace_rows[1]),
        None,
        "a newly observed context absent from the complete checkpoint is not idle"
    );
    let context = picker.workspace_rows[1]
        .context
        .as_ref()
        .unwrap()
        .target
        .clone();
    let mut empty = crate::machine_agents::test_support::checkpoint(Vec::new())
        .as_ref()
        .clone();
    empty.contexts = vec![context].into();
    picker.observe_agents(vec![(
        Choice::Local,
        crate::machine_agents::Observation::Ready(std::sync::Arc::new(empty)),
    )]);
    assert_eq!(picker.workspace_state(&picker.workspace_rows[1]), None);
    picker.observe_agents(vec![(
        Choice::Local,
        crate::machine_agents::Observation::Unavailable,
    )]);
    assert_eq!(picker.workspace_state(&picker.workspace_rows[1]), None);
}
#[test]
fn workspace_state_priority_uses_explicit_facts_without_acknowledgement() {
    let mut picker = picker(1);
    let context = picker.workspace_rows[0]
        .context
        .as_ref()
        .unwrap()
        .target
        .clone();
    for (states, expected) in [
        (
            vec![
                (PanelAgentState::Unknown, true),
                (PanelAgentState::Idle, true),
            ],
            MetadataState::Idle,
        ),
        (
            vec![
                (PanelAgentState::Idle, true),
                (PanelAgentState::Working, true),
            ],
            MetadataState::Working,
        ),
        (
            vec![
                (PanelAgentState::Working, true),
                (PanelAgentState::Done, false),
            ],
            MetadataState::Done,
        ),
        (
            vec![
                (PanelAgentState::Idle, false),
                (PanelAgentState::Blocked, true),
            ],
            MetadataState::Blocked,
        ),
    ] {
        let rows = states
            .into_iter()
            .enumerate()
            .map(|(i, (state, seen))| {
                let mut row = crate::machine_agents::test_support::row(i as u64 + 23);
                row.target.context = context.clone();
                row.state = state;
                row.seen = seen;
                row
            })
            .collect();
        picker.agent_observations = vec![(
            Choice::Local,
            crate::machine_agents::Observation::Ready(
                crate::machine_agents::test_support::checkpoint(rows),
            ),
        )];
        assert_eq!(
            picker.workspace_state(&picker.workspace_rows[0]),
            Some(expected)
        );
    }
}

/// R-002: same-named long Workspaces keep their identity suffix in result rows and details by
/// truncating only the name, and details show the distinguishing tail of each directory.
#[test]
fn long_duplicate_workspace_results_keep_identity_suffix_and_directory_tail() {
    use crate::machine_spaces::{Checkpoint, Observation as Spaces, Row};
    let long = "shared-long-workspace-name-used-for-reviewing-search-disambiguation-with-a-full-width-terminal-result";
    let mut contexts = contexts(2);
    for context in &mut contexts {
        context.project_id = None;
        context.project = None;
        context.workspace = Some(crate::machine_discovery::display_label(long));
    }
    let checkpoint = std::sync::Arc::new(Checkpoint {
        session_id: contexts[0].target.session_id.clone(),
        revision: WorkbenchRevision::new(2),
        metadata_revision: WorkbenchRevision::new(2),
        git_requested: false,
        projects: Vec::new().into(),
        rows: contexts
            .iter()
            .zip(0_u8..)
            .map(|(context, n)| Row {
                target: context.target.clone(),
                project_id: None,
                label: context.workspace.clone(),
                tokens: Default::default(),
                directory: Some(Directory {
                    directory_id: WorkbenchDirectoryId::parse(&format!("{:032x}", 10 + n)).unwrap(),
                    worker_id: WorkerId::parse(&format!("{:032x}", 11)).unwrap(),
                    path: WorkbenchDirectoryPath::parse(&format!(
                        "C:/checkouts/{}review-{n}",
                        "nested/".repeat(12)
                    ))
                    .unwrap(),
                }),
                git: None,
                git_stale: false,
            })
            .collect(),
    });
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.observe_local(contexts);
    picker.observe_spaces(vec![(Choice::Local, Spaces::Ready(checkpoint))]);
    picker.show();
    picker.key(KeyEvent::press(KeyCode::Tab, Modifiers::NONE));
    assert_eq!(picker.mode, Mode::Workspaces);
    for (selected, other) in [(0, 1), (1, 0)] {
        let painted = paint(&picker, Locale::EnUs, 80, 24);
        let suffix = |n: u8| format!("[{:08x}]", 100 + n);
        // The selected result repeats its suffix in details; the other result shows it once.
        let rows = painted.rows.join("\n");
        assert_eq!(painted.matches(&suffix(selected)).count(), 2, "{rows}");
        assert_eq!(painted.matches(&suffix(other)).count(), 1, "{rows}");
        assert!(painted.contains(&format!("review-{selected}")));
        assert!(!painted.contains(&format!("review-{other}")));
        assert!(painted.contains("Ungrouped"));
        picker.key(KeyEvent::press(KeyCode::Down, Modifiers::NONE));
    }
}
