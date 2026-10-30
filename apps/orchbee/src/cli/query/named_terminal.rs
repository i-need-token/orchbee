//! Route terminal-backed names without inventing canonical Agent observations.
use super::{
    BorrowedFd, Failure, FailureKind, Instant, LocalClient, Resource, Snapshot, Target,
    agent_target, agent_terminal, read_agent_observation, verify_agent_terminal,
};
use orchbee_agent_observation_protocol::{
    AgentLaunchBinding, AgentNameTarget, AgentSessionSelector,
};

pub(in crate::cli) use super::agent_target::resolve_named;

/// Resolve once, get once, and refuse a missing or ambiguous live binding.
/// The caller keeps the admission and original deadline for the terminal operation.
pub(crate) fn agent_terminal_target(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    target: &crate::invocation::AgentTarget,
    deadline: Instant,
) -> Result<Target, Failure> {
    if crate::cli::execution::admit(crate::cli::execution::Capability::AgentProjection).is_ok() {
        named_terminal_target(client, snapshot, target, deadline, None, false)
    } else {
        agent_terminal::resolve_agent_terminal(client, snapshot, target, deadline)
            .map(|resolved| resolved.terminal)
    }
}

pub(crate) fn named_terminal_target(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    target: &crate::invocation::AgentTarget,
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
    verify_process: bool,
) -> Result<Target, Failure> {
    let resolved = resolve_named(client, target, Some(snapshot), deadline, wake)?;
    match resolved.target {
        AgentNameTarget::Session { session_id } => {
            let observation = read_agent_observation(client, &session_id, deadline, wake)?;
            let terminal = agent_terminal::select_terminal_for_presentation(
                snapshot,
                &observation.session,
                resolved.presentation.as_deref(),
            )?;
            let selected = agent_terminal::ResolvedAgentTerminal {
                terminal,
                observation,
            };
            if verify_process {
                verify_agent_terminal(client, snapshot, selected, deadline)
                    .map(|item| item.terminal)
            } else {
                Ok(selected.terminal)
            }
        }
        AgentNameTarget::Terminal { binding } => {
            let row = terminal_row(snapshot, &binding)?;
            // The server validates creation, carrier and Host identities under its final
            // coordinator fence. Later terminal operations retain this snapshot's exact
            // logical worker/terminal pair. This preflight is not a process lease.
            verify_named_binding(client, &binding, deadline, wake)?;
            Ok(Target::Id(row.terminal_id))
        }
    }
}

pub(in crate::cli) fn terminal_row(
    snapshot: &Snapshot,
    binding: &AgentLaunchBinding,
) -> Result<super::super::results::TerminalRow, Failure> {
    super::super::results::terminal_rows(snapshot)
        .into_iter()
        .find(|row| row.presentation_id == binding.presentation_id.as_str())
        .ok_or_else(|| {
            super::super::results::not_found(
                Resource::Terminal,
                &Target::Id(binding.host_terminal_id.to_string()),
            )
            .source(super::super::results::snapshot_source(snapshot))
        })
}

pub(in crate::cli) fn verify_named_binding(
    client: &mut LocalClient,
    binding: &AgentLaunchBinding,
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<(), Failure> {
    let result = agent_target::resolve(
        client,
        AgentSessionSelector::Terminal {
            binding: binding.clone(),
        },
        deadline,
        wake,
    )?;
    match agent_target::resolved_target(&result)? {
        AgentNameTarget::Terminal { binding: current } if current == *binding => Ok(()),
        AgentNameTarget::Session { .. } | AgentNameTarget::Terminal { .. } => {
            Err(Failure::new(FailureKind::ProtocolInvalid)
                .failure_schema(crate::invocation::grammar::schemas::AGENT_NAMES_FAILURE_ID))
        }
    }
}
