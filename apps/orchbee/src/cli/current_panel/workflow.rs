//! One absolute budget, one admitted session, and optional read-only Client discovery.
use super::{
    Locale, Output, OutputFormat, PanelQueryTarget, QueryCommand, failure_output, success_output,
};
use crate::cli::{
    Failure, FailureKind,
    caller_context::PanelTarget,
    client_control::{exchange::exchange, selection},
    console_interrupt::{ArmedSignals, Finish},
    query, results, stage_details, workbench_query,
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
    let snapshot_result = query::stream_snapshot(deadline, signals.poll_fd());
    check_pending(deadline, signals)?;
    let (mut client, snapshot) = snapshot_result?;
    let source = results::snapshot_source(&snapshot);
    let (panel, caller_terminal, selected) = if let Some(prepared) = prepared {
        let (panel, terminal) = prepared
            .resolve_exact(&snapshot)
            .map_err(|error| error.source(source.clone()))?;
        (panel, terminal, None)
    } else {
        let reply = exchange(
            &mut client,
            &snapshot.session_id,
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
            ClientViewOutcome::Synchronized { .. }
            | ClientViewOutcome::Found { .. }
            | ClientViewOutcome::Operation { .. }
            | ClientViewOutcome::NotFound { .. }
            | ClientViewOutcome::Conflict { .. } => {
                return Err(failure(FailureKind::ProtocolInvalid));
            }
        };
        let explicit = match target {
            PanelQueryTarget::Client(id) => Some(id),
            _ => None,
        };
        let selected = selection::select(&clients, explicit, None)?;
        let panel = if matches!(command, QueryCommand::PanelCurrent(_)) {
            selection::observed_panel(selected, &snapshot)?
        } else {
            selection::current_panel(selected, &snapshot)?
        };
        (panel, None, Some(selected.clone()))
    };
    check_pending(deadline, signals)?;
    let result = match command {
        QueryCommand::PanelCurrent(_) => (|| {
            use crate::cli::current_presentation::{self, Origin, Resolved};
            let projection = results::project(&QueryCommand::PanelGet(panel), &snapshot)?;
            let results::Projection::PanelGet(doc) = &projection else {
                return Err(Failure::new(FailureKind::InternalFailure));
            };
            let origin = if let Some(record) = &selected {
                let panel_id =
                    orchbee_workbench_protocol::WorkbenchPanelId::parse(&doc.row.panel_id)
                        .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
                Origin::Client(Box::new(current_presentation::read_with_receipt(
                    &mut client,
                    &snapshot,
                    record,
                    &panel_id,
                    crate::cli::client_control::view_fence::ViewFence::Exact,
                    deadline,
                    signals,
                )?))
            } else {
                caller_terminal.map_or(Origin::Primary, Origin::Caller)
            };
            let member = Resolved::new(doc, origin)?;
            workbench_query::collect(
                &mut client,
                &snapshot,
                projection,
                selected,
                Some(member),
                deadline,
                signals,
            )
            .map(|(source, document)| {
                (
                    source,
                    results::Projection::WorkbenchQuery(Box::new(document)),
                )
            })
        })(),
        QueryCommand::PanelProcessInfo(_) => (|| {
            let panel_id = selected
                .as_ref()
                .and_then(|record| record.view.selection.as_ref())
                .and_then(|selection| selection.panel_id.clone());
            let member = if let (Some(selected), Some(panel_id)) = (&selected, &panel_id) {
                Some(crate::cli::current_presentation::read(
                    &mut client,
                    &snapshot,
                    selected,
                    panel_id,
                    deadline,
                    signals,
                )?)
            } else {
                None
            };
            let terminal = if let Some(terminal) = caller_terminal {
                crate::invocation::Target::Id(terminal.to_string())
            } else if let Some(member) = &member {
                let binding = member
                    .binding()
                    .ok_or_else(|| failure(FailureKind::NotFound))?;
                crate::invocation::Target::Id(binding.terminal_id.to_string())
            } else {
                query::primary_terminal(&snapshot, &panel)
                    .map_err(|error| error.source(source.clone()))?
            };
            let result = query::read_terminal_processes_cancellable(
                &mut client,
                deadline,
                &snapshot,
                &terminal,
                signals.poll_fd(),
            )
            .map(|(source, document)| (source, results::Projection::PanelProcessInfo(document)))?;
            let final_snapshot = query::refresh_snapshot(
                &mut client,
                &snapshot.session_id,
                snapshot.revision,
                deadline,
                signals,
            )?;
            if final_snapshot.revision != snapshot.revision {
                return Err(crate::cli::query::workbench_failure("workbench-changed"));
            }
            if let (Some(selected), Some(panel_id), Some(original)) =
                (&selected, &panel_id, &member)
            {
                let final_member = crate::cli::current_presentation::read(
                    &mut client,
                    &snapshot,
                    selected,
                    panel_id,
                    deadline,
                    signals,
                )?;
                if !original.same_member(&final_member) {
                    return Err(crate::cli::query::workbench_failure("client-changed"));
                }
            }
            Ok(result)
        })(),
        _ => Err(Failure::new(FailureKind::InternalFailure)),
    };
    // Cancellation/expiry wins over a provisional transport fault; session drops before output.
    check_pending(deadline, signals)?;
    result.map_err(|mut error| {
        // Enrichment failures have their own closed envelopes and provenance contracts.
        if error.failure_schema.is_some() {
            error
        } else {
            error
                .details
                .get_or_insert_with(|| stage_details("current-panel"));
            error.source(source)
        }
    })
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
