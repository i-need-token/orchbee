//! Painted Tab strip tokens as a drag sees them, and the drag preview a renderer paints.

use super::{FrameHitMap, HitResolution, HitTarget};
use ratatui::layout::Rect;

pub struct TabDragPreview {
    pub(crate) grabbed: Vec<Rect>,
    pub(crate) marker: Option<(u16, u16)>,
    /// A drag down the Tab list marks its drop with a line this wide from `marker`.
    pub(crate) marker_line: Option<u16>,
    pub(crate) count: usize,
}

/// Strip regions exposed at both ends: Tab and group tokens and the create control.
pub(crate) fn exposed_regions(map: &FrameHitMap) -> impl Iterator<Item = (Rect, &HitTarget)> {
    map.regions
        .iter()
        .filter(|region| {
            matches!(
                region.target,
                HitTarget::Tab(_)
                    | HitTarget::TabGroup { .. }
                    | HitTarget::TabCreate(_)
                    | HitTarget::TabCloseRequest { .. }
            ) && [region.rect.x, region.rect.right().saturating_sub(1)]
                .into_iter()
                .all(|x| {
                    matches!(
                        map.hit(x, region.rect.y),
                        HitResolution::Region(target) if target == &region.target
                    )
                })
        })
        .map(|region| (region.rect, &region.target))
}

/// The strip's tokens as a drag sees them. A Tab's close track joins its token whether the control
/// is revealed or hidden, so a pointer that reveals or hides it never changes the drag's geometry.
pub(crate) fn tab_regions(map: &FrameHitMap) -> Vec<(Rect, HitTarget)> {
    let mut tokens: Vec<(Rect, HitTarget)> = Vec::new();
    for (rect, target) in exposed_regions(map) {
        let target = match target {
            HitTarget::TabCloseRequest { tab, .. } => HitTarget::Tab(tab.clone()),
            target => target.clone(),
        };
        let adjacent = tokens.iter_mut().find(|(token, token_target)| {
            *token_target == target
                && matches!(target, HitTarget::Tab(_))
                && token.y == rect.y
                && (token.right() == rect.x || rect.right() == token.x)
        });
        match adjacent {
            Some((token, _)) => *token = token.union(rect),
            None => tokens.push((rect, target)),
        }
    }
    tokens
}
