//! The Start Agent picker as the shared Medium dialog: where the Agent opens, the query, the
//! grouped provider list and one hint row. The Board owner keeps its episode and capture; this
//! file only paints the picker rows and publishes their exact targets.

use ratatui::{Frame, layout::Rect, widgets::Paragraph};

use super::super::controls::{
    self, ControlSurface, KeyHint, QueryField, RowContentBand, RowFocusCue, RowTracks,
    render_divider, render_query_field,
};
use super::super::modal::{
    Dialog, DialogClass, DialogFooter, DialogRow, DialogTargets, dialog_layout, modal_key_label,
    paint_dialog,
};
use super::{interaction_state, pointer_presentation, render_header_close};
use crate::agent_board::picker::{Group, Item, Meta};
use crate::agent_board::start::StartPlace;
use crate::agent_board::{
    AgentBoardControl, AgentBoardDraw, AgentBoardView, AgentStartCatalogStatus,
};
use crate::client_view::{Binding, KeyCode, Modifiers};
use crate::localization::ShellText;
use crate::terminal_text::{display_width, truncate_display};
use crate::tui_hit_map::{
    HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, ListScrollTarget,
    PrimaryContract,
};
use crate::tui_theme::{Role, Theme};

/// The query row takes the Medium class's picker width: 56 content cells, 62 outside.
const QUERY_WIDTH: u16 = 56;
const QUERY: usize = 2;
const DIVIDER: usize = 3;
const LIST: usize = 4;

pub(super) fn render(
    frame: &mut Frame<'_>,
    text: ShellText,
    board: &AgentBoardView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> AgentBoardDraw {
    let Some(picker) = board.picker() else {
        return AgentBoardDraw::NotPresented(board.episode());
    };
    let copy = &text.board.picker;
    let items = board.picker_items();
    let place = match picker.place() {
        StartPlace::ThisPanel(_) => copy.this_panel,
        StartPlace::NewTab => copy.new_tab,
    };
    let context = copy.opens_in.replace("{place}", place);
    let status = match board.start_catalog_status() {
        AgentStartCatalogStatus::Unsupported => Some(DialogRow::Status(
            copy.unsupported.into(),
            Role::StatusWarning,
        )),
        _ if !items
            .iter()
            .any(|item| matches!(item, Item::Provider { .. } | Item::Fold(_))) =>
        {
            Some(DialogRow::Item(copy.no_match.into(), Role::TextSecondary))
        }
        AgentStartCatalogStatus::Loading if board.can_start_agent() => {
            Some(DialogRow::Status(copy.checking.into(), Role::StatusPending))
        }
        // With nothing to report the list takes the row, so no blank row sits under a list
        // that has more rows below.
        _ => None,
    };
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let up = modal_key_label(theme, &Binding::new(KeyCode::Up, Modifiers::NONE));
    let down = modal_key_label(theme, &Binding::new(KeyCode::Down, Modifiers::NONE));
    let vertical = format!("{up}{down}");
    let hints = [
        KeyHint::new(&escape, text.board.close_hint),
        KeyHint::new(&enter, copy.start_hint),
        KeyHint::new(&vertical, text.board.select_hint),
    ];
    // The dialog pads below its title and above its footer itself.
    let mut rows = vec![
        DialogRow::Context(context.into()),
        DialogRow::Blank,
        DialogRow::Field {
            minimum: QUERY_WIDTH,
        },
        DialogRow::Blank,
        DialogRow::List { len: items.len() },
    ];
    rows.extend(status);
    let dialog = Dialog {
        title: copy.title,
        class: DialogClass::Medium,
        rows,
        footer: DialogFooter::Hints(&hints),
    };
    let Some(layout) = dialog_layout(frame.area(), &dialog, theme) else {
        return AgentBoardDraw::NotPresented(board.episode());
    };
    let Some(query_row) = layout.row(QUERY) else {
        return AgentBoardDraw::NotPresented(board.episode());
    };
    // An empty list has no rows to draw; the status row says why.
    let list = layout
        .row(LIST)
        .unwrap_or(Rect::new(query_row.x, query_row.y, 0, 0));
    // A list too short for a heading and one row offers nothing to choose: the picker suspends.
    if usize::from(list.height) < items.len().min(2) {
        return AgentBoardDraw::NotPresented(board.episode());
    }
    let Some(draw) = board.actual_draw(usize::from(list.height).max(1)) else {
        return AgentBoardDraw::NotPresented(board.episode());
    };
    let scroll = draw.token().scroll();
    paint_dialog(
        frame,
        &dialog,
        &layout,
        &DialogTargets {
            close: HitTarget::AgentBoardControl(draw.control_target(AgentBoardControl::Close)),
            actions: &[],
            dismiss_outside: false,
            scroll: Some(HitTarget::ListScroll(ListScrollTarget::AgentBoard(
                draw.token().clone(),
            ))),
        },
        theme,
        hits,
    );
    // The Board's own close publishes the interaction its pointer lane routes.
    let _ = render_header_close(frame, layout.area, &draw, theme, hits);
    let _ = render_query_field(
        frame,
        query_row,
        QueryField::Editing {
            label: copy.search,
            query: picker.query(),
            cursor: picker.query_cursor(),
        },
        theme,
    );
    if let Some(divider) = layout.row(DIVIDER) {
        render_divider(frame, divider, theme);
    }
    let cursor = picker.cursor_index(&items);
    let visible = usize::from(list.height);
    for (offset, item) in items.iter().enumerate().skip(scroll).take(visible) {
        let Ok(dy) = u16::try_from(offset - scroll) else {
            break;
        };
        let row = Rect::new(list.x, list.y + dy, list.width, 1);
        paint_item(
            frame,
            row,
            text,
            *item,
            (offset, cursor == Some(offset)),
            board.can_start_agent(),
            &draw,
            theme,
            hits,
        );
    }
    AgentBoardDraw::Presented(draw)
}

#[allow(clippy::too_many_arguments)]
fn paint_item(
    frame: &mut Frame<'_>,
    row: Rect,
    text: ShellText,
    item: Item,
    (index, selected): (usize, bool),
    can_start: bool,
    draw: &crate::agent_board::AgentBoardActualDraw,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let copy = &text.board.picker;
    let ellipsis = theme.glyphs.ellipsis;
    let (label, meta, enabled) = match item {
        Item::Heading(group) => {
            let heading = match group {
                Group::Recent => copy.recent,
                Group::Installed => copy.installed,
                Group::NotFound => copy.not_found,
                Group::NotChecked => copy.not_checked,
            };
            frame.render_widget(
                Paragraph::new(truncate_display(heading, usize::from(row.width), ellipsis))
                    .style(theme.style(Role::TextSecondary)),
                row,
            );
            return;
        }
        Item::Provider { id, meta, .. } => (
            crate::agent_provider::product_name(id).to_owned(),
            match meta {
                Meta::Command => crate::agent_provider::launch_line(id).unwrap_or_default(),
                Meta::Missing => copy.meta_missing.to_owned(),
                Meta::Generic => copy.meta_generic.to_owned(),
            },
            can_start,
        ),
        Item::Fold(count) => (
            format!(
                "{} {}",
                copy.fold.replace("{count}", &count.to_string()),
                theme.glyphs.navigation_marker
            ),
            String::new(),
            true,
        ),
    };
    let navigation_width =
        u16::try_from(display_width(theme.glyphs.navigation_marker).max(1)).unwrap_or(u16::MAX);
    let Some(content) = row
        .width
        .checked_sub(navigation_width)
        .filter(|width| *width > 0)
    else {
        return;
    };
    let navigation = Rect::new(row.x, row.y, navigation_width, 1);
    // The command keeps its whole width while the name keeps at least half of the row.
    let meta_width = u16::try_from(display_width(&meta))
        .unwrap_or(u16::MAX)
        .min(content / 2);
    let gap = u16::from(meta_width > 0);
    let primary_width = content.saturating_sub(meta_width + gap).max(1);
    let primary = Rect::new(navigation.right(), row.y, primary_width, 1);
    let meta_rect = Rect::new(row.right().saturating_sub(meta_width), row.y, meta_width, 1);
    let primary_role = if enabled {
        Role::TextPrimary
    } else {
        Role::TextDisabled
    };
    frame.render_widget(
        Paragraph::new(truncate_display(
            &label,
            usize::from(primary.width),
            ellipsis,
        ))
        .style(theme.style(primary_role)),
        primary,
    );
    let mut bands = Vec::new();
    if meta_width > 0 {
        frame.render_widget(
            Paragraph::new(truncate_display(
                &meta,
                usize::from(meta_rect.width),
                ellipsis,
            ))
            .style(theme.style(Role::TextSecondary)),
            meta_rect,
        );
        bands.push(RowContentBand::semantic(meta_rect, Role::TextSecondary));
    }
    let interaction =
        InteractionTarget::AgentBoardControl(draw.control_target(AgentBoardControl::Pick(index)));
    let spec = if enabled {
        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
    } else {
        InteractionSpec::disabled(PrimaryContract::ActivateOnRelease)
    };
    let pointer = pointer_presentation(hits.projected_pointer(
        HitLayer::Overlay,
        row,
        row,
        &interaction,
        spec,
    ));
    let Some(painted) = controls::paint_row_interaction(
        frame,
        row,
        RowTracks::new(RowFocusCue::EnclosingFrame, primary)
            .with_primary_role(primary_role)
            .with_provisional(navigation),
        &bands,
        ControlSurface::Overlay,
        interaction_state(enabled, selected, false, pointer),
        theme,
    ) else {
        return;
    };
    hits.record_interaction(HitLayer::Overlay, row, painted, interaction, spec);
}
