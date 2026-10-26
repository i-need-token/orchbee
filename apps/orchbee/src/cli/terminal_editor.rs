//! One focused-Client editor intent after passive source validation; no source input.
use super::{
    Failure, FailureKind, Output,
    caller_context::PanelTarget,
    client_control, current_presentation, failure_output, output, query, results,
    source::{ArmedSignals, Finish, SignalWatch},
};
use crate::{
    invocation::{OutputFormat, Target},
    local_client::LocalClient,
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientPopupSource, ClientViewAction, ClientViewAvailability, ClientViewControl,
    ClientViewControlStatus, ClientViewIntent, ClientViewOutcome, ClientViewPhase,
    ScrollbackEditorSpec, Snapshot, WorkbenchClientId,
};
use std::time::{Duration, Instant};

pub(crate) fn run(
    target: &Target,
    explicit: Option<&WorkbenchClientId>,
    timeout_ms: u32,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let deadline = Instant::now() + Duration::from_millis(u64::from(timeout_ms));
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
    let mut result = execute(target, explicit, format, locale, deadline, &mut signals)
        .unwrap_or_else(|failure| failure_output(failure, Some(format), locale));
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled { finalization, .. }) => {
            result.status = 130;
            result.finalization = Some(finalization);
        }
        Err(_) if result.status != 8 => result.status = 1,
        Err(_) => {}
    }
    result
}
fn execute(
    target: &Target,
    explicit: Option<&WorkbenchClientId>,
    format: OutputFormat,
    locale: Locale,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Output, Failure> {
    signals.check_before(deadline)?;
    let (mut client, snapshot) = query::stream_snapshot(deadline, signals.poll_fd())?;
    let answer = execute_admitted(&mut client, &snapshot, target, explicit, deadline, signals)?;
    let document = answer.document();
    let text = match answer.reply.as_ref().map(|reply| &reply.outcome) {
        Some(ClientViewOutcome::Operation { operation }) => match &operation.status {
            ClientViewControlStatus::PopupOpened { popup_id, .. } => {
                locale.cli_popup_opened(popup_id)
            }
            ClientViewControlStatus::PopupClosed { .. }
            | ClientViewControlStatus::PresentationRead { .. }
            | ClientViewControlStatus::ViewportRead { .. }
            | ClientViewControlStatus::NavigationRead { .. }
            | ClientViewControlStatus::TerminalAttentionRead { .. }
            | ClientViewControlStatus::AttentionRead { .. }
            | ClientViewControlStatus::LayoutAdjusted { .. }
            | ClientViewControlStatus::DirectionFocused { .. }
            | ClientViewControlStatus::DirectionUnchanged { .. }
            | ClientViewControlStatus::LayoutRead { .. }
            | ClientViewControlStatus::Queued { .. }
            | ClientViewControlStatus::Delivered { .. }
            | ClientViewControlStatus::Applied { .. }
            | ClientViewControlStatus::ZoomUnchanged { .. }
            | ClientViewControlStatus::ZoomToggled { .. }
            | ClientViewControlStatus::Rejected { .. }
            | ClientViewControlStatus::Incomplete { .. } => locale.cli_client_control(&document),
        },
        // This step consumes its requested ClientView receipt class; other replies cannot confirm that observation.
        _ => locale.cli_client_control(&document),
    };
    let stdout = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text => output::encode_text(&text),
    }
    .map_err(|_| Failure::new(FailureKind::OutputTooLarge))?;
    Ok(Output {
        status: answer.exit_code(),
        stdout,
        stderr: String::new(),
        finalization: None,
    })
}
fn execute_admitted(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    target: &Target,
    explicit: Option<&WorkbenchClientId>,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<client_control::Answer, Failure> {
    signals.check_before(deadline)?;
    let terminal = results::terminal_select(snapshot, target)?;
    let caller = if explicit.is_none() {
        PanelTarget::prepare_optional(|name| std::env::var_os(name))?
            .map(|caller| caller.resolve(snapshot))
            .transpose()?
    } else {
        None
    };
    let listed = client_control::windows::exchange(
        client,
        &snapshot.session_id,
        ClientViewAction::List {},
        deadline,
        signals,
    )?;
    let ClientViewOutcome::Listed { clients } = listed.outcome else {
        return Err(Failure::new(FailureKind::ServiceUnavailable));
    };
    let selected = client_control::selection::select(&clients, explicit, caller.as_ref())?.clone();
    if selected.view.phase != ClientViewPhase::Ready {
        return Err(Failure::new(FailureKind::Rejected));
    }
    let selection = selected
        .view
        .selection
        .as_ref()
        .ok_or_else(|| Failure::new(FailureKind::Rejected))?;
    let panel_id = selection
        .panel_id
        .as_ref()
        .ok_or_else(|| Failure::new(FailureKind::Rejected))?;
    let member =
        current_presentation::read(client, snapshot, &selected, panel_id, deadline, signals)?;
    if !member.binding().is_some_and(|binding| {
        binding.worker_id.as_str() == terminal.worker_id
            && binding.terminal_id.as_str() == terminal.terminal_id
    }) {
        return Err(Failure::new(FailureKind::Rejected));
    }
    let spec = ScrollbackEditorSpec {
        source: ClientPopupSource {
            terminal_context_id: selection.terminal_context_id.clone(),
            tab_id: selection
                .tab_id
                .clone()
                .ok_or_else(|| Failure::new(FailureKind::Rejected))?,
            panel_id: panel_id.clone(),
            member,
        },
    };
    spec.validate()
        .map_err(|_| Failure::new(FailureKind::Rejected))?;
    let current = client_control::windows::exchange(
        client,
        &snapshot.session_id,
        ClientViewAction::Get {
            client_id: selected.client_id.clone(),
        },
        deadline,
        signals,
    )?;
    if !matches!(current.outcome,ClientViewOutcome::Found { client }
        if client.view_revision==selected.view_revision && client.view==selected.view
            && client.availability==ClientViewAvailability::Current)
    {
        return Err(Failure::new(FailureKind::Rejected));
    }
    let command = ClientViewControl {
        operation_id: query::request_id()?,
        client_id: selected.client_id,
        expected_view_revision: selected.view_revision,
        minimum_workbench_revision: snapshot.revision,
        intent: ClientViewIntent::EditorOpen {
            spec: Box::new(spec),
        },
    };
    let mut answer = client_control::windows::submit_control(
        client,
        &snapshot.session_id,
        command.clone(),
        deadline,
        signals,
    )?;
    answer.retain_matching_command(&command);
    Ok(answer)
}

#[cfg(test)]
mod tests;
