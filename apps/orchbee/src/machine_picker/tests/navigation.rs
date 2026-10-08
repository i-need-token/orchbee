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
                project_id: WorkbenchProjectId::parse(&format!("{project:032x}")).unwrap(),
                workspace_id: WorkbenchWorkspaceId::parse(&format!("{number:032x}")).unwrap(),
                terminal_context_id: WorkbenchTerminalContextId::parse(&format!(
                    "{:032x}",
                    100 + u16::from(number)
                ))
                .unwrap(),
            },
        },
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

fn key(picker: &mut Picker, code: KeyCode) -> SidebarIntent {
    picker.sidebar_key(KeyEvent::press(code, Modifiers::NONE))
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
            label: Some("same 工作区".into()),
            tokens: Default::default(),
            git: None,
        }]
        .into(),
    }))
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
fn sidebar_navigation_reveal_uses_visible_tree_heights_and_resumes_after_wheel() {
    let mut picker = picker();
    picker.enter_sidebar_navigation();
    key(&mut picker, KeyCode::End);
    let heights = [1, 1, 2, 3, 1, 2];
    let last = picker.space_window_for_heights(&heights, 3, 0);
    assert_eq!((last.top, last.end()), (4, 6));
    picker.queue_space_scroll(last, true);
    assert!(picker.settle_sidebar_scroll());
    assert!(picker.space_window_for_heights(&heights, 3, 0).end() < 6);
    key(&mut picker, KeyCode::End);
    assert_eq!(picker.space_window_for_heights(&heights, 3, 0).end(), 6);
    assert!(picker.request.is_none());
}

#[test]
fn entering_on_a_collapsed_active_workspace_focuses_its_parent_without_disclosure() {
    let mut picker = picker();
    let active = workspace(1, 22).target;
    picker.observe_focus(Choice::Local, Some(active.clone()), None);
    let parent = SidebarTarget::Context {
        machine: Choice::Local,
        target: active.clone(),
    }
    .parent()
    .unwrap();
    assert!(picker.toggle_sidebar_project(&parent));
    assert!(picker.navigation.focus.is_none());
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
    let parent = SidebarTarget::Context {
        machine: Choice::Local,
        target: active.clone(),
    }
    .parent()
    .unwrap();
    picker.toggle_sidebar_project(&parent);
    key(&mut picker, KeyCode::End);
    let focused = picker.sidebar_focused_target().cloned();
    let window = picker.space_window_for_heights(&[1; 4], 2, 0);
    picker.queue_space_scroll(window, true);
    assert!(picker.set_sidebar_width(SidebarWidth::DEFAULT, width(60)));
    assert!(picker.spaces.scroll.is_none());
    assert!(picker.sidebar_navigation_active());
    assert_eq!(picker.sidebar_focused_target(), focused.as_ref());
    assert!(!picker.sidebar_project_expanded(&parent));
    assert_eq!(picker.current_context, Some(active));
    assert!(picker.request.is_none());
    assert_eq!(picker.space_window_for_heights(&[1; 4], 2, 0).end(), 4);
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
    Hide,
    Enter,
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
                match random % 5 {
                    0 => Event::Retain(((random >> 8) % 8) as u8),
                    1 => Event::Reverse,
                    2 => Event::Hide,
                    3 => Event::Enter,
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
