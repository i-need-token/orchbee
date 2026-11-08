//! Integration and conformance tests through this crate's public boundaries.

// Aggregate suites share each fixture owner once; isolated targets retain their own owners.
#[path = "support/inherited_environment.rs"]
mod inherited_environment;
#[cfg(windows)]
#[path = "support/windows_agent_peer.rs"]
mod peer_support;
#[path = "support/staged_client.rs"]
mod staged_client;
#[cfg(target_os = "linux")]
mod support;
#[cfg(windows)]
#[path = "support/terminal_metadata_peer.rs"]
mod terminal_metadata_peer;

mod cli_mutation;
mod cli_query;
mod cli_send;
mod cli_windows_agent_doctor;
mod cli_windows_agent_explain;
mod cli_windows_agent_integration;
mod cli_windows_agent_manifests;
mod cli_windows_agent_prompt;
mod cli_windows_agent_runtime_get;
mod cli_windows_agent_view;
mod cli_windows_configuration;
mod cli_windows_configured_commands;
mod cli_windows_context;
mod cli_windows_creation_directory;
mod cli_windows_current_panel;
mod cli_windows_fixture_lifecycle;
mod cli_windows_input;
mod cli_windows_layout_members;
mod cli_windows_machine_routing;
mod cli_windows_machine_tunnel;
mod cli_windows_machines;
mod cli_windows_notifications;
mod cli_windows_pane_graphics;
mod cli_windows_panel_input;
mod cli_windows_panel_swap;
mod cli_windows_panel_terminals;
mod cli_windows_plugin_provision;
mod cli_windows_plugin_runtime;
mod cli_windows_plugins;
mod cli_windows_presentation_reports;
mod cli_windows_runtime_configuration;
mod cli_windows_runtime_status;
mod cli_windows_schema_export;
mod cli_windows_server_diagnose;
mod cli_windows_server_stop;
mod cli_windows_tab_index;
mod cli_windows_terminal_output;
mod cli_windows_terminal_run;
mod cli_windows_window_title;
mod cli_windows_workspace;
mod cli_windows_workspace_creation;
mod cli_windows_workspace_organization;
mod cli_windows_worktree_context;
mod cli_windows_worktree_query;
mod process;
mod tui_observer;
mod tui_process;
