//! Header geometry and exact member targets share one painter; selectors never overlap Pane hits.
use super::*;
use crate::terminal_session::{NativePanel, state::PaneRef};
use orchbee_workbench_protocol::TerminalPresentationId;

pub(super) struct PresentationSelector {
    pub(super) area: Rect,
    count: String,
    pane: PaneRef,
    previous: TerminalPresentationId,
    next: TerminalPresentationId,
}
impl PresentationSelector {
    pub(super) fn new(
        header: Rect,
        panel: &NativePanel,
        native: &NativeSessionView<'_>,
    ) -> Option<Self> {
        if header.is_empty()
            || panel.presentation_count() < 2
            || native.phase() != NativeDisplayPhase::Ready
            || native.retirement().is_some()
        {
            return None;
        }
        let selected =
            panel.presentation_index(native.selected_presentation(panel).presentation_id())?;
        let count = format!("[{}/{}]", selected + 1, panel.presentation_count());
        // Leading gap, three-cell arrows, and two spaces around the count are all ASCII.
        let width = u16::try_from(count.len() + 9).ok()?;
        if header.width < width + 12 {
            return None;
        }
        let total = panel.presentation_count();
        Some(Self {
            area: Rect::new(header.right() - width, header.y, width, 1),
            count,
            pane: native.pane_ref_for_panel(panel.panel_id())?,
            previous: panel
                .presentation((selected + total - 1) % total)?
                .presentation_id()
                .clone(),
            next: panel
                .presentation((selected + 1) % total)?
                .presentation_id()
                .clone(),
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
        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled("[<]", theme.style(Role::HintKey)),
            Span::styled(
                format!(" {} ", self.count),
                theme.style(Role::TextSecondary),
            ),
            Span::styled("[>]", theme.style(Role::HintKey)),
        ]);
        frame.render_widget(
            Paragraph::new(line).style(theme.style(Role::SurfaceChrome)),
            self.area,
        );
        for (x, presentation) in [
            (self.area.x + 1, &self.previous),
            (self.area.right() - 3, &self.next),
        ] {
            hits.record(
                HitLayer::Foreground,
                Rect::new(x, self.area.y, 3, 1),
                HitTarget::Presentation {
                    pane: self.pane.clone(),
                    presentation: presentation.clone(),
                },
            );
        }
    }
}

/// The selector occupies the right end of the first header row; that whole strip is chrome.
pub(super) fn record_leaf(
    hits: &mut HitMapBuilder,
    leaf: Rect,
    content: Rect,
    pane: PaneRef,
    selector: Option<Rect>,
) {
    let target = HitTarget::Pane { pane, content };
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
        hits.record(
            HitLayer::Foreground,
            Rect::new(
                leaf.x,
                leaf.y.saturating_add(1),
                leaf.width,
                leaf.height.saturating_sub(1),
            ),
            target,
        );
    } else {
        hits.record(HitLayer::Foreground, leaf, target);
    }
}
