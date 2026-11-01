//! Tooltip content a painted region offers, and the tooltip a candidate frame shows again.

use ratatui::{layout::Rect, style::Style};

use super::{FrameHitMap, HitLayer, HitRegion, HitRegionResolution, HitTarget};
use crate::tui_theme::Role;
use crate::workbench_state::PaneRef;

/// What offers one tooltip. A target that moves or changes what it does is a different key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Key {
    /// A recorded region, over its whole rectangle: its layer, where it is, and what it does.
    Region {
        layer: HitLayer,
        rect: Rect,
        target: HitTarget,
    },
    /// A glyph inside a recorded region, such as a row's state glyph: it explains itself only
    /// from its own cells, ahead of anything the whole region offers there.
    Part {
        layer: HitLayer,
        rect: Rect,
        region: Rect,
        target: HitTarget,
    },
    /// A pane title's painted text. It records no target of its own, so a press there still
    /// reaches the divider or dead frame beneath; only base-layer regions without a tooltip of
    /// their own let it through.
    PaneTitle { rect: Rect, pane: PaneRef },
}

impl Key {
    pub(crate) const fn new(layer: HitLayer, rect: Rect, target: HitTarget) -> Self {
        Self::Region {
            layer,
            rect,
            target,
        }
    }

    pub(crate) const fn part(layer: HitLayer, rect: Rect, region: Rect, target: HitTarget) -> Self {
        Self::Part {
            layer,
            rect,
            region,
            target,
        }
    }

    pub(crate) const fn pane_title(rect: Rect, pane: PaneRef) -> Self {
        Self::PaneTitle { rect, pane }
    }

    pub(crate) const fn rect(&self) -> Rect {
        match self {
            Self::Region { rect, .. } | Self::Part { rect, .. } | Self::PaneTitle { rect, .. } => {
                *rect
            }
        }
    }

    pub(crate) fn names(&self, region: &HitRegion) -> bool {
        match self {
            Self::Region {
                layer,
                rect,
                target,
            }
            | Self::Part {
                layer,
                region: rect,
                target,
                ..
            } => *layer == region.layer && *rect == region.rect && *target == region.target,
            Self::PaneTitle { .. } => false,
        }
    }

    /// Whether the pointer at this cell of `region` reads this tooltip.
    fn offers(&self, region: &HitRegion, column: u16, row: u16) -> bool {
        self.names(region)
            && match self {
                Self::Part { rect, .. } => super::contains(*rect, column, row),
                Self::Region { .. } | Self::PaneTitle { .. } => true,
            }
    }

    pub(crate) fn spans(&self, column: u16, row: u16) -> bool {
        matches!(self, Self::PaneTitle { rect, .. } if super::contains(*rect, column, row))
    }
}

/// What one tooltip says: an action and its key, or a cut name and the glyphs its target shows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TooltipContent {
    /// A glyph or bracket control's action, then the key that does the same to the same target now.
    Action { label: String, key: Option<String> },
    /// A cut name in full, then a legend of the target's glyphs. A legend follows the name.
    Describe {
        text: Option<TooltipText>,
        legend: Vec<LegendItem>,
    },
}

/// A complete name and where it may be cut when it outgrows the box.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TooltipText {
    pub(crate) text: String,
    /// Paths keep both ends; other names keep their start.
    pub(crate) middle: bool,
}

/// One glyph the target shows, in its own role and style, and the word for it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LegendItem {
    pub(crate) glyph: String,
    pub(crate) role: Role,
    pub(crate) style: Style,
    pub(crate) label: String,
}

/// The open tooltip one candidate frame paints: its target and the pointer cell it opened at.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Shown {
    pub(crate) key: Key,
    pub(crate) pointer: (u16, u16),
}

/// An open tooltip whose target this frame painted again, ready to place and paint.
#[derive(Clone, Copy, Debug)]
pub(crate) struct OpenTooltip<'a> {
    pub(crate) target: Rect,
    pub(crate) pointer: (u16, u16),
    pub(crate) content: &'a TooltipContent,
}

impl FrameHitMap {
    /// The tooltip key of the frontmost region at a cell, when that region offers one, else of a
    /// pane title over the base layer or dead space there. An open overlay resolves only its own
    /// regions, so nothing beneath it offers a tooltip.
    pub(crate) fn tooltip_at(&self, column: u16, row: u16) -> Option<Key> {
        let base = match self.hit_region(column, row) {
            HitRegionResolution::Region(region) => {
                if let Some(key) = self
                    .tooltips
                    .iter()
                    .find(|key| key.offers(region, column, row))
                {
                    return Some(key.clone());
                }
                region.layer == HitLayer::Foreground
            }
            HitRegionResolution::Dead => true,
            HitRegionResolution::OverlayBackdrop => false,
        };
        base.then(|| {
            self.tooltips
                .iter()
                .find(|key| key.spans(column, row))
                .cloned()
        })
        .flatten()
    }

    /// Whether this frame painted the target a shown tooltip belongs to.
    pub(crate) fn paints_tooltip_target(&self, shown: &Shown) -> bool {
        self.tooltips.contains(&shown.key)
            && (matches!(shown.key, Key::PaneTitle { .. })
                || self.regions.iter().any(|region| shown.key.names(region)))
    }
}

impl Shown {
    pub(crate) const fn key(&self) -> &Key {
        &self.key
    }

    pub(crate) const fn open<'a>(&self, content: &'a TooltipContent) -> OpenTooltip<'a> {
        OpenTooltip {
            target: self.key.rect(),
            pointer: self.pointer,
            content,
        }
    }
}
