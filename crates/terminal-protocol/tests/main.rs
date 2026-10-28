//! Integration and conformance tests through this crate's public boundaries.

// The aggregate suite owns the shared corpus decoder once.
#[path = "support/encoded_fixture.rs"]
mod encoded_fixture;

mod attachment_control;
mod conformance;
mod connection_control;
mod creation_control;
mod frame_extensions;
mod frame_route;
mod graphics_decode;
mod history_control;
mod input_message;
mod inventory_control;
mod lifecycle_control;
mod process_info;
mod projection_buffer;
mod projection_demand_control;
mod projection_feedback_control;
mod render_row_window;
mod resize_control;
mod row_access_control;
mod row_window;
mod screen_control;
mod selection_control;
mod selection_export_chunk;
mod selection_export_control;
mod semantic_input;
mod terminal_configuration_control;
mod terminal_effect_event;
mod text_frame;
mod theme_control;
