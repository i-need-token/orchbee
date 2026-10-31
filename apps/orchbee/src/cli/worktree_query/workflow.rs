//! Resolves an admitted Worktree query source and reads the bounded correlated Server result
//! under one deadline and signal watch. It does not fetch remote Git state or mutate canonical
//! Workspace associations.

use super::{Failure, FailureKind, Output, emit, failure_output};
use crate::{
    cli::{
        console_interrupt::{ArmedSignals, Finish},
        query, worktree_source,
    },
    invocation::{OutputFormat, WorktreeQueryCommand},
    local_client::WorktreeQueryError,
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
    let prepared = worktree_source::workflow::prepare(
        worktree_source::workflow::Request {
            project: &None,
            workspace: &command.workspace,
            cwd: &command.cwd,
            client: command.client.as_ref(),
            focus: false,
        },
        &mut client,
        &snapshot,
        deadline,
        signals,
    )?;
    let request = WorkbenchWorktreeQuery {
        protocol_version: snapshot.protocol_version,
        request_id: query::request_id()?,
        session_id: snapshot.session_id,
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
