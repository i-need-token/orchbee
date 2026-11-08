//! Dispatches admitted CLI requests to their feature owners and assembles stdout, stderr,
//! structured failures, and process outcomes. It does not own command grammar, canonical
//! Workbench state, or physical terminal execution.

use crate::cli_text::CliText;
mod agent_list;
pub(crate) mod agent_manifests;
pub(crate) mod agent_view_spec;
mod agent_wait;
mod command_interrupt;
mod console_interrupt;
mod destructive_request;
mod diagnostics;
mod execution;
mod input_source;
mod installation;
pub(crate) mod invocation_route;
mod link_activation;
pub(crate) mod machines;
pub(crate) mod notification;
pub(crate) mod operation;
pub(crate) mod panel_agent;
pub(crate) mod panel_metadata;
mod panel_metadata_query;
pub(crate) mod panel_terminal_creation;
mod plugin_preparation;
pub(crate) mod plugins;
mod report_target;
pub(crate) mod runtime_configuration;
pub(crate) use diagnostics::create as create_diagnostics;
mod schema_export;
mod terminal_run;
pub(crate) mod window_title;
pub(crate) mod workspace_metadata;
pub(crate) use schema_export::run as export_schema;
mod client_control;
pub(crate) mod configured_commands;
pub(crate) mod events;
pub(crate) mod pane_graphics;
mod stdin_bytes;
pub(crate) use client_control::run as run_client_control;
pub(crate) mod agent_names;
mod agent_projection;
mod agent_prompt;
mod agent_start;
mod agent_view;
mod handles;
mod panel_list;
pub(crate) use agent_start::run as run_agent_start;
mod caller_context;
mod creation;
mod creation_defaults;
mod current_panel;
mod current_presentation;
mod doctor_document;
mod launch_receipt;
mod source_context;
pub(crate) mod workspace_creation;
mod workspace_focus;
pub(crate) use creation::run_parsed as run_creation;
mod agent_request;
pub(crate) mod channel;
mod completion;
mod terminal_clear;
mod terminal_editor;
mod terminal_list;
pub(crate) use terminal_clear::run as clear_terminal;
mod layout_history;
pub(crate) use layout_history::{list as layout_history, reopen as layout_reopen};
pub(crate) use terminal_editor::run as edit_terminal_scrollback;
pub(crate) mod config;
pub(crate) mod config_reset;
mod configuration;
pub(crate) mod integration;
mod layout_topology;
pub(crate) mod layouts;
mod mutation;
pub(crate) mod named_sessions;
pub(crate) mod output;
pub(crate) mod panel_relocation;
pub(crate) mod present;
pub(crate) use crate::agent_runtime::processes;
mod query;
pub(crate) mod results;
pub(crate) mod runtime_status;
pub(crate) mod schema;
pub(crate) mod send;

mod server;
mod server_connect;
mod server_diagnose;
mod server_query;
mod server_stop;
mod terminal_observation;
pub(crate) mod update;
mod workbench_agents;
mod workbench_metadata;
pub(crate) mod workbench_query;
mod workbench_terminals;
mod workbench_viewports;
mod workbench_worktrees;
pub(crate) use server_connect::run as server_connect;
pub(crate) use server_stop::run as server_stop;
pub(crate) use server_stop::run_command as server_stop_command;
mod workspace_group_close;
mod worktree_operation;
pub(crate) use workspace_group_close::run as run_workspace_group_close;
mod worktree_query;
pub(crate) use worktree_query::run as run_worktree_query;
mod worktree_source;
pub(crate) use worktree_operation::run as run_worktree;
pub(crate) mod source;
mod source_finalization;
pub(crate) use source_finalization::SourceFinalization;
mod terminal_output;
pub(crate) use query::terminal_copy::run as copy_terminal;
pub(crate) use query::terminal_links::run as resolve_terminal_link;
pub(crate) use query::terminal_selection::run as read_terminal_selection;
pub(crate) mod terminal_stream;

pub(crate) fn terminal_attach(
    command: &crate::invocation::TerminalAttachCommand,
    locale: Locale,
) -> Output {
    terminal_stream::run_attach(command, locale)
}
pub(crate) fn terminal_observe(
    command: &crate::invocation::TerminalObserveCommand,
    locale: Locale,
) -> Output {
    terminal_stream::run(command, locale)
}
pub(crate) fn terminal_control(
    command: &crate::invocation::TerminalControlCommand,
    locale: Locale,
) -> Output {
    terminal_stream::run_control(command, locale)
}

mod text;
mod wait_deadline;

#[cfg(test)]
mod tests;

use serde_json::{Map, Value, json};

// The failure model is owned by `cli_outcome`; command owners under `cli` reach it through this
// composition root, while modules outside `cli` import it from `cli_outcome` directly.
pub(crate) use crate::cli_outcome::{
    CandidateDisplay, Failure, FailureKind, stage_details, stage_reason_details, target_details,
};
use crate::invocation::{
    CompletionShell, HelpTopic, MutationCommand, OutputFormat, ParseFailure, ParseFailureKind,
    QueryCommand,
};
use crate::localization::Locale;
use crate::presentation::ProcessResult;

/// One fully prepared process answer: status code plus already-encoded
/// streams. `finalization` is the deferred, noncloneable duty the common
/// `main` emit path must consume after its last write attempt — only the
/// Linux `terminal send` source signal guard ever produces one.
pub(crate) struct Output {
    pub(crate) status: u8,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: String,
    pub(crate) finalization: Option<SourceFinalization>,
}

/// Process results for TUI launch and version also finish
/// through the preparation bound; their static bodies are far under it, so
/// the overflow arm is unreachable — but if it ever ran it still reports a
/// localized failure, never a hardcoded English string.
/// A failed local startup exits 1 with its localized copy, which may name product versions.
pub(crate) fn startup_failure(
    failure: &crate::local_activation::StartupFailure,
    locale: Locale,
) -> Output {
    Output {
        status: 1,
        stdout: Vec::new(),
        stderr: locale.startup_failure(failure).into_owned(),
        finalization: None,
    }
}

pub(crate) fn process_result(result: ProcessResult, locale: Locale) -> Output {
    match output::encode_text(result.stdout) {
        Ok(stdout) => Output {
            status: result.status,
            stdout,
            stderr: result.stderr.to_owned(),
            finalization: None,
        },
        Err(output::TooLarge) => failure_output(
            Failure::new(FailureKind::OutputTooLarge).details(Map::from_iter([(
                "limitBytes".to_owned(),
                json!(output::LIMIT),
            )])),
            None,
            locale,
        ),
    }
}

/// Encode a failure into its process answer. `format` is the format selected by
/// a valid selector before the failure; `None` keeps stdout empty.
/// Refuses a data root whose layout this build cannot read, before anything else in it is read.
/// Only the root a command would open is admitted; resolving it reads no file.
pub(crate) fn data_root_layout_refusal(
    format: Option<OutputFormat>,
    locale: Locale,
) -> Option<Output> {
    let root = orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name)).ok()?;
    let refusal = root.admit_layout().err()?;
    let reason = if refusal.layout.is_some() {
        "layout-newer"
    } else {
        "layout-unreadable"
    };
    let mut details = serde_json::Map::from_iter([
        ("stage".to_owned(), Value::from("data-root")),
        ("reason".to_owned(), Value::from(reason)),
        (
            "dataRoot".to_owned(),
            Value::from(root.path().to_string_lossy().into_owned()),
        ),
    ]);
    if let Some(layout) = refusal.layout {
        details.insert("layout".to_owned(), Value::from(layout));
    }
    if let Some(version) = refusal.newest_version {
        details.insert("newestVersion".to_owned(), Value::from(version));
    }
    Some(failure_output(
        Failure::new(FailureKind::Incompatible).details(details),
        format,
        locale,
    ))
}

pub(crate) fn failure_output(
    failure: Failure,
    format: Option<OutputFormat>,
    locale: Locale,
) -> Output {
    let status = failure.kind.exit_code();
    let mut stderr = CliText::new(locale).cli_failure(&failure);
    if failure.failure_schema == Some("orchbee.cli.failure.agent-runtime")
        && let Some(hint) = failure
            .details
            .as_ref()
            .and_then(|details| details.get("reason"))
            .and_then(Value::as_str)
            .and_then(|reason| CliText::new(locale).cli_agent_runtime_hint(reason))
    {
        stderr.push_str(hint);
        stderr.push('\n');
    }
    match format {
        Some(OutputFormat::Json) => {
            let document = failure.document();
            // Failure documents are small and bounded well below the output cap.
            let stdout = output::encode_document(&document).unwrap_or_else(|_| Vec::new());
            Output {
                status,
                stdout,
                stderr,
                finalization: None,
            }
        }
        Some(OutputFormat::Text) | None => Output {
            status,
            stdout: Vec::new(),
            stderr,
            finalization: None,
        },
    }
}

/// `terminal send` source-stage failures carry the dedicated
/// `orchbee.cli.failure.source` envelope: the same failure object shape but
/// no authority source and no request identity, because nothing was admitted
/// yet. The caller's selected format still decides whether stdout carries
/// the document; the status is explicit since a handled source interruption
/// exits with the conventional `128 + signo`, not a kind-derived code.
pub(crate) fn source_failure_output(
    failure: Failure,
    status: u8,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let stderr = CliText::new(locale).cli_failure(&failure);
    match format {
        OutputFormat::Json => {
            let mut failure_object =
                Map::from_iter([("kind".to_owned(), json!(failure.kind.as_str()))]);
            if let Some(details) = failure.details {
                failure_object.insert("details".to_owned(), Value::Object(details));
            }
            let document = json!({
                "schema": schema::SOURCE_FAILURE_ID,
                "version": 1,
                "failure": Value::Object(failure_object),
            });
            let stdout = output::encode_document(&document).unwrap_or_else(|_| Vec::new());
            Output {
                status,
                stdout,
                stderr,
                finalization: None,
            }
        }
        OutputFormat::Text => Output {
            status,
            stdout: Vec::new(),
            stderr,
            finalization: None,
        },
    }
}

/// Parse failures answer before any runtime work; envelope rules come from the
/// parser's recorded selection.
pub(crate) fn parse_failure(failure: ParseFailure, locale: Locale) -> Output {
    let (kind, details) = match &failure.kind {
        ParseFailureKind::InvalidUsage => (FailureKind::InvalidUsage, None),
        ParseFailureKind::InvalidIdentifier { resource, target } => (
            FailureKind::InvalidIdentifier,
            Some(target_details(*resource, target)),
        ),
    };
    let record = Failure {
        kind,
        details,
        source: None,
        usage: failure.usage.map(Box::new),
        failure_schema: None,
        io_diagnostic: None,
        candidate_display: None,
        client_refusal: None,
    };
    let format = match failure.envelope {
        crate::invocation::FailureEnvelope::Json => Some(OutputFormat::Json),
        crate::invocation::FailureEnvelope::Text | crate::invocation::FailureEnvelope::Silent => {
            None
        }
    };
    failure_output(record, format, locale)
}

/// Fixed text outputs (help, completion scripts) still finish through the
/// preparation bound; their size is a small constant asserted by tests.
fn fixed_text(body: &str, locale: Locale) -> Output {
    match output::encode_text(body) {
        Ok(stdout) => Output {
            status: 0,
            stdout,
            stderr: String::new(),
            finalization: None,
        },
        Err(output::TooLarge) => failure_output(
            Failure::new(FailureKind::OutputTooLarge).details(Map::from_iter([(
                "limitBytes".to_owned(),
                json!(output::LIMIT),
            )])),
            None,
            locale,
        ),
    }
}

pub(crate) fn root_help(locale: Locale) -> Output {
    fixed_text(&text::root(locale), locale)
}

pub(crate) fn help(topic: HelpTopic, locale: Locale) -> Output {
    fixed_text(&text::help(topic, locale), locale)
}

pub(crate) fn completion(shell: CompletionShell, locale: Locale) -> Output {
    fixed_text(&completion::render(shell), locale)
}

pub(crate) fn default_config(locale: Locale) -> Output {
    if crate::tui_capabilities::Capability::ConfigurationFile
        .require()
        .is_err()
    {
        return unsupported_platform(None, locale);
    }
    let actions = crate::client_view::Bindings::default_windows_key_action_bindings();
    let config = orchbee_local_runtime::default_user_settings_toml_with_actions(&actions);
    fixed_text(&config, locale)
}

/// Print the same build's offline Agent instructions through the bounded sink.
pub(crate) fn skill(locale: Locale) -> Output {
    let guide = match locale {
        Locale::EnUs => include_str!("../docs/cli-agent-skill.md"),
        Locale::ZhCn => include_str!("../docs/cli-agent-skill.zh-CN.md"),
    };
    fixed_text(
        &released_guide(guide, crate::invocation::grammar::withholds_unreleased()),
        locale,
    )
}

/// The marker lines that enclose guide passages about surfaces a release withholds.
const UNRELEASED_START: &str = "<!-- unreleased -->";
const UNRELEASED_END: &str = "<!-- /unreleased -->";

/// A release leaves out the passages between the unreleased markers; every build drops the marker
/// lines themselves.
fn released_guide(guide: &str, withhold: bool) -> String {
    let mut inside = false;
    guide
        .split_inclusive('\n')
        .filter(|line| match line.trim_end() {
            UNRELEASED_START => {
                inside = true;
                false
            }
            UNRELEASED_END => {
                inside = false;
                false
            }
            _ => !(inside && withhold),
        })
        .collect()
}

/// One effectful command: the mutation executor owns Workbench actions, while
/// `terminal send` takes its own input-record path on the same session
/// discipline — both admit once, submit once, and never replay.
pub(crate) fn mutation(command: MutationCommand, format: OutputFormat, locale: Locale) -> Output {
    if let MutationCommand::TerminalRunWait {
        target,
        command,
        timeout_ms,
    } = &command
    {
        return terminal_run::run(target, command, *timeout_ms, format, locale);
    }
    if let MutationCommand::TerminalSend { target, input } = &command {
        return send::run(target, input, format, locale);
    }
    if let MutationCommand::AgentSendKeys { target, keys } = &command {
        return send::run_agent(target, keys, format, locale);
    }
    if let MutationCommand::AgentPrompt { target, text, wait } = &command {
        return agent_prompt::run(target, text, wait.as_ref(), format, locale);
    }
    mutation::run(&command, format, locale)
}

/// `server start` is neither a query nor a workbench mutation: it runs the
/// shared connect-or-activate core once and reports the branch it took.
pub(crate) fn session_start(format: OutputFormat, locale: Locale) -> Output {
    server::start(format, locale)
}

/// `server diagnose` is neither a query nor a mutation: it reports bounded
/// local runtime facts and the limits of those observations — it never
/// connects, creates, repairs, or predicts a later start.
pub(crate) fn server_diagnose(format: OutputFormat, locale: Locale) -> Output {
    server_diagnose::run(format, locale)
}

/// `agent integration` is provider onboarding through the local Session: each verb declares
/// activation, and the Session's Worker owns every provider command.
pub(crate) fn agent_integration(
    command: crate::invocation::AgentIntegrationCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    integration::run(&command, format, locale)
}

pub(crate) fn query(command: QueryCommand, format: OutputFormat, locale: Locale) -> Output {
    if execution::admit(execution::Capability::WorkbenchInspection).is_ok()
        && matches!(command, QueryCommand::TerminalList { .. })
    {
        return terminal_list::run(command, format, locale);
    }
    if execution::admit(execution::Capability::WorkbenchInspection).is_ok()
        && matches!(
            command,
            QueryCommand::WorkspaceList { .. }
                | QueryCommand::WorkspaceGet(_)
                | QueryCommand::TabList { .. }
                | QueryCommand::TabGet(_)
                | QueryCommand::PanelList { .. }
                | QueryCommand::PanelGet(_)
        )
    {
        return workbench_query::run(&command, None, format, locale);
    }
    match &command {
        QueryCommand::ApiSchema { schema_id } => schema::read(schema_id.as_deref(), format, locale),
        QueryCommand::ConfigCheck => config::check(format, locale),
        // `agent doctor` is fully offline: it answers bounded filesystem
        // facts itself and never opens a session or touches the network
        // query path.
        QueryCommand::AgentDoctor {
            providers,
            asset_roots,
        } => doctor_document::run(providers, asset_roots, format, locale),
        // `terminal get` answers the Server's own launch-read join on the
        // admitted session; the snapshot supplies only the typed target id.
        QueryCommand::TerminalGet(target) => match query::terminal_get(target) {
            Err(failure) => failure_output(failure, Some(format), locale),
            Ok((source, projection)) => {
                success_output(projection, source, &command, format, locale)
            }
        },
        // Agent reads exchange on their own admitted Client Connection; answers
        // arrive already projected with the correlated request identity.
        QueryCommand::AgentChanges { limit, cursor } => agent_output(
            query::agent_changes(*limit, cursor.clone()),
            &command,
            format,
            locale,
        ),
        QueryCommand::AgentList {
            include_ended,
            limit,
            cursor,
            client,
        } => {
            if client.is_some() || execution::admit(execution::Capability::AgentProjection).is_ok()
            {
                agent_list::run(&command, format, locale)
            } else {
                match agent_request::list_cursor(
                    cursor.as_deref(),
                    format,
                    orchbee_agent_observation_protocol::AgentListCursor::parse,
                ) {
                    Ok(cursor) => agent_output(
                        query::agent_list(*include_ended, *limit, cursor),
                        &command,
                        format,
                        locale,
                    ),
                    Err(failure) => parse_failure(failure, locale),
                }
            }
        }
        QueryCommand::AgentGet(session_id) => {
            if execution::admit(execution::Capability::AgentProjection).is_ok() {
                agent_view::run(&command, session_id, None, format, locale)
            } else {
                agent_output(query::agent_get(session_id), &command, format, locale)
            }
        }
        QueryCommand::AgentClientGet { target, client } => {
            agent_view::run(&command, target, Some(client), format, locale)
        }
        QueryCommand::AgentExplain(session_id) => {
            agent_output(query::agent_explain(session_id), &command, format, locale)
        }
        QueryCommand::AgentWait {
            session,
            client,
            until,
            after_revision,
            timeout_ms,
        } => agent_output(
            agent_wait::agent_wait(
                session,
                agent_request::wait_conditions(until.as_deref()),
                *after_revision,
                agent_request::wait_timeout(*timeout_ms),
                client.as_ref(),
            ),
            &command,
            format,
            locale,
        ),
        // `terminal read` couples the snapshot to one viewer observation on
        // the same session; its answer arrives already projected.
        QueryCommand::TerminalOutput {
            target,
            selection,
            ansi,
            ..
        } => match query::read_output(target, *selection, *ansi) {
            Err(failure) => failure_output(failure, Some(format), locale),
            Ok((source, projection)) => {
                success_output(projection, source, &command, format, locale)
            }
        },
        #[cfg(test)]
        QueryCommand::TerminalRead {
            target,
            history,
            unwrap,
            lines,
        } => {
            match query::read_terminal(
                target,
                *history,
                results::ReadPresentation {
                    unwrap: *unwrap,
                    tail: *lines,
                },
            ) {
                Err(failure) => failure_output(failure, Some(format), locale),
                Ok((source, projection)) => {
                    success_output(projection, source, &command, format, locale)
                }
            }
        }
        // `terminal wait-output` couples the snapshot to one bounded
        // read-only observation on the same session; its answer arrives
        // already projected, like `terminal read`.
        QueryCommand::TerminalSourceWait {
            target,
            matcher,
            selection,
            timeout_ms,
            after,
        } => match query::wait_source_output(
            target,
            matcher,
            *selection,
            *timeout_ms,
            after.as_ref(),
        ) {
            Err(failure) => failure_output(failure, Some(format), locale),
            Ok((source, projection)) => {
                success_output(projection, source, &command, format, locale)
            }
        },
        // `workspace close-status` reads the retained operation record on the
        // admitted session; the stable `(project, workspace)` pair is the
        // whole selector — the record outlives the active projection.
        QueryCommand::WorkspaceCloseStatus { workspace } => {
            match query::workspace_close_status(workspace) {
                Err(failure) => failure_output(failure, Some(format), locale),
                Ok((source, projection)) => {
                    success_output(projection, source, &command, format, locale)
                }
            }
        }
        // `worktree list` couples the snapshot to one Workbench worktree-list
        // read on the same session; the directory is a strict typed ID and
        // membership is the Server's call, never the CLI's.
        QueryCommand::WorktreeList { directory } => match query::worktree_list(directory) {
            Err(failure) => failure_output(failure, Some(format), locale),
            Ok((source, projection)) => {
                success_output(projection, source, &command, format, locale)
            }
        },
        // `launch-profile list`/`get` ride one typed catalog read on the
        // admitted session; selector resolution is the Server's call under
        // its lock — the CLI carries the intent verbatim.
        QueryCommand::LaunchProfileList => match query::launch_profile_catalog(None) {
            Err(failure) => failure_output(failure, Some(format), locale),
            Ok((source, projection)) => {
                success_output(projection, source, &command, format, locale)
            }
        },
        QueryCommand::LaunchProfileGet(selector) => {
            match query::launch_profile_catalog(Some(selector.clone())) {
                Err(failure) => failure_output(failure, Some(format), locale),
                Ok((source, projection)) => {
                    success_output(projection, source, &command, format, locale)
                }
            }
        }
        QueryCommand::PanelProcessInfo(target) | QueryCommand::PanelCurrent(target) => {
            current_panel::run(&command, target, format, locale)
        }
        _ => network_query(&command, format, locale),
    }
}

/// One Agent read answer: pre-admission failures keep the generic failure
/// document, every post-admission fault or semantic refusal answers with the
/// dedicated agent failure document, and a successful read emits the
/// verbatim wire outcome under the correlated `agent-read` source. A
/// captured signal answers the agent document's `cancelled` kind — its
/// owed finalization rides the common emit path like every guarded output.
fn agent_output(
    result: Result<(Value, results::Projection), query::AgentQueryError>,
    command: &QueryCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    match result {
        Err(query::AgentQueryError::Admission(failure)) => {
            failure_output(failure, Some(format), locale)
        }
        Err(query::AgentQueryError::Read(mut failure)) => {
            if failure.failure_schema.is_none() {
                failure = failure.failure_schema(schema::AGENT_FAILURE_ID);
            }
            failure_output(failure, Some(format), locale)
        }
        // The native Agent signal owner lends the deferred finalization token.
        Err(query::AgentQueryError::Cancelled(cancelled)) => {
            let query::AgentCancelled {
                failure,
                finalization,
            } = *cancelled;
            let mut output = failure_output(
                failure.failure_schema(schema::AGENT_FAILURE_ID),
                Some(format),
                locale,
            );
            output.finalization = Some(finalization);
            output
        }
        Ok((source, projection)) => success_output(projection, source, command, format, locale),
    }
}

fn network_query(command: &QueryCommand, format: OutputFormat, locale: Locale) -> Output {
    match query::read_snapshot() {
        Err(failure) => failure_output(failure, Some(format), locale),
        Ok(snapshot) => {
            let source = results::snapshot_source(&snapshot);
            match results::project(command, &snapshot) {
                Err(failure) => failure_output(failure.source(source), Some(format), locale),
                Ok(projection) => success_output(projection, source, command, format, locale),
            }
        }
    }
}

fn success_output(
    projection: results::Projection,
    source: Value,
    command: &QueryCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let schema_id = results::schema_id(command);
    // A preparation overflow keeps the caller's selected format, and a text
    // failure never writes a stdout document.
    let too_large = |source: &Value| {
        failure_output(
            Failure::new(FailureKind::OutputTooLarge)
                .details(Map::from_iter([(
                    "limitBytes".to_owned(),
                    json!(output::LIMIT),
                )]))
                .source(source.clone()),
            Some(format),
            locale,
        )
    };
    // The api commands answer machine documents in every format.
    let document_format = match command {
        QueryCommand::ApiSnapshot => OutputFormat::Json,
        _ => format,
    };
    match document_format {
        OutputFormat::Json => {
            let document = json!({
                "schema": schema_id,
                "version": 1,
                "source": source,
                "result": results::complete(command, projection.result()),
            });
            match output::encode_document(&document) {
                Ok(stdout) => Output {
                    status: 0,
                    stdout,
                    stderr: String::new(),
                    finalization: None,
                },
                Err(output::TooLarge) => too_large(&source),
            }
        }
        OutputFormat::Text => {
            match output::render_text(|out| present::render(out, &projection, command, locale)) {
                Ok(stdout) => Output {
                    status: 0,
                    stdout,
                    stderr: String::new(),
                    finalization: None,
                },
                Err(output::TooLarge) => too_large(&source),
            }
        }
    }
}

/// A recognized command whose selected execution adapter is not implemented here.
pub(crate) fn unsupported_platform(format: Option<OutputFormat>, locale: Locale) -> Output {
    failure_output(
        Failure::new(FailureKind::ServiceUnavailable)
            .details(stage_reason_details("platform", "unsupported-platform")),
        format,
        locale,
    )
}
