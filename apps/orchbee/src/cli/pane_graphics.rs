//! Native visual writes and reads with exact receipts, no uncertain replay and no PTY input.
use super::{
    Failure, FailureKind, Output,
    console_interrupt::{ArmedSignals as Signals, Finish, SignalWatch},
    failure_output, output, query, results,
};
use crate::cli_text::CliText;
use crate::{
    invocation::{
        OutputFormat, QueryCommand,
        pane_graphics::{Action as CommandAction, Command},
    },
    local_client::{
        LocalClient, workbench::metadata_exchange::MetadataExchangeReason as ExchangeReason,
    },
    localization::Locale,
};
use orchbee_workbench_protocol::{
    PaneGraphicsAction as Action, PaneGraphicsFailure as Reason, PaneGraphicsOutcome as Outcome,
    PaneGraphicsRequest, PaneGraphicsResult, PaneGraphicsTarget, ProtocolVersion, Snapshot,
    WorkbenchPanelId,
};
use serde_json::json;
use std::time::{Duration, Instant};
mod framing;
mod stream;
mod upload;

pub(crate) fn run(command: &Command, format: OutputFormat, locale: Locale) -> Output {
    if let Err(failure) = super::execution::admit(super::execution::Capability::PaneGraphics) {
        return failure_output(failure, Some(format), locale);
    }
    let mut signals = match Signals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let mut result = execute(command, format, locale, &mut signals).unwrap_or_else(|failure| {
        let failure = if failure.details.is_none() {
            failure
                .failure_schema("orchbee.cli.failure.graphics")
                .details(serde_json::Map::from_iter([(
                    "stage".into(),
                    json!("graphics"),
                )]))
        } else {
            failure
        };
        failure_output(failure, Some(format), locale)
    });
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled { finalization, .. }) => {
            if result.status != 8 {
                result.status = 130;
            }
            result.finalization = Some(finalization);
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
    signals: &mut Signals,
) -> Result<Output, Failure> {
    if let CommandAction::Stream(spec) = &command.action {
        return stream::run(command, spec, format, locale, signals);
    }
    let deadline =
        Instant::now() + Duration::from_millis(u64::from(command.timeout_ms.unwrap_or(5_000)));
    signals.check_before(deadline)?;
    let image = match &command.action {
        CommandAction::Set(input) => Some(upload::acquire(input, deadline, signals)?),
        _ => None,
    };
    signals.check_before(deadline)?;
    let (mut client, snapshot) = query::stream_snapshot(deadline, signals.poll_fd())?;
    let target = resolve(command, &snapshot)?;
    let action = match &command.action {
        CommandAction::Set(_) => {
            let spec = image.ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
            Action::Set {
                target: target.clone(),
                layer_id: spec.layer_id,
                z_index: spec.z_index,
                image: spec.image,
            }
        }
        CommandAction::Clear(layer_id) => Action::Clear {
            target: target.clone(),
            layer_id: layer_id.clone(),
        },
        CommandAction::Info(client_id) => Action::Info {
            target: target.clone(),
            client_id: client_id.clone(),
        },
        CommandAction::Stream(_) => unreachable!("stream dispatched above"),
    };
    let request = request(&snapshot, action)?;
    let observation = exchange(&mut client, &request, deadline, signals)?;
    let status = observation.status();
    let document = json!({"schema":"orchbee.panel.graphics", "version":1, "operation":command.operation(), "requestId":request.request_id, "sessionId":snapshot.session_id, "target":target, "observation":observation.document()});
    let stdout = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text => {
            let mut text = CliText::new(locale).pane_graphics_status(status).to_owned();
            text.push_str(&format!(
                "{}\t{}\n",
                command.operation(),
                crate::cli_text::escape::escape(
                    &serde_json::to_string(&observation.document())
                        .map_err(|_| Failure::new(FailureKind::InternalFailure))?
                )
            ));
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
fn resolve(command: &Command, snapshot: &Snapshot) -> Result<PaneGraphicsTarget, Failure> {
    let results::Projection::PanelGet(panel) =
        results::project(&QueryCommand::PanelGet(command.target.clone()), snapshot)?
    else {
        return Err(Failure::new(FailureKind::InternalFailure));
    };
    if command.presentation.as_ref().is_some_and(|id| {
        id.as_str() != panel.primary_presentation.presentation_id
            && !panel
                .child_presentations
                .iter()
                .any(|child| child.presentation_id == id.as_str())
    }) {
        return Err(Failure::new(FailureKind::NotFound));
    }
    Ok(PaneGraphicsTarget {
        panel_id: WorkbenchPanelId::parse(&panel.row.panel_id)
            .map_err(|_| Failure::new(FailureKind::ProtocolInvalid))?,
        presentation_id: command.presentation.clone(),
    })
}
fn request(snapshot: &Snapshot, action: Action) -> Result<PaneGraphicsRequest, Failure> {
    let request = PaneGraphicsRequest {
        protocol_version: ProtocolVersion,
        request_id: query::request_id()?,
        session_id: snapshot.session_id.clone(),
        action,
    };
    request
        .validate()
        .map_err(|_| Failure::new(FailureKind::InvalidUsage))?;
    Ok(request)
}
enum Observation {
    Server(PaneGraphicsResult),
    Unknown(&'static str),
}
impl Observation {
    fn status(&self) -> u8 {
        match self {
            Self::Server(result) => status(&result.outcome),
            Self::Unknown(_) => 8,
        }
    }
    fn document(&self) -> serde_json::Value {
        match self {
            Self::Server(reply) => json!({"kind":"server", "reply":reply}),
            Self::Unknown(reason) => json!({"kind":"unknown", "reason":reason}),
        }
    }
}
fn exchange(
    client: &mut LocalClient,
    request: &PaneGraphicsRequest,
    deadline: Instant,
    signals: &mut Signals,
) -> Result<Observation, Failure> {
    signals.check_before(deadline)?;
    match client.pane_graphics(request, deadline, signals.poll_fd()) {
        Ok(reply) => Ok(Observation::Server(reply)),
        Err(error) if error.submitted && request.action.writes() => {
            Ok(Observation::Unknown(error.reason.as_str()))
        }
        Err(error) => {
            signals.check_before(deadline)?;
            Err(Failure::new(match error.reason {
                ExchangeReason::Protocol
                | ExchangeReason::Correlation
                | ExchangeReason::Encoding => FailureKind::ProtocolInvalid,
                ExchangeReason::Deadline | ExchangeReason::ResponseDeadline => FailureKind::Timeout,
                ExchangeReason::ConnectionUnavailable | ExchangeReason::NotSubmitted => {
                    FailureKind::ServiceUnavailable
                }
                _ => FailureKind::TransportLoss,
            }))
        }
    }
}
fn status(outcome: &Outcome) -> u8 {
    match outcome {
        Outcome::Failed {
            reason: Reason::PhysicalUnknown,
        } => 8,
        Outcome::Failed {
            reason:
                Reason::Unavailable
                | Reason::SourceUnavailable
                | Reason::RevisionExhausted
                | Reason::SessionMismatch,
        } => 4,
        Outcome::Failed { .. } => 3,
        // This command consumes its own observation or terminal receipt; unrelated control results have no projection in this phase.
        _ => 0,
    }
}
