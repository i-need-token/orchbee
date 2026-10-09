//! Pure menu projection; the successful draw receipt alone arms input.
use crate::{
    action_menu::{MenuDraw, MenuView},
    localization::Locale,
    terminal_session::ModalPresentation,
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::Line,
    widgets::{Clear, Padding, Paragraph},
};

pub(super) fn render(
    frame: &mut Frame<'_>,
    content: Rect,
    menu: MenuView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> ModalPresentation<MenuDraw> {
    hits.start_action_menu();
    if content.width < 16 || content.height < 5 {
        return ModalPresentation::NotPresented;
    }
    let desired = menu
        .actions
        .iter()
        // One padding cell on each side, plus the frame.
        .enumerate()
        .map(|(index, a)| {
            Line::raw(action_label(locale, *a, menu.labels[index].as_deref())).width() + 4
        })
        .max()
        .unwrap_or(16);
    let width = u16::try_from(desired.max(26))
        .unwrap_or(u16::MAX)
        .min(content.width);
    let reason_rows = usize::from(menu.unavailable.iter().any(Option::is_some));
    let height = u16::try_from(menu.actions.len() + 3 + reason_rows)
        .unwrap_or(u16::MAX)
        .min(content.height);
    let (x, y) = menu.anchor.unwrap_or((
        content.x + (content.width - width) / 2,
        content.y + (content.height - height) / 2,
    ));
    let area = Rect::new(
        x.clamp(content.x, content.right() - width),
        y.clamp(content.y, content.bottom() - height),
        width,
        height,
    );
    let title = if menu.title.is_empty() {
        locale.menu_title().to_owned()
    } else {
        format!(
            "{} {} {}",
            locale.menu_title(),
            theme.glyphs.middle_dot,
            sanitize_terminal_text(menu.title)
        )
    };
    let block =
        super::overlay_surface(theme, true, Padding::horizontal(1)).title(truncate_display(
            &title,
            usize::from(area.width.saturating_sub(2)),
            theme.glyphs.ellipsis,
        ));
    let inner = block.inner(area);
    let rows = usize::from(inner.height).saturating_sub(1 + reason_rows);
    if rows == 0 {
        return ModalPresentation::NotPresented;
    }
    let first = menu.first_visible_row(rows);
    // A surviving wide lead outside the menu would make the terminal diff skip
    // its border. Remove only crossing glyph remnants, retaining their style.
    let buffer = frame.buffer_mut();
    for y in area.y..area.bottom() {
        let mut x = buffer.area.x;
        while x < area.right() {
            let cell = &buffer[(x, y)];
            let width = u16::try_from(display_width(cell.symbol()).max(1)).unwrap_or(u16::MAX);
            let next = x.saturating_add(width).min(buffer.area.right());
            if (x < area.x && next > area.x) || next > area.right() {
                // Continuations can carry Reset/Skip even when the lead has an
                // explicit background. Keep the visible lead style, not diff flags.
                let style = cell.style();
                for column in x..next {
                    if column < area.x || column >= area.right() {
                        let cell = &mut buffer[(column, y)];
                        cell.reset();
                        cell.set_style(style);
                    }
                }
            }
            x = next;
        }
    }
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
    for (offset, action) in menu.actions.iter().enumerate().skip(first).take(rows) {
        let row_area = Rect::new(
            inner.x - 1,
            inner.y + u16::try_from(offset - first).unwrap_or(0),
            inner.width + 2,
            1,
        );
        let selected = offset == menu.selected;
        let disabled = menu.unavailable[offset].is_some();
        let text = theme.style(if disabled {
            Role::TextDisabled
        } else {
            Role::TextPrimary
        });
        let mut style = if selected {
            theme.style(Role::SurfaceSelection).patch(text)
        } else {
            text
        };
        if selected && !disabled {
            style = style.add_modifier(Modifier::BOLD);
        }
        let marker = if selected
            && theme.style(Role::SurfaceSelection).bg == theme.style(Role::SurfaceOverlay).bg
        {
            ">"
        } else {
            " "
        };
        let label = format!(
            "{marker}{} ",
            truncate_display(
                &action_label(locale, *action, menu.labels[offset].as_deref()),
                usize::from(inner.width),
                theme.glyphs.ellipsis,
            ),
        );
        frame.render_widget(Paragraph::new(label).style(style), row_area);
        if !menu.transferring {
            hits.record(
                HitLayer::Overlay,
                row_area,
                HitTarget::ActionMenuRow {
                    token: menu.token,
                    row: offset,
                },
            );
        }
    }
    let position = format!("{}/{}", menu.selected + 1, menu.actions.len());
    let position_width = u16::try_from(position.len()).unwrap_or(u16::MAX);
    let hints_width = inner.width.saturating_sub(position_width + 1);
    let unavailable = menu.unavailable[menu.selected];
    if let Some(reason) = unavailable {
        let full = locale.menu_unavailable(reason, false);
        let reason = if Line::raw(full).width() <= usize::from(inner.width) {
            full
        } else {
            locale.menu_unavailable(reason, true)
        };
        frame.render_widget(
            Paragraph::new(reason).style(theme.style(Role::TextSecondary)),
            Rect::new(inner.x, inner.bottom() - 2, inner.width, 1),
        );
    }
    let footer = locale.menu_footer(
        unavailable.is_some()
            || usize::from(hints_width) < Line::raw(locale.menu_footer(false)).width(),
    );
    frame.render_widget(
        Paragraph::new(footer).style(theme.style(Role::HintKey)),
        Rect::new(inner.x, inner.bottom() - 1, hints_width, 1),
    );
    frame.render_widget(
        Paragraph::new(position).style(theme.style(Role::TextSecondary)),
        Rect::new(
            inner.right() - position_width,
            inner.bottom() - 1,
            position_width,
            1,
        ),
    );
    ModalPresentation::Presented(MenuDraw {
        token: menu.token,
        rows,
    })
}

fn action_label(
    locale: Locale,
    action: crate::action_menu::MenuAction,
    label: Option<&str>,
) -> String {
    label.map_or_else(
        || locale.menu_action(action).to_owned(),
        |label| {
            format!(
                "{} · {}",
                locale.menu_action(action),
                sanitize_terminal_text(if label.is_empty() {
                    locale.shell().unnamed_tab_group
                } else {
                    label
                })
            )
        },
    )
}
