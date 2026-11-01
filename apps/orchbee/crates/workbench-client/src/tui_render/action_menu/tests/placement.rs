//! Anchored placement, cascade geometry and hidden-candidate evidence for compact context menus.
use super::*;
use crate::action_menu::{
    MenuAnchor, MenuHeading, MenuSelection, MenuTarget, MenuUnavailable, PROJECT_ACTIONS,
};
use orchbee_workbench_protocol::{WorkbenchDirectoryPath, WorkbenchProjectId};

/// Lines of the Project menu: two rows, a separator and the dissolve row.
const PROJECT_LINES: u16 = 4;

fn project() -> WorkbenchProjectId {
    WorkbenchProjectId::parse(&format!("{:032x}", 2)).unwrap()
}

/// A painted sidebar row that owns the pressed or focused cell.
fn sidebar_row(y: u16) -> (Rect, HitTarget) {
    let target = crate::machine_picker::SidebarTarget::Project {
        machine: crate::machine_picker::Choice::Local,
        session_id: orchbee_workbench_protocol::SessionId::parse(&format!("{:032x}", 1)).unwrap(),
        project_id: project(),
    };
    (Rect::new(0, y, 29, 3), HitTarget::SidebarMenu(target))
}

fn inside(rect: Rect, x: u16, y: u16) -> bool {
    (rect.x..rect.right()).contains(&x) && (rect.y..rect.bottom()).contains(&y)
}

struct Painted {
    draw: Option<MenuDraw>,
    buffer: Buffer,
    map: FrameHitMap,
}

impl Painted {
    fn draw(&self) -> &MenuDraw {
        self.draw.as_ref().expect("the menu is presented")
    }

    fn row(&self, position: usize) -> Rect {
        action_row_rect_for(&self.map, self.buffer.area, self.draw().token, position)
    }

    /// The active frame from its first painted row; `lines` counts its rows, separators and
    /// headings.
    fn frame(&self, lines: u16) -> Rect {
        let row = self.row(self.draw().first);
        Rect::new(row.x - 1, row.y - 1, row.width + 2, lines + 2)
    }

    /// Whether every listed cell of one line uses the selection surface.
    fn selected(&self, y: u16, mut columns: impl Iterator<Item = u16>, theme: &Theme) -> bool {
        columns.all(|x| self.buffer[(x, y)].bg == theme.color(Role::SurfaceSelection))
    }
}

fn paint(
    size: (u16, u16),
    trigger: Option<(Rect, HitTarget)>,
    menu: MenuView<'_>,
    parents: &[MenuView<'_>],
    theme: &Theme,
) -> Painted {
    let (width, height) = size;
    let client = ClientView::new(width);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    if let Some((rect, target)) = trigger {
        hits.record(HitLayer::Foreground, rect, target);
    }
    let mut presentation = ModalPresentation::NotPresented;
    terminal
        .draw(|frame| {
            presentation = render(
                frame,
                frame.area(),
                menu,
                parents,
                client.bindings(),
                Locale::EnUs,
                theme,
                &mut hits,
            );
        })
        .unwrap();
    Painted {
        draw: presentation.into_presented(),
        buffer: terminal.backend().buffer().clone(),
        map: hits.finish(),
    }
}

fn project_menu(anchor: MenuAnchor) -> ActionMenu {
    let mut menu = ActionMenu::default();
    assert!(menu.open_for(
        MenuTarget::Project(project()),
        "Project".into(),
        PROJECT_ACTIONS.to_vec(),
        Some(anchor),
    ));
    menu
}

/// The New Workspace submenu and its line count: its new-Worktree row, a separator, the Folders
/// heading and one row per folder.
fn creation_menu(folders: usize, anchor: MenuAnchor) -> (ActionMenu, u16) {
    let mut actions = vec![MenuAction::NewWorktree];
    actions.extend((0..folders).map(|index| {
        MenuAction::WorkspaceInFolder(
            WorkbenchDirectoryPath::parse(&format!("C:/folder-{index}")).unwrap(),
        )
    }));
    let mut menu = ActionMenu::default();
    assert!(menu.open_for(
        MenuTarget::WorkspaceCreation(project()),
        String::new(),
        actions,
        Some(anchor),
    ));
    menu.set_labels(|action| match action {
        MenuAction::WorkspaceInFolder(path) => {
            Some(path.as_str().trim_start_matches("C:/").to_owned())
        }
        _ => None,
    });
    (menu, u16::try_from(folders).unwrap() + 3)
}

#[test]
fn pointer_menu_opens_at_the_pressed_column_below_its_trigger_else_above_and_shifts_left() {
    let theme = Theme::terminal_native();
    // Below the complete sidebar row, starting at the pressed column.
    let (row, target) = sidebar_row(4);
    let menu = project_menu(MenuAnchor::Pointer { column: 6, row: 5 });
    let painted = paint(
        (80, 24),
        Some((row, target)),
        menu.view().unwrap(),
        &[],
        &theme,
    );
    let frame = painted.frame(PROJECT_LINES);
    assert_eq!((frame.x, frame.y), (6, row.bottom()));

    // Without room below, the frame ends on the row above the trigger.
    let (row, target) = sidebar_row(18);
    let menu = project_menu(MenuAnchor::Pointer { column: 6, row: 19 });
    let painted = paint(
        (80, 24),
        Some((row, target)),
        menu.view().unwrap(),
        &[],
        &theme,
    );
    let frame = painted.frame(PROJECT_LINES);
    assert_eq!((frame.x, frame.bottom()), (6, row.y));

    // Near the right edge, it shifts left to stay inside the viewport.
    let menu = project_menu(MenuAnchor::Pointer { column: 70, row: 2 });
    let painted = paint((80, 24), None, menu.view().unwrap(), &[], &theme);
    let frame = painted.frame(PROJECT_LINES);
    assert_eq!((frame.right(), frame.y), (80, 3));
    assert!(frame.x < 70);

    // With neither side fitting, the larger side shortens the frame and its rows scroll.
    let (row, target) = sidebar_row(2);
    let menu = project_menu(MenuAnchor::Pointer { column: 6, row: 3 });
    let painted = paint(
        (80, 8),
        Some((row, target)),
        menu.view().unwrap(),
        &[],
        &theme,
    );
    assert_eq!(painted.draw().visible_rows, 1);
    let first = painted.row(painted.draw().first);
    assert_eq!(first.y - 1, row.bottom(), "the trigger row stays uncovered");

    // A window shrunk below the pressed cell keeps the frame inside the new viewport.
    let menu = project_menu(MenuAnchor::Pointer {
        column: 100,
        row: 30,
    });
    let painted = paint((80, 24), None, menu.view().unwrap(), &[], &theme);
    let frame = painted.frame(PROJECT_LINES);
    assert!(frame.right() <= 80 && frame.bottom() <= 24);

    // Fewer than three rows on either side refuses the menu and its input.
    let (row, target) = sidebar_row(1);
    let menu = project_menu(MenuAnchor::Pointer { column: 6, row: 2 });
    let painted = paint(
        (80, 5),
        Some((row, target)),
        menu.view().unwrap(),
        &[],
        &theme,
    );
    assert!(painted.draw.is_none());
    assert!((0..5).all(|y| (0..80).all(|x| !matches!(
        painted.map.hit_target_at(x, y),
        Some(HitTarget::ActionMenuRow { .. })
    ))));
}

#[test]
fn keyboard_row_menu_opens_beside_its_row_and_moves_up_only_to_fit() {
    let theme = Theme::terminal_native();
    let (row, target) = sidebar_row(4);
    let menu = project_menu(MenuAnchor::Row { column: 0, row: 4 });
    let painted = paint(
        (80, 24),
        Some((row, target)),
        menu.view().unwrap(),
        &[],
        &theme,
    );
    let frame = painted.frame(PROJECT_LINES);
    assert_eq!((frame.x, frame.y), (row.right(), row.y));

    let (row, target) = sidebar_row(20);
    let menu = project_menu(MenuAnchor::Row { column: 0, row: 20 });
    let painted = paint(
        (80, 24),
        Some((row, target)),
        menu.view().unwrap(),
        &[],
        &theme,
    );
    let frame = painted.frame(PROJECT_LINES);
    assert_eq!((frame.x, frame.bottom()), (row.right(), 24));
    assert!(frame.y < row.y);
}

#[derive(Clone, Copy, Debug)]
enum Side {
    Right,
    Left,
    Over,
}

#[test]
fn cascade_opens_level_with_its_row_right_else_left_else_over_the_parent_end() {
    // Viewport, pressed cell, folder count, expected side, and whether the child moves up to fit.
    let cases = [
        ((80, 24), (6, 2), 2, Side::Right, false),
        ((80, 24), (60, 2), 2, Side::Left, false),
        ((60, 24), (20, 2), 2, Side::Over, false),
        ((80, 10), (6, 9), 3, Side::Right, true),
    ];
    for theme in [Theme::terminal_native(), Theme::one_dark()] {
        for (size, (column, row), folders, side, moved) in cases {
            let anchor = MenuAnchor::Pointer { column, row };
            let parent = project_menu(anchor);
            let alone = paint(size, None, parent.view().unwrap(), &[], &theme);
            let parent_frame = alone.frame(PROJECT_LINES);
            let open = alone.row(0);
            let retained = parent.retain().unwrap();

            let (child, lines) = creation_menu(folders, anchor);
            let mut view = child.view().unwrap();
            view.has_parent = true;
            let painted = paint(size, None, view, &[retained.view(false)], &theme);
            let token = painted.draw().token;
            let frame = painted.frame(lines);
            let first = painted.row(0);
            let context = format!("{side:?} {size:?}: {frame:?} beside {parent_frame:?}");
            match side {
                Side::Right => assert_eq!(frame.x, parent_frame.right(), "{context}"),
                Side::Left => assert_eq!(frame.right(), parent_frame.x, "{context}"),
                Side::Over => {
                    assert_eq!(frame.right(), size.0, "{context}");
                    assert!(
                        parent_frame.x < frame.x && frame.x < parent_frame.right(),
                        "{context}"
                    );
                }
            }
            if moved {
                assert_eq!(frame.bottom(), size.1, "{context}");
                assert!(first.y < open.y, "{context}");
            } else {
                assert_eq!(first.y, open.y, "level with the open row: {context}");
            }

            // The parent keeps its frame and title. Its open row keeps the fill without the
            // marker, and every visible parent cell returns to it instead of acting on a row.
            let uncovered = |y: u16| (open.x..open.right()).filter(move |x| !inside(frame, *x, y));
            assert_eq!(painted.buffer[(open.x, open.y)].symbol(), " ", "{context}");
            assert!(
                painted.selected(open.y, uncovered(open.y), &theme),
                "{context}"
            );
            assert!(
                !painted.selected(open.y + 1, uncovered(open.y + 1), &theme),
                "{context}"
            );
            let title = (parent_frame.x..parent_frame.right())
                .filter(|x| !inside(frame, *x, parent_frame.y))
                .map(|x| painted.buffer[(x, parent_frame.y)].symbol())
                .collect::<String>();
            assert!(title.contains("Project"), "{title}: {context}");
            // Each other parent row returns with itself as the candidate, so a resting pointer
            // can choose it; the open row and the frame return to the opening row.
            let rows = [1, 2].map(|position| (alone.row(position).y, position));
            for y in parent_frame.y..parent_frame.bottom() {
                for x in (parent_frame.x..parent_frame.right()).filter(|x| !inside(frame, *x, y)) {
                    assert!(
                        matches!(
                            painted.map.hit_target_at(x, y),
                            Some(HitTarget::ActionMenuControl { token: owner, back: true })
                                if *owner == token
                        ),
                        "({x}, {y}) {context}"
                    );
                    let row = rows
                        .iter()
                        .find(|(row_y, _)| *row_y == y)
                        .filter(|_| x > parent_frame.x && x < parent_frame.right() - 1);
                    let expected = row.map_or(MenuControl::Back, |(_, position)| {
                        MenuControl::ParentRow(*position)
                    });
                    assert!(
                        matches!(
                            painted.map.interaction_at(x, y).map(|hit| hit.target),
                            Some(InteractionTarget::ActionMenuControl { control, .. })
                                if control == expected
                        ),
                        "({x}, {y}) {expected:?} {context}"
                    );
                }
            }

            // The untitled child holds the keyboard candidate. Its heading starts in the label
            // column, is secondary text and is never a row; a folder row shows only its name.
            let border = (frame.x + 1..frame.right() - 1)
                .map(|x| painted.buffer[(x, frame.y)].symbol())
                .collect::<String>();
            assert!(!border.chars().any(char::is_alphanumeric), "{border}");
            assert_eq!(
                visible_row_text(&painted.buffer, painted.row(1)).trim(),
                "folder-0"
            );
            assert_text_at(
                &painted.buffer,
                first.x,
                first.y,
                theme.glyphs.navigation_marker,
            );
            let heading = Locale::EnUs.menu_heading(MenuHeading::Folders);
            let heading_y = (frame.y + 1..frame.bottom() - 1)
                .find(|y| {
                    visible_row_text(&painted.buffer, Rect::new(frame.x, *y, frame.width, 1))
                        .contains(heading)
                })
                .expect("the Folders heading is painted");
            let label_x =
                first.x + u16::try_from(display_width(theme.glyphs.navigation_marker)).unwrap();
            assert_text_at(&painted.buffer, label_x, heading_y, heading);
            let secondary = theme.style(Role::TextSecondary);
            let cell = &painted.buffer[(label_x, heading_y)];
            assert!(secondary.fg.is_none_or(|fg| cell.fg == fg), "{context}");
            assert!(cell.modifier.contains(secondary.add_modifier), "{context}");
            assert!((frame.x..frame.right()).all(|x| !matches!(
                painted.map.hit_target_at(x, heading_y),
                Some(HitTarget::ActionMenuRow { .. })
            )));
        }
    }
}

#[test]
fn clicked_submenu_paints_no_candidate_or_reason_until_a_key_reveals_its_first_row() {
    let theme = Theme::terminal_native();
    let anchor = MenuAnchor::Pointer { column: 6, row: 2 };
    let retained = project_menu(anchor).retain().unwrap();
    let (mut child, _) = creation_menu(1, anchor);
    child.refresh_availability(|action| {
        matches!(action, MenuAction::NewWorktree).then_some(MenuUnavailable::WorktreeChecking)
    });
    child.hide_candidate();
    let reasons = [
        Locale::EnUs.menu_unavailable(MenuUnavailable::WorktreeChecking, false),
        Locale::EnUs.menu_unavailable(MenuUnavailable::WorktreeChecking, true),
    ];
    for revealed in [false, true] {
        if revealed {
            child.select(MenuSelection::Next);
        }
        let mut view = child.view().unwrap();
        view.has_parent = true;
        let painted = paint((80, 24), None, view, &[retained.view(false)], &theme);
        let draw = painted.draw().clone();
        assert_eq!(draw.candidate_visible, revealed);
        let first = painted.row(0);
        assert_eq!(
            painted.buffer[(first.x, first.y)].symbol() == theme.glyphs.navigation_marker,
            revealed
        );
        assert_eq!(
            painted.selected(first.y, first.x..first.right(), &theme),
            revealed
        );
        let text = visible_buffer_text(&painted.buffer);
        assert_eq!(
            reasons.iter().any(|reason| text.contains(reason)),
            revealed,
            "only a visible candidate explains why it is unavailable: {text}"
        );
        assert!(child.presented(draw));
    }
}
