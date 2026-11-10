//! One atomic Workspace launch with optional organization, then separately receipted Client focus.
mod canonical;
mod receipt;
mod source;
use super::{
    Failure, FailureKind, Output,
    console_interrupt::{ArmedSignals, Finish, SignalWatch},
    failure_output, mutation, query, workspace_focus,
};
use crate::{
    invocation::{
        MutationCommand, OutputFormat, ParsedWorkspaceCreationCommand, Target,
        WorkspaceCreationCommand,
    },
    localization::Locale,
};
use orchbee_workbench_protocol::{WorkbenchPanelId, WorkbenchTabId, WorkbenchWorkspaceId};
use receipt::Receipt;
use std::time::{Duration, Instant};

pub(crate) fn run(
    command: ParsedWorkspaceCreationCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if super::execution::admit(super::execution::Capability::ExtendedCreation).is_ok() {
        run_extended(command, format, locale)
    } else {
        canonical::run(command, format, locale)
    }
}

fn run_extended(
    command: ParsedWorkspaceCreationCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let named = matches!(
        command.launch,
        Some(crate::invocation::program::ParsedLaunch::Path { .. })
    );
    let command =
        match command.into_resolved(format, |name| std::env::var_os(name), std::env::current_dir) {
            Ok(command) => command,
            Err(failure) => return super::parse_failure(failure, locale),
        };
    let launch = command.launch.clone();
    let output = run_resolved(command, format, locale);
    super::launch_receipt::annotate(output, launch.as_ref().filter(|_| named), format, locale)
}

fn run_resolved(command: WorkspaceCreationCommand, format: OutputFormat, locale: Locale) -> Output {
    let command = &command;
    let deadline = Instant::now() + Duration::from_millis(u64::from(command.timeout_ms));
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => return failure_output(internal(), Some(format), locale),
    };
    let mut receipt = None;
    let mut result = execute(command, deadline, &mut signals, &mut receipt, locale);
    let mut finalization = None;
    let mut cancelled = false;
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            cancelled = true;
            if !matches!(&result,Err(error) if error.kind==FailureKind::OutcomeUnknown) {
                result = Err(Failure::new(FailureKind::Cancelled));
            }
        }
        Err(_) => {
            if !matches!(&result,Err(error) if error.kind==FailureKind::OutcomeUnknown) {
                result = Err(internal());
            }
        }
    }
    let mut output = if let Some(mut receipt) = receipt {
        if let Err(error) = result {
            receipt.failure = Some(error);
        }
        receipt.emit(format, locale)
    } else {
        failure_output(
            preparation_failure(result.err().unwrap_or_else(internal)),
            Some(format),
            locale,
        )
    };
    if cancelled {
        output.status = 130;
    }
    output.finalization = finalization;
    output
}
fn preparation_failure(mut failure: Failure) -> Failure {
    if failure.details.is_none()
        && failure.failure_schema.is_none()
        && matches!(
            failure.kind,
            FailureKind::NotFound
                | FailureKind::ServiceUnavailable
                | FailureKind::ProtocolInvalid
                | FailureKind::TransportLoss
                | FailureKind::Timeout
                | FailureKind::Cancelled
        )
    {
        failure.details = Some(super::stage_details("workspace-creation"));
    }
    failure
}

fn internal() -> Failure {
    Failure::new(FailureKind::InternalFailure)
}
fn execute(
    command: &WorkspaceCreationCommand,
    deadline: Instant,
    signals: &mut ArmedSignals,
    output: &mut Option<Receipt>,
    locale: Locale,
) -> Result<(), Failure> {
    signals.check_before(deadline)?;
    let (mut client, _, mut snapshot) = query::snapshot_session_until(deadline)?;
    let mut configured = command.clone();
    super::creation_defaults::apply_directory(
        &mut client,
        &snapshot.session_id,
        deadline,
        signals,
        &mut configured.directory,
        &mut configured.options,
    )?;
    let command = &configured;
    // Every stale-view retry prepares the label, its first-free suffix included, the directory,
    // the Project and the Client again from the fresh snapshot (ORC-413).
    let (prepared, workspace) = mutation::submit_creation(
        &mut client,
        &mut snapshot,
        deadline,
        signals,
        |client, snapshot, signals| {
            let prepared = source::prepare(command, client, snapshot, deadline, signals, locale)?;
            signals.check_before(deadline)?;
            let client_id = prepared
                .selected
                .as_ref()
                .map(|client| client.client_id.clone());
            // A resubmission carries the same Client and directory, so the first attempt's
            // receipt describes every submitted attempt and any refusal that stands.
            output.get_or_insert_with(|| Receipt {
                workspace: None,
                directory: prepared.directory.clone(),
                client_id: client_id.clone(),
                requested_focus: command.focus,
                focus: None,
                stage: "workspace",
                failure: None,
            });
            signals.check_before(deadline)?;
            let mut options = command.options.clone();
            options.cwd = Some(prepared.directory.path.clone());
            let launch = MutationCommand::WorkspaceLaunch {
                project: prepared
                    .project
                    .as_ref()
                    .map(|project| Target::Id(project.to_string())),
                label: prepared.label.clone(),
                automatic_label: prepared.automatic_label,
                geometry: command.geometry,
                launch: command.launch.clone(),
                profile: command.profile.clone(),
                options,
            };
            // Besides the launch itself, a resubmission must find the same Client and
            // directory provenance.
            let derived = (client_id, prepared.directory.document());
            Ok((prepared, launch, derived))
        },
    )?;
    let receipt = output.as_mut().ok_or_else(internal)?;
    receipt.workspace = Some(workspace);
    let canonical = receipt.workspace.as_ref().ok_or_else(internal)?;
    receipt.stage = "terminal";
    if let Some(kind) = canonical.launch_failure() {
        return Err(Failure::new(kind));
    }
    if command.focus {
        receipt.stage = "focus";
        let get = |name| canonical.id_value(name).ok_or_else(internal);
        let workspace = WorkbenchWorkspaceId::parse(get("workspaceId")?).map_err(|_| internal())?;
        let tab = WorkbenchTabId::parse(get("tabId")?).map_err(|_| internal())?;
        let panel = WorkbenchPanelId::parse(get("panelId")?).map_err(|_| internal())?;
        let minimum = canonical.committed_revision().ok_or_else(internal)?;
        receipt.focus = Some(workspace_focus::execute(
            &mut client,
            &snapshot,
            workspace_focus::Target {
                workspace: &workspace,
                minimum,
                navigation: workspace_focus::Navigation::Created {
                    tab: &tab,
                    panel: &panel,
                },
            },
            prepared.selected.as_ref().ok_or_else(internal)?,
            deadline,
            signals,
        )?);
    }
    if receipt.complete() {
        receipt.stage = "complete";
    }
    Ok(())
}
