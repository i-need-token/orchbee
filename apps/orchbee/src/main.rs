//! Native Client composition and invocation dispatch for CLI operations and retained-terminal
//! TUI sessions. Feature owners define command and interaction semantics; Server and Worker
//! retain shared Workbench authority and physical terminal execution.

mod agent_doctor;
mod agent_integrations;
mod agent_projection;
mod cli;
mod cli_outcome;
mod cli_text;
mod help;
mod invocation;
mod local_startup;
mod native_path;
mod presentation;
mod release_store;
mod startup_wait;
mod tui_backend;
mod tui_shell;

use orchbee_workbench_client::application::agent_provider;
use orchbee_workbench_client::application::agent_runtime;
use orchbee_workbench_client::application::agent_start;
use orchbee_workbench_client::application::background_thread;
use orchbee_workbench_client::application::client_view;
use orchbee_workbench_client::application::local_activation;
use orchbee_workbench_client::application::local_client;
use orchbee_workbench_client::application::localization;
use orchbee_workbench_client::application::machine_agents;
use orchbee_workbench_client::application::machine_discovery;
use orchbee_workbench_client::application::machine_observation;
use orchbee_workbench_client::application::machine_picker;
use orchbee_workbench_client::application::machine_spaces;
use orchbee_workbench_client::application::machine_ssh;
#[cfg(windows)]
use orchbee_workbench_client::application::machine_tunnel;
use orchbee_workbench_client::application::machines;
use orchbee_workbench_client::application::notifications;
use orchbee_workbench_client::application::onboarding;
use orchbee_workbench_client::application::terminal_buffer_text;
use orchbee_workbench_client::application::terminal_content;
use orchbee_workbench_client::application::terminal_copy;
use orchbee_workbench_client::application::terminal_device;
use orchbee_workbench_client::application::terminal_graphics;
use orchbee_workbench_client::application::terminal_host_appearance;
use orchbee_workbench_client::application::terminal_input;
use orchbee_workbench_client::application::terminal_lifecycle;
use orchbee_workbench_client::application::terminal_link_activation;
use orchbee_workbench_client::application::terminal_links;
use orchbee_workbench_client::application::terminal_projection;
use orchbee_workbench_client::application::terminal_text;
use orchbee_workbench_client::application::terminal_title;
use orchbee_workbench_client::application::termination_signal;
use orchbee_workbench_client::application::tui_capabilities;
use orchbee_workbench_client::application::tui_composition;
use orchbee_workbench_client::application::tui_graphics;
use orchbee_workbench_client::application::tui_layout;
use orchbee_workbench_client::application::tui_pointer;
use orchbee_workbench_client::application::tui_render;
use orchbee_workbench_client::application::tui_theme;
#[cfg(windows)]
use orchbee_workbench_client::application::windows_signal;
use orchbee_workbench_client::application::workbench_client;
use orchbee_workbench_client::application::worktree_destination;
use std::{
    env,
    io::{self, Write},
    process::ExitCode,
};

use invocation::{Invocation, parse_selected_request};
use localization::{Locale, select_locale};
use presentation::{ProcessResult, present};

// Keep the additive shell integration seam type-checked without making it an invocation route.
#[cfg(any(unix, windows))]
const _: fn(localization::Locale, bool) -> Result<(), tui_shell::TuiFailure> = tui_shell::run;

fn main() -> ExitCode {
    let selected = parse_selected_request(env::args_os().skip(1));
    if let Ok(selected) = &selected
        && let Some(output) = cli::invocation_route::offline_default_config(&selected.request)
    {
        // The canonical template is locale-neutral. Keep this sole offline route ahead of every
        // environment read, including locale selection.
        return emit(output, Locale::EnUs);
    }
    let locale = select_locale(|name| env::var_os(name));
    if let Ok(selected) = &selected
        && let Err(failure) = cli::invocation_route::admit(selected)
    {
        return emit(
            cli::failure_output(failure, selected.format, locale),
            locale,
        );
    }
    let parsed = selected.map(cli::invocation_route::prepare);
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    if let Ok(invocation::Request::Tui { mouse }) = &parsed {
        return match local_startup::run(locale, *mouse) {
            Ok(()) => ExitCode::SUCCESS,
            Err(failure) => emit(
                cli::process_result(
                    ProcessResult {
                        status: 1,
                        stdout: "",
                        stderr: locale.startup_failure(failure),
                    },
                    locale,
                ),
                locale,
            ),
        };
    }
    emit(
        match parsed {
            Err(failure) => cli::parse_failure(failure, locale),
            Ok(invocation::Request::Tui { .. }) => {
                cli::process_result(present(Ok(Invocation::Tui), locale), locale)
            }
            Ok(invocation::Request::MachineRoute(route)) => {
                cli::machines::routing::run(route, locale)
            }
            Ok(invocation::Request::MachineRelay) => cli::machines::routing::serve(locale),
            #[cfg(windows)]
            Ok(invocation::Request::MachineTunnel { cohort }) => cli::Output {
                status: if machine_tunnel::serve(&cohort).is_ok() {
                    0
                } else {
                    5
                },
                stdout: Vec::new(),
                stderr: String::new(),
                finalization: None,
            },
            Ok(invocation::Request::Machine { command, format }) => {
                cli::machines::run(&command, format, locale)
            }
            #[cfg(not(windows))]
            Ok(invocation::Request::MachineTunnel { .. }) => {
                cli::unsupported_platform(None, locale)
            }
            Ok(invocation::Request::Version) => {
                cli::process_result(present(Ok(Invocation::Version), locale), locale)
            }
            Ok(invocation::Request::RootHelp) => cli::root_help(locale),
            Ok(invocation::Request::Help(topic)) => cli::help(topic, locale),
            Ok(invocation::Request::Skill) => cli::skill(locale),
            Ok(invocation::Request::DefaultConfig) => cli::default_config(locale),
            Ok(invocation::Request::Completion(shell)) => cli::completion(shell, locale),
            Ok(invocation::Request::TerminalClear { target, format }) => {
                cli::clear_terminal(&target, format, locale)
            }
            Ok(invocation::Request::TerminalEditor {
                target,
                client,
                timeout_ms,
                format,
            }) => {
                cli::edit_terminal_scrollback(&target, client.as_ref(), timeout_ms, format, locale)
            }
            Ok(invocation::Request::TerminalLink { command, format }) => {
                cli::resolve_terminal_link(&command, format, locale)
            }
            Ok(invocation::Request::TerminalCopy { command, format }) => {
                cli::copy_terminal(&command, format, locale)
            }
            Ok(invocation::Request::TerminalSelection { command, format }) => {
                cli::read_terminal_selection(&command, format, locale)
            }
            Ok(invocation::Request::TerminalAttach { command }) => {
                cli::terminal_attach(&command, locale)
            }
            Ok(invocation::Request::TerminalControl { command }) => {
                cli::terminal_control(&command, locale)
            }
            Ok(invocation::Request::TerminalObserve { command }) => {
                cli::terminal_observe(&command, locale)
            }
            Ok(invocation::Request::ClientQuery {
                command,
                client,
                format,
            }) => cli::workbench_query::run(&command, Some(&client), format, locale),
            Ok(invocation::Request::Query { command, format }) => {
                cli::query(command, format, locale)
            }
            Ok(invocation::Request::Mutation { command, format }) => {
                cli::mutation(command, format, locale)
            }
            Ok(invocation::Request::PanelTerminalCreation { command, format }) => {
                cli::panel_terminal_creation::run_parsed(&command, format, locale)
            }
            Ok(invocation::Request::Creation { command, format }) => {
                cli::run_creation(&command, format, locale)
            }
            Ok(invocation::Request::PanelRelocation { command, format }) => {
                cli::panel_relocation::run(&command, format, locale)
            }
            Ok(invocation::Request::LayoutExport { command, format }) => {
                cli::layouts::export(&command, format, locale)
            }
            Ok(invocation::Request::LayoutApply { command, format }) => {
                cli::layouts::apply(&command, format, locale)
            }
            Ok(invocation::Request::WorkspaceCreation { command, format }) => {
                cli::workspace_creation::run(command, format, locale)
            }
            Ok(invocation::Request::ServerStart { format }) => cli::session_start(format, locale),
            Ok(invocation::Request::Plugin { command, format }) => {
                cli::plugins::run(command, format, locale)
            }
            Ok(invocation::Request::ConfiguredCommand { command, format }) => {
                cli::configured_commands::run(&command, format, locale)
            }
            Ok(invocation::Request::PaneGraphics { command, format }) => {
                cli::pane_graphics::run(&command, format, locale)
            }
            Ok(invocation::Request::Events { command, format }) => {
                cli::events::run(&command, format, locale)
            }
            Ok(invocation::Request::ChannelShow { format }) => cli::channel::show(format, locale),
            Ok(invocation::Request::Update { channel, format }) => {
                cli::update::run(channel, 600_000, format, locale)
            }
            Ok(invocation::Request::ConfigResetKeys { format }) => {
                cli::config_reset::run(format, locale)
            }
            Ok(invocation::Request::NamedSession { command, format }) => {
                cli::named_sessions::run(&command, format, locale)
            }
            Ok(invocation::Request::RuntimeStatus { scope, format }) => {
                cli::runtime_status::run(scope, format, locale)
            }
            Ok(invocation::Request::ServerStop { command, format }) => {
                cli::server_stop(&command, format, locale)
            }
            Ok(invocation::Request::SchemaExport {
                schema_id,
                path,
                format,
            }) => cli::export_schema(schema_id.as_deref(), &path, format, locale),
            Ok(invocation::Request::RetainedClose {
                family,
                operation_id,
                generation,
                format,
            }) => cli::operation::run_close(family, &operation_id, generation, format, locale),
            Ok(invocation::Request::ServerOperationStatus {
                operation_id,
                format,
            }) => cli::operation::run_server(&operation_id, format, locale),
            Ok(invocation::Request::Client { command, format }) => {
                cli::run_client_control(&command, format, locale)
            }
            Ok(invocation::Request::AgentView { action, format }) => {
                cli::agent_view_spec::run(&action, format, locale)
            }
            Ok(invocation::Request::PanelAgent { command, format }) => {
                cli::panel_agent::run(&command, format, locale)
            }
            Ok(invocation::Request::PanelMetadata { command, format }) => {
                cli::panel_metadata::run(&command, format, locale)
            }
            Ok(invocation::Request::WorkspaceMetadata { command, format }) => {
                cli::workspace_metadata::run(&command, format, locale)
            }
            Ok(invocation::Request::WorkspaceGroupClose { command, format }) => {
                cli::run_workspace_group_close(&command, format, locale)
            }
            Ok(invocation::Request::WorktreeQuery { command, format }) => {
                cli::run_worktree_query(command, format, locale)
            }
            Ok(invocation::Request::Worktree { command, format }) => {
                cli::run_worktree(command, format, locale)
            }
            Ok(invocation::Request::WindowTitle { title, format }) => {
                cli::window_title::run(title, format, locale)
            }
            Ok(invocation::Request::Notification { message, format }) => {
                cli::notification::run(message, format, locale)
            }
            Ok(invocation::Request::RuntimeConfiguration { action, format }) => {
                cli::runtime_configuration::run(action, format, locale)
            }
            Ok(invocation::Request::AgentManifests { command, format }) => {
                cli::agent_manifests::run(&command, format, locale)
            }
            Ok(invocation::Request::ServerDiagnose { format }) => {
                cli::server_diagnose(format, locale)
            }
            Ok(invocation::Request::AgentStart { command, format }) => {
                cli::run_agent_start(&command, format, locale)
            }
            Ok(invocation::Request::AgentNames { command, format }) => {
                cli::agent_names::run(command, format, locale)
            }
            Ok(invocation::Request::AgentIntegration { command, format }) => {
                cli::agent_integration(command, format, locale)
            }
        },
        locale,
    )
}

fn emit(result: cli::Output, locale: Locale) -> ExitCode {
    emit_with(
        result,
        locale,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    )
}

/// The shared emit body behind `emit`, writable over injected streams so the
/// write-failure and finalization ordering is provable in tests.
fn emit_with(
    result: cli::Output,
    locale: Locale,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> ExitCode {
    let cli::Output {
        status,
        stdout: out,
        stderr: err,
        finalization,
    } = result;
    let mut failed = false;
    if write_stream(&mut *stdout, &out).is_err() {
        // The prepared document failed at the OS write: a localized diagnostic
        // on stderr is the only remaining channel; no second document follows.
        let _ = write_stream(
            &mut *stderr,
            crate::cli_text::CliText::new(locale)
                .cli_output_failed()
                .as_bytes(),
        );
        failed = true;
    } else if write_stream(&mut *stderr, err.as_bytes()).is_err() {
        failed = true;
    }
    // The owed finalization is consumed on every path — normal, write
    // failure, and the output-failure diagnostic itself — so the inherited
    // signal mask stays blocked through the last write attempt and is then
    // restored exactly. A restore failure is a terminal internal failure.
    if let Some(token) = finalization
        && token.release().is_err()
    {
        let _ = write_stream(
            &mut *stderr,
            crate::cli_text::CliText::new(locale)
                .cli_failure(&cli::Failure::new(cli::FailureKind::InternalFailure))
                .as_bytes(),
        );
        return ExitCode::FAILURE;
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::from(status)
    }
}

fn write_stream(mut writer: impl Write, payload: &[u8]) -> io::Result<()> {
    if payload.is_empty() {
        return Ok(());
    }
    writer.write_all(payload)?;
    writer.flush()
}

#[cfg(test)]
mod tests;
