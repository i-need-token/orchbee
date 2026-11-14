//! Native Client composition and invocation dispatch for CLI operations and retained-terminal
//! TUI sessions. Feature owners define command and interaction semantics; Server and Worker
//! retain shared Workbench authority and physical terminal execution.

mod agent_doctor;
mod agent_integrations;
mod agent_projection;
mod cli;
mod cli_outcome;
mod cli_text;
mod diagnostic_events;
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
use orchbee_workbench_client::application::machine_refusal;
use orchbee_workbench_client::application::machine_spaces;
use orchbee_workbench_client::application::machine_ssh;
use orchbee_workbench_client::application::machines;
use orchbee_workbench_client::application::notifications;
use orchbee_workbench_client::application::terminal_copy;
use orchbee_workbench_client::application::terminal_device;
use orchbee_workbench_client::application::terminal_graphics;
use orchbee_workbench_client::application::terminal_host_appearance;
use orchbee_workbench_client::application::terminal_input;
use orchbee_workbench_client::application::terminal_lifecycle;
use orchbee_workbench_client::application::terminal_projection;
use orchbee_workbench_client::application::terminal_text;
use orchbee_workbench_client::application::terminal_title;
use orchbee_workbench_client::application::terminal_viewport;
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
use presentation::present;

// Keep the additive shell integration seam type-checked without making it an invocation route.
#[cfg(any(unix, windows))]
const _: fn(localization::Locale, bool) -> Result<(), tui_shell::TuiFailure> = tui_shell::run;

/// Whether a request reads or writes the data root, so its layout must be admitted first. Help,
/// the version, the skill text, completion, schemas, the default settings template and the release
/// store's install and completion read no file in it.
fn opens_data_root(request: &invocation::Request) -> bool {
    !matches!(
        request,
        invocation::Request::Version
            | invocation::Request::RootHelp
            | invocation::Request::Help(_)
            | invocation::Request::Skill
            | invocation::Request::Withheld(_)
            | invocation::Request::DefaultConfig
            | invocation::Request::Completion(_)
            | invocation::Request::SchemaExport { .. }
            | invocation::Request::Install { .. }
            | invocation::Request::CompleteInstall { .. }
    )
}

fn main() -> ExitCode {
    // `ssh` starts the askpass helper with a prompt for its one argument, never a command line.
    #[cfg(windows)]
    if let Some(status) = cli::machines::askpass::run_if_selected() {
        return status;
    }
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
    let format = selected.as_ref().ok().and_then(|selected| selected.format);
    let parsed = selected.map(cli::invocation_route::prepare);
    if parsed.as_ref().is_ok_and(opens_data_root)
        && let Some(refusal) = cli::data_root_layout_refusal(format, locale)
    {
        return emit(refusal, locale);
    }
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    if let Ok(invocation::Request::Tui { mouse }) = &parsed {
        diagnostic_events::tui_started();
        let outcome = local_startup::run(locale, *mouse);
        diagnostic_events::tui_exited(&outcome);
        return match outcome {
            Ok(()) => ExitCode::SUCCESS,
            Err(failure) => emit(cli::startup_failure(&failure, locale), locale),
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
            Ok(invocation::Request::MachineStop(stop)) => {
                cli::machines::routing::stop::run(&stop, locale)
            }
            Ok(invocation::Request::MachineHub) => cli::machines::hub::serve(locale),
            Ok(invocation::Request::MachineHubState) => cli::machines::hub::print_state(locale),
            Ok(invocation::Request::MachineHubHold) => cli::machines::hub::hold(locale),
            Ok(invocation::Request::MachineLink) => cli::machines::link::serve(locale),
            Ok(invocation::Request::Machine { command, format }) => {
                cli::machines::run(&command, format, locale)
            }
            Ok(invocation::Request::Version) => {
                cli::process_result(present(Ok(Invocation::Version), locale), locale)
            }
            Ok(invocation::Request::RootHelp) => cli::root_help(locale),
            Ok(invocation::Request::Help(topic)) => cli::help(topic, locale),
            Ok(invocation::Request::Skill) => cli::skill(locale),
            // Admission refuses a withheld surface before dispatch; this keeps the same answer.
            Ok(invocation::Request::Withheld(feature)) => {
                cli::failure_output(cli::invocation_route::withheld(feature), None, locale)
            }
            Ok(invocation::Request::DefaultConfig) => cli::default_config(locale),
            Ok(invocation::Request::Completion(shell)) => cli::completion(shell, locale),
            Ok(invocation::Request::TerminalClear { target, format }) => {
                cli::clear_terminal(&target, format, locale)
            }
            Ok(invocation::Request::LayoutHistory { format }) => {
                cli::layout_history(format, locale)
            }
            Ok(invocation::Request::LayoutReopen { saved_at, format }) => {
                cli::layout_reopen(saved_at, format, locale)
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
            Ok(invocation::Request::ServerStart { format }) => {
                diagnostic_events::activation_started("server-start");
                let output = cli::session_start(format, locale);
                diagnostic_events::activation_finished("server-start-finished", output.status);
                output
            }
            Ok(invocation::Request::ServerConnect) => {
                diagnostic_events::activation_started("relay-started");
                let output = cli::server_connect(locale);
                diagnostic_events::activation_finished("relay-exited", output.status);
                output
            }
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
            Ok(invocation::Request::Install {
                no_path,
                allow_downgrade,
                format,
            }) => cli::update::install(no_path, allow_downgrade, format, locale),
            Ok(invocation::Request::CompleteInstall {
                no_path,
                allow_downgrade,
            }) => cli::update::complete_install(no_path, allow_downgrade, locale),
            Ok(invocation::Request::Uninstall {
                delete_data,
                confirm,
                format,
            }) => cli::update::uninstall(delete_data, confirm, format, locale),
            Ok(invocation::Request::InstallerUninstall { delete_data }) => {
                cli::update::installer_uninstall(delete_data, locale)
            }
            Ok(invocation::Request::NamedSession { command, format }) => {
                cli::named_sessions::run(&command, format, locale)
            }
            Ok(invocation::Request::RuntimeStatus { scope, format }) => {
                cli::runtime_status::run(scope, format, locale)
            }
            Ok(invocation::Request::ServerStop { command, format }) => {
                cli::server_stop_command(&command, format, locale)
            }
            Ok(invocation::Request::SchemaExport {
                schema_id,
                path,
                format,
            }) => cli::export_schema(schema_id.as_deref(), &path, format, locale),
            Ok(invocation::Request::DiagnosticsCreate { path, format }) => {
                cli::create_diagnostics(&path, format, locale)
            }
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

fn emit(mut result: cli::Output, locale: Locale) -> ExitCode {
    // A record a store could not use is named once, after the command's own diagnostics.
    for notice in orchbee_local_runtime::take_unshown_record_notices() {
        result.stderr.push_str(&locale.record_notice(&notice));
        result.stderr.push('\n');
    }
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
