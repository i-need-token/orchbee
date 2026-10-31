//! Sidebar project-choice identity at the rendered input boundary.
use super::*;
use crate::action_menu::MenuTarget;
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
                &[],
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

fn workspace() -> crate::workbench_projection::ClientContextLocation {
    crate::workbench_projection::ClientContextLocation::Workspace {
        workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 1)).unwrap(),
        terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{:032x}", 2)).unwrap(),
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
