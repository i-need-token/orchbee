use super::{
    AgentCatalog, AgentCatalogSnapshot, AgentRowState, LOOKUP_DEADLINE, LookupStatus,
    POLL_INTERVAL, REFRESH_FLOOR, Step, Ticket,
};
use orchbee_agent_observation_protocol::STARTABLE_AGENT_PROVIDER_IDS;
use orchbee_capability_node_protocol::{
    AgentProbeJobAction as Action, AgentProbeJobOutcome as Outcome,
    AgentProbeJobPollOutcome as Poll, AgentProbeJobResult, AgentProbeJobStartOutcome as Start,
    AgentProbeResult, OperationId, WorkerId,
};
use std::{
    num::NonZeroU64,
    time::{Duration, Instant},
};

const UNIX_MS: u64 = 1_794_300_000_000;

fn job() -> OperationId {
    OperationId::from_bytes([1; 16]).unwrap()
}

fn generation(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).unwrap()
}

fn begin(catalog: &AgentCatalog, value: u64) {
    catalog.begin(WorkerId::from_bytes([7; 16]).unwrap(), generation(value));
}

/// The exchange the pump takes at `now`, which must be due.
fn exchange(catalog: &AgentCatalog, now: Instant) -> (Ticket, Action) {
    match catalog.next(now) {
        Step::Exchange { ticket, action, .. } => (ticket, action),
        step => panic!("no exchange is due: {step:?}"),
    }
}

fn answer(
    catalog: &AgentCatalog,
    ticket: Ticket,
    outcome: Option<Outcome>,
    now: Instant,
) -> AgentCatalogSnapshot {
    let result = outcome.map(|outcome| AgentProbeJobResult {
        operation_id: job(),
        job_id: job(),
        outcome,
    });
    catalog.apply(ticket, result, now, UNIX_MS);
    catalog.read().unwrap()
}

fn state(snapshot: &AgentCatalogSnapshot, id: &str) -> AgentRowState {
    snapshot
        .agents
        .iter()
        .find(|row| row.provider_id == id)
        .unwrap()
        .state
}

#[test]
fn a_first_lookup_absorbs_refreshes_and_a_failed_later_one_keeps_its_rows() {
    let catalog = AgentCatalog::default();
    let start = Instant::now();
    assert_eq!(catalog.read(), None, "no Worker yet");
    assert_eq!(catalog.next(start), Step::Wait(None));

    // First lookup: once the Worker registers, with only checkable commands.
    begin(&catalog, 1);
    let first = catalog.read().unwrap();
    assert_eq!(
        (first.revision.get(), first.status, first.checked_at),
        (1, LookupStatus::Running, None)
    );
    assert!(first.recent.is_empty());
    let ids: Vec<_> = first.agents.iter().map(|row| row.provider_id).collect();
    assert_eq!(ids, STARTABLE_AGENT_PROVIDER_IDS);
    assert_eq!(state(&first, "codex"), AgentRowState::Checking);
    assert_eq!(state(&first, "cursor"), AgentRowState::CantCheck);
    assert_eq!(state(&first, "snow"), AgentRowState::CantCheck);
    let openclaw = first
        .agents
        .iter()
        .find(|row| row.provider_id == "openclaw");
    assert_eq!(openclaw.unwrap().command, "openclaw chat");
    let (ticket, Action::Start { commands }) = exchange(&catalog, start) else {
        panic!("a lookup opens with a Start");
    };
    assert_eq!(commands.len(), STARTABLE_AGENT_PROVIDER_IDS.len() - 2);
    assert!(commands.iter().any(|command| command == "claude"));
    for generic in ["agent", "snow"] {
        assert!(
            !commands.iter().any(|command| command == generic),
            "{generic}"
        );
    }

    // Refresh absorbed while running: nothing changes and no second lookup follows.
    assert_eq!(catalog.refresh(start), Some(first.clone()));
    assert_eq!(
        catalog.next(start),
        Step::Wait(Some(start + LOOKUP_DEADLINE))
    );
    answer(
        &catalog,
        ticket,
        Some(Outcome::Start(Start::Accepted)),
        start,
    );
    let mut now = start + POLL_INTERVAL;
    assert_eq!(catalog.next(start), Step::Wait(Some(now)));
    let (ticket, poll) = exchange(&catalog, now);
    assert_eq!(poll, Action::Poll { job_id: job() });
    answer(&catalog, ticket, Some(Outcome::Poll(Poll::Pending)), now);
    now += POLL_INTERVAL;
    let (ticket, _) = exchange(&catalog, now);
    let mut results = vec![AgentProbeResult::Missing; commands.len()];
    let position = |name: &str| commands.iter().position(|command| command == name).unwrap();
    results[position("codex")] = AgentProbeResult::Found;
    results[position("claude")] = AgentProbeResult::Unavailable;
    let completed = answer(
        &catalog,
        ticket,
        Some(Outcome::Poll(Poll::Completed { results })),
        now,
    );
    assert_eq!(
        (
            completed.revision.get(),
            completed.status,
            completed.checked_at
        ),
        (2, LookupStatus::Complete, Some(UNIX_MS))
    );
    assert_eq!(state(&completed, "codex"), AgentRowState::Found);
    assert_eq!(state(&completed, "claude-code"), AgentRowState::Unknown);
    assert_eq!(state(&completed, "gemini"), AgentRowState::NotFound);
    assert_eq!(state(&completed, "cursor"), AgentRowState::CantCheck);
    assert_eq!(catalog.next(now), Step::Wait(None));

    // 2 s floor: a refresh sooner is ignored; one at the floor starts a later lookup, during
    // which rows keep their states.
    let soon = now + REFRESH_FLOOR - Duration::from_millis(1);
    assert_eq!(catalog.refresh(soon), Some(completed.clone()));
    assert_eq!(catalog.next(soon), Step::Wait(None));
    now += REFRESH_FLOOR;
    let running = catalog.refresh(now).unwrap();
    assert_eq!(
        (running.revision.get(), running.status),
        (3, LookupStatus::Running)
    );
    assert_eq!(running.agents, completed.agents);

    // Failed later lookup keeps the last completed rows.
    let (ticket, _) = exchange(&catalog, now);
    let failed = answer(&catalog, ticket, Some(Outcome::Start(Start::Busy)), now);
    assert_eq!(
        (failed.revision.get(), failed.status, failed.checked_at),
        (4, LookupStatus::Failed, Some(UNIX_MS))
    );
    assert_eq!(failed.agents, completed.agents);
}

#[test]
fn generation_changes_worker_loss_and_failed_first_lookups_end_as_the_model_says() {
    let catalog = AgentCatalog::default();
    let now = Instant::now();

    // A failed first lookup leaves checkable rows unknown.
    begin(&catalog, 1);
    let (first, _) = exchange(&catalog, now);
    let failed = answer(&catalog, first, None, now);
    assert_eq!(
        (failed.status, failed.checked_at),
        (LookupStatus::Failed, None)
    );
    assert_eq!(state(&failed, "codex"), AgentRowState::Unknown);
    assert_eq!(state(&failed, "cursor"), AgentRowState::CantCheck);

    // Generation change: the new Worker is looked up at once, and the older lookup's answer is
    // ignored.
    begin(&catalog, 2);
    let (second, _) = exchange(&catalog, now);
    let fenced = answer(&catalog, first, Some(Outcome::Start(Start::Accepted)), now);
    assert_eq!(fenced.status, LookupStatus::Running);
    assert_eq!(state(&fenced, "codex"), AgentRowState::Checking);
    assert_eq!(catalog.next(now), Step::Wait(Some(now + LOOKUP_DEADLINE)));

    // Losing the Worker ends its lookup failed; without a Worker nothing starts.
    catalog.retire(generation(2));
    let lost = catalog.read().unwrap();
    assert_eq!(lost.status, LookupStatus::Failed);
    assert_eq!(state(&lost, "codex"), AgentRowState::Unknown);
    let late = answer(&catalog, second, Some(Outcome::Start(Start::Accepted)), now);
    assert_eq!(late, lost);
    assert_eq!(catalog.refresh(now + REFRESH_FLOOR), Some(lost));
    assert_eq!(catalog.next(now + REFRESH_FLOOR), Step::Wait(None));

    // An accepted lookup that outlives its deadline fails and cancels its Worker job once.
    begin(&catalog, 3);
    let (third, _) = exchange(&catalog, now);
    answer(&catalog, third, Some(Outcome::Start(Start::Accepted)), now);
    let (_, cancel) = exchange(&catalog, now + LOOKUP_DEADLINE);
    assert_eq!(cancel, Action::Cancel { job_id: job() });
    assert_eq!(catalog.read().unwrap().status, LookupStatus::Failed);
    assert_eq!(catalog.next(now + LOOKUP_DEADLINE), Step::Wait(None));

    // A Worker without a lookup answers Unsupported, which leaves checkable rows unknown.
    begin(&catalog, 4);
    let (fourth, _) = exchange(&catalog, now);
    let unsupported = answer(
        &catalog,
        fourth,
        Some(Outcome::Start(Start::Unsupported)),
        now,
    );
    assert_eq!(unsupported.status, LookupStatus::Unsupported);
    assert_eq!(state(&unsupported, "codex"), AgentRowState::Unknown);
}
