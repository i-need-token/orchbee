//! Capturing navigator paint; stable rows and their proofs publish in one frame.
use super::*;
use crate::client_view::{PrefixDesc, navigator::Target};

#[expect(
    clippy::too_many_arguments,
    reason = "shell projection and successful-frame hit sink stay explicit"
)]
pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    text: ShellText,
    locale: Locale,
    view: &ClientView,
    native: Option<&NativeSessionView<'_>>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    // A covering owner may appear between reconciliation and this draw. Covered
    // rows publish no authority even before the next dispatcher reconciliation.
    if view.machines.open
        || view.resize_mode()
        || native.is_some_and(|native| {
            native.close_confirmation().is_some()
                || native.panel_close_confirmation().is_some()
                || native.workspace_close_confirmation().is_some()
                || native.rename_draft().is_some()
                || native.setup_draft().is_some()
                || native.agent_board().is_some()
                || native.worktree().is_some()
                || native.popup().is_some()
                || native.action_menu().is_some()
                || native.settings().is_some()
                || native.copy_mode().is_some()
        })
    {
        return None;
    }
    let area = place_overlay(area, 88, 15)?;
    let block = overlay_block(text.navigator_title, theme, true);
    let content = block.inner(area);
    // Two row columns, their headings, refresh feedback and two instruction rows.
    // Refusal publishes no receipt and the dispatcher retires capture after paint.
    let essential = locale.navigator_hint(false);
    if content.height < 7
        || content.width < 40
        || display_width(&essential) > usize::from(content.width)
    {
        return None;
    }
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    let [heading, rows, refresh_area, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(2),
        Constraint::Length(1),
        Constraint::Length(2),
    ])
    .areas(content);
    let [contexts_area, tabs_area] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Min(0)]).areas(rows);
    let [locations_heading, tabs_heading] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Min(0)]).areas(heading);
    let titles = locale.navigator_columns();
    for (area, title) in [(locations_heading, titles.0), (tabs_heading, titles.1)] {
        frame.render_widget(
            Paragraph::new(title).style(theme.style(Role::TextSecondary)),
            area,
        );
    }
    let full = locale.navigator_hint(true);
    let movement = locale.navigator_movement();
    let toggle = view
        .bindings()
        .labels(PrefixDesc::NavigatorToggle)
        .join(" / ");
    let mut return_hint = essential;
    if !toggle.is_empty() {
        let candidate = format!("{return_hint} · {toggle}");
        if display_width(&candidate) <= usize::from(content.width) {
            return_hint = candidate;
        }
    }
    let movement = if display_width(&full) <= usize::from(content.width) {
        full
    } else {
        movement
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(return_hint, theme.style(Role::HintKey)),
            Line::styled(movement, theme.style(Role::HintLabel)),
        ]),
        footer,
    );
    let page_rows = if matches!(view.navigator.focus, Some(Target::Location(_))) {
        usize::from(rows.height).min(contexts::MAX_VISIBLE_CONTEXTS)
    } else {
        usize::from(rows.height)
    };
    hits.start_navigator(view.navigator.focus.clone(), page_rows);
    if let Some(rect) = super::controls::close(frame, area, theme) {
        hits.record(
            HitLayer::Overlay,
            rect,
            HitTarget::NavigatorClose(view.navigator.episode()),
        );
    }
    for (rect, tabs) in [(contexts_area, false), (tabs_area, true)] {
        if let Some(row) = view.navigator.scroll_row(tabs) {
            hits.record(
                HitLayer::OverlaySurface,
                rect,
                HitTarget::ListScroll(crate::tui_pointer::ListScrollTarget::Navigator(row)),
            );
        }
    }
    let Some(native) = native else {
        return Some(area);
    };
    let locations_focus = (0..native.context_count())
        .find(|index| {
            native.context_at(*index).is_some_and(|context| {
                view.navigator.focus.as_ref() == Some(&Target::Location(context.location().clone()))
            })
        })
        .map(|index| index + 1);
    let tabs_focus = (0..native.tab_count())
        .find(|index| {
            native.tab(*index).is_some_and(|tab| {
                view.navigator.focus.as_ref() == Some(&Target::Tab(tab.tab_id().clone()))
            })
        })
        .map(|index| index + 1);
    contexts::render_navigator(
        frame,
        contexts_area,
        native.context_count(),
        native.active_context_position(),
        locations_focus,
        |index| {
            native
                .context_at(index)
                .map(|context| contexts::ContextDisplay {
                    position: context.position(),
                    active: context.active(),
                    project_label: context.project_label(),
                    workspace_label: context.workspace_label(),
                    standalone_label: text.standalone,
                })
        },
        theme,
        &mut |index, offset| {
            if let Some(row) = native.context_at(index).and_then(|context| {
                view.navigator
                    .row(Target::Location(context.location().clone()))
            }) {
                hits.record_navigator_row(
                    Rect::new(
                        contexts_area.x,
                        contexts_area.y + offset,
                        contexts_area.width,
                        1,
                    ),
                    row,
                );
            }
        },
    );
    tabs::render_navigator(
        frame,
        tabs_area,
        native.tab_count(),
        native.active_position(),
        tabs_focus,
        |index| native_tab(native, index, text.pinned_tab),
        theme,
        &mut |index, offset| {
            if let Some(row) = native
                .tab(index)
                .and_then(|tab| view.navigator.row(Target::Tab(tab.tab_id().clone())))
            {
                hits.record_navigator_row(
                    Rect::new(tabs_area.x, tabs_area.y + offset, tabs_area.width, 1),
                    row,
                );
            }
        },
    );
    if let Some(refresh) = native.refresh_status() {
        let style = match refresh {
            NativeRefreshStatus::Refreshing => theme.style(Role::TextSecondary),
            NativeRefreshStatus::Success(_) => theme.style(Role::StatusSuccess),
            NativeRefreshStatus::Failure(_) => theme.style(Role::StatusError),
        };
        frame.render_widget(
            Paragraph::new(locale.native_refresh_status(refresh)).style(style),
            refresh_area,
        );
    }
    Some(area)
}
