//! One installed command invocation, preserving creation and Client control receipts.
use super::{
    Failure, FailureKind, Output,
    caller_context::PanelTarget,
    client_control::{self, exchange::exchange, selection},
    console_interrupt::{ArmedSignals, Finish, SignalWatch},
    failure_output, output, query,
};
use crate::cli_text::CliText;
use crate::{
    invocation::{ClientNavigationTarget, OutputFormat, Target, configured_commands::Command},
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientPopupSource, ClientViewAction, ClientViewControl, ClientViewIntent, ClientViewOutcome,
    ConfiguredCommandAction as Action, ConfiguredCommandFailure as Reason,
    ConfiguredCommandInvocation, ConfiguredCommandKind, ConfiguredCommandOutcome as Outcome,
    ConfiguredCommandRequest, ConfiguredCommandResult, ConfiguredCommandSource, ProtocolVersion,
    Snapshot, WorkbenchCommandOutcome,
};
use std::time::{Duration, Instant};

pub(crate) fn run(command: &Command, format: OutputFormat, locale: Locale) -> Output {
    if let Err(failure) = super::execution::admit(super::execution::Capability::ConfiguredCommands)
    {
        return failure_output(failure, Some(format), locale);
    }
    let timeout = match command {
        Command::Invoke(invocation) => invocation.timeout_ms,
        Command::List => 5_000,
    };
    let deadline = Instant::now() + Duration::from_millis(u64::from(timeout));
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let mut result = execute(command, format, locale, deadline, &mut signals)
        .unwrap_or_else(|failure| failure_output(failure, Some(format), locale));
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled { finalization, .. }) => {
            result.finalization = Some(finalization);
            result.status = 130;
        }
        Err(_) if result.status != 8 => result.status = 1,
        Err(_) => {}
    }
    result
}
fn execute(
    command: &Command,
    format: OutputFormat,
    locale: Locale,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Output, Failure> {
    signals.check_before(deadline)?;
    let (mut client, mut snapshot) = query::stream_snapshot(deadline, signals.poll_fd())?;
    let catalog_request = ConfiguredCommandRequest {
        protocol_version: ProtocolVersion,
        request_id: query::request_id()?,
        session_id: snapshot.session_id.clone(),
        action: Action::List {},
    };
    let catalog = client
        .configured_command(catalog_request, deadline, signals.poll_fd())
        .map_err(|_| Failure::new(FailureKind::TransportLoss))?;
    let Command::Invoke(invocation) = command else {
        return emit(Some(catalog), None, true, "list", format, locale);
    };
    let crate::invocation::configured_commands::Invocation {
        id,
        workspace,
        tab,
        panel,
        client: explicit,
        context: provided,
        selection,
        ..
    } = &**invocation;
    let kind = match &catalog.outcome {
        Outcome::Catalog { entries, .. } => entries
            .iter()
            .find(|entry| &entry.id == id)
            .map(|entry| entry.action)
            .ok_or_else(|| {
                Failure::new(FailureKind::NotFound)
                    .failure_schema("orchbee.cli.failure.command")
                    .details(serde_json::Map::from_iter([(
                        "commandId".into(),
                        serde_json::json!(id),
                    )]))
            })?,
        Outcome::ShellStarted { .. }
        | Outcome::Pane { .. }
        | Outcome::Plugin { .. }
        | Outcome::PopupControl { .. }
        | Outcome::PopupOpened { .. }
        | Outcome::PopupState { .. }
        | Outcome::PopupClosing { .. }
        | Outcome::PopupClosed { .. }
        | Outcome::PopupUnknown { .. }
        | Outcome::Failed { .. } => {
            return emit(Some(catalog), None, true, "invoke", format, locale);
        }
    };
    let caller = if explicit.is_none() {
        PanelTarget::prepare_optional(|name| std::env::var_os(name))?
            .map(|caller| caller.resolve(&snapshot))
            .transpose()?
    } else {
        None
    };
    let listed = exchange(
        &mut client,
        &snapshot.session_id,
        ClientViewAction::List {},
        deadline,
        signals,
    )?;
    let ClientViewOutcome::Listed { clients } = listed.outcome else {
        return Err(Failure::new(FailureKind::ServiceUnavailable));
    };
    let global = clients.is_empty()
        && explicit.is_none()
        && caller.is_none()
        && workspace.is_none()
        && tab.is_none()
        && panel.is_none()
        && matches!(
            kind,
            ConfiguredCommandKind::Shell | ConfiguredCommandKind::PluginAction
        );
    let mut selected = if global {
        None
    } else {
        Some(selection::select(&clients, explicit.as_ref(), caller.as_ref())?.clone())
    };
    if let Some(record) = &selected {
        let intent = target_intent(
            workspace.as_ref(),
            tab.as_ref(),
            panel.as_ref(),
            record,
            caller.as_ref(),
            &snapshot,
        )?;
        if let Some(intent) = intent {
            let answer = client_control::exchange::submit_control(
                &mut client,
                &snapshot.session_id,
                ClientViewControl {
                    operation_id: query::request_id()?,
                    client_id: record.client_id.clone(),
                    expected_view_revision: record.view_revision,
                    minimum_workbench_revision: snapshot.revision,
                    intent,
                },
                deadline,
                signals,
            )?;
            if answer.exit_code() != 0 {
                return emit(
                    None,
                    Some(Followup {
                        status: answer.exit_code(),
                        document: answer.document(),
                    }),
                    false,
                    "invoke",
                    format,
                    locale,
                );
            }
            snapshot = query::refresh_snapshot(
                &mut client,
                &snapshot.session_id,
                snapshot.revision,
                deadline,
                signals,
            )?;
            let reply = exchange(
                &mut client,
                &snapshot.session_id,
                ClientViewAction::Get {
                    client_id: record.client_id.clone(),
                },
                deadline,
                signals,
            )?;
            selected = match reply.outcome {
                ClientViewOutcome::Found { client } => Some(*client),
                ClientViewOutcome::Synchronized { .. }
                | ClientViewOutcome::Listed { .. }
                | ClientViewOutcome::Operation { .. }
                | ClientViewOutcome::NotFound { .. }
                | ClientViewOutcome::Conflict { .. }
                | ClientViewOutcome::Denied { .. }
                | ClientViewOutcome::Unavailable { .. } => {
                    return Err(Failure::new(FailureKind::ServiceUnavailable));
                }
            };
        }
    }
    let operation_id = query::request_id()?;
    let (context, source) = if let Some(record) = &selected {
        let (context, member) = super::plugin_preparation::context::observe(
            &mut client,
            &snapshot,
            super::plugin_preparation::context::Source::current(Some(&record.client_id)),
            (**provided).clone(),
            operation_id.as_str(),
            deadline,
            signals,
        )?;
        let source = match (record.view.selection.as_ref(), member) {
            (Some(selection), Some(member)) => Some(ConfiguredCommandSource {
                client_id: record.client_id.clone(),
                view_revision: record.view_revision,
                presentation: ClientPopupSource {
                    terminal_context_id: selection.terminal_context_id.clone(),
                    tab_id: selection.tab_id.clone().ok_or_else(rejected)?,
                    panel_id: selection.panel_id.clone().ok_or_else(rejected)?,
                    member,
                },
            }),
            _ => None,
        };
        (context, source)
    } else {
        ((**provided).clone(), None)
    };
    let request = ConfiguredCommandRequest {
        protocol_version: ProtocolVersion,
        request_id: operation_id,
        session_id: snapshot.session_id.clone(),
        action: Action::Invoke {
            invocation: Box::new(ConfiguredCommandInvocation {
                command_id: id.clone(),
                expected_revision: snapshot.revision,
                source,
                selection: *selection,
                context: Box::new(context),
            }),
        },
    };
    signals.check_before(deadline)?;
    let reply = match client.configured_command(request, deadline, signals.poll_fd()) {
        Ok(reply) => reply,
        Err(error) if error.submitted => return emit(None, None, false, "invoke", format, locale),
        Err(_) => {
            signals.check_before(deadline)?;
            return Err(Failure::new(FailureKind::TransportLoss));
        }
    };
    let mut followup = None;
    let mut complete = true;
    if let Outcome::PopupControl { result, .. } = &reply.outcome {
        let answer = client_control::exchange::wait_submitted(
            &mut client,
            (**result).clone(),
            deadline,
            signals,
        );
        match answer {
            Ok(answer) => {
                complete = answer.exit_code() == 0;
                followup = Some(Followup {
                    status: answer.exit_code(),
                    document: answer.document(),
                });
            }
            Err(_) => complete = false,
        }
    }
    if let Outcome::Pane { result, .. } = &reply.outcome
        && let WorkbenchCommandOutcome::PanelSplitCreated {
            revision,
            terminal_context_id,
            tab_id,
            new_panel_id,
            ..
        } = &result.outcome
        && let Some(selected) = selected
    {
        let answer = client_control::exchange::submit_control(
            &mut client,
            &snapshot.session_id,
            ClientViewControl {
                operation_id: query::request_id()?,
                client_id: selected.client_id,
                expected_view_revision: selected.view_revision,
                minimum_workbench_revision: *revision,
                intent: ClientViewIntent::Panel {
                    terminal_context_id: terminal_context_id.clone(),
                    tab_id: tab_id.clone(),
                    panel_id: new_panel_id.clone(),
                },
            },
            deadline,
            signals,
        );
        match answer {
            Ok(answer) => {
                complete = answer.exit_code() == 0;
                followup = Some(Followup {
                    status: answer.exit_code(),
                    document: answer.document(),
                });
            }
            Err(_) => complete = false,
        }
    }
    emit(Some(reply), followup, complete, "invoke", format, locale)
}
fn target_intent(
    workspace: Option<&Target>,
    tab: Option<&Target>,
    panel: Option<&Target>,
    client: &orchbee_workbench_protocol::ClientViewRecord,
    caller: Option<&Target>,
    snapshot: &Snapshot,
) -> Result<Option<ClientViewIntent>, Failure> {
    let workspace = workspace
        .map(|target| {
            selection::intent(
                &ClientNavigationTarget::Workspace(target.clone()),
                client,
                caller,
                snapshot,
            )
        })
        .transpose()?;
    let tab = tab
        .map(|target| {
            selection::intent(
                &ClientNavigationTarget::Tab(target.clone()),
                client,
                caller,
                snapshot,
            )
        })
        .transpose()?;
    let panel = panel
        .map(|target| {
            selection::intent(
                &ClientNavigationTarget::Panel(Some(target.clone())),
                client,
                caller,
                snapshot,
            )
        })
        .transpose()?;
    let location = |intent: &ClientViewIntent| match intent {
        ClientViewIntent::Context {
            terminal_context_id,
        } => Some((terminal_context_id.clone(), None)),
        ClientViewIntent::Tab {
            terminal_context_id,
            tab_id,
        }
        | ClientViewIntent::Panel {
            terminal_context_id,
            tab_id,
            ..
        } => Some((terminal_context_id.clone(), Some(tab_id.clone()))),
        // This handler implements its own ClientView intent subset; other intents belong to their feature handlers.
        _ => None,
    };
    if let (Some(workspace), Some(child)) = (&workspace, panel.as_ref().or(tab.as_ref()))
        && location(workspace).map(|value| value.0) != location(child).map(|value| value.0)
    {
        return Err(rejected());
    }
    if let (Some(tab), Some(panel)) = (&tab, &panel)
        && location(tab) != location(panel)
    {
        return Err(rejected());
    }
    Ok(panel.or(tab).or(workspace))
}
fn rejected() -> Failure {
    Failure::new(FailureKind::Rejected)
}
struct Followup {
    status: u8,
    document: serde_json::Value,
}
fn emit(
    reply: Option<ConfiguredCommandResult>,
    followup: Option<Followup>,
    complete: bool,
    operation: &str,
    format: OutputFormat,
    locale: Locale,
) -> Result<Output, Failure> {
    let mut status = followup
        .as_ref()
        .map_or(if complete { 0 } else { 8 }, |answer| answer.status);
    if let Some(reply) = &reply {
        status = match &reply.outcome {
            Outcome::Failed {
                reason: Reason::PhysicalUnknown,
            }
            | Outcome::PopupUnknown { .. } => 8,
            Outcome::Failed {
                reason: Reason::Unavailable,
            } => 4,
            Outcome::Failed { .. } => 3,
            Outcome::Plugin { result, .. }
                if matches!(
                    result.outcome,
                    orchbee_workbench_protocol::PluginOutcome::Unknown { .. }
                ) =>
            {
                8
            }
            Outcome::Plugin { result, .. }
                if matches!(
                    result.outcome,
                    orchbee_workbench_protocol::PluginOutcome::Failed { .. }
                ) =>
            {
                3
            }
            Outcome::Pane { result, .. }
                if !matches!(
                    result.outcome,
                    WorkbenchCommandOutcome::PanelSplitCreated { .. }
                ) =>
            {
                3
            }
            Outcome::Pane { result, .. } => match &result.outcome {
                WorkbenchCommandOutcome::PanelSplitCreated {
                    terminal_outcome:
                        orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
                    ..
                } => status,
                WorkbenchCommandOutcome::PanelSplitCreated {
                    terminal_outcome:
                        orchbee_workbench_protocol::WorkbenchTerminalOutcome::OutcomeUnknown {},
                    ..
                } => 8,
                // This step consumes its specific mutation receipt; other command results cannot confirm the requested postcondition.
                _ => 3,
            },
            // This step consumes its specific mutation receipt; other command results cannot confirm the requested postcondition.
            _ => status,
        };
    }
    let document = serde_json::json!({"schema":"orchbee.command","version":1,"source":{"kind":"local-runtime"},"result":{"operation":operation,"reply":reply,"followup":followup.map(|answer| answer.document),"complete":complete && status==0}});
    let stdout = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text => {
            let text: String = match document["result"]["reply"]["outcome"]["entries"].as_array() {
                Some(entries) if entries.is_empty() => {
                    CliText::new(locale).cli_command_empty().into()
                }
                Some(entries) => super::present::columns(
                    &CliText::new(locale).cli_command_headers(),
                    &entries
                        .iter()
                        .map(|entry| {
                            vec![
                                entry["id"].as_str().unwrap_or("").to_owned(),
                                entry["action"].as_str().unwrap_or("").to_owned(),
                                entry["bindings"]
                                    .as_array()
                                    .map(|values| {
                                        values
                                            .iter()
                                            .filter_map(serde_json::Value::as_str)
                                            .collect::<Vec<_>>()
                                            .join(" / ")
                                    })
                                    .filter(|keys| !keys.is_empty())
                                    .unwrap_or_else(|| "-".to_owned()),
                                crate::cli_text::escape::escape(
                                    entry["description"].as_str().unwrap_or(""),
                                ),
                                crate::cli_text::escape::escape(
                                    entry["display"].as_str().unwrap_or(""),
                                ),
                            ]
                        })
                        .collect::<Vec<_>>(),
                ),
                None => CliText::new(locale)
                    .configured_command_status(status)
                    .into(),
            };
            output::encode_text(&text)
        }
    }
    .map_err(|_| Failure::new(FailureKind::OutputTooLarge))?;
    Ok(Output {
        status,
        stdout,
        stderr: String::new(),
        finalization: None,
    })
}
