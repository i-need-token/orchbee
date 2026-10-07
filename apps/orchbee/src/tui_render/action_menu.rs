//! Pure menu projection; the successful draw receipt alone arms input.
use crate::{
    action_menu::{MenuDraw, MenuView},
    localization::Locale,
    terminal_session::ModalPresentation,
    terminal_text::sanitize_terminal_text,
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{Block, Borders, Clear, Padding, Paragraph},
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
        .map(|a| Line::raw(locale.menu_action(*a)).width() + 4)
        .max()
        .unwrap_or(16);
    let width = u16::try_from(desired.max(26))
        .unwrap_or(u16::MAX)
        .min(content.width);
    let height = u16::try_from(menu.actions.len() + 4)
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
            "{} · {}",
            locale.menu_title(),
            sanitize_terminal_text(menu.title)
        )
    };
    let block = Block::new()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(1))
        .title(title)
        .style(
            theme
                .style(Role::SurfaceOverlay)
                .fg(theme.color(Role::TextPrimary)),
        )
        .border_style(theme.style(Role::AccentFocus));
    let inner = block.inner(area);
    let rows = usize::from(inner.height.saturating_sub(2));
    if rows == 0 {
        return ModalPresentation::NotPresented;
    }
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
    for (offset, action) in menu.actions.iter().enumerate().skip(menu.first).take(rows) {
        let row_area = Rect::new(
            inner.x,
            inner.y + u16::try_from(offset - menu.first).unwrap_or(0),
            inner.width,
            1,
        );
        let selected = offset == menu.selected;
        let disabled = menu.unavailable[offset].is_some();
        let text = theme.style(if disabled {
            Role::TextDisabled
        } else {
            Role::TextPrimary
        });
        let style = if selected {
            theme.style(Role::SurfaceSelection).patch(text)
        } else {
            text
        };
        let label = format!(
            "{}{}",
            if selected { "> " } else { "  " },
            locale.menu_action(*action)
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
