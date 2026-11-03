//! The sidebar toggle and Help icon actions around the sidebar's fixed chrome.
//!
//! The toggle sits at the trailing end of the sidebar's top padding row while the sidebar is
//! visible and at the leading end of the Tab strip while it is hidden; Help shares the Settings
//! row. Each paints only when its complete extent fits beside the content it must not cover, and
//! publishes the existing sidebar visibility or shortcut-help action through global chrome.

use ratatui::{Frame, layout::Rect};

use super::controls::{
    Availability, ControlSurface, IconActionGlyph, IconActionSpec, PointerPresentation,
    icon_action_rect, render_icon_action,
};
use crate::{
    client_view::{ClientView, PrefixAction},
    localization::{Locale, TooltipLabel},
    tui_hit_map::{
        GlobalChromeAction, HitLayer, HitMapBuilder, InteractionSpec, InteractionTarget,
        PrimaryContract, ProjectedPointer, TooltipContent,
    },
    tui_theme::Theme,
};

/// Blank cells between Settings and Help, the same separation a fixed action keeps from its hint.
const SETTINGS_GAP: u16 = 2;
/// Blank strip cells between the hidden sidebar's toggle and the first Tab.
const STRIP_GAP: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Control {
    Hide,
    Show,
    Help,
}

impl Control {
    const fn glyph(self) -> IconActionGlyph {
        match self {
            Self::Hide => IconActionGlyph::HideSidebar,
            Self::Show => IconActionGlyph::ShowSidebar,
            Self::Help => IconActionGlyph::Help,
        }
    }

    const fn action(self) -> GlobalChromeAction {
        match self {
            Self::Hide => GlobalChromeAction::HideSidebar,
            Self::Show => GlobalChromeAction::RestoreSidebar,
            Self::Help => GlobalChromeAction::ShortcutHelp,
        }
    }

    const fn label(self) -> TooltipLabel {
        match self {
            Self::Hide => TooltipLabel::HideSidebar,
            Self::Show => TooltipLabel::ShowSidebar,
            Self::Help => TooltipLabel::Shortcuts,
        }
    }

    /// The binding whose key the tooltip names: the same action from the keyboard.
    const fn binding(self) -> PrefixAction {
        match self {
            Self::Hide | Self::Show => PrefixAction::SidebarToggle,
            Self::Help => PrefixAction::ShowHelp,
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

fn paint(
    frame: &mut Frame<'_>,
    rect: Rect,
    control: Control,
    surface: ControlSurface,
    context: &Context<'_>,
    hits: Option<&mut HitMapBuilder>,
) -> Option<Rect> {
    let Context {
        view,
        locale,
        theme,
        layer,
    } = *context;
    let target = InteractionTarget::GlobalChrome(control.action());
    let interaction = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let pointer = hits.as_deref().map_or(PointerPresentation::Rest, |hits| {
        match hits.projected_pointer(layer, rect, rect, &target, interaction) {
            ProjectedPointer::Rest => PointerPresentation::Rest,
            ProjectedPointer::Hovered => PointerPresentation::Hovered,
            ProjectedPointer::Pressed => PointerPresentation::Pressed,
        }
    });
    let painted = render_icon_action(
        frame,
        rect,
        IconActionSpec::new(control.glyph(), surface, Availability::Enabled, pointer),
        theme,
    )?;
    if let Some(hits) = hits {
        hits.record_interaction(layer, painted, painted, target.clone(), interaction);
        hits.record_interaction_tooltip(
            layer,
            painted,
            &target,
            TooltipContent::Action {
                label: locale.tooltip_label(control.label()).to_owned(),
                key: super::tooltip::binding_key(view, control.binding(), theme),
            },
        );
    }
    Some(painted)
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

/// The visible sidebar's toggle at the trailing end of `top`, the padding row above the fixed
/// actions over the list's own columns. A row narrower than the control drops it.
pub(super) fn render_sidebar_toggle(
    frame: &mut Frame<'_>,
    top: Rect,
    context: &Context<'_>,
    hits: Option<&mut HitMapBuilder>,
) {
    if let Some(x) = top.right().checked_sub(3)
        && x >= top.x
    {
        let _ = paint(
            frame,
            Rect::new(x, top.y, 3, 1),
            Control::Hide,
            ControlSurface::Chrome,
            context,
            hits,
        );
    }
}

/// Help at the trailing end of `row`, the row Settings painted `settings` into. Help never covers
/// or crowds Settings: without two blank cells after it, Help drops.
pub(super) fn render_help(
    frame: &mut Frame<'_>,
    row: Rect,
    settings: Option<Rect>,
    context: &Context<'_>,
    hits: Option<&mut HitMapBuilder>,
) {
    if let Some(settings) = settings
        && let Some(x) = row.right().checked_sub(3)
        && x >= settings.right().saturating_add(SETTINGS_GAP)
    {
        let _ = paint(
            frame,
            Rect::new(x, row.y, 3, 1),
            Control::Help,
            ControlSurface::Chrome,
            context,
            hits,
        );
    }
}

#[cfg(test)]
mod tests;
