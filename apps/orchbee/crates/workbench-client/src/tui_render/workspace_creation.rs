//! Pending Workspace chrome projects one local launch without inventing actionable resource IDs.

use super::{Frame, Locale, Paragraph, Role, Span, Theme, tabs};
use crate::machine_picker::SidebarCommand;
use crate::tui_hit_map::ProjectedPointer;
use crate::tui_layout::{PaneFrame, ShellAreas, native_terminal_area};
use crate::workbench_state::workspace_setup::WorkspaceCreationPreview;
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
    // The Mobile tier's bar centers the pending Tab itself.
    if areas.tier != crate::tui_layout::ViewportTier::Mobile {
        render_pending_tab(frame, areas.tabs, preview, locale, theme);
    }
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

/// The pending Workspace's first Tab, which the strip and the Mobile bar show while it launches.
pub(super) fn pending_tab_label(preview: WorkspaceCreationPreview<'_>, locale: Locale) -> String {
    let progress = locale.workspace_creation_progress(preview.phase);
    format!("{} · {progress}", locale.automatic_tab_label(1))
}

fn render_pending_tab(
    frame: &mut Frame<'_>,
    tabs_area: ratatui::layout::Rect,
    preview: WorkspaceCreationPreview<'_>,
    locale: Locale,
    theme: &Theme,
) {
    frame
        .buffer_mut()
        .set_style(tabs_area, theme.style(Role::SurfaceStrip));
    tabs::groups::render(
        frame,
        tabs_area,
        &[tabs::groups::GroupTabDisplay {
            tab: tabs::TabDisplay {
                position: 1,
                active: true,
                label: pending_tab_label(preview, locale).into(),
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
            &mut |_, _, _, _, _| {},
        ),
    );
}
