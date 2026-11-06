//! The sidebar toggle and Help icon actions around the sidebar's fixed chrome.
//!
//! The toggle ends the sidebar's first fixed-action row while the sidebar is visible and leads the
//! Tab strip while it is hidden, so it stays on the first row in both states; Help ends the
//! Settings row. Each paints only when its complete extent fits beside the content it must not
//! cover, and publishes the existing sidebar visibility or shortcut-help action through global
//! chrome. On a touch-spaced list both take their five-cell size and a target one cell wider on
//! each side that spans their row's touch lines; the Mobile bar's toggle takes the same size.

use ratatui::{Frame, layout::Rect};

use super::controls::{
    Availability, ControlSurface, IconActionGlyph, IconActionSpec, PointerPresentation,
    TOUCH_ICON_ACTION_WIDTH, icon_action_rect, render_icon_action, render_touch_icon_action,
};
use crate::{
    client_view::{ClientView, PrefixAction},
    localization::{Locale, TooltipLabel},
    tui_hit_map::{
        GlobalChromeAction, HitLayer, HitMapBuilder, InteractionSpec, InteractionTarget,
        PrimaryContract, ProjectedPointer, TooltipContent,
    },
    tui_theme::{Role, Theme},
};

/// Blank cells between a fixed action and the icon action ending its row, the same separation a
/// fixed action keeps from its hint.
pub(super) const TRAILING_GAP: u16 = 2;
/// Blank strip cells between the hidden sidebar's toggle and the first Tab.
const STRIP_GAP: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Control {
    Hide,
    Show,
    Help,
    /// The Tab list's way back, which shares the visible sidebar toggle's glyph and place.
    CloseTabs,
}

impl Control {
    const fn glyph(self) -> IconActionGlyph {
        match self {
            Self::Hide | Self::CloseTabs => IconActionGlyph::HideSidebar,
            Self::Show => IconActionGlyph::ShowSidebar,
            Self::Help => IconActionGlyph::Help,
        }
    }

    const fn action(self) -> GlobalChromeAction {
        match self {
            Self::Hide => GlobalChromeAction::HideSidebar,
            Self::Show => GlobalChromeAction::RestoreSidebar,
            Self::Help => GlobalChromeAction::ShortcutHelp,
            Self::CloseTabs => GlobalChromeAction::CloseTabList,
        }
    }

    const fn label(self) -> TooltipLabel {
        match self {
            Self::Hide => TooltipLabel::HideSidebar,
            Self::Show => TooltipLabel::ShowSidebar,
            Self::Help => TooltipLabel::Shortcuts,
            Self::CloseTabs => TooltipLabel::HideTabs,
        }
    }

    /// The binding whose key the tooltip names: the same action from the keyboard.
    const fn binding(self) -> Option<PrefixAction> {
        match self {
            Self::Hide | Self::Show => Some(PrefixAction::SidebarToggle),
            Self::Help => Some(PrefixAction::ShowHelp),
            Self::CloseTabs => None,
        }
    }
}

/// What every control paints with: the shared view, copy, theme and hit layer.
pub(super) struct Context<'a> {
    pub(super) view: &'a ClientView,
    pub(super) locale: Locale,
    pub(super) theme: &'a Theme,
    pub(super) layer: HitLayer,
}

fn projected(
    hits: Option<&HitMapBuilder>,
    layer: HitLayer,
    target: Rect,
    control: Control,
) -> PointerPresentation {
    let interaction = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    hits.map_or(PointerPresentation::Rest, |hits| {
        match hits.projected_pointer(
            layer,
            target,
            target,
            &InteractionTarget::GlobalChrome(control.action()),
            interaction,
        ) {
            ProjectedPointer::Rest => PointerPresentation::Rest,
            ProjectedPointer::Hovered => PointerPresentation::Hovered,
            ProjectedPointer::Pressed => PointerPresentation::Pressed,
        }
    })
}

fn publish(hits: &mut HitMapBuilder, target: Rect, control: Control, context: &Context<'_>) {
    let Context {
        view,
        locale,
        theme,
        layer,
    } = *context;
    let interaction = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let action = InteractionTarget::GlobalChrome(control.action());
    hits.record_interaction(layer, target, target, action.clone(), interaction);
    hits.record_interaction_tooltip(
        layer,
        target,
        &action,
        TooltipContent::Action {
            label: locale.tooltip_label(control.label()).to_owned(),
            key: control
                .binding()
                .and_then(|binding| super::tooltip::binding_key(view, binding, theme)),
        },
    );
}

fn paint(
    frame: &mut Frame<'_>,
    rect: Rect,
    control: Control,
    surface: ControlSurface,
    context: &Context<'_>,
    hits: Option<&mut HitMapBuilder>,
) -> Option<Rect> {
    let pointer = projected(hits.as_deref(), context.layer, rect, control);
    let painted = render_icon_action(
        frame,
        rect,
        IconActionSpec::new(control.glyph(), surface, Availability::Enabled, pointer),
        context.theme,
    )?;
    if let Some(hits) = hits {
        publish(hits, painted, control, context);
    }
    Some(painted)
}

/// Where a touch-sized control sits: its content row and the lines its target spans.
#[derive(Clone, Copy, Debug)]
pub(super) struct TouchRow {
    pub(super) row: Rect,
    pub(super) top: u16,
    pub(super) height: u16,
}

/// Where a touch-sized control ending `row` paints its brackets, after `leading` and the
/// [`TRAILING_GAP`] it keeps, with one cell before it for its target; `None` when it drops.
pub(super) fn touch_trailing_x(row: Rect, leading: Option<Rect>) -> Option<u16> {
    let x = row
        .right()
        .checked_sub(TOUCH_ICON_ACTION_WIDTH)
        .filter(|&x| x > row.x)?;
    (!leading.is_some_and(|leading| x < leading.right().saturating_add(TRAILING_GAP))).then_some(x)
}

/// The touch-sized `control` ending `place.row`, a content row of a touch-spaced list, with a
/// target that also owns one cell beyond each bracket and every line of `place`. A press fills
/// all seven cells on the row; hover only brightens the glyph. Returns the five painted cells.
fn paint_touch_trailing(
    frame: &mut Frame<'_>,
    place: TouchRow,
    leading: Option<Rect>,
    control: Control,
    context: &Context<'_>,
    halos: &mut super::sidebar_touch::Halos,
    hits: Option<&mut HitMapBuilder>,
) -> Option<Rect> {
    let x = touch_trailing_x(place.row, leading)?;
    paint_touch(
        frame,
        place,
        x,
        control,
        ControlSurface::Chrome,
        context,
        halos,
        hits,
    )
}

/// The touch-sized `control` with its brackets from column `x` on `place.row`, its target one
/// cell wider on each side and spanning every line of `place`.
#[expect(
    clippy::too_many_arguments,
    reason = "one control paint takes its placement, surface and shared ports together"
)]
fn paint_touch(
    frame: &mut Frame<'_>,
    place: TouchRow,
    x: u16,
    control: Control,
    surface: ControlSurface,
    context: &Context<'_>,
    halos: &mut super::sidebar_touch::Halos,
    hits: Option<&mut HitMapBuilder>,
) -> Option<Rect> {
    let TouchRow { row, top, height } = place;
    let target = Rect::new(x.checked_sub(1)?, top, TOUCH_ICON_ACTION_WIDTH + 2, height);
    if frame.area().intersection(target) != target {
        return None;
    }
    let theme = context.theme;
    let pointer = projected(hits.as_deref(), context.layer, target, control);
    let band = Rect::new(target.x, row.y, target.width, 1);
    let fill = (pointer == PointerPresentation::Pressed).then_some(Role::SurfacePressed);
    if let Some(role) = fill {
        frame.buffer_mut().set_style(
            band,
            theme.content_on_surface(
                surface.role(),
                Some(role),
                None,
                theme.style(surface.role()),
            ),
        );
    }
    halos.fill(theme, band, fill);
    let painted = render_touch_icon_action(
        frame,
        Rect::new(x, row.y, TOUCH_ICON_ACTION_WIDTH, 1),
        IconActionSpec::new(control.glyph(), surface, Availability::Enabled, pointer),
        theme,
    )?;
    if let Some(hits) = hits {
        publish(hits, target, control, context);
    }
    Some(painted)
}

/// The Mobile bar's sidebar toggle at the bar's leading edge: at touch size, one cell in from the
/// edge with a target from the edge across every line of `place`; closed up, the plain toggle.
/// Returns the cells its target takes.
pub(super) fn render_bar_toggle(
    frame: &mut Frame<'_>,
    place: TouchRow,
    touch: bool,
    context: &Context<'_>,
    halos: &mut super::sidebar_touch::Halos,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    if touch {
        let x = place.row.x.checked_add(1)?;
        paint_touch(
            frame,
            place,
            x,
            Control::Show,
            ControlSurface::Strip,
            context,
            halos,
            Some(hits),
        )?;
        return Some(Rect::new(
            place.row.x,
            place.row.y,
            TOUCH_ICON_ACTION_WIDTH + 2,
            1,
        ));
    }
    let rect = icon_action_rect(frame.area(), place.row)?;
    paint(
        frame,
        rect,
        Control::Show,
        ControlSurface::Strip,
        context,
        Some(hits),
    )
}

/// The hidden sidebar's toggle at the leading end of `tabs`; `tabs` gives up its extent and the
/// gap after it. A strip too narrow for the whole control keeps every cell for its Tabs.
pub(super) fn take_strip_toggle(bounds: Rect, tabs: &mut Rect) -> Option<Rect> {
    let rect = icon_action_rect(bounds, *tabs)?;
    let taken = (rect.width + STRIP_GAP).min(tabs.width);
    tabs.x += taken;
    tabs.width -= taken;
    Some(rect)
}

/// Paints the toggle [`take_strip_toggle`] placed, over the Tab strip's surface.
pub(super) fn render_strip_toggle(
    frame: &mut Frame<'_>,
    rect: Rect,
    context: &Context<'_>,
    hits: &mut HitMapBuilder,
) {
    let painted = paint(
        frame,
        rect,
        Control::Show,
        ControlSurface::Strip,
        context,
        Some(hits),
    );
    debug_assert_eq!(painted, Some(rect));
}

/// Paints `control` at the trailing end of `row`, over the list's own columns. It never covers or
/// crowds `leading`, the action the row already painted: without [`TRAILING_GAP`] blank cells
/// after it, the control drops, as it does on a row narrower than itself.
fn paint_trailing(
    frame: &mut Frame<'_>,
    row: Rect,
    leading: Option<Rect>,
    control: Control,
    context: &Context<'_>,
    hits: Option<&mut HitMapBuilder>,
) -> Option<Rect> {
    let x = row.right().checked_sub(3).filter(|&x| x >= row.x)?;
    if leading.is_some_and(|leading| x < leading.right().saturating_add(TRAILING_GAP)) {
        return None;
    }
    paint(
        frame,
        Rect::new(x, row.y, 3, 1),
        control,
        ControlSurface::Chrome,
        context,
        hits,
    )
}

/// The visible sidebar's toggle at the trailing end of `row`, the first fixed-action row, after
/// the action painted into it, if any. Returns the painted extent so a shortcut hint on the same
/// row can end before it.
pub(super) fn render_sidebar_toggle(
    frame: &mut Frame<'_>,
    row: Rect,
    action: Option<Rect>,
    context: &Context<'_>,
    hits: Option<&mut HitMapBuilder>,
) -> Option<Rect> {
    paint_trailing(frame, row, action, Control::Hide, context, hits)
}

/// Help at the trailing end of `row`, the row Settings painted `settings` into; without Settings,
/// Help drops too.
pub(super) fn render_help(
    frame: &mut Frame<'_>,
    row: Rect,
    settings: Option<Rect>,
    context: &Context<'_>,
    hits: Option<&mut HitMapBuilder>,
) -> Option<Rect> {
    settings.and_then(|_| paint_trailing(frame, row, settings, Control::Help, context, hits))
}

/// The touch-spaced list's toggle: [`render_sidebar_toggle`] at touch size.
pub(super) fn render_touch_sidebar_toggle(
    frame: &mut Frame<'_>,
    place: TouchRow,
    action: Option<Rect>,
    context: &Context<'_>,
    halos: &mut super::sidebar_touch::Halos,
    hits: Option<&mut HitMapBuilder>,
) -> Option<Rect> {
    paint_touch_trailing(frame, place, action, Control::Hide, context, halos, hits)
}

/// The Tab list's way back at the trailing end of `place.row`, after `action`: touch-sized on a
/// touch-spaced list, else the plain three-cell control.
pub(super) fn render_close_tabs(
    frame: &mut Frame<'_>,
    place: TouchRow,
    action: Option<Rect>,
    touch: bool,
    context: &Context<'_>,
    halos: &mut super::sidebar_touch::Halos,
    hits: Option<&mut HitMapBuilder>,
) -> Option<Rect> {
    if touch {
        paint_touch_trailing(
            frame,
            place,
            action,
            Control::CloseTabs,
            context,
            halos,
            hits,
        )
    } else {
        paint_trailing(frame, place.row, action, Control::CloseTabs, context, hits)
    }
}

/// The touch-spaced list's Help: [`render_help`] at touch size.
pub(super) fn render_touch_help(
    frame: &mut Frame<'_>,
    place: TouchRow,
    settings: Option<Rect>,
    context: &Context<'_>,
    halos: &mut super::sidebar_touch::Halos,
    hits: Option<&mut HitMapBuilder>,
) -> Option<Rect> {
    settings.and_then(|_| {
        paint_touch_trailing(frame, place, settings, Control::Help, context, halos, hits)
    })
}

#[cfg(test)]
mod tests;
