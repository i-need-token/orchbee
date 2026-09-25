//! One absolute budget, one admitted session, and optional read-only Client discovery.
use super::{
    Locale, Output, OutputFormat, PanelQueryTarget, QueryCommand, failure_output, success_output,
};
use crate::cli::{
    Failure, FailureKind,
    caller_context::PanelTarget,
    client_control::{selection, windows::exchange},
    query, results,
    source::{ArmedSignals, Finish, SignalWatch},
    stage_details,
};
use orchbee_workbench_protocol::{ClientViewAction, ClientViewOutcome};
use serde_json::Value;
use std::time::{Duration, Instant};

pub(super) fn run(
    command: &QueryCommand,
    target: &PanelQueryTarget,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let deadline = Instant::now() + Duration::from_secs(5);
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
    let result = collect(command, target, deadline, &mut signals);
    let mut finalization = None;
    let result = match signals.finish() {
        Ok(Finish::Clear) => result,
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            Err(failure(FailureKind::Cancelled))
        }
        Err(_) => Err(Failure::new(FailureKind::InternalFailure)),
    };
    let mut output = match result {
        Ok((source, projection)) => success_output(projection, source, command, format, locale),
        Err(error) => failure_output(error, Some(format), locale),
    };
    output.finalization = finalization;
    output
}

fn collect(
    command: &QueryCommand,
    target: &PanelQueryTarget,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<(Value, results::Projection), Failure> {
    let prepared = match target {
        PanelQueryTarget::Panel(panel) => Some(PanelTarget::prepare(Some(panel), |_| None)?),
        PanelQueryTarget::Current => PanelTarget::prepare_optional(|name| std::env::var_os(name))?,
        PanelQueryTarget::Client(_) => None,
    };
    check_pending(deadline, signals)?;
    let snapshot_result = query::snapshot_session_until(deadline);
    check_pending(deadline, signals)?;
    let (mut client, _, snapshot) = snapshot_result?;
    let source = results::snapshot_source(&snapshot);
    let panel = if let Some(prepared) = prepared {
        prepared
            .resolve(&snapshot)
            .map_err(|error| error.source(source.clone()))?
    } else {
        let reply = exchange(
            &mut client,
            &snapshot.site_id,
            ClientViewAction::List {},
            deadline,
            signals,
        );
        check_pending(deadline, signals)?;
        let reply = reply.map_err(|error| match error.kind {
            FailureKind::InternalFailure => error,
            _ => error.details(stage_details("current-panel")),
        })?;
        let clients = match reply.outcome {
            ClientViewOutcome::Listed { clients } => clients,
            ClientViewOutcome::Denied {} => return Err(Failure::new(FailureKind::Denied)),
            ClientViewOutcome::Unavailable { .. } => {
                return Err(failure(FailureKind::ServiceUnavailable));
            }
            _ => return Err(failure(FailureKind::ProtocolInvalid)),
        };
        let explicit = match target {
            PanelQueryTarget::Client(id) => Some(id),
            _ => None,
        };
        selection::current_panel(selection::select(&clients, explicit, None)?, &snapshot)?
    };
    check_pending(deadline, signals)?;
    let result = match command {
        QueryCommand::PanelCurrent(_) => {
            results::project(&QueryCommand::PanelGet(panel), &snapshot)
                .map(|projection| (source.clone(), projection))
        }
        QueryCommand::PanelProcessInfo(_) => {
            let terminal = query::primary_terminal(&snapshot, &panel)
                .map_err(|error| error.source(source.clone()))?;
            query::read_terminal_processes_cancellable(
                &mut client,
                deadline,
                &snapshot,
                &terminal,
                signals.poll_fd(),
            )
            .map(|(source, document)| (source, results::Projection::PanelProcessInfo(document)))
        }
        _ => Err(Failure::new(FailureKind::InternalFailure)),
    };
    // Cancellation/expiry wins over a provisional transport fault; session drops before output.
    check_pending(deadline, signals)?;
    result.map_err(|error| error.source(source))
}

fn failure(kind: FailureKind) -> Failure {
    Failure::new(kind).details(stage_details("current-panel"))
}

fn check_pending(deadline: Instant, signals: &mut ArmedSignals) -> Result<(), Failure> {
    if signals
        .take()
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?
        .is_some()
    {
        Err(failure(FailureKind::Cancelled))
    } else if Instant::now() >= deadline {
        Err(failure(FailureKind::Timeout))
    } else {
        Ok(())
    }
}
