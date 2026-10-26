//! Async sidebar menu geometry and project-choice identity at the rendered input boundary.
use super::*;
use crate::action_menu::{MenuTarget, MenuUnavailable};
use orchbee_workbench_protocol::{
    WorkbenchProjectId, WorkbenchTerminalContextId, WorkbenchWorkspaceId,
};

fn paint(
    menu: &mut ActionMenu,
    width: u16,
    height: u16,
    locale: Locale,
    theme: &Theme,
) -> (Buffer, FrameHitMap) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    let mut presentation = ModalPresentation::NotPresented;
    terminal
        .draw(|frame| {
            presentation = render(
                frame,
                frame.area(),
                menu.view().unwrap(),
                ClientView::new(width).bindings(),
                locale,
                theme,
                &mut hits,
            );
        })
        .unwrap();
    let ModalPresentation::Presented(draw) = presentation else {
        panic!("menu fits")
    };
    assert!(menu.presented(draw));
    (terminal.backend().buffer().clone(), hits.finish())
}

fn workspace() -> crate::workbench_client::NativeContextLocation {
    crate::workbench_client::NativeContextLocation::Workspace {
        workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 1)).unwrap(),
        terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{:032x}", 2)).unwrap(),
    }
}

#[test]
fn sidebar_probe_outcomes_never_move_rename_or_delete_even_at_clamped_edges() {
    for (width, height) in [(120, 40), (80, 24), (60, 12), (40, 10)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [Theme::terminal_native(), Theme::one_light()] {
                for project in [false, true] {
                    let (source, actions) = if project {
                        (
                            MenuTarget::Project(
                                WorkbenchProjectId::parse(&format!("{:032x}", 25)).unwrap(),
                            ),
                            crate::action_menu::PROJECT_ACTIONS.to_vec(),
                        )
                    } else {
                        (
                            MenuTarget::Workspace(workspace()),
                            crate::action_menu::WORKSPACE_ACTIONS.to_vec(),
                        )
                    };
                    let mut menu = ActionMenu::default();
                    assert!(menu.open_for(
                        source,
                        String::new(),
                        actions.clone(),
                        Some((width - 1, height - 1))
                    ));
                    let mut baseline = None;
                    for reason in [
                        Some(MenuUnavailable::WorktreeChecking),
                        None,
                        Some(MenuUnavailable::WorktreeUnavailable),
                    ] {
                        menu.refresh_availability(|action| {
                            (action == MenuAction::WorkspaceWorktrees)
                                .then_some(reason)
                                .flatten()
                        });
                        let (_, map) = paint(&mut menu, width, height, locale, &theme);
                        let positions = (0..height)
                            .flat_map(|y| (0..width).map(move |x| (x, y)))
                            .filter_map(|(x, y)| match map.hit_target_at(x, y) {
                                Some(HitTarget::ActionMenuRow { row, .. })
                                    if matches!(
                                        actions[*row],
                                        MenuAction::RenameProject
                                            | MenuAction::RenameWorkspace
                                            | MenuAction::DeleteProject
                                            | MenuAction::DeleteWorkspace
                                    ) =>
                                {
                                    Some((*row, x, y))
                                }
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        assert!(!positions.is_empty());
                        if let Some(before) = &baseline {
                            assert_eq!(&positions, before);
                        }
                        baseline = Some(positions);
                    }
                }
            }
        }
    }
}

#[test]
fn duplicate_and_truncated_project_names_keep_complete_sidebar_identity_suffixes() {
    for labels in [
        ["project", "project"],
        [
            "项目 repeated very long prefix that fills the menu first",
            "项目 repeated very long prefix that fills the menu second",
        ],
    ] {
        for (width, height) in [(120, 40), (80, 24), (40, 12), (20, 8)] {
            for locale in [Locale::EnUs, Locale::ZhCn] {
                let ids =
                    [25, 26].map(|n| WorkbenchProjectId::parse(&format!("{n:032x}")).unwrap());
                let mut menu = ActionMenu::default();
                let actions = ids
                    .iter()
                    .cloned()
                    .map(MenuAction::JoinWorkspaceProject)
                    .collect::<Vec<_>>();
                assert!(menu.open_for(
                    MenuTarget::WorkspaceProjects(workspace()),
                    String::new(),
                    actions.clone(),
                    None
                ));
                menu.set_labels(|action| match action {
                    MenuAction::JoinWorkspaceProject(id) => {
                        Some(labels[usize::from(id == ids[1])].into())
                    }
                    _ => None,
                });
                let (buffer, _) =
                    paint(&mut menu, width, height, locale, &Theme::terminal_native());
                let text = buffer
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>();
                if labels[0] == labels[1] || width <= 40 {
                    assert!(text.contains("[00000019]"), "{width}: {text}");
                    assert!(text.contains("[0000001a]"), "{width}: {text}");
                }
                assert_eq!(
                    menu.view().unwrap().actions,
                    actions,
                    "display suffixes never change target IDs"
                );
            }
        }
    }
}
