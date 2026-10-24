//! Bounded Windows layout application; retained effects survive incomplete composition.
mod engine;
mod exchange;
mod ownership;
mod receipt;
mod targeting;
use super::{launch, template, workflow};
use crate::{
    cli::{
        Failure, FailureKind, Output,
        caller_context::PanelTarget,
        client_control::{selection, windows as control},
        query,
        source::{self, ArmedSignals, Finish, SignalWatch, SourceFailure},
    },
    invocation::{LayoutApplyCommand, LayoutDestination, OutputFormat},
    localization::Locale,
};
use orchbee_workbench_protocol::{ClientViewAction, ClientViewOutcome, ClientViewPhase};
use serde_json::{Map, json};
use std::time::{Duration, Instant};

pub(super) fn error(kind: FailureKind, reason: &'static str) -> Failure {
    Failure::new(kind)
        .failure_schema("orchbee.cli.failure.layout-apply")
        .details(Map::from_iter([("reason".into(), json!(reason))]))
}

/// Apply a fully admitted template and retain every known effect when later phases fail.
pub(crate) fn run(command: &LayoutApplyCommand, format: OutputFormat, locale: Locale) -> Output {
    let total = Duration::from_millis(u64::from(command.timeout_ms));
    let deadline = Instant::now() + total;
    let work_deadline = deadline - total.div_f32(5.0).min(Duration::from_secs(2));
    let mut receipt = receipt::Receipt::new();
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(value) => value,
        Err(_) => {
            return crate::cli::failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let mut result = execute(command, work_deadline, deadline, &mut signals, &mut receipt);
    let mut finalization = None;
    let mut cancelled = false;
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            cancelled = true;
            if !matches!(&result,Err(failure) if failure.kind==FailureKind::OutcomeUnknown) {
                result = Err(error(FailureKind::Cancelled, "interrupted"));
            }
        }
        Err(_) => {
            if result.is_ok() {
                result = Err(error(FailureKind::InternalFailure, "interrupted"));
            }
        }
    }
    let mut output = receipt.emit(result.err(), format, locale);
    if cancelled {
        output.status = 130;
    }
    output.finalization = finalization;
    output
}

fn execute(
    command: &LayoutApplyCommand,
    work_deadline: Instant,
    deadline: Instant,
    signals: &mut ArmedSignals,
    receipt: &mut receipt::Receipt,
) -> Result<(), Failure> {
    signals.check_before(work_deadline)?;
    let bytes = source::acquire_document(
        command.file.as_deref(),
        template::MAX_BYTES,
        work_deadline,
        signals,
    )
    .map_err(|failure| match failure {
        SourceFailure::Standard(failure) => failure,
        SourceFailure::Timeout { .. } => error(FailureKind::Timeout, "input-unavailable"),
        SourceFailure::Cancelled { .. } => error(FailureKind::Cancelled, "input-unavailable"),
    })?;
    let plan = template::parse(&bytes)
        .map_err(|error| self::error(FailureKind::InvalidUsage, error.reason()))?
        .plan();
    let prepared = launch::prepare(&plan, work_deadline, signals)?;
    receipt.stage = "discovery";
    let needs_client = command.client.is_some()
        || command.focus
        || matches!(command.destination, LayoutDestination::New(None));
    let caller = if needs_client && command.client.is_none() {
        PanelTarget::prepare_optional(|name| std::env::var_os(name))?
    } else {
        None
    };
    let session = query::stream_snapshot(work_deadline, signals.poll_fd());
    signals.check_before(work_deadline)?;
    let (mut client, snapshot) = session?;
    let caller = caller.map(|value| value.resolve(&snapshot)).transpose()?;
    let selected = if needs_client {
        let reply = control::exchange(
            &mut client,
            &snapshot.site_id,
            ClientViewAction::List {},
            work_deadline,
            signals,
        )?;
        let clients = match reply.outcome {
            ClientViewOutcome::Listed { clients } => clients,
            ClientViewOutcome::Denied {} => {
                return Err(error(FailureKind::Denied, "client-unavailable"));
            }
            _ => return Err(error(FailureKind::ServiceUnavailable, "client-unavailable")),
        };
        let selected =
            selection::select(&clients, command.client.as_ref(), caller.as_ref())?.clone();
        if selected.view.phase != ClientViewPhase::Ready {
            return Err(error(FailureKind::ServiceUnavailable, "client-unavailable"));
        }
        Some(selected)
    } else {
        None
    };
    let destination = targeting::resolve(command, &snapshot, selected.as_ref())?;
    receipt.source = Some(
        json!({"kind":"layout-application","siteId":snapshot.site_id,"baseRevision":snapshot.revision,"clientId":selected.as_ref().map(|client| &client.client_id)}),
    );
    receipt.old_tab = destination.old.as_ref().map(|tab| tab.tab_id.clone());
    let mut engine = engine::Engine::new(
        command,
        &plan,
        prepared,
        destination,
        client,
        snapshot,
        selected,
        work_deadline,
        deadline,
        signals,
        receipt,
    );
    engine.prepare_directory()?;
    workflow::execute(&plan, &mut engine)
}
