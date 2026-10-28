//! Workbench Client runtime for the native application: projected Workbench state, views,
//! notices, input dispatch, terminal presentation, Agent and machine workflows, and Client
//! communication lanes. The `orchbee` binary owns process entry, command-line grammar, and the
//! terminal shell loop; this crate never depends on them.

mod action_menu;
mod action_projection;
#[cfg(windows)]
pub mod agent_configuration_windows;
pub mod agent_integrations;
pub mod agent_provider;
pub mod agent_runtime;
pub mod agent_start;
mod client_context;
pub mod client_view;
mod host_capabilities;
pub mod local_activation;
pub mod local_client;
pub mod localization;
pub mod machine_agents;
pub mod machine_discovery;
pub mod machine_observation;
pub mod machine_picker;
pub mod machine_spaces;
pub mod machine_ssh;
pub mod machine_tunnel;
pub mod machines;
mod native_settings;
pub mod notifications;
pub mod onboarding;
mod startup_status;
pub mod terminal_buffer_text;
pub mod terminal_content;
pub mod terminal_copy;
mod terminal_copy_mode;
mod terminal_copy_query;
pub mod terminal_device;
mod terminal_events;
pub mod terminal_graphics;
pub mod terminal_host_appearance;
pub mod terminal_input;
pub mod terminal_lifecycle;
pub mod terminal_link_activation;
pub mod terminal_links;
mod terminal_live_view;
pub mod terminal_pointer_admission;
pub mod terminal_projection;
mod terminal_scrollbar;
pub mod terminal_text;
pub mod terminal_title;
pub mod termination_signal;
mod transient_notice;
pub mod tui_capabilities;
pub mod tui_composition;
pub mod tui_graphics;
pub mod tui_layout;
pub mod tui_pointer;
mod tui_presentation;
pub mod tui_render;
mod tui_shortcut_help;
pub mod tui_theme;
#[cfg(windows)]
mod windows_console_input;
#[cfg(windows)]
pub mod windows_signal;
pub mod workbench_client;
pub mod worktree_destination;
