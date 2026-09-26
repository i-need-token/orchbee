//! Optional first Project, one canonical Workspace launch, then separately receipted Client focus.
mod receipt;
mod source;
use super::{
    Failure, FailureKind, Output, default_project, failure_output, mutation, query,
    source::{ArmedSignals, Finish, SignalWatch},
    workspace_focus,
};
use crate::{
    invocation::{MutationCommand, OutputFormat, Target, WorkspaceCreationCommand},
    localization::Locale,
};
use orchbee_workbench_protocol::{
    WorkbenchLabel, WorkbenchPanelId, WorkbenchTabId, WorkbenchWorkspaceId,
};
use receipt::Receipt;
use std::time::{Duration, Instant};

pub(crate) fn run(
    command: &WorkspaceCreationCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let settings = match crate::cli::configuration::load() {
        Ok(settings) => settings,
        Err(error) => return failure_output(error.failure(), Some(format), locale),
    };
    let mut configured = command.clone();
    settings.apply_directory(&mut configured.directory, &mut configured.options);
    let command = &configured;

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
        failure.version = 2;
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
    let prepared = source::prepare(command, &mut client, &snapshot, deadline, signals, locale)?;
    signals.check_before(deadline)?;
    *output = Some(Receipt {
        project: None,
        workspace: None,
        directory: prepared.directory,
        client_id: prepared
            .selected
            .as_ref()
            .map(|client| client.client_id.clone()),
        requested_focus: command.focus,
        focus: None,
        stage: "project",
        failure: None,
    });
    let receipt = output.as_mut().ok_or_else(internal)?;
    let project = match prepared.project {
        default_project::Selection::Existing(project) => project,
        default_project::Selection::CreateDefault => {
            let (project, fresh) = default_project::create(
                &mut client,
                &snapshot,
                WorkbenchLabel::parse(locale.cli_default_project_label())
                    .map_err(|_| internal())?,
                deadline,
                signals,
                &mut receipt.project,
            )?;
            snapshot = fresh;
            project
        }
    };
    signals.check_before(deadline)?;
    receipt.stage = "workspace";
    let mut options = command.options.clone();
    options.cwd = Some(receipt.directory.path.clone());
    receipt.workspace = Some(mutation::submit_creation(
        &mut client,
        &snapshot,
        &MutationCommand::WorkspaceLaunch {
            project: Target::Id(project.to_string()),
            label: prepared.label,
            geometry: command.geometry,
            launch: command.launch.clone(),
            profile: command.profile.clone(),
            options,
        },
        deadline,
        signals.poll_fd(),
    )?);
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
                project: &project,
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
