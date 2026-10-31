//! Popup commands address one current Client and preserve one mutation's receipt.
use crate::cli::plugin_preparation::{
    context,
    failure::{self, Reason},
};
use crate::cli_text::CliText;
use crate::{
    cli::{
        Failure, FailureKind, Output,
        caller_context::PanelTarget,
        client_control::{
            self, Answer,
            view_fence::{self, ViewFence},
        },
        console_interrupt::{ArmedSignals, Finish, SignalWatch},
        failure_output, output, query,
    },
    invocation::{OutputFormat, plugins::panes::Open},
    local_client::LocalClient,
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientPopupSource, ClientViewAction, ClientViewControl, ClientViewControlStatus,
    ClientViewIntent, ClientViewOutcome, ClientViewPhase, ClientViewRecord,
    PluginInvocationContext, PluginPopupSpec, Snapshot, WorkbenchClientId, WorkbenchRequestId,
};
use std::time::{Duration, Instant};

pub(super) fn open(command: &Open, format: OutputFormat, locale: Locale) -> Output {
    run(
        Some(command),
        command.creation.client.as_ref(),
        command.creation.timeout_ms,
        format,
        locale,
    )
}
pub(super) fn close(
    client: Option<&WorkbenchClientId>,
    timeout: u32,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    run(None, client, timeout, format, locale)
}
fn run(
    command: Option<&Open>,
    explicit: Option<&WorkbenchClientId>,
    timeout: u32,
    format: OutputFormat,
    locale: Locale,
) -> Output {
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
    let mut result = execute(command, explicit, deadline, &mut signals, format, locale)
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
    command: Option<&Open>,
    explicit: Option<&WorkbenchClientId>,
    deadline: Instant,
    signals: &mut ArmedSignals,
    format: OutputFormat,
    locale: Locale,
) -> Result<Output, Failure> {
    signals.check_before(deadline)?;
    let (mut client, snapshot) = query::stream_snapshot(deadline, signals.poll_fd())?;
    // Popup terminals deliberately have no canonical Pane; Close addresses a Client.
    let caller = if command.is_some() && explicit.is_none() {
        PanelTarget::prepare_optional(|name| std::env::var_os(name))?
            .map(|caller| caller.resolve(&snapshot))
            .transpose()?
    } else {
        None
    };
    let listed = client_control::exchange::exchange(
        &mut client,
        &snapshot.session_id,
        ClientViewAction::List {},
        deadline,
        signals,
    )?;
    let ClientViewOutcome::Listed { clients } = listed.outcome else {
        return Err(Reason::ClientListingUnavailable.failure());
    };
    let selected = client_control::selection::select(&clients, explicit, caller.as_ref())?.clone();
    let id = query::request_id()?;
    let answer = if let Some(command) = command {
        // A busy Client still answers the passive source reads and names why it refuses the
        // popup itself, such as an open modal surface or a window too small for it.
        if matches!(
            selected.view.phase,
            ClientViewPhase::Starting | ClientViewPhase::Unavailable
        ) {
            return Err(Reason::ClientNotReady.failure());
        }
        let selection = selected
            .view
            .selection
            .as_ref()
            .ok_or_else(|| Reason::BindingUnavailable.failure())?;
        let (context, member) = context::observe(
            &mut client,
            &snapshot,
            context::Source::current(Some(&selected.client_id)),
            PluginInvocationContext::default(),
            id.as_str(),
            deadline,
            signals,
        )
        .map_err(failure::client_refusal)?;
        let member = member.ok_or_else(|| Reason::BindingUnavailable.failure())?;
        let spec = PluginPopupSpec {
            selector: command
                .creation
                .options
                .plugin_pane
                .clone()
                .ok_or_else(|| Reason::SelectorUnavailable.failure())?,
            source: ClientPopupSource {
                terminal_context_id: selection.terminal_context_id.clone(),
                tab_id: selection
                    .tab_id
                    .clone()
                    .ok_or_else(|| Reason::BindingUnavailable.failure())?,
                panel_id: selection
                    .panel_id
                    .clone()
                    .ok_or_else(|| Reason::BindingUnavailable.failure())?,
                member,
            },
            context: Box::new(context),
            cwd: command.creation.options.cwd.clone(),
            environment: command.creation.options.environment.clone(),
            width: command.width,
            height: command.height,
        };
        spec.validate()
            .map_err(|_| Reason::InvalidPopupSpec.failure())?;
        open_popup(
            &mut client,
            &snapshot,
            &selected,
            id,
            spec,
            deadline,
            signals,
        )?
    } else {
        if ViewFence::Exact
            .reread(
                &mut client,
                &snapshot.session_id,
                &selected,
                deadline,
                signals,
            )?
            .is_none()
        {
            return Err(Reason::ClientChanged.failure());
        }
        client_control::exchange::submit_control(
            &mut client,
            &snapshot.session_id,
            ClientViewControl {
                operation_id: id,
                client_id: selected.client_id,
                expected_view_revision: selected.view_revision,
                minimum_workbench_revision: snapshot.revision,
                intent: ClientViewIntent::PopupClose {},
            },
            deadline,
            signals,
        )?
    };
    let mut document = answer.document();
    document["schema"] = serde_json::json!("orchbee.popup.result");
    document["version"] = serde_json::json!(1);
    let text = match answer.reply.as_ref().map(|reply| &reply.outcome) {
        Some(ClientViewOutcome::Operation { operation }) => match &operation.status {
            ClientViewControlStatus::PopupOpened { popup_id, .. } => {
                CliText::new(locale).cli_popup_opened(popup_id)
            }
            ClientViewControlStatus::PopupClosed { had_popup, .. } => {
                CliText::new(locale).cli_popup_closed(*had_popup).into()
            }
            ClientViewControlStatus::PresentationRead { .. }
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
            | ClientViewControlStatus::Incomplete { .. } => {
                CliText::new(locale).cli_client_control(&document)
            }
        },
        // This step consumes its requested ClientView receipt class; other replies cannot confirm that observation.
        _ => CliText::new(locale).cli_client_control(&document),
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

/// Opens the popup at the Client's current view. The source reads may have moved its view
/// revision without changing its selection, so each attempt addresses the Client's current
/// record; an open the Client refused only because its view moved or a transient step held it
/// is repeated as a new operation. Any other refusal fails as `client-refused` with the Client's
/// reason: nothing was opened.
fn open_popup(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    selected: &ClientViewRecord,
    mut operation_id: WorkbenchRequestId,
    mut spec: PluginPopupSpec,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Answer, Failure> {
    let mut attempt = 1;
    loop {
        let current = ViewFence::Selection
            .reread(client, &snapshot.session_id, selected, deadline, signals)?
            .ok_or_else(|| Reason::ClientChanged.failure())?;
        let answer = client_control::exchange::submit_control(
            client,
            &snapshot.session_id,
            ClientViewControl {
                operation_id: operation_id.clone(),
                client_id: current.client_id,
                expected_view_revision: current.view_revision,
                minimum_workbench_revision: snapshot.revision,
                intent: ClientViewIntent::PopupOpen {
                    spec: Box::new(spec.clone()),
                },
            },
            deadline,
            signals,
        )?;
        let reason = match answer.reply.as_ref().map(|reply| &reply.outcome) {
            Some(ClientViewOutcome::Operation { operation }) => match operation.status {
                ClientViewControlStatus::Rejected { reason } => reason,
                // Every other status keeps the retained receipt for the caller.
                _ => return Ok(answer),
            },
            // Every other reply keeps the retained receipt for the caller.
            _ => return Ok(answer),
        };
        if attempt == view_fence::ADDRESSES || !view_fence::readdressable(Some(reason)) {
            let mut refused = Failure::new(FailureKind::Rejected);
            refused.client_refusal = Some(reason);
            return Err(failure::client_refusal(refused));
        }
        view_fence::settle(Some(reason), deadline, signals)?;
        attempt += 1;
        operation_id = query::request_id()?;
        spec.context.correlation_id = Some(operation_id.to_string());
    }
}
