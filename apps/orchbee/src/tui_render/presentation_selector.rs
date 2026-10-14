//! Header geometry and exact member targets share one painter; selectors never overlap Pane hits.
use super::{
    Frame, HitLayer, HitMapBuilder, HitTarget, NativeDisplayPhase, NativeSessionView, Paragraph,
    Rect, Role, Theme,
    controls::{
        Availability, ControlSurface, IconActionGlyph, IconActionSpec, PointerPresentation,
        icon_action_rect, render_icon_action,
    },
};
use crate::{
    terminal_session::{DrawnPresentationTarget, NativePanel},
    terminal_text::display_width,
    tui_pointer::{InteractionSpec, InteractionTarget, PrimaryContract, ProjectedPointer},
};

pub(super) struct PresentationSelector {
    pub(super) area: Rect,
    count: String,
    count_area: Rect,
    previous: (Rect, DrawnPresentationTarget),
    next: (Rect, DrawnPresentationTarget),
}
impl PresentationSelector {
    pub(super) fn new(
        header: Rect,
        panel: &NativePanel,
        native: &NativeSessionView<'_>,
    ) -> Option<Self> {
        if header.is_empty()
            || panel.presentation_count() < 2
            || !matches!(
                native.phase(),
                NativeDisplayPhase::Ready | NativeDisplayPhase::SelectedWithoutAttachment
            )
            || native.retirement().is_some()
        {
            return None;
        }
        let selected =
            panel.presentation_index(native.selected_presentation(panel).presentation_id())?;
        let count = format!("[{}/{}]", selected + 1, panel.presentation_count());
        let count_width = u16::try_from(display_width(&count)).ok()?;
        // Leading gap, two complete three-cell actions, and the spaces around
        // the count are all reserved before any selector cell is painted.
        let width = count_width.checked_add(9)?;
        if header.height == 0 || header.width.checked_sub(width)? < 12 {
            return None;
        }
        let area = Rect::new(header.right().checked_sub(width)?, header.y, width, 1);
        if header.intersection(area) != area {
            return None;
        }
        let previous_area =
            icon_action_rect(header, Rect::new(area.x.checked_add(1)?, area.y, 3, 1))?;
        let next_area = icon_action_rect(
            header,
            Rect::new(area.right().checked_sub(3)?, area.y, 3, 1),
        )?;
        let count_area = Rect::new(area.x.checked_add(5)?, area.y, count_width, 1);
        if header.intersection(count_area) != count_area
            || count_area.right().checked_add(1)? != next_area.x
        {
            return None;
        }
        let total = panel.presentation_count();
        Some(Self {
            area,
            count,
            count_area,
            previous: (
                previous_area,
                native.drawn_presentation_target(panel, (selected + total - 1) % total)?,
            ),
            next: (
                next_area,
                native.drawn_presentation_target(panel, (selected + 1) % total)?,
            ),
        })
    }

    pub(super) fn title_area(&self, header: Rect) -> Rect {
        Rect::new(
            header.x,
            header.y,
            self.area.x.saturating_sub(header.x),
            header.height,
        )
    }
    pub(super) fn paint(&self, frame: &mut Frame<'_>, theme: &Theme, hits: &mut HitMapBuilder) {
        if icon_action_rect(frame.area(), self.previous.0) != Some(self.previous.0)
            || icon_action_rect(frame.area(), self.next.0) != Some(self.next.0)
            || frame.area().intersection(self.count_area) != self.count_area
        {
            return;
        }
        frame.render_widget(
            Paragraph::new(self.count.as_str()).style(theme.style(Role::TextSecondary)),
            self.count_area,
        );
        for (glyph, (area, target)) in [
            (IconActionGlyph::Previous, &self.previous),
            (IconActionGlyph::Next, &self.next),
        ] {
            let interaction = InteractionTarget::Presentation(target.clone());
            let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
            let pointer = pointer_presentation(hits.projected_pointer(
                HitLayer::Foreground,
                *area,
                *area,
                &interaction,
                spec,
            ));
            let painted = render_icon_action(
                frame,
                *area,
                IconActionSpec::new(
                    glyph,
                    ControlSurface::Chrome,
                    Availability::Enabled,
                    pointer,
                ),
                theme,
            )
            .expect("preflighted presentation action remains paintable");
            debug_assert_eq!(painted, *area);
            hits.record_interaction(HitLayer::Foreground, *area, painted, interaction, spec);
        }
    }
}

fn pointer_presentation(pointer: ProjectedPointer) -> PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => PointerPresentation::Rest,
        ProjectedPointer::Hovered => PointerPresentation::Hovered,
        ProjectedPointer::Pressed => PointerPresentation::Pressed,
    }
}

/// The selector occupies the right end of the first header row; that whole strip is chrome.
pub(super) fn record_leaf(
    hits: &mut HitMapBuilder,
    leaf: Rect,
    target: HitTarget,
    selector: Option<Rect>,
) {
    let body = match &target {
        HitTarget::Pane { content, .. } => *content,
        _ => Rect::new(
            leaf.x,
            leaf.y.saturating_add(1),
            leaf.width,
            leaf.height.saturating_sub(1),
        ),
    };
    if let Some(selector) = selector {
        hits.record(
            HitLayer::Foreground,
            Rect::new(
                leaf.x,
                leaf.y,
                selector.x.saturating_sub(leaf.x),
                1.min(leaf.height),
            ),
            target.clone(),
        );
        hits.record(HitLayer::Foreground, body, target);
    } else {
        hits.record(
            HitLayer::Foreground,
            Rect::new(leaf.x, leaf.y, leaf.width, 1.min(leaf.height)),
            target.clone(),
        );
        hits.record(HitLayer::Foreground, body, target);
    }
}
