//! Sidebar reorder geometry published with a successful sidebar frame, and the preview it paints.

use crate::machine_picker::{SidebarSource, SidebarWindow};
use ratatui::layout::Rect;

/// Geometry and semantics published only with the successful sidebar frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SidebarReorderPaint {
    pub(crate) viewport: Rect,
    pub(crate) layer: super::HitLayer,
    pub(crate) body: Rect,
    pub(crate) footer: Rect,
    pub(crate) window: SidebarWindow,
    pub(crate) source: SidebarSource,
}

pub struct SidebarReorderDraw {
    pub(crate) grabbed: Option<Rect>,
    pub(crate) destination: Option<(Rect, Placement, String)>,
    pub(crate) footer: Rect,
    pub(crate) workspace: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Placement {
    Before,
    After,
    Into,
    Ungrouped,
    End,
}
impl From<Placement> for crate::localization::SidebarPlacementLabel {
    fn from(placement: Placement) -> Self {
        match placement {
            Placement::Before => Self::Before,
            Placement::After => Self::After,
            Placement::Into => Self::Into,
            Placement::Ungrouped => Self::Ungrouped,
            Placement::End => Self::End,
        }
    }
}
