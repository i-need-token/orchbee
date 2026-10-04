//! Native decoder timelines for release-only resize, Esc and prefix capture boundaries.
use super::*;
use crate::tui_layout::{DividerSource, DrawnDivider};
use crate::tui_pointer::{HitLayer, HitMapBuilder, HitTarget};
use orchbee_workbench_protocol::{
    ClientSplitRatio, PanelOrientation, SiteId, WorkbenchPanelId, WorkbenchPanelSlotId,
    WorkbenchRevision, WorkbenchSplitId, WorkbenchTabId, WorkbenchTerminalContextId,
};
use ratatui::layout::Rect;

struct Port {
    bytes: Vec<u8>,
    submissions: usize,
}
impl TerminalInputPort for Port {
    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn pointer_divider_ready(
        &mut self,
        _: &DrawnDivider,
    ) -> Result<(), crate::client_view::Notice> {
        Ok(())
    }
    fn pointer_divider_resize(&mut self, _: &DrawnDivider, _: ClientSplitRatio) {
        self.submissions += 1;
    }
}
fn fixture() -> (InputDispatcher, ClientView, Port) {
    let source = DividerSource {
        site: SiteId::parse(&format!("{:032x}", 1)).unwrap(),
        context: WorkbenchTerminalContextId::parse(&format!("{:032x}", 2)).unwrap(),
        tab: WorkbenchTabId::parse(&format!("{:032x}", 3)).unwrap(),
        focus: WorkbenchPanelId::parse(&format!("{:032x}", 4)).unwrap(),
        revision: WorkbenchRevision::new(1),
    };
    let target = DrawnDivider {
        source,
        viewport: Rect::new(0, 0, 120, 40),
        split: WorkbenchSplitId::parse(&format!("{:032x}", 5)).unwrap(),
        first: WorkbenchPanelSlotId::parse(&format!("{:032x}", 6)).unwrap(),
        second: WorkbenchPanelSlotId::parse(&format!("{:032x}", 7)).unwrap(),
        orientation: PanelOrientation::Horizontal,
        ratio: ClientSplitRatio::default(),
        area: Rect::new(0, 0, 101, 30),
        divider: Rect::new(50, 0, 1, 30),
    };
    let mut hits = HitMapBuilder::new();
    hits.record(
        HitLayer::Foreground,
        target.divider,
        HitTarget::Divider(target),
    );
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    let mut port = Port {
        bytes: Vec::new(),
        submissions: 0,
    };
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher.publish_hit_map(hits.finish());
    (dispatcher, view, port)
}

#[test]
fn divider_decoded_escape_cancels_without_forwarding_and_later_typing_reaches_the_pane() {
    let (mut dispatcher, mut view, mut port) = fixture();
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;51;6M\x1b[<32;61;6M",
    );
    assert!(dispatcher.divider_preview().is_some());
    assert_eq!(port.submissions, 0);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b");
    dispatcher
        .synchronize(
            &mut view,
            &mut port,
            Instant::now() + Duration::from_secs(1),
            &mut |_| Ok::<(), io::Error>(()),
        )
        .unwrap();
    assert!(dispatcher.divider_preview().is_none());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;61;6mx");
    assert_eq!(port.bytes, b"x");
    assert_eq!(port.submissions, 0);
}

#[test]
fn divider_decoded_release_is_once_and_prefix_shift_or_map_invalidation_cancels() {
    for ending in 0..4 {
        let (mut dispatcher, mut view, mut port) = fixture();
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            b"\x1b[<0;51;6M\x1b[<32;61;6M",
        );
        match ending {
            0 => {}
            1 => dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02?"),
            2 => dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<36;61;6M"),
            _ => dispatcher.invalidate_hit_map(&mut port),
        }
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            b"\x1b[<0;61;6m\x1b[<0;61;6m",
        );
        assert_eq!(port.submissions, usize::from(ending == 0));
        assert!(port.bytes.is_empty());
        assert!(dispatcher.divider_preview().is_none());
    }
}
