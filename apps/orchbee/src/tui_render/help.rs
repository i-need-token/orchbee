//! Shortcut-help geometry, effective binding discovery, and exact draw authority.

#[cfg(windows)]
use crate::client_view::PrefixDesc;
use crate::{
    client_view::{
        ClientView, KeyLabelGlyphs, LabelStyle, PrefixAction,
        help::{DrawnHelpControl, HelpControl, HelpDrawCandidate, HelpDrawSource},
        join_key_labels,
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
    icon_action_rect, measure_key_hints, render_icon_action, render_key_hints,
};

/// Rows the footer always owns: close/input semantics, then paging and position.
const FOOTER_ROWS: u16 = 2;
const MINIMUM_ROWS_PER_PAGE: u16 = 2;
const PREFERRED_WIDTH: u16 = 76;
const TITLE_CONTROL_GAP: u16 = 1;
const FOOTER_GAP: u16 = 1;

#[derive(Debug)]
struct OwnedHint {
    key: String,
    label: String,
}

impl OwnedHint {
    fn new(key: String, label: impl Into<String>) -> Self {
        Self {
            key,
            label: label.into(),
        }
    }

    fn borrowed(&self) -> KeyHint<'_> {
        KeyHint::new(self.key.as_str(), self.label.as_str())
    }
}

#[derive(Debug)]
struct FooterModel {
    top: Vec<OwnedHint>,
    paging: Vec<OwnedHint>,
    typing: Option<&'static str>,
}

#[derive(Debug)]
struct FooterPlan {
    top_hints: Option<Rect>,
    typing: Option<(Rect, String)>,
    position: Rect,
    paging_hints: Option<Rect>,
}

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
    candidate: HelpDrawCandidate,
}

impl<'a> RenderContext<'a> {
    pub(super) const fn new(
        text: ShellText<'a>,
        locale: Locale,
        view: &'a ClientView,
        candidate: HelpDrawCandidate,
    ) -> Self {
        Self {
            text,
            locale,
            view,
            candidate,
        }
    }
}

/// Paints a complete Help panel and publishes the exact episode/page source.
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
        candidate,
    } = context;
    let rows = text.configured_shortcut_rows(view);
    let height = u16::try_from(rows.len())
        .unwrap_or(u16::MAX)
        .saturating_add(FOOTER_ROWS)
        .saturating_add(2);
    let area = place_overlay(area, PREFERRED_WIDTH, height)?;
    if frame.area().intersection(area) != area {
        return None;
    }

    let [previous_rect, next_rect, close_rect] = title_control_rects(frame.area(), area)?;
    let block = help_block(area, previous_rect, text.shortcut_title, theme)?;
    let inner = block.inner(area);
    let rows_per_page = inner.height.saturating_sub(FOOTER_ROWS);
    if rows_per_page < MINIMUM_ROWS_PER_PAGE || inner.width == 0 {
        return None;
    }
    let rows_per_page = usize::from(rows_per_page);
    let width = usize::from(inner.width);

    // Each registry row owns a padded chord column. Continuations align below
    // its localized description, and no grapheme or logical row crosses pages.
    let row_lines = rows
        .iter()
        .map(|(chord, label)| {
            let indent = display_width(chord);
            if indent > width {
                return None;
            }
            let wrapped = wrap_display(label, width - indent)?;
            let mut lines = Vec::with_capacity(wrapped.len());
            let mut parts = wrapped.into_iter();
            lines.push(Line::from(vec![
                Span::styled(chord.clone(), theme.style(Role::HintKey)),
                Span::styled(
                    parts.next().unwrap_or_default(),
                    theme.style(Role::TextPrimary),
                ),
            ]));
            lines.extend(parts.map(|part| {
                Line::styled(
                    format!("{:<indent$}{part}", ""),
                    theme.style(Role::TextPrimary),
                )
            }));
            Some(lines)
        })
        .collect::<Option<Vec<_>>>()?;
    let pages = paginate_rows(
        &row_lines.iter().map(Vec::len).collect::<Vec<_>>(),
        rows_per_page,
    )?;
    let last = pages.len().checked_sub(1)?;
    let page = candidate.page().min(last);
    let source = candidate.presented(page, last)?;
    let (start, end) = pages[page];

    let [rows_area, footer_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(FOOTER_ROWS)]).areas(inner);
    let footer = footer_model(view, locale, text, key_label_glyphs(theme));
    let top_hints = footer
        .top
        .iter()
        .map(OwnedHint::borrowed)
        .collect::<Vec<_>>();
    let paging_hints = footer
        .paging
        .iter()
        .map(OwnedHint::borrowed)
        .collect::<Vec<_>>();
    let position = format!("{}/{}", page + 1, last + 1);
    let footer_plan = footer_plan(
        footer_area,
        &top_hints,
        &paging_hints,
        footer.typing,
        &position,
        theme,
    )?;

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

    let items = row_lines[start..end]
        .iter()
        .flatten()
        .map(|line| ListItem::new(line.clone()))
        .collect::<Vec<_>>();

    // All required geometry and text have been admitted above this boundary.
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    frame.render_widget(List::new(items), rows_area);
    if let Some(rect) = footer_plan.top_hints {
        let painted = render_key_hints(frame, rect, &top_hints, theme)
            .expect("the first complete Help hint was preflighted in this exact area");
        debug_assert_eq!(rect.intersection(painted), painted);
    }
    if let Some((rect, typing)) = footer_plan.typing {
        frame.render_widget(
            Paragraph::new(typing).style(theme.style(Role::TextSecondary)),
            rect,
        );
    }
    frame.render_widget(
        Paragraph::new(position).style(theme.style(Role::TextSecondary)),
        footer_plan.position,
    );
    if let Some(rect) = footer_plan.paging_hints {
        let painted = render_key_hints(frame, rect, &paging_hints, theme)
            .expect("the first complete Help paging hint was preflighted in this exact area");
        debug_assert_eq!(rect.intersection(painted), painted);
    }

    // Specific controls publish before the broad list/surface regions. Disabled
    // endpoints remain visible and frontmost but can never arm an activation.
    for control in [close, previous, next] {
        let painted = render_icon_action(frame, control.rect, control.visual, theme)
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
/// No page boundary or paging control is invented for this surface.
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
    if area.width < 3 {
        return None;
    }
    let close_offer = Rect::new(area.right().checked_sub(3)?, area.y, 3, 1);
    let close_rect = icon_action_rect(frame.area(), close_offer)
        .filter(|rect| area.intersection(*rect) == *rect)?;
    let source = candidate.refused();
    let close = control_plan(
        hits,
        source,
        HelpControl::Close,
        IconActionGlyph::Close,
        close_rect,
        true,
    );
    let notice_width = close_rect.x.saturating_sub(area.x);
    let notice_width = notice_width.saturating_sub(u16::from(notice_width > 0));
    let notice = truncate_display(
        text.overlay_refused,
        usize::from(notice_width),
        theme.glyphs.ellipsis,
    );

    frame.render_widget(Clear, area);
    frame
        .buffer_mut()
        .set_style(area, theme.style(Role::SurfaceOverlay));
    if notice_width > 0 {
        frame.render_widget(
            Paragraph::new(notice).style(theme.style(Role::TextSecondary)),
            Rect::new(area.x, area.y, notice_width, 1),
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
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
    hits.record_help_draw(source);
    Some(close_rect)
}

fn help_block(
    area: Rect,
    previous: Rect,
    title: &str,
    theme: &Theme,
) -> Option<ratatui::widgets::Block<'static>> {
    let title_budget = previous.x.checked_sub(area.x.checked_add(2)?)?;
    if title_budget < 3 {
        return None;
    }
    let title = truncate_display(
        title,
        usize::from(title_budget.saturating_sub(2)),
        theme.glyphs.ellipsis,
    );
    if display_width(&title) == 0 {
        return None;
    }
    let title = format!(" {title} ");
    if display_width(&title) > usize::from(title_budget) {
        return None;
    }
    Some(
        super::overlay_surface(theme, true, Padding::horizontal(2)).title(Span::styled(
            title,
            theme
                .style(Role::TextPrimary)
                .add_modifier(ratatui::style::Modifier::BOLD),
        )),
    )
}

fn title_control_rects(bounds: Rect, area: Rect) -> Option<[Rect; 3]> {
    let close_x = area.right().checked_sub(4)?;
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

fn footer_plan(
    area: Rect,
    top_hints: &[KeyHint<'_>],
    paging_hints: &[KeyHint<'_>],
    typing: Option<&str>,
    position: &str,
    theme: &Theme,
) -> Option<FooterPlan> {
    if area.height != FOOTER_ROWS || area.width == 0 {
        return None;
    }
    let top_row = Rect::new(area.x, area.y, area.width, 1);
    let paging_row = Rect::new(area.x, area.y.checked_add(1)?, area.width, 1);

    let typing = typing.map(|text| sanitize_terminal_text(text).into_owned());
    let typing_width = typing.as_deref().map(display_width).unwrap_or(0);
    if (typing.is_some() && typing_width == 0) || typing_width > usize::from(top_row.width) {
        return None;
    }
    let typing_rect = if typing.is_some() {
        let width = u16::try_from(typing_width).ok()?;
        Some(Rect::new(
            top_row.right().checked_sub(width)?,
            top_row.y,
            width,
            1,
        ))
    } else {
        None
    };

    let top_hints_rect = if top_hints.is_empty() {
        None
    } else {
        let _ = measure_key_hints(top_hints, theme)?;
        let end = typing_rect.map_or(top_row.right(), |rect| rect.x.saturating_sub(FOOTER_GAP));
        let rect = Rect::new(top_row.x, top_row.y, end.saturating_sub(top_row.x), 1);
        let first = measure_key_hints(&top_hints[..1], theme)?;
        (first <= usize::from(rect.width)).then_some(rect)?;
        Some(rect)
    };

    let position = sanitize_terminal_text(position).into_owned();
    let position_width = display_width(&position);
    if position_width == 0 || position_width > usize::from(paging_row.width) {
        return None;
    }
    let position_width = u16::try_from(position_width).ok()?;
    let position_rect = Rect::new(paging_row.x, paging_row.y, position_width, 1);
    let paging_hints_rect = if paging_hints.is_empty() {
        None
    } else {
        let _ = measure_key_hints(paging_hints, theme)?;
        let x = position_rect.right().checked_add(FOOTER_GAP)?;
        let rect = Rect::new(x, paging_row.y, paging_row.right().saturating_sub(x), 1);
        let first = measure_key_hints(&paging_hints[..1], theme)?;
        (first <= usize::from(rect.width)).then_some(rect)?;
        Some(rect)
    };

    Some(FooterPlan {
        top_hints: top_hints_rect,
        typing: typing_rect.zip(typing),
        position: position_rect,
        paging_hints: paging_hints_rect,
    })
}

fn footer_model(
    view: &ClientView,
    locale: Locale,
    text: ShellText<'_>,
    glyphs: KeyLabelGlyphs,
) -> FooterModel {
    #[cfg(not(windows))]
    let _ = locale;
    #[cfg(windows)]
    if view.machines.sidebar_navigation_active() {
        use crate::machine_picker::{SidebarCommand, sidebar_binding_label};
        let top = [
            (SidebarCommand::Help, text.help_close_label),
            (
                SidebarCommand::Exit,
                locale.sidebar_navigation_action(SidebarCommand::Exit),
            ),
        ]
        .into_iter()
        .filter_map(|(command, label)| {
            sidebar_binding_label(command, glyphs).map(|key| OwnedHint::new(key, label))
        })
        .collect();
        let paging = vec![OwnedHint::new(
            ClientView::sidebar_help_page_label(),
            text.help_pages_label,
        )];
        return FooterModel {
            top,
            paging,
            typing: None,
        };
    }

    #[cfg(windows)]
    if view.copy_mode_active() {
        let rows = view.copy_help_rows();
        let top = [
            (PrefixDesc::ShowHelp, text.help_close_label),
            (
                PrefixDesc::CopyExit,
                locale.copy_hint_label(crate::client_view::CopyHintLabel::Exit),
            ),
        ]
        .into_iter()
        .filter_map(|(desc, label)| registry_key(&rows, desc).map(|key| OwnedHint::new(key, label)))
        .collect();
        let paging = registry_key(&rows, PrefixDesc::CopyPage)
            .map(|key| vec![OwnedHint::new(key, text.help_pages_label)])
            .unwrap_or_default();
        return FooterModel {
            top,
            paging,
            typing: None,
        };
    }

    #[cfg(windows)]
    if view.resize_mode() {
        let rows = view.resize_help_rows_with_glyphs(glyphs);
        let top = [
            (PrefixDesc::ShowHelp, text.help_close_label),
            (
                PrefixDesc::ResizeExit,
                (text.prefix_desc)(PrefixDesc::ResizeExit),
            ),
        ]
        .into_iter()
        .filter_map(|(desc, label)| registry_key(&rows, desc).map(|key| OwnedHint::new(key, label)))
        .collect();
        let paging = registry_key(&rows, PrefixDesc::HelpPage)
            .map(|key| vec![OwnedHint::new(key, text.help_pages_label)])
            .unwrap_or_default();
        return FooterModel {
            top,
            paging,
            typing: None,
        };
    }

    let top = effective_action_label(view, PrefixAction::ShowHelp, glyphs)
        .map(|key| vec![OwnedHint::new(key, text.help_close_label)])
        .unwrap_or_default();
    let paging = [
        (PrefixAction::HelpPagePrevious, text.help_previous_label),
        (PrefixAction::HelpPageNext, text.help_next_label),
    ]
    .into_iter()
    .filter_map(|(action, label)| {
        effective_action_label(view, action, glyphs).map(|key| OwnedHint::new(key, label))
    })
    .collect();
    FooterModel {
        top,
        paging,
        typing: Some(text.help_typing_continues),
    }
}

#[cfg(windows)]
fn registry_key(rows: &[(String, PrefixDesc)], desc: PrefixDesc) -> Option<String> {
    rows.iter()
        .find(|(_, candidate)| *candidate == desc)
        .map(|(key, _)| sanitize_terminal_text(key).trim().to_owned())
        .filter(|key| display_width(key) > 0)
}

fn effective_action_label(
    view: &ClientView,
    action: PrefixAction,
    glyphs: KeyLabelGlyphs,
) -> Option<String> {
    let labels = view.bindings().action_labels_with_glyphs(action, glyphs);
    if labels.is_empty() {
        return None;
    }
    let label = join_key_labels(&labels, LabelStyle::for_host(), " / ");
    let label = sanitize_terminal_text(&label).into_owned();
    (display_width(&label) > 0).then_some(label)
}

const fn key_label_glyphs(theme: &Theme) -> KeyLabelGlyphs {
    match theme.glyphs.mode() {
        GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
        #[cfg(test)]
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

/// Groups whole logical rows into rendered-line budgets. A logical shortcut
/// never splits across pages, and an over-tall row rejects the candidate.
fn paginate_rows(row_lines: &[usize], rows_per_page: usize) -> Option<Vec<(usize, usize)>> {
    let mut pages = Vec::new();
    let mut start = 0;
    let mut used = 0;
    for (index, &lines) in row_lines.iter().enumerate() {
        if lines > rows_per_page {
            return None;
        }
        if used + lines > rows_per_page {
            pages.push((start, index));
            start = index;
            used = 0;
        }
        used += lines;
    }
    pages.push((start, row_lines.len()));
    Some(pages)
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
