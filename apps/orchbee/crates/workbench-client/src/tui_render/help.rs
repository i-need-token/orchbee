//! Shortcut-help geometry, semantic binding discovery, and exact draw authority.

use crate::tui_shortcut_help::ShortcutHelpEntry;
use crate::{
    client_view::{
        ClientView, KeyLabelGlyphs,
        help::{DrawnHelpControl, HelpControl, HelpDrawCandidate, HelpDrawSource},
    },
    localization::{Locale, ShellText},
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
    tui_layout::place_overlay,
    tui_pointer::{
        HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, PrimaryContract,
        ProjectedPointer,
    },
    tui_theme::{GlyphMode, Role, Theme},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, Padding, Paragraph},
};

use super::controls::{
    Availability, ControlSurface, IconActionGlyph, IconActionSpec, KeyHint, PointerPresentation,
    QueryField, icon_action_rect, render_divider, render_frame_icon_action, render_icon_action,
    render_overlay_hints, render_query_field,
};

/// Rows the query slot always owns: the filter editor and its divider.
const QUERY_ROWS: u16 = 2;
/// Rows the footer always owns: key hints and the scope control.
const FOOTER_ROWS: u16 = 2;
const PREFERRED_WIDTH: u16 = 76;
const TITLE_CONTROL_GAP: u16 = 1;

#[derive(Debug)]
struct ControlPlan {
    rect: Rect,
    target: InteractionTarget,
    interaction: InteractionSpec,
    visual: IconActionSpec,
}

/// Semantic Help content resolved by the shell before geometry admission.
pub(super) struct RenderContext<'a> {
    text: ShellText<'a>,
    locale: Locale,
    view: &'a ClientView,
    session_available: bool,
    candidate: HelpDrawCandidate,
}

impl<'a> RenderContext<'a> {
    pub(super) const fn new(
        text: ShellText<'a>,
        locale: Locale,
        view: &'a ClientView,
        session_available: bool,
        candidate: HelpDrawCandidate,
    ) -> Self {
        Self {
            text,
            locale,
            view,
            session_available,
            candidate,
        }
    }
}

/// Paints a complete Help panel and publishes the exact episode/scroll source.
///
/// Every required row, footer field, title control, and source is resolved before
/// the first cell changes. `None` therefore leaves both the candidate buffer and
/// hit builder untouched so the caller can paint the compact refusal surface.
pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    context: RenderContext<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    let RenderContext {
        text,
        locale,
        view,
        session_available,
        candidate,
    } = context;
    let mut entries =
        crate::tui_shortcut_help::entries(locale, view, session_available, key_label_glyphs(theme));
    if !view.help_expanded() && view.help_filter().trim().is_empty() {
        entries.retain(ShortcutHelpEntry::common);
    }
    let area = place_overlay(area, PREFERRED_WIDTH, area.height)?;
    if frame.area().intersection(area) != area {
        return None;
    }

    let [previous_rect, next_rect, close_rect] = title_control_rects(frame.area(), area)?;
    let block = help_block(area, previous_rect, text.shortcut_title, theme)?;
    let inner = block.inner(area);
    if inner.height <= QUERY_ROWS + FOOTER_ROWS || inner.width == 0 {
        return None;
    }
    let width = usize::from(inner.width);
    let [query_area, divider_area, rows_area, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(FOOTER_ROWS),
    ])
    .areas(inner);
    if rows_area.height == 0 || rows_area.width == 0 || footer_area.height != FOOTER_ROWS {
        return None;
    }
    let lines = shortcut_lines(
        &entries,
        view.help_filter(),
        locale,
        width,
        theme,
        view.help_expanded(),
    )?;
    let last = lines.len().saturating_sub(usize::from(rows_area.height));
    let page = candidate.page().min(last);
    let source = candidate.presented_rows(page, last, usize::from(rows_area.height))?;
    let footer_keys = FooterKeys::new(theme);
    let footer_hints = footer_keys.hints(locale);

    let close = control_plan(
        hits,
        source,
        HelpControl::Close,
        IconActionGlyph::Close,
        close_rect,
        true,
    );
    let previous = control_plan(
        hits,
        source,
        HelpControl::Previous,
        IconActionGlyph::Previous,
        previous_rect,
        page > 0,
    );
    let next = control_plan(
        hits,
        source,
        HelpControl::Next,
        IconActionGlyph::Next,
        next_rect,
        page < last,
    );

    let visible_end = page
        .saturating_add(usize::from(rows_area.height))
        .min(lines.len());
    let items = lines[page..visible_end]
        .iter()
        .map(|line| ListItem::new(line.clone()))
        .collect::<Vec<_>>();

    // All required geometry and text have been admitted above this boundary.
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    frame.render_widget(List::new(items), rows_area);
    // Typing always reaches the filter, so the query slot stays in its editing presentation.
    let _ = render_query_field(
        frame,
        query_area,
        QueryField::Editing {
            label: locale.shortcut_search_label(),
            query: view.help_filter(),
            cursor: view.help_filter().len(),
        },
        theme,
    );
    render_divider(frame, divider_area, theme);
    let _ = render_overlay_hints(
        frame,
        Rect::new(footer_area.x, footer_area.y, footer_area.width, 1),
        &footer_hints,
        theme,
    );

    let toggle_label = locale.shortcut_scope_label(view.help_expanded());
    let toggle_width = display_width(toggle_label) as u16;
    if toggle_width <= footer_area.width {
        let toggle_rect = Rect::new(footer_area.x, footer_area.y + 1, toggle_width, 1);
        let target = InteractionTarget::Help(DrawnHelpControl::new(source, HelpControl::ToggleAll));
        let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
        let hovered =
            hits.projected_pointer(HitLayer::Overlay, toggle_rect, toggle_rect, &target, spec)
                != ProjectedPointer::Rest;
        let style = theme.style(Role::HintKey).add_modifier(if hovered {
            ratatui::style::Modifier::REVERSED
        } else {
            ratatui::style::Modifier::UNDERLINED
        });
        frame.render_widget(Paragraph::new(toggle_label).style(style), toggle_rect);
        hits.record_interaction(HitLayer::Overlay, toggle_rect, toggle_rect, target, spec);
        let hint_width = footer_area.width.saturating_sub(toggle_width + 2);
        if display_width(locale.shortcut_buttons_hint()) <= usize::from(hint_width) {
            frame.render_widget(
                Paragraph::new(locale.shortcut_buttons_hint())
                    .style(theme.style(Role::TextSecondary)),
                Rect::new(toggle_rect.right() + 2, toggle_rect.y, hint_width, 1),
            );
        }
    }

    // Specific controls publish before the broad list/surface regions. Disabled
    // endpoints remain visible and frontmost but can never arm an activation.
    for control in [close, previous, next] {
        let painted = render_frame_icon_action(frame, control.rect, control.visual, theme)
            .expect("the exact three-cell Help control was preflighted before paint");
        debug_assert_eq!(painted, control.rect);
        hits.record_interaction(
            HitLayer::Overlay,
            control.rect,
            painted,
            control.target,
            control.interaction,
        );
    }
    hits.record(
        HitLayer::OverlaySurface,
        rows_area,
        HitTarget::ListScroll(crate::tui_pointer::ListScrollTarget::Help(source)),
    );
    hits.record_help_draw(source);
    Some(area)
}

/// Paints the complete three-cell recovery close when the full panel refuses.
/// No scroll boundary or list-control authority is invented for this surface.
pub(super) fn render_refused(
    frame: &mut Frame<'_>,
    area: Rect,
    text: ShellText<'_>,
    candidate: HelpDrawCandidate,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    if area.is_empty() || frame.area().intersection(area) != area {
        return None;
    }
    // The refusal floats as the status chip: one padding cell at each end, the notice, one gap,
    // then the close control, end-aligned so it covers only its own cells.
    let content = display_width(text.overlay_refused)
        .saturating_add(1)
        .saturating_add(3);
    let width = u16::try_from(content.saturating_add(2))
        .unwrap_or(u16::MAX)
        .min(area.width);
    if width < 5 {
        return None;
    }
    let chip = Rect::new(area.right() - width, area.y, width, 1);
    let close_offer = Rect::new(chip.right() - 4, chip.y, 3, 1);
    let close_rect = icon_action_rect(frame.area(), close_offer)
        .filter(|rect| chip.intersection(*rect) == *rect)?;
    let source = candidate.refused();
    let close = control_plan(
        hits,
        source,
        HelpControl::Close,
        IconActionGlyph::Close,
        close_rect,
        true,
    );
    let notice_x = chip.x + 1;
    let notice_width = close_rect.x.saturating_sub(notice_x);
    let notice_width = notice_width.saturating_sub(u16::from(notice_width > 0));
    let notice = truncate_display(
        text.overlay_refused,
        usize::from(notice_width),
        theme.glyphs.ellipsis,
    );

    frame.render_widget(Clear, chip);
    frame
        .buffer_mut()
        .set_style(chip, theme.style(Role::SurfaceOverlay));
    if notice_width > 0 {
        frame.render_widget(
            Paragraph::new(notice).style(theme.style(Role::TextSecondary)),
            Rect::new(notice_x, chip.y, notice_width, 1),
        );
    }
    let painted = render_icon_action(frame, close.rect, close.visual, theme)
        .expect("the exact recovery close was preflighted before paint");
    debug_assert_eq!(painted, close.rect);
    hits.mark_overlay_active();
    hits.record_interaction(
        HitLayer::Overlay,
        close.rect,
        painted,
        close.target,
        close.interaction,
    );
    hits.record(HitLayer::OverlaySurface, chip, HitTarget::OverlaySurface);
    hits.record_help_draw(source);
    Some(close_rect)
}

fn help_block(
    area: Rect,
    previous: Rect,
    title: &str,
    theme: &Theme,
) -> Option<ratatui::widgets::Block<'static>> {
    // The corner and `─ ` lead the heading; a trailing blank, one rule cell, and the blank
    // before the first title control follow it.
    let title_budget = usize::from(previous.x.checked_sub(area.x.checked_add(6)?)?);
    if display_width(&truncate_display(
        title,
        title_budget,
        theme.glyphs.ellipsis,
    )) == 0
    {
        return None;
    }
    Some(
        super::overlay_surface(theme, Padding::new(2, 2, 1, 0)).title(super::overlay_heading(
            title,
            title_budget,
            theme,
        )),
    )
}

/// Effective key labels for the Help footer hints, exit first.
struct FooterKeys {
    close: String,
    scroll: String,
    page: String,
    erase: String,
    clear: String,
}

impl FooterKeys {
    fn new(theme: &Theme) -> Self {
        use crate::client_view::{Binding, KeyCode, Modifiers};
        let label =
            |code, modifiers| super::modal::modal_key_label(theme, &Binding::new(code, modifiers));
        Self {
            close: label(KeyCode::Esc, Modifiers::NONE),
            scroll: format!(
                "{}/{}",
                label(KeyCode::Up, Modifiers::NONE),
                label(KeyCode::Down, Modifiers::NONE)
            ),
            page: format!(
                "{}/{}",
                label(KeyCode::PageUp, Modifiers::NONE),
                label(KeyCode::PageDown, Modifiers::NONE)
            ),
            erase: label(KeyCode::Backspace, Modifiers::NONE),
            clear: label(KeyCode::Char('u'), Modifiers::CTRL),
        }
    }

    fn hints(&self, locale: Locale) -> [KeyHint<'_>; 5] {
        [
            KeyHint::new(&self.close, locale.shortcut_close_action()),
            KeyHint::new(&self.scroll, locale.shortcut_scroll_action()),
            KeyHint::new(&self.page, locale.shortcut_page_action()),
            KeyHint::new(&self.erase, locale.shortcut_erase_action()),
            KeyHint::new(&self.clear, locale.shortcut_clear_action()),
        ]
    }
}

fn title_control_rects(bounds: Rect, area: Rect) -> Option<[Rect; 3]> {
    // `─ [<] [>] [x] ─┐`: blanks separate the controls, and one rule cell precedes the corner.
    let close_x = area.right().checked_sub(6)?;
    let next_x = close_x.checked_sub(3 + TITLE_CONTROL_GAP)?;
    let previous_x = next_x.checked_sub(3 + TITLE_CONTROL_GAP)?;
    let rects = [
        Rect::new(previous_x, area.y, 3, 1),
        Rect::new(next_x, area.y, 3, 1),
        Rect::new(close_x, area.y, 3, 1),
    ];
    let [previous, next, close] = rects.map(|rect| {
        icon_action_rect(bounds, rect).filter(|control| area.intersection(*control) == *control)
    });
    Some([previous?, next?, close?])
}

fn control_plan(
    hits: &HitMapBuilder,
    source: HelpDrawSource,
    control: HelpControl,
    glyph: IconActionGlyph,
    rect: Rect,
    enabled: bool,
) -> ControlPlan {
    let target = InteractionTarget::Help(DrawnHelpControl::new(source, control));
    let interaction = if enabled {
        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
    } else {
        InteractionSpec::disabled(PrimaryContract::ActivateOnRelease)
    };
    let pointer = pointer_presentation(hits.projected_pointer(
        HitLayer::Overlay,
        rect,
        rect,
        &target,
        interaction,
    ));
    ControlPlan {
        rect,
        target,
        interaction,
        visual: IconActionSpec::new(
            glyph,
            ControlSurface::Overlay,
            if enabled {
                Availability::Enabled
            } else {
                Availability::Disabled
            },
            pointer,
        ),
    }
}

const fn key_label_glyphs(theme: &Theme) -> KeyLabelGlyphs {
    match theme.glyphs.mode() {
        GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
        #[cfg(any(test, feature = "test-support"))]
        GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
    }
}

const fn pointer_presentation(pointer: ProjectedPointer) -> PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => PointerPresentation::Rest,
        ProjectedPointer::Hovered => PointerPresentation::Hovered,
        ProjectedPointer::Pressed => PointerPresentation::Pressed,
    }
}

fn shortcut_lines(
    entries: &[ShortcutHelpEntry],
    query: &str,
    locale: Locale,
    width: usize,
    theme: &Theme,
    include_legend: bool,
) -> Option<Vec<Line<'static>>> {
    let mut matching = entries
        .iter()
        .filter(|entry| {
            crate::tui_shortcut_help::shortcut_filter_matches(query, &entry.search_text)
        })
        .collect::<Vec<_>>();
    if !query.trim().is_empty() {
        // Title matches lead within their group; groups stay contiguous so each heading appears
        // once.
        matching.sort_by_key(|entry| {
            (
                crate::tui_shortcut_help::group_order(entry.group),
                !crate::tui_shortcut_help::shortcut_filter_matches(query, &entry.title),
            )
        });
    }
    if matching.is_empty() {
        return Some(vec![Line::styled(
            locale.shortcut_no_matches(),
            theme.style(Role::TextSecondary),
        )]);
    }

    let mut lines = Vec::new();
    let mut group = None;
    for entry in matching {
        if group != Some(entry.group) {
            group = Some(entry.group);
            lines.push(Line::styled(
                locale.shortcut_group_name(entry.group),
                theme
                    .style(Role::TextSecondary)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ));
        }

        let key = sanitize_terminal_text(&entry.key).trim().to_owned();
        let title = sanitize_terminal_text(&entry.title).into_owned();
        let description = match entry.disabled_reason.as_deref() {
            Some(reason) => format!("{title} · {}", sanitize_terminal_text(reason)),
            None => title,
        };
        let disabled = entry.disabled_reason.is_some();
        let key_style = theme.style(if disabled {
            Role::TextDisabled
        } else {
            Role::HintKey
        });
        let description_style = theme.style(if disabled {
            Role::TextDisabled
        } else {
            Role::TextPrimary
        });
        let indent = display_width(&key).saturating_add(2);
        if indent < width {
            let wrapped = wrap_display(&description, width - indent)?;
            let mut parts = wrapped.into_iter();
            lines.push(Line::from(vec![
                Span::styled(format!("{key}  "), key_style),
                Span::styled(parts.next().unwrap_or_default(), description_style),
            ]));
            lines.extend(
                parts.map(|part| Line::styled(format!("{:indent$}{part}", ""), description_style)),
            );
        } else {
            for key_line in wrap_key_aliases(&key, width)? {
                lines.push(Line::styled(key_line, key_style));
            }
            let description_indent = 2.min(width.saturating_sub(1));
            let wrapped = wrap_display(&description, width - description_indent)?;
            lines.extend(wrapped.into_iter().map(|part| {
                Line::styled(
                    format!("{:description_indent$}{part}", ""),
                    description_style,
                )
            }));
        }
    }
    if include_legend && query.trim().is_empty() {
        lines.push(Line::styled(
            locale.sidebar_marker_label("legend"),
            theme.style(Role::TextSecondary),
        ));
        for (glyph, label) in [
            (
                theme.glyphs.workspace,
                locale.sidebar_marker_label("workspace"),
            ),
            (
                theme.glyphs.active_marker,
                locale.sidebar_marker_label("current"),
            ),
            (
                theme.glyphs.status_bullet,
                locale.sidebar_marker_label("unread"),
            ),
            (
                theme.glyphs.disclosure_open,
                locale.sidebar_marker_label("expanded"),
            ),
            (
                theme.glyphs.disclosure_closed,
                locale.sidebar_marker_label("collapsed"),
            ),
        ] {
            lines.push(Line::raw(format!("{glyph}  {label}")));
        }
        use orchbee_workbench_protocol::MetadataState::*;
        for state in [Working, Blocked, Done, Error, Idle, Unknown] {
            lines.push(Line::raw(format!(
                "{}  {}",
                theme.agent_glyph(state),
                locale.machine_picker_text(super::sidebar_text::state_role(state).0)
            )));
        }
    }
    Some(lines)
}

/// Keeps every alias chord intact when the alias list wraps. A single chord
/// wider than the available column is still split safely so it cannot refuse
/// the rest of Help.
fn wrap_key_aliases(key: &str, width: usize) -> Option<Vec<String>> {
    if width == 0 {
        return None;
    }
    let mut lines = Vec::new();
    let mut line = String::new();
    for alias in key.split(" / ") {
        if display_width(alias) > width {
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            lines.extend(wrap_display(alias, width)?);
        } else if line.is_empty() {
            line.push_str(alias);
        } else if display_width(&line) + 3 + display_width(alias) <= width {
            line.push_str(" / ");
            line.push_str(alias);
        } else {
            lines.push(std::mem::take(&mut line));
            line.push_str(alias);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    Some(lines)
}

/// Wraps terminal text at grapheme boundaries while preferring word breaks.
fn wrap_display(text: &str, width: usize) -> Option<Vec<String>> {
    if width == 0 {
        return text.is_empty().then(|| vec![String::new()]);
    }
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut line_width = 0_usize;
    for word in text.split(' ') {
        let gap = usize::from(!line.is_empty());
        if !line.is_empty() && line_width + gap + display_width(word) > width {
            lines.push(std::mem::take(&mut line));
            line_width = 0;
        } else if gap == 1 {
            line.push(' ');
            line_width += 1;
        }
        for grapheme in Span::raw(word).styled_graphemes(ratatui::style::Style::default()) {
            let cell = display_width(grapheme.symbol);
            if cell > width {
                return None;
            }
            if line_width + cell > width {
                lines.push(std::mem::take(&mut line));
                line_width = 0;
            }
            line.push_str(grapheme.symbol);
            line_width += cell;
        }
    }
    lines.push(line);
    Some(lines)
}

#[cfg(test)]
mod tests;
