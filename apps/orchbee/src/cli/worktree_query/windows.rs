use super::{Failure, FailureKind, Output, emit, failure_output};
use crate::{
    cli::{
        query,
        source::{ArmedSignals, Finish, SignalWatch},
        worktree_source,
    },
    invocation::{OutputFormat, WorktreeQueryCommand},
    local_client::workbench::worktree_query::WorktreeQueryError,
    localization::Locale,
};
use orchbee_workbench_protocol::{WorkbenchWorktreeQuery, WorkbenchWorktreeQueryResult};
use std::time::{Duration, Instant};

pub(super) fn run(command: &WorktreeQueryCommand, format: OutputFormat, locale: Locale) -> Output {
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
    let result = observe(command, &mut signals, deadline);
    let mut finalization = None;
    let result = match signals.finish() {
        Ok(Finish::Clear) => result,
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            Err(Failure::new(FailureKind::Cancelled))
        }
        Err(_) => Err(Failure::new(FailureKind::InternalFailure)),
    };
    let mut output = match result {
        Ok(reply) => emit(reply, format, locale),
        Err(failure) => failure_output(failure, Some(format), locale),
    };
    output.finalization = finalization;
    output
}

fn observe(
    command: &WorktreeQueryCommand,
    signals: &mut ArmedSignals,
    deadline: Instant,
) -> Result<WorkbenchWorktreeQueryResult, Failure> {
    check(signals, deadline)?;
    let (mut client, _, snapshot) = query::snapshot_session_until(deadline)?;
    check(signals, deadline)?;
    let prepared = worktree_source::windows::prepare(
        worktree_source::windows::Request {
            project: &None,
            workspace: &command.workspace,
            cwd: &command.cwd,
            client: command.client.as_ref(),
            focus: false,
            require_project: false,
        },
        &mut client,
        &snapshot,
        deadline,
        signals,
    )?;
    let request = WorkbenchWorktreeQuery {
        protocol_version: snapshot.protocol_version,
        request_id: query::request_id()?,
        site_id: snapshot.site_id,
        source: prepared.source,
        trust_repository: command.trust_repository,
    };
    check(signals, deadline)?;
    let result = client.query_worktrees(request, deadline, signals.poll_fd());
    check(signals, deadline)?;
    result.map_err(|error| {
        Failure::new(match error {
            WorktreeQueryError::Unavailable => FailureKind::ServiceUnavailable,
            WorktreeQueryError::Encoding => FailureKind::InternalFailure,
            WorktreeQueryError::Protocol => FailureKind::ProtocolInvalid,
            WorktreeQueryError::Transport => FailureKind::TransportLoss,
            WorktreeQueryError::Timeout => FailureKind::Timeout,
        })
    })
}

fn check(signals: &mut ArmedSignals, deadline: Instant) -> Result<(), Failure> {
    if signals
        .take()
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?
        .is_some()
    {
        return Err(Failure::new(FailureKind::Cancelled));
    }
    if Instant::now() >= deadline {
        return Err(Failure::new(FailureKind::Timeout));
    }
    Ok(())
}
