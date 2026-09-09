use std::convert::Infallible;

use ratatui::{Terminal, backend::TestBackend};

use super::{ContextDisplay, MAX_VISIBLE_CONTEXTS, location_label, render_navigator};

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
    let mut terminal = infallible(Terminal::new(TestBackend::new(width, height)));
    infallible(terminal.draw(|frame| {
        let context_at = |index: usize| {
            contexts.get(index).copied().map(|context| ContextDisplay {
                position: index.saturating_add(1),
                active: context.active,
                project_label: context.project_label,
                workspace_label: context.workspace_label,
                standalone_label: "Standalone terminal",
            })
        };
        render_navigator(
            frame,
            frame.area(),
            contexts.len(),
            active_position,
            context_at,
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
    text
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

    assert!(text.contains("[ 1] Project / Workspace"), "{text:?}");
    assert!(text.contains("[*2] Project / Workspace"), "{text:?}");
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

    assert!(text.contains("[ 11]"), "{text:?}");
    assert!(text.contains("[*15]"), "{text:?}");
    assert!(text.contains("[ 19]"), "{text:?}");
    assert!(!text.contains("[  1]"), "{text:?}");
    assert!(!text.contains("[ 20]"), "{text:?}");
}

#[test]
fn navigator_is_bounded_by_narrow_and_zero_sized_rectangles() {
    let contexts = [TestContext {
        project_label: Some("Project"),
        workspace_label: Some("Workspace"),
        active: true,
    }];

    let narrow = rendered(1, 1, &contexts, Some(1));
    assert_eq!(narrow, "[\n");

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

    assert!(text.contains("Project���� / Workspace���"), "{text:?}");
    assert!(text.lines().all(|line| !line.chars().any(char::is_control)));
    assert_eq!(project, project_copy);
    assert_eq!(workspace, workspace_copy);
}

#[test]
fn location_label_handles_standalone_and_workspace_locations() {
    assert_eq!(
        location_label(ContextDisplay {
            position: 1,
            active: true,
            project_label: None,
            workspace_label: None,
            standalone_label: "Standalone terminal",
        }),
        "Standalone terminal"
    );
    assert_eq!(
        location_label(ContextDisplay {
            position: 2,
            active: false,
            project_label: Some("Project"),
            workspace_label: Some("Workspace"),
            standalone_label: "Standalone terminal",
        }),
        "Project / Workspace"
    );
}
