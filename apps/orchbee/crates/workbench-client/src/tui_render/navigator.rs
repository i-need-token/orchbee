//! Capturing navigator paint; stable rows and their proofs publish in one frame.
use super::{
    ClientView, Constraint, Frame, HitLayer, HitMapBuilder, HitTarget, Layout, Line, Locale,
    Paragraph, Rect, RefreshStatus, Role, ShellText, Theme, WorkbenchClientView, contexts,
    native_tab, overlay_block, place_overlay, tabs,
};
use crate::{
    client_view::{
        PrefixAction,
        navigator::{Command, Target, label_with_glyphs},
    },
    tui_hit_map::{InteractionSpec, InteractionTarget, PrimaryContract, ProjectedPointer},
};

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
    native: Option<&WorkbenchClientView<'_>>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    // A covering owner may appear between reconciliation and this draw. Covered
    // rows publish no authority even before the next dispatcher reconciliation.
    if view.machines.captures()
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
    let block = overlay_block(text.navigator_title, area.width, theme);
    let content = block.inner(area);
    // Two row columns, their headings, refresh feedback and two instruction rows.
    // Refusal publishes no receipt and the dispatcher retires capture after paint.
    let copy = locale.navigator();
    let label_glyphs = theme.glyphs.key_labels();
    let toggle = view
        .bindings()
        .action_labels_with_glyphs(PrefixAction::NavigatorToggle, label_glyphs)
        .join(" / ");
    let exit_key = if toggle.is_empty() {
        label_with_glyphs(Command::Exit, label_glyphs)
    } else {
        format!(
            "{} / {toggle}",
            label_with_glyphs(Command::Exit, label_glyphs)
        )
    };
    let open_key = label_with_glyphs(Command::Open, label_glyphs);
    let action_hints = [
        super::controls::KeyHint::new(&exit_key, copy.return_action),
        super::controls::KeyHint::new(&open_key, copy.open_action),
    ];
    let exit_hint = [action_hints[0]];
    if content.height < 7
        || content.width < 40
        || super::controls::measure_overlay_hints(&exit_hint, theme)
            .is_none_or(|width| width > usize::from(content.width))
    {
        return None;
    }
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
    let previous_next = format!(
        "{} / {}",
        label_with_glyphs(Command::Previous, label_glyphs),
        label_with_glyphs(Command::Next, label_glyphs)
    );
    let columns = format!(
        "{} / {} / {}",
        label_with_glyphs(Command::Column, label_glyphs),
        label_with_glyphs(Command::Locations, label_glyphs),
        label_with_glyphs(Command::Tabs, label_glyphs)
    );
    let ends = format!(
        "{} / {}",
        label_with_glyphs(Command::First, label_glyphs),
        label_with_glyphs(Command::Last, label_glyphs)
    );
    let pages = format!(
        "{} / {}",
        label_with_glyphs(Command::PagePrevious, label_glyphs),
        label_with_glyphs(Command::PageNext, label_glyphs)
    );
    let movement_hints = [
        super::controls::KeyHint::new(&previous_next, copy.select_action),
        super::controls::KeyHint::new(&columns, copy.column_action),
        super::controls::KeyHint::new(&ends, copy.ends_action),
        super::controls::KeyHint::new(&pages, copy.page_action),
    ];
    let action_row = Rect::new(footer.x, footer.y, footer.width, 1);
    let movement_row = Rect::new(footer.x, footer.y.saturating_add(1), footer.width, 1);
    let _ = super::controls::render_overlay_hints(frame, action_row, &action_hints, theme);
    let _ = super::controls::render_overlay_hints(frame, movement_row, &movement_hints, theme);
    let page_rows = if matches!(view.navigator.focus, Some(Target::Location(_))) {
        usize::from(rows.height).min(contexts::MAX_VISIBLE_CONTEXTS)
    } else {
        usize::from(rows.height)
    };
    let source = view.navigator.draw_source();
    hits.start_navigator(source.clone(), view.navigator.focus.clone(), page_rows);
    if let Some(rect) = super::controls::frame_close_rect(frame.area(), area) {
        let (availability, pointer, target) = source.map_or(
            (
                super::controls::Availability::Disabled,
                ProjectedPointer::Rest,
                None,
            ),
            |source| {
                let target = InteractionTarget::NavigatorClose(source);
                let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
                (
                    super::controls::Availability::Enabled,
                    hits.projected_pointer(HitLayer::Overlay, rect, rect, &target, spec),
                    Some((target, spec)),
                )
            },
        );
        let painted = super::controls::render_frame_icon_action(
            frame,
            rect,
            super::controls::IconActionSpec::new(
                super::controls::IconActionGlyph::Close,
                super::controls::ControlSurface::Overlay,
                availability,
                tabs::pointer_presentation(pointer),
            ),
            theme,
        );
        if let (Some(painted), Some((target, spec))) = (painted, target) {
            hits.record_interaction(HitLayer::Overlay, rect, painted, target, spec);
        }
    }
    for (rect, tabs) in [(contexts_area, false), (tabs_area, true)] {
        if let Some(row) = view.navigator.scroll_row(tabs) {
            hits.record(
                HitLayer::OverlaySurface,
                rect,
                HitTarget::ListScroll(crate::tui_hit_map::ListScrollTarget::Navigator(row)),
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
                    workspace_label: context
                        .workspace_label()
                        .map(|label| locale.workspace_display_label(label)),
                    no_workspace_label: text.no_workspace,
                })
        },
        theme,
        hits,
        &mut |hits, index, rect| {
            let Some(row) = native.context_at(index).and_then(|context| {
                view.navigator
                    .row(Target::Location(context.location().clone()))
            }) else {
                return ProjectedPointer::Rest;
            };
            let target = InteractionTarget::NavigatorRow(row);
            hits.projected_pointer(
                HitLayer::Overlay,
                rect,
                rect,
                &target,
                InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
            )
        },
        &mut |hits, index, rect| {
            if let Some(row) = native.context_at(index).and_then(|context| {
                view.navigator
                    .row(Target::Location(context.location().clone()))
            }) {
                hits.record_navigator_row(rect, row);
            }
        },
    );
    tabs::render_navigator(
        frame,
        tabs_area,
        native.tab_count(),
        native.active_position(),
        tabs_focus,
        |index| native_tab(native, index, locale, text.pinned_tab),
        theme,
        hits,
        &mut |hits, index, rect| {
            let Some(row) = native
                .tab(index)
                .and_then(|tab| view.navigator.row(Target::Tab(tab.tab_id().clone())))
            else {
                return ProjectedPointer::Rest;
            };
            let target = InteractionTarget::NavigatorRow(row);
            hits.projected_pointer(
                HitLayer::Overlay,
                rect,
                rect,
                &target,
                InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
            )
        },
        &mut |hits, index, rect| {
            if let Some(row) = native
                .tab(index)
                .and_then(|tab| view.navigator.row(Target::Tab(tab.tab_id().clone())))
            {
                hits.record_navigator_row(rect, row);
            }
        },
    );
    if let Some(refresh) = native.refresh_status() {
        let role = match refresh {
            RefreshStatus::Refreshing => Role::StatusWorking,
            RefreshStatus::Success(_) => Role::StatusSuccess,
            RefreshStatus::Failure(_) => Role::StatusError,
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                ratatui::text::Span::styled(theme.glyphs.status_bullet, theme.style(role)),
                ratatui::text::Span::styled(
                    format!(" {}", locale.native_refresh_status(refresh)),
                    theme.style(Role::TextPrimary),
                ),
            ])),
            refresh_area,
        );
    }
    Some(area)
}
