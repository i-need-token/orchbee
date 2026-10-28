//! Pane graphics placed through the real fixed-viewport backend inside one native frame.

use std::sync::Arc;

use crate::{
    terminal_lifecycle::SynchronizedUpdateLease,
    tui_backend::{FixedViewportBackend, ResizableBackend},
    workbench_client::test_support,
};

#[test]
fn actual_native_shell_places_confirmed_pixels_inside_one_frame_and_clears_under_a_board() {
    test_support::actual_native_shell_places_confirmed_pixels_inside_one_frame_and_clears_under_a_board(
        |wire, dimensions, probe| {
            let mut backend =
                FixedViewportBackend::new(wire, dimensions, Arc::new(SynchronizedUpdateLease::new()));
            backend.enable_graphics_probe(probe);
            let graphics = backend
                .graphics_surface()
                .expect("an enabled probe owns a graphics surface");
            (backend, graphics)
        },
    );
}
