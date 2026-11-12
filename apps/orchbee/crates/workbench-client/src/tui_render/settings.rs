//! The full Settings page: a category column and the framed groups of the chosen category, one
//! column at a time on a narrow viewport. Only controls from the last successful frame authorize
//! input.
use crate::{
    client_view::{Binding, KeyLabelGlyphs, LabelStyle, format_binding_with_glyphs},
    localization::{IntegrationFactLabel, IntegrationFinding, Locale},
    native_settings::{
        Category, Choice, Control, Diagnosis, Entry, Group, Page, PageView, Pane, Row,
        SettingsDraw, View, footer_bindings,
    },
    terminal_text::{
        display_width, project_terminal_text, sanitize_terminal_text, truncate_display,
        truncate_display_start,
    },
    tui_hit_map::{
        HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, ListScrollTarget,
        PrimaryContract, ProjectedPointer,
    },
    tui_presentation::ModalPresentation,
    tui_theme::{GlyphMode, Role, Theme},
};
use orchbee_agent_integration::{InspectionDetail, IntegrationId};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Clear, Paragraph},
};

use super::controls::{KeyHint, QueryField, render_overlay_hints, render_query_field};

#[cfg(test)]
mod tests;

/// Below this width the page shows one column at a time.
pub(crate) const NARROW_WIDTH: u16 = 56;
/// The smallest viewport the page draws in; a smaller one shows the recovery notice instead.
const MINIMUM_WIDTH: u16 = 24;
const MINIMUM_HEIGHT: u16 = 8;
/// The right column's widest reading measure.
const CONTENT_WIDTH: u16 = 76;
/// Viewports at least this tall keep a blank row above both columns.
const PADDED_HEIGHT: u16 = 16;

/// A row's text starts this many cells into its group frame and ends this many before its edge.
const NAME_INSET: u16 = 4;
const END_INSET: u16 = 2;

/// The category column's width beside the rows of a page `width` cells wide.
fn nav_width(width: u16) -> u16 {
    (width.saturating_add(2) / 5).clamp(20, 26)
}

/// The width of the right column's groups in a region `width` cells wide.
fn column_width(width: u16, narrow: bool) -> u16 {
    if narrow {
        width.saturating_sub(2)
    } else {
        CONTENT_WIDTH.min(width.saturating_sub(4))
    }
}

/// The cells a row's description line has on a page `width` cells wide that shows two columns.
#[cfg(test)]
fn description_width(width: u16) -> usize {
    let region = width.saturating_sub(nav_width(width) + 1);
    usize::from(column_width(region, false).saturating_sub(NAME_INSET + END_INSET))
}

#[expect(clippy::too_many_arguments, reason = "the page's drawing inputs")]
pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    view: View<'_>,
    pending: bool,
    notice: bool,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> ModalPresentation<SettingsDraw> {
    let theme = &theme.paired_pointer_surfaces();
    if area.width < MINIMUM_WIDTH
        || area.height < MINIMUM_HEIGHT
        || frame.area().intersection(area) != area
    {
        return ModalPresentation::NotPresented;
    }
    hits.start_settings();
    frame.render_widget(Clear, area);
    frame
        .buffer_mut()
        .set_style(area, theme.style(Role::SurfaceCanvas));
    let body = Rect::new(area.x, area.y, area.width, area.height - 1);
    let status = Rect::new(
        area.x + 1,
        area.bottom() - 1,
        area.width.saturating_sub(2),
        1,
    );
    let draw = match view {
        View::Loading { token } => {
            let top = body.y + u16::from(body.height >= PADDED_HEIGHT);
            put(
                frame,
                body.x + 2,
                top,
                body.width.saturating_sub(4),
                locale.settings_title(),
                theme.style(Role::TextPrimary),
            );
            if pending {
                frame.render_widget(
                    Paragraph::new(super::modal::status_line(
                        locale.settings_pending(),
                        Role::StatusPending,
                        theme,
                    )),
                    Rect::new(body.x + 2, top + 2, body.width.saturating_sub(4), 1),
                );
            }
            SettingsDraw::surface(token)
        }
        View::Page(page) => render_page(frame, body, page, locale, theme, hits),
    };
    if !notice {
        render_hints(frame, status, view, locale, theme);
    }
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
    ModalPresentation::Presented(draw)
}

fn render_page(
    frame: &mut Frame<'_>,
    body: Rect,
    page: PageView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> SettingsDraw {
    let narrow = body.width < NARROW_WIDTH;
    let top = u16::from(body.height >= PADDED_HEIGHT);
    let (nav, content) = if narrow {
        if page.pane() == Pane::Categories && page.page() == Page::General {
            (Some(body), None)
        } else {
            (None, Some(body))
        }
    } else {
        let width = nav_width(body.width);
        let nav = Rect::new(body.x, body.y, width, body.height);
        let divider = Rect::new(body.x + width, body.y, 1, body.height);
        let glyph = theme.glyphs.vertical_rule;
        for y in divider.y..divider.bottom() {
            put(
                frame,
                divider.x,
                y,
                1,
                glyph,
                theme.style(Role::FrameDivider),
            );
        }
        let content = Rect::new(
            divider.right(),
            body.y,
            body.right().saturating_sub(divider.right()),
            body.height,
        );
        (Some(nav), Some(content))
    };
    if let Some(nav) = nav {
        render_nav(frame, nav, top, narrow, page, locale, theme, hits);
    }
    let visible = match content {
        Some(content) => render_content(frame, content, top, narrow, page, locale, theme, hits),
        None => 1,
    };
    page.presentation(visible, narrow)
}

/// The left column: the way back to the app, search, and the seven categories.
#[expect(
    clippy::too_many_arguments,
    reason = "one page column's drawing inputs"
)]
fn render_nav(
    frame: &mut Frame<'_>,
    nav: Rect,
    top: u16,
    narrow: bool,
    page: PageView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let base = if narrow {
        Role::SurfaceCanvas
    } else {
        Role::SurfaceChrome
    };
    frame.buffer_mut().set_style(nav, theme.style(base));
    let token = page.token();
    let x = nav.x + 1;
    let width = nav.width.saturating_sub(2);
    let mut y = nav.y + top;
    put(
        frame,
        x,
        y,
        width,
        locale.settings_title(),
        on(theme, base, None, Role::TextPrimary).add_modifier(ratatui::style::Modifier::BOLD),
    );
    y += 1;
    // Back to the app.
    let back = Rect::new(x, y, width, 1);
    let label = format!("{} {}", arrow(theme), locale.settings_back_button());
    paint_line_target(
        frame,
        back,
        &label,
        Role::TextSecondary,
        base,
        false,
        InteractionTarget::SettingsControl {
            token,
            control: Control::Close,
        },
        true,
        theme,
        hits,
    );
    y += 2;
    // Search.
    if y < nav.bottom() {
        let field = Rect::new(x, y, width, 1);
        let key = "/";
        let query = page.query();
        let field_state = if page.searching() {
            QueryField::Editing {
                label: key,
                query,
                cursor: page.search_cursor(),
            }
        } else if query.is_empty() {
            QueryField::Rest {
                key,
                prompt: locale.settings_search_prompt(),
            }
        } else {
            QueryField::Retained { label: key, query }
        };
        let target = InteractionTarget::SettingsControl {
            token,
            control: Control::Search,
        };
        let enabled = page.page() == Page::General && page.text().is_none();
        let spec = interaction_spec(enabled);
        let pointer = hits.projected_pointer(HitLayer::Overlay, field, field, &target, spec);
        if let Some(interaction) = pointer_surface(pointer) {
            fill(frame, field, base, Some(interaction), theme);
        }
        if render_query_field(frame, field, field_state, theme).is_some() {
            hits.record_interaction(HitLayer::Overlay, field, field, target, spec);
        }
    }
    y += 2;
    for category in Category::ALL {
        if y >= nav.bottom() {
            break;
        }
        let current = page.query().is_empty() && page.category() == category;
        let focused = current && page.pane() == Pane::Categories && !page.searching();
        let row = Rect::new(nav.x, y, nav.width, 1);
        let target = InteractionTarget::SettingsControl {
            token,
            control: Control::Category(category),
        };
        let spec = interaction_spec(true);
        let pointer = hits.projected_pointer(HitLayer::Overlay, row, row, &target, spec);
        let interaction = pointer_surface(pointer).or(if focused {
            Some(Role::SurfaceSelection)
        } else if current {
            Some(Role::SurfaceCurrent)
        } else {
            None
        });
        fill(frame, row, base, interaction, theme);
        if focused {
            put(
                frame,
                nav.x,
                y,
                1,
                theme.glyphs.navigation_marker,
                on(theme, base, interaction, Role::AccentFocus),
            );
        }
        put(
            frame,
            x + 1,
            y,
            width.saturating_sub(1),
            locale.settings_category_name(category),
            on(
                theme,
                base,
                interaction,
                if current {
                    Role::TextPrimary
                } else {
                    Role::TextSecondary
                },
            ),
        );
        hits.record_interaction(HitLayer::Overlay, row, row, target, spec);
        y += 1;
    }
}

/// The right column; returns how many list lines it drew.
#[expect(
    clippy::too_many_arguments,
    reason = "one page column's drawing inputs"
)]
fn render_content(
    frame: &mut Frame<'_>,
    region: Rect,
    top: u16,
    narrow: bool,
    page: PageView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> usize {
    let token = page.token();
    let width = column_width(region.width, narrow);
    let x = region.x + (region.width - width) / 2;
    let mut y = region.y + top;
    // The title: the category, search results, or a child page with its way back.
    let child = page.page().is_child();
    let title = match page.page() {
        Page::Profiles => locale
            .settings_group_name(Group::LaunchProfiles)
            .unwrap_or_default()
            .to_owned(),
        Page::Integrations => locale.integrations_title().to_owned(),
        Page::Diagnosis(id) => locale.agent_integration_name(id).to_owned(),
        Page::General if !page.query().is_empty() || page.searching() => {
            locale.settings_search_results().to_owned()
        }
        Page::General => locale.settings_category_name(page.category()).to_owned(),
    };
    let title_rect = Rect::new(x, y, width, 1);
    if child || narrow {
        paint_line_target(
            frame,
            title_rect,
            &format!("{} {title}", arrow(theme)),
            Role::TextPrimary,
            Role::SurfaceCanvas,
            false,
            InteractionTarget::SettingsControl {
                token,
                control: Control::Back,
            },
            true,
            theme,
            hits,
        );
    } else {
        put(frame, x, y, width, &title, theme.style(Role::TextPrimary));
    }
    y += 1;
    // The file line names a settings file the page cannot use; otherwise the line says what
    // keeps a page's controls from working, or the integration lane's state.
    if let Some(fact) = page.file_fact() {
        let full = locale.settings_file_fact(fact);
        let text = if display_width(&full) <= usize::from(width) {
            full
        } else {
            locale.settings_file_fact_compact(fact)
        };
        frame.render_widget(
            Paragraph::new(super::modal::status_line(&text, Role::StatusWarning, theme)),
            Rect::new(x, y, width, 1),
        );
    } else if page.page().is_integrations() {
        let (text, role) = integration_fact(page, locale);
        frame.render_widget(
            Paragraph::new(super::modal::status_line(text, role, theme)),
            Rect::new(x, y, width, 1),
        );
    } else if page.page() == Page::General
        && page.query().is_empty()
        && page.category() == Category::Themes
        && (theme.no_color() || page.reduced_color())
    {
        let text = if theme.no_color() {
            locale.settings_no_color().to_owned()
        } else {
            locale.settings_detail(crate::localization::SettingsDetail::Message(
                crate::localization::SettingsDetailKind::TruecolorRequired,
            ))
        };
        put(frame, x, y, width, &text, theme.style(Role::TextMuted));
    }
    y += 1;
    let detail = matches!(page.page(), Page::Profiles | Page::Integrations);
    let bottom = region.bottom().saturating_sub(u16::from(detail));
    let list = Rect::new(x, y, width, bottom.saturating_sub(y));
    if list.height == 0 || list.width < 8 {
        return 1;
    }
    let visible = usize::from(list.height);
    let first = page.first_visible(visible);
    let column = Column {
        area: list,
        first,
        visible,
    };
    if let Page::Diagnosis(id) = page.page() {
        render_entries(frame, column, page, locale, theme, hits);
        let below = Rect::new(x, list.y + 2, width, list.height.saturating_sub(2));
        if !below.is_empty() {
            render_diagnosis(frame, below, page, id, locale, theme);
        }
    } else {
        render_entries(frame, column, page, locale, theme, hits);
    }
    let total = total_lines(page);
    if total > visible {
        render_scrollbar(frame, list, region, first, visible, total, theme);
    }
    if detail && bottom < region.bottom() {
        let line = Rect::new(x, bottom, width, 1);
        let (text, role) = child_detail(page, locale);
        put(frame, x, bottom, width, &text, theme.style(role));
        let _ = line;
    }
    hits.record(
        HitLayer::OverlaySurface,
        list,
        HitTarget::ListScroll(ListScrollTarget::Settings(token)),
    );
    visible
}

fn total_lines(page: PageView<'_>) -> usize {
    page.entries()
        .iter()
        .enumerate()
        .map(|(index, entry)| page.height(*entry, index == 0))
        .sum()
}

/// The part of the right column that scrolls.
#[derive(Clone, Copy)]
struct Column {
    area: Rect,
    first: usize,
    visible: usize,
}

impl Column {
    fn y(self, line: usize) -> Option<u16> {
        (line >= self.first && line < self.first + self.visible)
            .then(|| self.area.y + u16::try_from(line - self.first).unwrap_or(0))
    }

    /// The drawn part of an entry's lines.
    fn rect(self, line: usize, height: usize) -> Option<Rect> {
        let start = line.max(self.first);
        let end = (line + height).min(self.first + self.visible);
        (start < end).then(|| {
            Rect::new(
                self.area.x,
                self.y(start).unwrap_or(self.area.y),
                self.area.width,
                u16::try_from(end - start).unwrap_or(1),
            )
        })
    }
}

struct Frames {
    top_left: &'static str,
    top_right: &'static str,
    bottom_left: &'static str,
    bottom_right: &'static str,
    horizontal: &'static str,
    vertical: &'static str,
}

const fn frames(theme: &Theme) -> Frames {
    match theme.glyphs.mode() {
        GlyphMode::Unicode => Frames {
            top_left: "┌",
            top_right: "┐",
            bottom_left: "└",
            bottom_right: "┘",
            horizontal: "─",
            vertical: "│",
        },
        #[cfg(any(test, feature = "test-support"))]
        GlyphMode::Simplified => Frames {
            top_left: "+",
            top_right: "+",
            bottom_left: "+",
            bottom_right: "+",
            horizontal: "-",
            vertical: "|",
        },
    }
}

const fn arrow(theme: &Theme) -> &'static str {
    match theme.glyphs.mode() {
        GlyphMode::Unicode => "←",
        #[cfg(any(test, feature = "test-support"))]
        GlyphMode::Simplified => "<",
    }
}

fn render_entries(
    frame: &mut Frame<'_>,
    column: Column,
    page: PageView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let entries = page.entries();
    let child = page.page().is_child();
    let mut line = 0;
    for (index, entry) in entries.iter().copied().enumerate() {
        let height = page.height(entry, index == 0);
        let gap = usize::from(index != 0);
        match entry {
            Entry::Heading(group) if child => {
                if let Some(y) = column.y(line + gap)
                    && let Some(name) = locale.settings_group_name(group)
                {
                    put(
                        frame,
                        column.area.x,
                        y,
                        column.area.width,
                        name,
                        theme.style(Role::TextSecondary),
                    );
                }
            }
            Entry::Heading(group) => {
                render_heading(frame, column, line + gap, group, page, locale, theme);
            }
            Entry::Setting(row) if child => {
                if let Some(rect) = column.rect(line, 1) {
                    render_child_row(frame, rect, row, page, locale, theme, hits);
                }
            }
            Entry::Setting(row) => {
                if let Some(rect) = column.rect(line, height) {
                    let first_drawn = column.y(line).is_some();
                    render_row(frame, rect, first_drawn, row, page, locale, theme, hits);
                }
            }
            Entry::Option(choice) => {
                if let Some(y) = column.y(line) {
                    render_option(frame, column.area, y, choice, page, locale, theme, hits);
                }
            }
            Entry::End => {
                if let Some(y) = column.y(line) {
                    let frames = frames(theme);
                    let width = usize::from(column.area.width.saturating_sub(2));
                    let text = format!(
                        "{}{}{}",
                        frames.bottom_left,
                        frames.horizontal.repeat(width),
                        frames.bottom_right
                    );
                    put(
                        frame,
                        column.area.x,
                        y,
                        column.area.width,
                        &text,
                        theme.style(Role::FrameUnfocused),
                    );
                }
            }
            Entry::ConfigFile => {
                if let Some(y) = column.y(line + 1) {
                    let label = locale.settings_config_file();
                    let text = match page.file_path() {
                        Some(path) => {
                            let lead = format!("{label} {} ", theme.glyphs.middle_dot);
                            let budget = usize::from(column.area.width.saturating_sub(1))
                                .saturating_sub(display_width(&lead));
                            format!("{lead}{}", fit_path(&sanitize_terminal_text(path), budget))
                        }
                        None => label.to_owned(),
                    };
                    put(
                        frame,
                        column.area.x + 1,
                        y,
                        column.area.width.saturating_sub(1),
                        &text,
                        theme.style(Role::TextMuted),
                    );
                }
            }
            Entry::NoResults => {
                if let Some(y) = column.y(line) {
                    put(
                        frame,
                        column.area.x + 1,
                        y,
                        column.area.width.saturating_sub(1),
                        locale.settings_no_results(),
                        theme.style(Role::TextMuted),
                    );
                }
            }
        }
        line += height;
    }
}

/// A group's top edge with its name, the in-use tag of a theme list, and its description line.
fn render_heading(
    frame: &mut Frame<'_>,
    column: Column,
    line: usize,
    group: Group,
    page: PageView<'_>,
    locale: Locale,
    theme: &Theme,
) {
    let frames = frames(theme);
    let area = column.area;
    let width = usize::from(area.width);
    if let Some(y) = column.y(line) {
        let frame_style = theme.style(Role::FrameUnfocused);
        let name = locale
            .settings_group_name(group)
            .map(|name| truncate_display(name, width.saturating_sub(6), theme.glyphs.ellipsis));
        let tag = (group.is_theme_list() && page.in_use(group, system_dark(theme)))
            .then(|| locale.settings_in_use());
        let mut used = 1;
        put(frame, area.x, y, 1, frames.top_left, frame_style);
        // A theme list whose slot the file sets carries the changed marker; Del on a line restores it.
        let slot = match group {
            Group::LightThemes => Some(Row::LightTheme),
            Group::DarkThemes => Some(Row::DarkTheme),
            _ => None,
        };
        if slot.is_some_and(|slot| page.changed(slot)) && width > 8 {
            put(frame, area.x + 1, y, 1, " ", frame_style);
            put(
                frame,
                area.x + 2,
                y,
                1,
                theme.glyphs.status_bullet,
                theme.style(Role::AccentAction),
            );
            used = 3;
        }
        if let Some(name) = &name {
            let text = format!(" {name} ");
            put(
                frame,
                area.x + u16::try_from(used).unwrap_or(0),
                y,
                area.width
                    .saturating_sub(u16::try_from(used + 1).unwrap_or(u16::MAX)),
                &text,
                theme.style(Role::TextPrimary),
            );
            used += display_width(&text);
        }
        let tag_width = tag.map_or(0, |tag| display_width(tag) + 2);
        let rule = width.saturating_sub(used + tag_width + 1);
        put(
            frame,
            area.x + u16::try_from(used).unwrap_or(0),
            y,
            u16::try_from(rule).unwrap_or(0),
            &frames.horizontal.repeat(rule),
            frame_style,
        );
        if let Some(tag) = tag
            && used + rule + tag_width < width
        {
            put(
                frame,
                area.x + u16::try_from(used + rule).unwrap_or(0),
                y,
                u16::try_from(tag_width).unwrap_or(0),
                &format!(" {tag} "),
                theme.style(Role::AccentAction),
            );
        }
        put(frame, area.right() - 1, y, 1, frames.top_right, frame_style);
    }
    if page.described(group)
        && let Some(y) = column.y(line + 1)
    {
        paint_sides(frame, area, y, theme);
        if let Some(description) = locale.settings_group_description(group) {
            put(
                frame,
                area.x + 2,
                y,
                area.width.saturating_sub(4),
                description,
                theme.style(Role::TextMuted),
            );
        }
    }
}

fn paint_sides(frame: &mut Frame<'_>, area: Rect, y: u16, theme: &Theme) {
    let frames = frames(theme);
    let style = theme.style(Role::FrameUnfocused);
    put(frame, area.x, y, 1, frames.vertical, style);
    put(frame, area.right() - 1, y, 1, frames.vertical, style);
}

/// Whether the host shows the dark slot, which a System appearance follows.
fn system_dark(theme: &Theme) -> bool {
    match theme.style(Role::SurfaceCanvas).bg {
        Some(Color::Rgb(red, green, blue)) => {
            u32::from(red) * 299 + u32::from(green) * 587 + u32::from(blue) * 114 < 128_000
        }
        _ => true,
    }
}

fn focused(page: PageView<'_>, row: Row) -> bool {
    page.pane() == Pane::Rows
        && !page.searching()
        && page.picker().is_none()
        && page.selected() == row
}

/// One setting inside its group's frame: name, effect and control on the first line, the
/// always-visible description, or the open text field, on the second.
#[expect(clippy::too_many_arguments, reason = "one row's drawing inputs")]
fn render_row(
    frame: &mut Frame<'_>,
    rect: Rect,
    first_drawn: bool,
    row: Row,
    page: PageView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let token = page.token();
    let enabled = page.enabled(row);
    let is_focused = focused(page, row);
    let target = InteractionTarget::SettingsRow { token, row };
    let spec = interaction_spec(enabled);
    let inner = Rect::new(
        rect.x + 1,
        rect.y,
        rect.width.saturating_sub(2),
        rect.height,
    );
    let pointer = hits.projected_pointer(HitLayer::Overlay, inner, inner, &target, spec);
    let interaction = pointer_surface(pointer).or(is_focused.then_some(Role::SurfaceSelection));
    let base = Role::SurfaceCanvas;
    for y in rect.y..rect.bottom() {
        paint_sides(frame, rect, y, theme);
    }
    fill(frame, inner, base, interaction, theme);
    let name_x = rect.x + NAME_INSET;
    let end = rect.right().saturating_sub(END_INSET);
    let pending = page.waits(row);
    let editing = is_focused && page.text().is_some();
    let content_role = if enabled {
        Role::TextPrimary
    } else {
        Role::TextDisabled
    };
    let style = |role| on(theme, base, interaction, role);
    if let Row::ThemeOption(theme_option) = row {
        if is_focused {
            put(
                frame,
                rect.x + 1,
                rect.y,
                1,
                theme.glyphs.navigation_marker,
                style(Role::AccentFocus),
            );
        }
        if theme_option.current(page.values()) {
            put(
                frame,
                rect.x + 2,
                rect.y,
                1,
                theme.glyphs.checked_marker,
                style(Role::AccentAction),
            );
        }
        put(
            frame,
            name_x,
            rect.y,
            end.saturating_sub(name_x),
            locale.settings_choice(Choice::ColorTheme(theme_option)),
            style(if pending {
                Role::StatusPending
            } else {
                content_role
            }),
        );
        // Painted after the row's fill so hover and focus never tint the theme's own colors.
        if let Some(swatch) = theme.swatch(theme_option.theme())
            && end >= name_x + SWATCH_WIDTH + 12
        {
            render_swatch(frame, end - SWATCH_WIDTH, rect.y, swatch);
        }
        hits.record_interaction(HitLayer::Overlay, inner, inner, target, spec);
        return;
    }
    let mut recorded_controls = Vec::new();
    // The restore action of a focused changed row sits beside the row's control, where the
    // Desktop puts it, and never takes cells from the name or the description: it moves to the
    // description line while both fit there whole, and is left to its key when neither line has
    // room.
    let mut restore = (is_focused && !editing && page.changed(row))
        .then(|| format!("[{}]", locale.settings_restore()));
    let restore_width = restore.as_ref().map_or(0, |label| display_width(label));
    let restore_target = || InteractionTarget::SettingsControl {
        token,
        control: Control::Restore(row),
    };
    if first_drawn {
        let y = rect.y;
        if is_focused {
            put(
                frame,
                rect.x + 1,
                y,
                1,
                theme.glyphs.navigation_marker,
                style(Role::AccentFocus),
            );
        }
        if page.changed(row) {
            put(
                frame,
                rect.x + 2,
                y,
                1,
                theme.glyphs.status_bullet,
                style(Role::AccentAction),
            );
        }
        let available = usize::from(end.saturating_sub(name_x));
        let control = control_text(row, page, locale, theme);
        let control_width = control
            .as_ref()
            .map_or(0, |control| display_width(&control.text).min(available / 2));
        let control_role = if pending {
            Role::StatusPending
        } else if enabled {
            Role::TextSecondary
        } else {
            Role::TextDisabled
        };
        let mut name = locale.settings_row(row).to_owned();
        if let Some(effect) = locale.settings_effect(row) {
            name = format!("{name} {} {effect}", theme.glyphs.middle_dot);
        }
        put(
            frame,
            name_x,
            y,
            u16::try_from(available.saturating_sub(control_width + 1)).unwrap_or(0),
            &name,
            style(content_role),
        );
        let control_span = if control_width == 0 {
            0
        } else {
            control_width + RESTORE_GAP
        };
        if display_width(&name) + RESTORE_GAP + restore_width + control_span <= available
            && let Some(label) = restore.take()
        {
            let x = end.saturating_sub(u16::try_from(control_span + restore_width).unwrap_or(0));
            recorded_controls.push(render_restore(
                frame,
                hits,
                (x, y),
                &label,
                restore_target(),
                interaction,
                theme,
            ));
        }
        if let Some(control) = control {
            let control_x = end.saturating_sub(u16::try_from(control_width).unwrap_or(0));
            let text = truncate_display(&control.text, control_width, theme.glyphs.ellipsis);
            put(
                frame,
                control_x,
                y,
                u16::try_from(control_width).unwrap_or(0),
                &text,
                style(control_role),
            );
            if control.adjust && display_width(&text) == display_width(&control.text) {
                let adjustment = page.adjustment(row);
                let stepper = |forward: bool| {
                    let width = 3;
                    let x = if forward {
                        end.saturating_sub(width)
                    } else {
                        control_x
                    };
                    (Rect::new(x, y, width, 1), forward)
                };
                for (step, forward) in [stepper(false), stepper(true)] {
                    let enabled = enabled
                        && !pending
                        && adjustment.is_some_and(|adjustment| {
                            if forward {
                                adjustment.forward
                            } else {
                                adjustment.backward
                            }
                        });
                    recorded_controls.push((
                        step,
                        InteractionTarget::SettingsControl {
                            token,
                            control: Control::Adjust { row, forward },
                        },
                        interaction_spec(enabled),
                    ));
                }
            }
        }
    }
    let second = if first_drawn { rect.y + 1 } else { rect.y };
    if second < rect.bottom() {
        let available = end.saturating_sub(name_x);
        if editing && let Some((text, cursor)) = page.text() {
            render_field(
                frame,
                Rect::new(name_x, second, available, 1),
                text,
                cursor,
                theme,
            );
        } else {
            // NO_COLOR, not the color depth it implies, is why a color setting cannot show; a
            // focused unavailable row says why it is unavailable.
            let description = if theme.no_color()
                && matches!(
                    row,
                    Row::Theme
                        | Row::ColorSource
                        | Row::TerminalTheme
                        | Row::Foreground
                        | Row::Background
                ) {
                locale.settings_no_color().to_owned()
            } else if is_focused && !enabled {
                locale.settings_detail(page)
            } else {
                locale.settings_description(row).to_owned()
            };
            put(
                frame,
                name_x,
                second,
                available,
                &description,
                style(Role::TextMuted),
            );
            if display_width(&description) + RESTORE_GAP + restore_width <= usize::from(available)
                && let Some(label) = restore.take()
            {
                let x = end.saturating_sub(u16::try_from(restore_width).unwrap_or(0));
                recorded_controls.push(render_restore(
                    frame,
                    hits,
                    (x, second),
                    &label,
                    restore_target(),
                    interaction,
                    theme,
                ));
            }
        }
    }
    for (rect, target, spec) in recorded_controls {
        hits.record_interaction(HitLayer::Overlay, rect, rect, target, spec);
    }
    hits.record_interaction(HitLayer::Overlay, inner, inner, target, spec);
}

/// Blank cells between a row's restore action and the text or control beside it.
const RESTORE_GAP: usize = 2;

/// Paints a changed row's restore action from `origin` and returns its pointer target.
fn render_restore(
    frame: &mut Frame<'_>,
    hits: &mut HitMapBuilder,
    origin: (u16, u16),
    label: &str,
    target: InteractionTarget,
    row_surface: Option<Role>,
    theme: &Theme,
) -> (Rect, InteractionTarget, InteractionSpec) {
    let width = u16::try_from(display_width(label)).unwrap_or(0);
    let button = Rect::new(origin.0, origin.1, width, 1);
    let spec = interaction_spec(true);
    let pointer = hits.projected_pointer(HitLayer::Overlay, button, button, &target, spec);
    let surface = pointer_surface(pointer).or(row_surface);
    put(
        frame,
        button.x,
        button.y,
        width,
        label,
        on(theme, Role::SurfaceCanvas, surface, Role::AccentAction),
    );
    (button, target, spec)
}

/// Cells of a theme swatch: a foreground sample on the theme canvas, then five ANSI fills.
const SWATCH_WIDTH: u16 = 14;

fn render_swatch(frame: &mut Frame<'_>, x: u16, y: u16, swatch: [Style; 6]) {
    let [sample, fills @ ..] = swatch;
    put(frame, x, y, 4, " Aa ", sample);
    for (offset, fill) in (0_u16..).zip(fills) {
        put(frame, x + 4 + offset * 2, y, 2, "  ", fill);
    }
}

struct ControlText {
    text: String,
    /// The text starts and ends with a one-step stepper.
    adjust: bool,
}

/// The control a row shows at its right edge.
fn control_text(
    row: Row,
    page: PageView<'_>,
    locale: Locale,
    theme: &Theme,
) -> Option<ControlText> {
    let value = locale.settings_value((page, row));
    let plain = |text: String| {
        Some(ControlText {
            text,
            adjust: false,
        })
    };
    match row {
        Row::LaunchProfiles | Row::AgentIntegrations => plain(
            format!("{value} {}", theme.glyphs.navigation_marker)
                .trim_start()
                .to_owned(),
        ),
        _ if page.checked(row).is_some() => plain(
            if page.checked(row) == Some(true) {
                "[x]"
            } else {
                "[ ]"
            }
            .to_owned(),
        ),
        _ if page.adjustment(row).is_some() => Some(ControlText {
            text: format!("[-] {value} [+]"),
            adjust: true,
        }),
        _ if !Choice::options(row).is_empty() => {
            plain(format!("{value} {}", theme.glyphs.disclosure_open))
        }
        _ if row.text_limit().is_some() => plain(if value.is_empty() {
            locale.settings_value(crate::localization::SettingsValue::Default)
        } else {
            sanitize_terminal_text(&value).into_owned()
        }),
        _ => (!value.is_empty()).then_some(ControlText {
            text: value,
            adjust: false,
        }),
    }
}

/// One option of the open list under its row; the candidate carries focus, the value in effect
/// carries the checked marker.
#[expect(clippy::too_many_arguments, reason = "one option's drawing inputs")]
fn render_option(
    frame: &mut Frame<'_>,
    area: Rect,
    y: u16,
    choice: Choice,
    page: PageView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let Some(picker) = page.picker() else {
        return;
    };
    let base = Role::SurfaceCanvas;
    let inner = Rect::new(area.x + 1, y, area.width.saturating_sub(2), 1);
    let target = InteractionTarget::SettingsControl {
        token: page.token(),
        control: Control::Choose(choice),
    };
    let spec = interaction_spec(true);
    let pointer = hits.projected_pointer(HitLayer::Overlay, inner, inner, &target, spec);
    let candidate = picker.candidate() == choice;
    let interaction = pointer_surface(pointer).or(candidate.then_some(Role::SurfaceSelection));
    paint_sides(frame, area, y, theme);
    fill(frame, inner, base, interaction, theme);
    let x = area.x + 6;
    if candidate {
        put(
            frame,
            area.x + 4,
            y,
            1,
            theme.glyphs.navigation_marker,
            on(theme, base, interaction, Role::AccentFocus),
        );
    }
    if picker.original() == choice {
        put(
            frame,
            x,
            y,
            1,
            theme.glyphs.checked_marker,
            on(theme, base, interaction, Role::AccentAction),
        );
    }
    put(
        frame,
        x + 2,
        y,
        area.right().saturating_sub(x + 4),
        locale.settings_choice(choice),
        on(theme, base, interaction, Role::TextPrimary),
    );
    hits.record_interaction(HitLayer::Overlay, inner, inner, target, spec);
}

/// One line of a child page: name at the left, value or field at the right.
fn render_child_row(
    frame: &mut Frame<'_>,
    rect: Rect,
    row: Row,
    page: PageView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let token = page.token();
    let enabled = page.enabled(row);
    let is_focused = focused(page, row);
    let target = InteractionTarget::SettingsRow { token, row };
    let spec = interaction_spec(enabled);
    let pointer = hits.projected_pointer(HitLayer::Overlay, rect, rect, &target, spec);
    let base = Role::SurfaceCanvas;
    let interaction = pointer_surface(pointer).or(is_focused.then_some(Role::SurfaceSelection));
    fill(frame, rect, base, interaction, theme);
    let style = |role| on(theme, base, interaction, role);
    if is_focused {
        put(
            frame,
            rect.x,
            rect.y,
            1,
            theme.glyphs.navigation_marker,
            style(Role::AccentFocus),
        );
    }
    let half = rect.width / 2;
    let name_role = if enabled {
        Role::TextPrimary
    } else {
        Role::TextDisabled
    };
    let mut controls = Vec::new();
    let mut end = rect.right();
    if let Row::Agent(id) = row {
        let label = locale.integrations_explain();
        let width = u16::try_from(display_width(label)).unwrap_or(0);
        if width + 4 < half {
            let button = Rect::new(end - width, rect.y, width, 1);
            let explain_target = InteractionTarget::SettingsControl {
                token,
                control: Control::Explain(id),
            };
            let explain_spec = interaction_spec(!page.integrations().pending());
            let pointer = hits.projected_pointer(
                HitLayer::Overlay,
                button,
                button,
                &explain_target,
                explain_spec,
            );
            put(
                frame,
                button.x,
                rect.y,
                width,
                label,
                on(
                    theme,
                    base,
                    pointer_surface(pointer).or(interaction),
                    Role::AccentAction,
                ),
            );
            controls.push((button, explain_target, explain_spec));
            end = button.x.saturating_sub(2);
        }
    }
    put(
        frame,
        rect.x + 2,
        rect.y,
        half.saturating_sub(3),
        locale.settings_row(row),
        style(name_role),
    );
    let value_x = rect.x + half;
    let value_width = end.saturating_sub(value_x);
    if is_focused && let Some((text, cursor)) = page.text() {
        render_field(
            frame,
            Rect::new(value_x, rect.y, value_width, 1),
            text,
            cursor,
            theme,
        );
    } else {
        let value = sanitize_terminal_text(&locale.settings_value((page, row))).into_owned();
        let value = match page.adjustment(row) {
            Some(_) => format!("< {value} >"),
            None => value,
        };
        let text = truncate_display(&value, usize::from(value_width), theme.glyphs.ellipsis);
        let width = u16::try_from(display_width(&text)).unwrap_or(0);
        let x = end.saturating_sub(width);
        put(
            frame,
            x,
            rect.y,
            value_width,
            &text,
            style(if enabled {
                Role::TextSecondary
            } else {
                Role::TextDisabled
            }),
        );
        if let Some(adjustment) = page.adjustment(row)
            && text == value
            && width >= 5
        {
            for (forward, step_x) in [(false, x), (true, end - 1)] {
                let step_enabled = enabled
                    && if forward {
                        adjustment.forward
                    } else {
                        adjustment.backward
                    };
                controls.push((
                    Rect::new(step_x, rect.y, 1, 1),
                    InteractionTarget::SettingsControl {
                        token,
                        control: Control::Adjust { row, forward },
                    },
                    interaction_spec(step_enabled),
                ));
            }
        }
    }
    for (rect, target, spec) in controls {
        hits.record_interaction(HitLayer::Overlay, rect, rect, target, spec);
    }
    hits.record_interaction(HitLayer::Overlay, rect, rect, target, spec);
}

/// The integration lane's state: a running job, the last failure, or what Orchbee changes.
fn integration_fact(page: PageView<'_>, locale: Locale) -> (&'static str, Role) {
    let integrations = page.integrations();
    let working = locale.integration_status(crate::localization::IntegrationStatus::Working);
    match (page.page(), integrations.failure()) {
        (Page::Diagnosis(id), _) => match integrations.diagnosis(id) {
            Some(Diagnosis::Pending) => (working, Role::StatusPending),
            Some(Diagnosis::Failed(failure)) => {
                (locale.integration_failure(*failure), Role::StatusError)
            }
            _ => (locale.integration_diagnosis_read_only(), Role::TextMuted),
        },
        (_, Some(failure)) => (locale.integration_failure(failure), Role::StatusError),
        _ if integrations.pending() => (working, Role::StatusPending),
        _ if !integrations.reported() => (locale.integrations_unchecked(), Role::TextMuted),
        _ => (locale.integrations_disclosure(), Role::TextMuted),
    }
}

/// The line under a child page's rows: a field rule, a confirmation, or why a row is unavailable.
fn child_detail(page: PageView<'_>, locale: Locale) -> (String, Role) {
    match page.page() {
        Page::Integrations => {
            let text = match page.selected() {
                Row::Agent(id) => match page.integrations().block(id) {
                    Some(block) => locale.integration_block(block).to_owned(),
                    None => {
                        locale.integration_provider_description(locale.agent_integration_name(id))
                    }
                },
                Row::RemoteHosts => locale.integrations_remote_description().to_owned(),
                Row::RefreshIntegrations if page.integrations().pending() => locale
                    .integration_block(crate::localization::IntegrationBlock::Pending)
                    .to_owned(),
                Row::RefreshIntegrations => locale.integrations_refresh_description().to_owned(),
                _ => locale.integrations_back_description().to_owned(),
            };
            (text, Role::TextMuted)
        }
        _ => {
            let role = if page.confirm_delete() {
                Role::StatusWarning
            } else if page.invalid() {
                Role::StatusError
            } else {
                Role::TextMuted
            };
            (locale.settings_detail(page), role)
        }
    }
}

/// One Agent's explain result: a definite finding is named as such, an unknown result says it
/// is unknown, and no path or command line is shown.
fn render_diagnosis(
    frame: &mut Frame<'_>,
    area: Rect,
    page: PageView<'_>,
    id: IntegrationId,
    locale: Locale,
    theme: &Theme,
) {
    let integrations = page.integrations();
    let mut lines: Vec<Line<'static>> = Vec::new();
    let wrap = |text: &str, lines: &mut Vec<Line<'static>>| {
        lines.extend(
            crate::terminal_text::wrap_text(&Text::from(text.to_owned()), area.width)
                .lines
                .into_iter()
                .map(|line| line.style(theme.style(Role::TextSecondary))),
        );
    };
    let fact = |label, value: &str| {
        Line::from(truncate_display(
            &locale.integration_fact(label, value),
            usize::from(area.width),
            theme.glyphs.ellipsis,
        ))
        .style(theme.style(Role::TextPrimary))
    };
    match integrations.diagnosis(id) {
        Some(Diagnosis::Pending) => lines.push(super::modal::status_line(
            locale.integration_diagnosis_pending(),
            Role::StatusPending,
            theme,
        )),
        Some(Diagnosis::Failed(failure)) => {
            lines.push(super::modal::status_line(
                locale.integration_finding(IntegrationFinding::Unknown(
                    crate::localization::IntegrationUnknown::Undetermined,
                )),
                Role::StatusWarning,
                theme,
            ));
            wrap(locale.integration_failure(*failure), &mut lines);
        }
        Some(Diagnosis::Explained(explanation)) => {
            let finding = explanation.finding();
            let role = match finding {
                IntegrationFinding::Ready => Role::StatusSuccess,
                IntegrationFinding::ManualSetup | IntegrationFinding::Disabled => Role::StatusInfo,
                IntegrationFinding::Conflict => Role::StatusError,
                IntegrationFinding::AgentMissing
                | IntegrationFinding::NotInstalled
                | IntegrationFinding::UpdateRequired
                | IntegrationFinding::Unknown(_) => Role::StatusWarning,
            };
            lines.push(super::modal::status_line(
                locale.integration_finding(finding),
                role,
                theme,
            ));
            wrap(locale.integration_finding_detail(finding), &mut lines);
            lines.push(Line::default());
            let item = &explanation.item;
            lines.push(fact(
                IntegrationFactLabel::Agent,
                locale.integration_presence(item.availability),
            ));
            lines.push(fact(
                IntegrationFactLabel::Integration,
                locale.integration_status(integrations.status(id)),
            ));
            if let Some(issue) = item.issue {
                lines.push(fact(
                    IntegrationFactLabel::Issue,
                    locale.integration_issue(issue),
                ));
            }
            lines.push(fact(
                IntegrationFactLabel::Adapter,
                locale.integration_adapter(explanation.adapter),
            ));
            match explanation.detail {
                InspectionDetail::Cli {
                    marketplace,
                    plugin_present,
                } => lines.extend([
                    fact(
                        IntegrationFactLabel::Marketplace,
                        locale.integration_marketplace(marketplace),
                    ),
                    fact(
                        IntegrationFactLabel::Plugin,
                        locale.integration_presence_flag(plugin_present),
                    ),
                ]),
                InspectionDetail::Codex {
                    marketplace,
                    plugin_present,
                } => lines.extend([
                    fact(
                        IntegrationFactLabel::Marketplace,
                        locale.integration_marketplace(Some(marketplace)),
                    ),
                    fact(
                        IntegrationFactLabel::Plugin,
                        locale.integration_presence_flag(Some(plugin_present)),
                    ),
                ]),
                InspectionDetail::Guarded {
                    target_state,
                    source_ready,
                    ..
                } => lines.extend([
                    fact(
                        IntegrationFactLabel::Entry,
                        locale.integration_entry(target_state),
                    ),
                    fact(
                        IntegrationFactLabel::AdapterSource,
                        locale.integration_source(source_ready),
                    ),
                ]),
                InspectionDetail::Plain {} => {}
            }
            lines.push(fact(
                IntegrationFactLabel::Plan,
                &locale.integration_plan(&explanation.plan),
            ));
        }
        None => lines.push(fact(
            IntegrationFactLabel::Integration,
            locale.integration_status(integrations.status(id)),
        )),
    }
    frame.render_widget(Paragraph::new(Text::from(lines)), area);
}

fn render_scrollbar(
    frame: &mut Frame<'_>,
    list: Rect,
    region: Rect,
    first: usize,
    visible: usize,
    total: usize,
    theme: &Theme,
) {
    let x = list.right();
    if x >= region.right() {
        return;
    }
    let height = usize::from(list.height);
    let thumb = (height * visible / total).clamp(1, height);
    let start = (height - thumb) * first / total.saturating_sub(visible).max(1);
    for offset in 0..height {
        let (glyph, role) = if offset >= start && offset < start + thumb {
            (theme.glyphs.scrollbar_thumb, Role::TextSecondary)
        } else {
            (theme.glyphs.scrollbar_track, Role::FrameDivider)
        };
        put(
            frame,
            x,
            list.y + u16::try_from(offset).unwrap_or(0),
            1,
            glyph,
            theme.style(role),
        );
    }
}

/// A one-line text field with its cursor; the text scrolls to keep the cursor visible.
fn render_field(frame: &mut Frame<'_>, area: Rect, text: &str, cursor: usize, theme: &Theme) {
    if area.width < 4 {
        return;
    }
    let style = theme.style(Role::AccentFocus);
    put(frame, area.x, area.y, 1, "[", style);
    put(frame, area.right() - 1, area.y, 1, "]", style);
    let value_width = usize::from(area.width - 2);
    let projection = project_terminal_text(text);
    let displayed = projection.display();
    let cursor_column = display_width(&displayed[..projection.cursor_boundary(cursor)]);
    let left = cursor_column.saturating_sub(value_width.saturating_sub(1));
    let value_area = Rect::new(area.x + 1, area.y, area.width - 2, 1);
    frame.render_widget(
        Paragraph::new(displayed.as_ref())
            .scroll((0, u16::try_from(left).unwrap_or(u16::MAX)))
            .style(
                theme
                    .style(Role::SurfaceField)
                    .patch(theme.style(Role::TextPrimary)),
            ),
        value_area,
    );
    frame.set_cursor_position((
        value_area.x + u16::try_from(cursor_column - left).unwrap_or(0),
        value_area.y,
    ));
}

/// The keys the focused row answers, local return first, on the page's status line. A row editor
/// holding a text the page refused names what it accepts there instead, until the text changes.
fn render_hints(frame: &mut Frame<'_>, area: Rect, view: View<'_>, locale: Locale, theme: &Theme) {
    if area.is_empty() {
        return;
    }
    if let View::Page(page) = view
        && let Some(requirement) = page.text_requirement()
    {
        let reason = locale.settings_text_requirement(&requirement);
        put(
            frame,
            area.x,
            area.y,
            area.width,
            &reason,
            theme.style(Role::StatusError),
        );
        return;
    }
    let resolved: Vec<(String, &str)> = footer_bindings(view)
        .iter()
        .map(|hint| {
            (
                format_settings_bindings(hint.bindings(), theme),
                locale.settings_hint_label(hint.label()),
            )
        })
        .collect();
    let hints: Vec<KeyHint<'_>> = resolved
        .iter()
        .map(|(key, label)| KeyHint::new(key, label))
        .collect();
    let _ = render_overlay_hints(frame, area, &hints, theme);
}

fn format_settings_bindings(bindings: impl Iterator<Item = Binding>, theme: &Theme) -> String {
    let key_glyphs = match theme.glyphs.mode() {
        GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
        #[cfg(any(test, feature = "test-support"))]
        GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
    };
    bindings
        .map(|binding| format_binding_with_glyphs(&binding, LabelStyle::for_host(), key_glyphs))
        .collect::<Vec<_>>()
        .join("/")
}

/// A whole-line control such as Back: painted, hovered and recorded at its exact extent.
#[expect(
    clippy::too_many_arguments,
    reason = "one line control's drawing inputs"
)]
fn paint_line_target(
    frame: &mut Frame<'_>,
    rect: Rect,
    label: &str,
    role: Role,
    base: Role,
    selected: bool,
    target: InteractionTarget,
    enabled: bool,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let width = u16::try_from(display_width(label)).unwrap_or(rect.width);
    let hit = Rect::new(rect.x, rect.y, width.min(rect.width), 1);
    let spec = interaction_spec(enabled);
    let pointer = hits.projected_pointer(HitLayer::Overlay, hit, hit, &target, spec);
    let interaction = pointer_surface(pointer).or(selected.then_some(Role::SurfaceSelection));
    put(
        frame,
        rect.x,
        rect.y,
        rect.width,
        label,
        on(theme, base, interaction, role),
    );
    hits.record_interaction(HitLayer::Overlay, hit, hit, target, spec);
}

fn interaction_spec(enabled: bool) -> InteractionSpec {
    if enabled {
        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
    } else {
        InteractionSpec::disabled(PrimaryContract::ActivateOnRelease)
    }
}

const fn pointer_surface(pointer: ProjectedPointer) -> Option<Role> {
    match pointer {
        ProjectedPointer::Rest => None,
        ProjectedPointer::Hovered => Some(Role::SurfaceHover),
        ProjectedPointer::Pressed => Some(Role::SurfacePressed),
    }
}

fn on(theme: &Theme, base: Role, interaction: Option<Role>, content: Role) -> Style {
    theme.content_on_surface(base, interaction, Some(content), theme.style(content))
}

fn fill(frame: &mut Frame<'_>, rect: Rect, base: Role, interaction: Option<Role>, theme: &Theme) {
    let style = on(theme, base, interaction, Role::TextPrimary);
    for y in rect.y..rect.bottom() {
        put(
            frame,
            rect.x,
            y,
            rect.width,
            &" ".repeat(usize::from(rect.width)),
            style,
        );
    }
}

/// Writes text in `width` cells. Text that does not fit is cut at a grapheme boundary and ends in
/// the ellipsis, measured in display cells.
fn put(frame: &mut Frame<'_>, x: u16, y: u16, width: u16, text: &str, style: Style) {
    let area = frame.area();
    if width == 0 || y >= area.bottom() || x >= area.right() {
        return;
    }
    let width = usize::from(width.min(area.right() - x));
    let text = truncate_display(text, width, ELLIPSIS);
    frame.buffer_mut().set_stringn(x, y, &text, width, style);
}

/// The standard ellipsis the presented page's glyph set draws.
const ELLIPSIS: &str = "\u{2026}";

/// A path in `budget` cells that keeps its file name: leading folders go first, behind the
/// ellipsis, and only a file name wider than the budget loses its start.
fn fit_path(path: &str, budget: usize) -> String {
    if display_width(path) <= budget {
        return path.to_owned();
    }
    path.match_indices(['/', '\\'])
        .map(|(index, _)| format!("{ELLIPSIS}{}", &path[index..]))
        .find(|candidate| display_width(candidate) <= budget)
        .unwrap_or_else(|| truncate_display_start(path, budget, ELLIPSIS))
}
