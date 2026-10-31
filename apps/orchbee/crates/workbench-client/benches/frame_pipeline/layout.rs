//! Shell layout through the public projection entries. Initialization is the startup geometry
//! query: a new `ClientView` for the window and the first pane's terminal geometry. Steady state
//! is the shell projection one drawn frame recomputes for a live view: shell areas, the single-pane
//! terminal area, and the split-tree root area. The panel-tree walk that places each leaf of a split
//! Tab has no public entry and is not included.

use crate::{
    decode_apply::SIZES,
    harness::{Case, Harness, Phase},
    obtf::Size,
};
use orchbee_workbench_client::application::{
    client_view::ClientView,
    tui_layout::{
        PaneFrame, active_pane_area, native_terminal_area, project_shell_areas, terminal_geometry,
    },
};

pub fn register(harness: &mut Harness) {
    for size in SIZES {
        initial(harness, size);
        frame(harness, size);
    }
}

fn initial(harness: &mut Harness, size: Size) {
    let window = size.window();
    harness.run(
        &Case {
            name: &format!("layout/init/{}", size.label()),
            phase: Phase::Init,
            bytes: 0,
        },
        || {
            let view = ClientView::new(window.columns);
            let geometry = terminal_geometry(window, &view)
                .expect("benchmark window projects")
                .expect("benchmark window presents a pane");
            (view, geometry)
        },
    );
}

fn frame(harness: &mut Harness, size: Size) {
    let view = ClientView::new(size.columns);
    let area = size.area();
    harness.run(
        &Case {
            name: &format!("layout/frame/{}", size.label()),
            phase: Phase::Steady,
            bytes: 0,
        },
        || {
            let areas = project_shell_areas(area, view.sidebar_surface());
            let pane = active_pane_area(&areas).expect("benchmark window presents a pane");
            let split = native_terminal_area(
                areas.panel,
                PaneFrame::of(areas.tier.shows_panel_frame(), 2, false),
            );
            (areas, pane, split)
        },
    );
}
