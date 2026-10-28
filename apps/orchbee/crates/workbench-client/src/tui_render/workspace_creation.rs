//! Pending Workspace chrome projects one local launch without inventing actionable resource IDs.

use super::{Frame, Locale, Paragraph, Role, Span, Theme, tabs};
use crate::{
    machine_picker::SidebarCommand,
    tui_layout::{PaneFrame, ShellAreas, native_terminal_area},
    tui_pointer::ProjectedPointer,
    workbench_client::WorkspaceCreationPreview,
};
use ratatui::text::Line;

pub(super) fn label<'a>(preview: WorkspaceCreationPreview<'a>, locale: Locale) -> &'a str {
    preview
        .label
        .unwrap_or(locale.sidebar_navigation_action(SidebarCommand::NewWorkspace))
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    areas: &ShellAreas,
    preview: WorkspaceCreationPreview<'_>,
    locale: Locale,
    theme: &Theme,
) {
    let progress = locale.workspace_creation_progress(preview.phase);
    frame
        .buffer_mut()
        .set_style(areas.tabs, theme.style(Role::SurfaceStrip));
    tabs::groups::render(
        frame,
        areas.tabs,
        &[tabs::groups::GroupTabDisplay {
            tab: tabs::TabDisplay {
                position: 1,
                active: true,
                label: format!("{} · {progress}", locale.automatic_tab_label(1)).into(),
            },
            group: None,
            collapsed: false,
            selected: false,
            pinned: false,
            agent: None,
        }],
        theme,
        locale.shell().unnamed_tab_group,
        0,
        None,
        tabs::groups::GroupInteractions::new(
            &mut (),
            &mut |_, _, _, _| ProjectedPointer::Rest,
            &mut |_, _, _, _| {},
        ),
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{} ", theme.glyphs.status_bullet),
                theme.style(Role::StatusPending),
            ),
            Span::styled(progress, theme.style(Role::TextSecondary)),
        ])),
        native_terminal_area(areas.panel, PaneFrame::Bare),
    );
}
