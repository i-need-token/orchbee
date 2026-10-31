use std::convert::Infallible;

use ratatui::{Terminal, backend::TestBackend};

use super::{ContextDisplay, MAX_VISIBLE_CONTEXTS, context_label, render_navigator};

#[derive(Clone, Copy)]
struct TestContext<'a> {
    project_label: Option<&'a str>,
    workspace_label: Option<&'a str>,
    active: bool,
}

fn rendered(
    width: u16,
    height: u16,
    contexts: &[TestContext<'_>],
    active_position: Option<usize>,
) -> String {
    rendered_with_focus(width, height, contexts, active_position, None).0
}

fn rendered_with_focus(
    width: u16,
    height: u16,
    contexts: &[TestContext<'_>],
    active_position: Option<usize>,
    focused_position: Option<usize>,
) -> (String, ratatui::buffer::Buffer) {
    let mut terminal = infallible(Terminal::new(TestBackend::new(width, height)));
    infallible(terminal.draw(|frame| {
        let context_at = |index: usize| {
            contexts.get(index).copied().map(|context| ContextDisplay {
                position: index.saturating_add(1),
                active: context.active,
                project_label: context.project_label,
                workspace_label: context.workspace_label,
                no_workspace_label: "No workspace",
            })
        };
        render_navigator(
            frame,
            frame.area(),
            contexts.len(),
            active_position,
            focused_position,
            context_at,
            &crate::tui_theme::Theme::rgb_test_dark(),
            &mut (),
            &mut |_, _, _| crate::tui_hit_map::ProjectedPointer::Rest,
            &mut |_, _, _| {},
        );
    }));

    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    for y in 0..height {
        for x in 0..width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    (text, buffer.clone())
}

fn infallible<T>(result: Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(never) => match never {},
    }
}

#[test]
fn navigator_keeps_same_labels_distinguishable_by_position() {
    let contexts = [
        TestContext {
            project_label: Some("Project"),
            workspace_label: Some("Workspace"),
            active: false,
        },
        TestContext {
            project_label: Some("Project"),
            workspace_label: Some("Workspace"),
            active: true,
        },
    ];

    let text = rendered(64, 3, &contexts, Some(2));

    assert!(text.contains("    1 Project / Workspace"), "{text:?}");
    assert!(text.contains("  * 2 Project / Workspace"), "{text:?}");
}

#[test]
fn navigator_window_follows_an_active_context_beyond_the_first_nine() {
    let contexts = (0..20)
        .map(|index| TestContext {
            project_label: Some(if index % 2 == 0 {
                "Project A"
            } else {
                "Project B"
            }),
            workspace_label: Some("Workspace"),
            active: index == 14,
        })
        .collect::<Vec<_>>();

    let text = rendered(64, MAX_VISIBLE_CONTEXTS as u16, &contexts, Some(15));

    // The clipped window keeps its last row for the current/total position.
    assert!(text.contains("    11 "), "{text:?}");
    assert!(text.contains("  * 15 "), "{text:?}");
    assert!(text.contains("    18 "), "{text:?}");
    assert!(text.contains("15/20"), "{text:?}");
    assert!(!text.contains("     1 "), "{text:?}");
    assert!(!text.contains("    19 "), "{text:?}");
    assert!(!text.contains("    20 "), "{text:?}");
}

#[test]
fn navigator_is_bounded_by_narrow_and_zero_sized_rectangles() {
    let contexts = [TestContext {
        project_label: Some("Project"),
        workspace_label: Some("Workspace"),
        active: true,
    }];

    let narrow = rendered(1, 1, &contexts, Some(1));
    assert_eq!(narrow, " \n");

    let zero = rendered(0, 0, &contexts, Some(1));
    assert!(zero.is_empty());
}

#[test]
fn server_labels_are_sanitized_without_mutating_the_source() {
    let project = String::from("Project\0\x1b\r\n");
    let workspace = String::from("Workspace\u{7f}\u{2028}\u{2029}");
    let project_copy = project.clone();
    let workspace_copy = workspace.clone();
    let contexts = [TestContext {
        project_label: Some(project.as_str()),
        workspace_label: Some(workspace.as_str()),
        active: true,
    }];

    let text = rendered(80, 1, &contexts, Some(1));

    assert!(text.contains("Project / Workspace"), "{text:?}");
    assert!(text.lines().all(|line| !line.chars().any(char::is_control)));
    assert_eq!(project, project_copy);
    assert_eq!(workspace, workspace_copy);
}

#[test]
fn navigator_label_handles_standalone_and_workspace_locations() {
    assert_eq!(
        context_label(ContextDisplay {
            position: 1,
            active: true,
            project_label: None,
            workspace_label: None,
            no_workspace_label: "No workspace",
        }),
        "No workspace"
    );
    assert_eq!(
        context_label(ContextDisplay {
            position: 2,
            active: false,
            project_label: Some("Project"),
            workspace_label: Some("Workspace"),
            no_workspace_label: "No workspace",
        }),
        "Project / Workspace"
    );
}

#[test]
fn navigator_keeps_current_and_provisional_focus_as_independent_tracks() {
    let contexts = [
        TestContext {
            project_label: Some("Current"),
            workspace_label: None,
            active: true,
        },
        TestContext {
            project_label: Some("Focused"),
            workspace_label: None,
            active: false,
        },
    ];
    let theme = crate::tui_theme::Theme::rgb_test_dark();

    let (text, buffer) = rendered_with_focus(24, 2, &contexts, Some(1), Some(2));

    assert!(text.contains("  * 1 Current"), "{text:?}");
    assert!(text.contains("›   2 Focused"), "{text:?}");
    assert_eq!(buffer[(2, 0)].symbol(), theme.glyphs.active_marker);
    assert_eq!(buffer[(0, 1)].symbol(), theme.glyphs.navigation_marker);
    assert_eq!(buffer[(2, 1)].symbol(), " ");
    assert_eq!(
        buffer[(6, 0)].bg,
        theme.color(crate::tui_theme::Role::SurfaceCurrent)
    );
    assert_eq!(
        buffer[(6, 1)].bg,
        theme.color(crate::tui_theme::Role::SurfaceSelection)
    );
}

#[test]
fn navigator_pointer_surface_keeps_current_and_provisional_context_tracks_independent() {
    use crate::{
        tui_hit_map::ProjectedPointer,
        tui_theme::{Role, Theme},
    };

    let contexts = [
        TestContext {
            project_label: Some("Current"),
            workspace_label: None,
            active: true,
        },
        TestContext {
            project_label: Some("Focused"),
            workspace_label: None,
            active: false,
        },
    ];
    let theme = Theme::rgb_test_dark();
    let mut terminal = infallible(Terminal::new(TestBackend::new(24, 2)));
    let mut rows = Vec::new();
    infallible(terminal.draw(|frame| {
        render_navigator(
            frame,
            frame.area(),
            contexts.len(),
            Some(1),
            Some(2),
            |index| {
                contexts.get(index).copied().map(|context| ContextDisplay {
                    position: index + 1,
                    active: context.active,
                    project_label: context.project_label,
                    workspace_label: context.workspace_label,
                    no_workspace_label: "No workspace",
                })
            },
            &theme,
            &mut (),
            &mut |_, index, _| {
                if index == 0 {
                    ProjectedPointer::Hovered
                } else {
                    ProjectedPointer::Pressed
                }
            },
            &mut |_, index, rect| rows.push((index, rect)),
        );
    }));

    assert_eq!(rows.len(), 2);
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(2, 0)].symbol(), theme.glyphs.active_marker);
    assert_eq!(buffer[(0, 1)].symbol(), theme.glyphs.navigation_marker);
    assert_eq!(buffer[(6, 0)].bg, theme.color(Role::SurfaceHover));
    assert_eq!(buffer[(6, 1)].bg, theme.color(Role::SurfacePressed));
}

#[test]
fn navigator_preflights_the_complete_visible_source_before_paint_and_hits() {
    let mut terminal = infallible(Terminal::new(TestBackend::new(24, 2)));
    let mut rows = Vec::new();

    infallible(terminal.draw(|frame| {
        for y in 0..2 {
            frame
                .buffer_mut()
                .set_string(0, y, ".".repeat(24), ratatui::style::Style::new());
        }
        render_navigator(
            frame,
            frame.area(),
            2,
            Some(1),
            Some(1),
            |index| {
                (index == 0).then_some(ContextDisplay {
                    position: 1,
                    active: true,
                    project_label: Some("Only"),
                    workspace_label: None,
                    no_workspace_label: "No workspace",
                })
            },
            &crate::tui_theme::Theme::rgb_test_dark(),
            &mut (),
            &mut |_, _, _| crate::tui_hit_map::ProjectedPointer::Rest,
            &mut |_, index, rect| rows.push((index, rect)),
        );
    }));

    assert!(rows.is_empty());
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .all(|cell| cell.symbol() == ".")
    );
}
