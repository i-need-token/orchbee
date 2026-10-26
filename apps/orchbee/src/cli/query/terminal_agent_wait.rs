//! Physical Agent waits hold one Client Connection and consume contiguous semantic history.
//! They never infer canonical outcomes or Client acknowledgements from terminal reports.
use std::time::{Duration, Instant};

use orchbee_agent_observation_protocol::{AgentLaunchBinding, AgentNameTarget, AgentSafeInteger};
use orchbee_workbench_protocol::{Snapshot, TerminalAgentChange, WorkbenchRevision};
use serde_json::{Value, json};

use super::terminal_agent::{self, TerminalAgentRead, failure as error};
use crate::cli::source::{ArmedSignals, SignalWatch};
use crate::cli::wait_deadline::WaitDeadline;
use crate::cli::{Failure, FailureKind};
use crate::invocation::{AgentTarget, AgentWaitCondition};
use crate::local_client::LocalClient;

mod state;
pub(in crate::cli) use state::admit;

const READ_BUDGET: Duration = Duration::from_secs(5);

/// A physical match retains the latest read separately from a transient matched change.
#[derive(Debug)]
pub(crate) struct Matched {
    pub(in crate::cli) client_view:
        Option<crate::cli::agent_view::windows::terminal_attention::Projection>,
    pub(in crate::cli) read: TerminalAgentRead,
    pub(in crate::cli) conditions: Vec<AgentWaitCondition>,
    pub(in crate::cli) change: Option<TerminalAgentChange>,
    until: Vec<AgentWaitCondition>,
    after_revision: Option<AgentSafeInteger>,
    timeout_ms: Option<u32>,
    observations: u64,
    pub(in crate::cli) elapsed_ms: u64,
}
impl Matched {
    pub(in crate::cli) fn document(&self) -> Value {
        let wait = json!({"until":self.until.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
            "afterRevision":self.after_revision,"timeoutMs":self.timeout_ms,"observations":self.observations,"elapsedMs":self.elapsed_ms});
        let mut result = json!({"kind":"terminal-matched","target":{"kind":"terminal","binding":self.read.observation.binding},
            "semantic":self.read.document(),"matchedConditions":self.conditions.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
            "matchedChange":self.change,"wait":wait});
        result["clientView"] = self
            .client_view
            .as_ref()
            .map_or(Value::Null, |view| view.document());
        result
    }
}

pub(in crate::cli) struct Options<'a> {
    pub selected: Option<&'a orchbee_workbench_protocol::WorkbenchClientId>,
    pub target: &'a AgentTarget,
    pub binding: &'a AgentLaunchBinding,
    pub until: &'a [AgentWaitCondition],
    pub after_revision: Option<AgentSafeInteger>,
    pub timeout_ms: Option<u32>,
    pub start: Instant,
    pub total: WaitDeadline,
}

/// Revalidate the original selector and physical identity in the same bounded read span.
pub(in crate::cli) fn read_pinned(
    client: &mut LocalClient,
    target: &AgentTarget,
    binding: &AgentLaunchBinding,
    after: Option<WorkbenchRevision>,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<(Snapshot, TerminalAgentRead), Failure> {
    interrupt(signals)?;
    let snapshot = super::agent_target::read_snapshot(client, deadline, signals.poll_fd())?;
    let read = terminal_agent::read(
        client,
        &snapshot,
        binding,
        after,
        deadline,
        signals.poll_fd(),
    )?;
    let resolved = super::agent_target::resolve_named(
        client,
        target,
        Some(&snapshot),
        deadline,
        signals.poll_fd(),
    )?;
    if resolved.target
        != (AgentNameTarget::Terminal {
            binding: binding.clone(),
        })
    {
        return Err(error(FailureKind::NotFound, "binding-changed"));
    }
    interrupt(signals)?;
    if Instant::now() >= deadline {
        return Err(error(FailureKind::Timeout, "read-deadline"));
    }
    Ok((snapshot, read))
}

/// `initial` is the checkpoint immediately before accepted prompt submission; absent for plain wait.
pub(in crate::cli) fn observe(
    client: &mut LocalClient,
    options: Options<'_>,
    initial: Option<TerminalAgentRead>,
    signals: &mut ArmedSignals,
) -> Result<Matched, Failure> {
    let prompt = initial.is_some();
    let effect = options.total.step(Instant::now(), READ_BUDGET)?;
    let mut observations = 0_u64;
    let first = match initial {
        Some(read) => read,
        None => {
            let (_, read) = read_pinned(
                client,
                options.target,
                options.binding,
                None,
                effect,
                signals,
            )
            .map_err(|failure| read_failure(failure, &options, false, effect, observations))?;
            observations += 1;
            read
        }
    };
    if options.total.expired(Instant::now()) {
        return Err(timeout(&options, "wait-deadline", observations));
    }
    let mut state = state::State::new(&first, options.until, options.after_revision, prompt)?;
    // With a selected Client, read its acknowledgement before the next pinned Server sample.
    // The pre-send checkpoint remains exclusively the prompt activity fence.
    if options.selected.is_none()
        && let Some(matched) = state.initial_match(&first)
    {
        return Ok(matched_result(&options, first, matched, observations));
    }
    let mut backoff = Duration::from_millis(50);
    loop {
        let now = Instant::now();
        let gated = prompt && !state.active;
        if options.total.expired(now) {
            return Err(timeout(&options, "wait-deadline", observations));
        }
        if gated && now >= effect {
            return Err(timeout(&options, "effect-deadline", observations));
        }
        let mut deadline = options.total.step(now, READ_BUDGET)?;
        if gated {
            deadline = deadline.min(effect);
        }
        let attention = options
            .selected
            .map(|selected| {
                let snapshot =
                    super::agent_target::read_snapshot(client, deadline, signals.poll_fd())?;
                crate::cli::agent_view::windows::terminal_attention::read(
                    client,
                    &snapshot,
                    selected,
                    options.binding,
                    deadline,
                    signals,
                )
            })
            .transpose()
            .map_err(|failure| read_failure(failure, &options, gated, effect, observations))?;
        let (_, read) = read_pinned(
            client,
            options.target,
            options.binding,
            Some(state.sequence),
            deadline,
            signals,
        )
        .map_err(|failure| read_failure(failure, &options, gated, effect, observations))?;
        observations = observations.saturating_add(1);
        let client_view = attention.map(|a| a.corroborate(&read));
        let now = Instant::now();
        if options.total.expired(now) {
            return Err(timeout(&options, "wait-deadline", observations));
        }
        if gated && now >= effect {
            return Err(timeout(&options, "effect-deadline", observations));
        }
        if let Some(matched) = state.observe(&read)? {
            let mut result = matched_result(&options, read, matched, observations);
            result.client_view = client_view;
            return Ok(result);
        }
        let mut pause = options.total.backoff(now, backoff);
        if prompt && !state.active {
            pause = pause.min(effect.saturating_duration_since(now));
        }
        if super::wait_backoff(signals.poll_fd(), pause) {
            return Err(error(FailureKind::InternalFailure, "interrupted"));
        }
        backoff = (backoff * 2).min(Duration::from_millis(500));
    }
}

pub(in crate::cli) fn interrupt(signals: &mut ArmedSignals) -> Result<(), Failure> {
    if super::wait_backoff(signals.poll_fd(), Duration::from_millis(1)) {
        Err(error(FailureKind::InternalFailure, "interrupted"))
    } else {
        Ok(())
    }
}

fn matched_result(
    options: &Options<'_>,
    read: TerminalAgentRead,
    matched: state::Match,
    observations: u64,
) -> Matched {
    Matched {
        client_view: None,
        read,
        conditions: matched.conditions,
        change: matched.change,
        until: options.until.to_vec(),
        after_revision: options.after_revision,
        timeout_ms: options.timeout_ms,
        observations,
        elapsed_ms: u64::try_from(options.start.elapsed().as_millis()).unwrap_or(u64::MAX),
    }
}
fn timeout(options: &Options<'_>, reason: &'static str, observations: u64) -> Failure {
    let mut failure = error(FailureKind::Timeout, reason);
    let details = failure.details.get_or_insert_default();
    details.insert("timeoutMs".into(), json!(options.timeout_ms));
    details.insert(
        "elapsedMs".into(),
        json!(u64::try_from(options.start.elapsed().as_millis()).unwrap_or(u64::MAX)),
    );
    details.insert("completedObservations".into(), json!(observations));
    failure
}
fn read_failure(
    failure: Failure,
    options: &Options<'_>,
    gated: bool,
    effect: Instant,
    observations: u64,
) -> Failure {
    if failure.kind != FailureKind::Timeout {
        return failure;
    }
    let now = Instant::now();
    timeout(
        options,
        if options.total.expired(now) {
            "wait-deadline"
        } else if gated && now >= effect {
            "effect-deadline"
        } else {
            "read-deadline"
        },
        observations,
    )
}
