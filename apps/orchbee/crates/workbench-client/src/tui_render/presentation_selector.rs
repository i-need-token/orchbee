//! Title-line geometry and exact member targets share one painter; selectors never overlap Pane
//! hits.
use super::{
    DisplayPhase, Frame, HitLayer, HitMapBuilder, Paragraph, Rect, Role, Theme,
    WorkbenchClientView,
    controls::{
        Availability, ControlSurface, IconActionGlyph, IconActionSpec, PointerPresentation,
        icon_action_rect, render_icon_action,
    },
};
use crate::{
    terminal_text::display_width,
    tui_hit_map::{InteractionSpec, InteractionTarget, PrimaryContract, ProjectedPointer},
    workbench_client::{ClientPanel, DrawnPresentationTarget},
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
        panel: &ClientPanel,
        native: &WorkbenchClientView<'_>,
    ) -> Option<Self> {
        if header.is_empty()
            || panel.presentation_count() < 2
            || !matches!(
                native.phase(),
                DisplayPhase::Ready | DisplayPhase::SelectedWithoutAttachment
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

    /// While a bare pane's selector is unavailable, its member position stays visible as inert
    /// text at the same strip end.
    pub(super) fn pending_count(
        strip: Rect,
        panel: &ClientPanel,
        native: &WorkbenchClientView<'_>,
    ) -> Option<(Rect, String)> {
        let total = panel.presentation_count();
        if total < 2 || native.retirement().is_some() {
            return None;
        }
        let position =
            panel.presentation_index(native.selected_presentation(panel).presentation_id())? + 1;
        let count = format!(" [{position}/{total}] ");
        let width = u16::try_from(display_width(&count)).ok()?;
        if strip.height == 0 || strip.width.checked_sub(width)? < 12 {
            return None;
        }
        Some((
            Rect::new(strip.right().checked_sub(width)?, strip.y, width, 1),
            count,
        ))
    }

    pub(super) fn paint(
        &self,
        frame: &mut Frame<'_>,
        surface: ControlSurface,
        theme: &Theme,
        hits: &mut HitMapBuilder,
    ) {
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
                IconActionSpec::new(glyph, surface, Availability::Enabled, pointer),
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
