//! Validate plugin membership before one canonical close or Client focus submission.
use crate::{
    cli::{
        Failure, FailureKind, Output,
        caller_context::PanelTarget,
        client_control,
        console_interrupt::{ArmedSignals, Finish, SignalWatch},
        failure_output, mutation, query, results,
    },
    invocation::{OutputFormat, plugins::panes::Manage},
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewControl, ClientViewIntent, ClientViewOutcome, PanelNode,
    PluginAction, PluginFailure, PluginOutcome, PluginRequest, Presentation, ProtocolVersion,
    WorkbenchAction, WorkbenchPanelId,
};
use std::time::{Duration, Instant};

pub(in crate::cli::plugins) fn run(
    command: &Manage,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let deadline = Instant::now() + Duration::from_millis(u64::from(command.timeout_ms));
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
    let result = execute(command, deadline, &mut signals, format, locale);
    let mut output = result.unwrap_or_else(|failure| failure_output(failure, Some(format), locale));
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled { finalization, .. }) => {
            output.finalization = Some(finalization);
            output.status = 130;
        }
        Err(_) if output.status != 8 => output.status = 1,
        Err(_) => {}
    }
    output
}

fn execute(
    command: &Manage,
    deadline: Instant,
    signals: &mut ArmedSignals,
    format: OutputFormat,
    locale: Locale,
) -> Result<Output, Failure> {
    signals.check_before(deadline)?;
    let (mut client, snapshot) = query::stream_snapshot(deadline, signals.poll_fd())?;
    let (panel_id, context, tab) = results::resolve_or_fail(
        results::panel_entries(&snapshot)
            .into_iter()
            .map(|(id, context, tab)| (id.as_str(), "", (id, context, tab))),
        &command.target,
        crate::invocation::Resource::Panel,
    )?;
    let reply = client
        .plugin(
            PluginRequest {
                protocol_version: ProtocolVersion,
                request_id: query::request_id()?,
                session_id: snapshot.session_id.clone(),
                action: PluginAction::PaneGet {
                    panel_id: panel_id.clone(),
                },
            },
            deadline,
            signals.poll_fd(),
        )
        .map_err(|_| super::Reason::TransportLoss.failure())?;
    signals.check_before(deadline)?;
    let (revision, pane) = match reply.outcome {
        PluginOutcome::PaneFound { revision, pane } => (revision, pane),
        PluginOutcome::Failed {
            reason: PluginFailure::NotFound,
        } => return Err(super::Reason::PaneNotFound.failure()),
        PluginOutcome::Failed {
            reason: PluginFailure::Denied,
        } => return Err(Failure::new(FailureKind::Denied)),
        PluginOutcome::Failed { .. } => return Err(super::Reason::PluginUnavailable.failure()),
        // This phase consumes its requested plugin/popup receipt; other receipt kinds cannot confirm that operation.
        _ => return Err(super::Reason::InvalidReply.failure()),
    };
    if revision != snapshot.revision {
        return Err(super::Reason::PaneChanged.failure());
    }
    if pane.panel_id != *panel_id
        || pane.tab_id != tab.tab_id
        || pane.terminal_context_id != context.terminal_context_id
        || primary(&tab.root, panel_id).is_none_or(|presentation| {
            presentation.presentation_id != pane.presentation_id
                || presentation.binding.as_ref() != Some(&pane.binding)
        })
    {
        return Err(super::Reason::InvalidReply.failure());
    }
    if !command.focus {
        let receipt = mutation::submit_action(
            &mut client,
            &snapshot,
            WorkbenchAction::PanelClose {
                terminal_context_id: pane.terminal_context_id,
                tab_id: pane.tab_id,
                panel_id: pane.panel_id,
            },
            deadline,
            signals.poll_fd(),
        )?;
        return Ok(mutation::success_output(&receipt, format, locale));
    }
    let caller = if command.client.is_none() {
        PanelTarget::prepare_optional(|name| std::env::var_os(name))?
            .map(|target| target.resolve(&snapshot))
            .transpose()?
    } else {
        None
    };
    let reply = client_control::exchange::exchange(
        &mut client,
        &snapshot.session_id,
        ClientViewAction::List {},
        deadline,
        signals,
    )?;
    let clients = match reply.outcome {
        ClientViewOutcome::Listed { clients } => clients,
        ClientViewOutcome::Denied {} => return Err(Failure::new(FailureKind::Denied)),
        ClientViewOutcome::Synchronized { .. }
        | ClientViewOutcome::Found { .. }
        | ClientViewOutcome::Operation { .. }
        | ClientViewOutcome::NotFound { .. }
        | ClientViewOutcome::Conflict { .. }
        | ClientViewOutcome::Unavailable { .. } => {
            return Err(super::Reason::ClientListingUnavailable.failure());
        }
    };
    let selected =
        client_control::selection::select(&clients, command.client.as_ref(), caller.as_ref())?;
    let answer = client_control::exchange::submit_control(
        &mut client,
        &snapshot.session_id,
        ClientViewControl {
            operation_id: query::request_id()?,
            client_id: selected.client_id.clone(),
            expected_view_revision: selected.view_revision,
            minimum_workbench_revision: revision,
            intent: ClientViewIntent::Panel {
                terminal_context_id: pane.terminal_context_id,
                tab_id: pane.tab_id,
                panel_id: pane.panel_id,
            },
        },
        deadline,
        signals,
    )?;
    Ok(client_control::emit(&answer, format, locale))
}
fn primary<'a>(node: &'a PanelNode, panel_id: &WorkbenchPanelId) -> Option<&'a Presentation> {
    match node {
        PanelNode::Panel {
            panel_id: id,
            primary_presentation,
            ..
        } => (id == panel_id).then_some(primary_presentation),
        PanelNode::Split { children, .. } => children
            .iter()
            .find_map(|child| primary(&child.node, panel_id)),
    }
}
