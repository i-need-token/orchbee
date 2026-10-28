//! Fixed target/path content for Tab menus; the existing menu owner handles the Back intent.

use super::super::controls::{
    ActionKind, ActionSpec, CheckedState, ControlSurface, PointerPresentation, action_rect,
    render_action,
};
use super::{interaction_spec, interaction_state, pointer_presentation, tab_title};
use crate::{
    action_menu::{MenuControl, MenuView},
    localization::Locale,
    terminal_text::display_width,
    tui_pointer::{HitLayer, HitMapBuilder, InteractionTarget},
    tui_theme::{Role, Theme},
};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

pub(super) struct TabHeader {
    area: Rect,
    label: Rect,
    back: Option<Rect>,
}

fn back_spec<'a>(
    label: &'a str,
    shortcut: &'a str,
    enabled: bool,
    pointer: PointerPresentation,
) -> ActionSpec<'a> {
    ActionSpec::new(
        label,
        ActionKind::Auxiliary,
        ControlSurface::Overlay,
        interaction_state(false, enabled, pointer, CheckedState::Unchecked),
    )
    .with_shortcut(shortcut)
}

pub(super) fn back_width(
    bounds: Rect,
    menu: &MenuView<'_>,
    locale: Locale,
    shortcut: &str,
    theme: &Theme,
) -> usize {
    if !menu.context_menu || menu.tabs.is_none() || !menu.has_parent {
        return 0;
    }
    action_rect(
        bounds,
        Rect::new(bounds.x, bounds.y, bounds.width, 1),
        back_spec(
            locale.menu_back_button(),
            shortcut,
            true,
            PointerPresentation::Rest,
        ),
        theme,
    )
    .map_or(usize::MAX, |rect| usize::from(rect.width) + 1)
}

impl TabHeader {
    /// A clipped return control or semantic path refuses the whole candidate before any hit exists.
    pub(super) fn layout(
        bounds: Rect,
        inner: Rect,
        menu: &MenuView<'_>,
        locale: Locale,
        shortcut: &str,
        theme: &Theme,
    ) -> Option<Self> {
        let tabs = menu.tabs?;
        let area = Rect::new(inner.x, inner.y, inner.width, 1);
        let back = if menu.has_parent {
            Some(action_rect(
                bounds,
                area,
                back_spec(
                    locale.menu_back_button(),
                    shortcut,
                    !menu.transferring,
                    PointerPresentation::Rest,
                ),
                theme,
            )?)
        } else {
            None
        };
        let left = back.map_or(area.x, |rect| rect.right().saturating_add(1));
        let label = Rect::new(left, area.y, area.right().checked_sub(left)?, 1);
        let path = locale.menu_tab_path(menu.source, tabs.scope, theme.glyphs.disclosure_closed);
        let minimum = display_width(&path)
            + display_width(theme.glyphs.middle_dot)
            + 2
            + display_width(theme.glyphs.ellipsis);
        if minimum > usize::from(label.width) {
            return None;
        }
        Some(Self { area, label, back })
    }

    pub(super) fn paint(
        &self,
        frame: &mut Frame<'_>,
        menu: &MenuView<'_>,
        locale: Locale,
        shortcut: &str,
        theme: &Theme,
        hits: &mut HitMapBuilder,
    ) {
        frame.render_widget(
            Paragraph::new(tab_title(
                menu,
                locale,
                theme,
                usize::from(self.label.width),
            ))
            .style(theme.style(Role::TextSecondary)),
            self.label,
        );
        if let Some(rect) = self.back {
            let target = InteractionTarget::ActionMenuControl {
                source: menu.source.clone(),
                token: menu.token,
                control: MenuControl::Back,
            };
            let enabled = !menu.transferring;
            let spec = interaction_spec(enabled);
            let pointer = pointer_presentation(hits.projected_pointer(
                HitLayer::Overlay,
                rect,
                rect,
                &target,
                spec,
            ));
            let painted = render_action(
                frame,
                self.area,
                back_spec(locale.menu_back_button(), shortcut, enabled, pointer),
                theme,
            )
            .expect("the Tab return action was admitted with the same geometry");
            debug_assert_eq!(painted, rect);
            if enabled {
                hits.record_interaction(HitLayer::Overlay, rect, painted, target, spec);
            }
        }
    }
}
