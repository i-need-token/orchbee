//! Pending Workspace chrome projects one local launch without inventing actionable resource IDs.

use super::{Frame, Locale, Paragraph, Rect, Role, Span, Theme, tabs};
use crate::{
    machine_picker::SidebarCommand,
    terminal_session::WorkspaceCreationPreview,
    tui_layout::{ShellAreas, native_terminal_area},
    tui_pointer::ProjectedPointer,
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
        }],
        theme,
        locale.shell().unnamed_tab_group,
        tabs::groups::GroupInteractions::new(
            &mut (),
            &mut |_, _, _, _| ProjectedPointer::Rest,
            &mut |_, _, _, _| {},
        ),
    );
    if areas.tier.shows_panel_frame() {
        frame.render_widget(
            Paragraph::new(format!(" {} 1 ", locale.shell().panel_title))
                .style(theme.style(Role::TextSecondary)),
            Rect::new(
                areas.panel.x,
                areas.panel.y,
                areas.panel.width,
                1.min(areas.panel.height),
            ),
        );
    }
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{} ", theme.glyphs.status_bullet),
                theme.style(Role::StatusPending),
            ),
            Span::styled(progress, theme.style(Role::TextSecondary)),
        ])),
        native_terminal_area(areas.panel, areas.tier.shows_panel_frame(), false),
    );
}
