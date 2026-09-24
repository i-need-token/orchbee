use super::super::{
    AdmissionError, COMMAND_CAPACITY, CausalSettlement, CommandLease, CommandMailbox, CreateMarker,
    CreatePrecommitOutcome, DesiredInstall, EventDeadline, EventError, MailboxClock,
    MailboxMembership, MoveAdmissionError, MutationResult, PanelSettlement, QueuedCommand,
    RefreshSettlement, StructuralMutationFlight, SwitchMarker, Tagged, UiCorrelation, UiReply,
    UiReplyKind, UiReplyTransaction, UiWaitKey, WorkerEvent,
};
use super::fixture_viewer_id;
use crate::{
    client_view::PanelDirection,
    local_client::workbench::{
        WorkbenchTabCreateCommit, WorkbenchTabCreatePrecommit, WorkbenchTabCreateRequest,
        panel_close::WorkbenchPanelCloseAccepted, panel_move::WorkbenchPanelMoveCommit,
        panel_split::WorkbenchPanelSplitCommit,
    },
    terminal_projection::TerminalProjection,
    terminal_session::{
        state::{
            AttachTarget, AttachmentKey, CarrierFailure, CleanupContinuation, ControllerMode,
            CreateOperation, CreateOperationKey, CreateProjectionDecision,
            CreateTerminalProvenance, FrameIdentity, MembershipCleanup, MembershipInstallDecision,
            MembershipResultKey, NativeSessionState, NavigatorRefreshStatus, OperationDeadlines,
            PaneRef, PanelCommit, PanelDecision, PanelIntent, PanelOperation, PanelPrecommit,
            PanelRequestKind, PanelResult, PendingCreateCommit, PendingRefreshStage, ReadyDecision,
            RefreshDecision, SessionNotice, SessionPhaseKind, SetupEventDisposition,
        },
        workbench_projection::project_native_standalone_tabs,
    },
};
use orchbee_client_server_protocol::{ControlInput, ControllerId, Geometry, TerminalViewerAction};
use orchbee_terminal_protocol::{AttachmentId, HostEpoch};
use orchbee_workbench_protocol::{
    Navigation, PanelNode, PanelOrientation, PanelSlot, Presentation, PresentationBinding,
    ProtocolVersion, SiteId, Snapshot, Tab, TerminalContext, TerminalId, TerminalPresentationId,
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandUnavailableReason, WorkbenchLabel,
    WorkbenchPanelId, WorkbenchPanelSlotId, WorkbenchPanelSplitDirection, WorkbenchRejectionReason,
    WorkbenchRequestId, WorkbenchRevision, WorkbenchSnapshotOutcome,
    WorkbenchSnapshotUnavailableReason, WorkbenchSplitId, WorkbenchTabId,
    WorkbenchTerminalContextId, WorkbenchTerminalGeometry, WorkerId,
};
use std::{
    cell::Cell,
    fmt,
    time::{Duration, Instant},
};

const IDENTITY_SEED: u128 = 0x400;
const FIXTURE_SEED: u128 = 0x2000;
const SEEDS: [u64; 8] = [
    0x243f_6a88,
    0x85a3_08d3,
    0x1319_8a2e,
    0x0370_7344,
    0xa409_3822,
    0x299f_31d0,
    0x082e_fa98,
    0xec4e_6c89,
];
const ACTIONS_PER_TRACE: usize = 9;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LaterFailure {
    Deadline,
    OutcomeUnknown,
    Protocol,
    Transport,
}

impl LaterFailure {
    const COUNT: usize = 4;

    const fn from_index(index: usize) -> Self {
        match index % Self::COUNT {
            0 => Self::Deadline,
            1 => Self::OutcomeUnknown,
            2 => Self::Protocol,
            _ => Self::Transport,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Deadline => 0,
            Self::OutcomeUnknown => 1,
            Self::Protocol => 2,
            Self::Transport => 3,
        }
    }

    const fn failure(self) -> CarrierFailure {
        match self {
            Self::Deadline => CarrierFailure::Deadline,
            Self::OutcomeUnknown => CarrierFailure::OutcomeUnknown,
            Self::Protocol => CarrierFailure::Protocol,
            Self::Transport => CarrierFailure::Transport,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TraceAction {
    PreludeCreate {
        split: u8,
        first_byte: u8,
        feedback: bool,
        returned: bool,
    },
    PanelCycle {
        kind: u8,
        outcome: u8,
        probe: u8,
    },
    CommitSecondCreate,
    RefreshAppend {
        extra_tabs: u8,
        control: u8,
    },
    SupersedeSwitch {
        target_hint: u8,
    },
    ExhaustWithKnownCommit {
        later_failure: LaterFailure,
    },
    PanelNegativeSweep {
        terminal_probe: u8,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActionClass {
    PreludeCreate { feedback: bool, returned: bool },
    PanelCycle { kind: u8 },
    CommitSecondCreate,
    RefreshAppend,
    SupersedeSwitch,
    ExhaustWithKnownCommit(LaterFailure),
    PanelNegativeSweep,
}

impl TraceAction {
    const fn class(self) -> ActionClass {
        match self {
            Self::PreludeCreate {
                feedback, returned, ..
            } => ActionClass::PreludeCreate { feedback, returned },
            Self::PanelCycle { kind, .. } => ActionClass::PanelCycle { kind },
            Self::CommitSecondCreate => ActionClass::CommitSecondCreate,
            Self::RefreshAppend { .. } => ActionClass::RefreshAppend,
            Self::SupersedeSwitch { .. } => ActionClass::SupersedeSwitch,
            Self::ExhaustWithKnownCommit { later_failure } => {
                ActionClass::ExhaustWithKnownCommit(later_failure)
            }
            Self::PanelNegativeSweep { .. } => ActionClass::PanelNegativeSweep,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TracePhase {
    Ready,
    FirstRestored,
    SecondProjected,
    Refreshed,
    SwitchedReady,
    Retired,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Coverage {
    completed_traces: usize,
    capacity_refusals: usize,
    no_burn_refusals: usize,
    fifo_records: usize,
    causal_before_marker: usize,
    causal_empty: usize,
    causal_feedback: usize,
    precommit_not_submitted: usize,
    precommit_returned: usize,
    precommit_reentries: usize,
    burned_create_pairs: usize,
    pending_commits: usize,
    refresh_stages: [usize; 3],
    refresh_controls_held: usize,
    retained_provenance: usize,
    active_nonfirst_refreshes: usize,
    switch_supersessions: usize,
    stale_switch_settlements: usize,
    current_switch_setups: usize,
    known_commit_handoffs: usize,
    identity_exhaustions: usize,
    later_failures: [usize; LaterFailure::COUNT],
    panel_cycles: usize,
    panel_admissions: usize,
    panel_kinds: [usize; 3],
    panel_admission_refusals: usize,
    panel_dup_refusals: usize,
    panel_refresh_blocks: usize,
    panel_switch_blocks: usize,
    panel_marks: usize,
    panel_stale_marks: usize,
    panel_stages: [usize; 3],
    panel_publishes: usize,
    panel_replies: usize,
    panel_stale_replies: usize,
    panel_prev_wait_stale: usize,
    panel_post_settle_redeposits: usize,
    panel_decisions: [usize; 5],
    panel_outcomes: [usize; OutcomeClass::COUNT],
    panel_membership_cleanups: usize,
    panel_membership_detachments: usize,
    panel_membership_installs: usize,
    panel_ordering_probes: usize,
    panel_attaches: usize,
    panel_recovery_arms: usize,
    panel_recovery_clears: usize,
    panel_publication_drains: usize,
    panel_negative_probes: usize,
    panel_terminal_probes: usize,
    ambiguous_abandons: usize,
    divergent_retires: usize,
    panel_retained_commits: usize,
    all_refused_cycles: usize,
}

impl Coverage {
    fn add(&mut self, other: &Self) {
        self.completed_traces += other.completed_traces;
        self.capacity_refusals += other.capacity_refusals;
        self.no_burn_refusals += other.no_burn_refusals;
        self.fifo_records += other.fifo_records;
        self.causal_before_marker += other.causal_before_marker;
        self.causal_empty += other.causal_empty;
        self.causal_feedback += other.causal_feedback;
        self.precommit_not_submitted += other.precommit_not_submitted;
        self.precommit_returned += other.precommit_returned;
        self.precommit_reentries += other.precommit_reentries;
        self.burned_create_pairs += other.burned_create_pairs;
        self.pending_commits += other.pending_commits;
        add_counts(&mut self.refresh_stages, &other.refresh_stages);
        self.refresh_controls_held += other.refresh_controls_held;
        self.retained_provenance += other.retained_provenance;
        self.active_nonfirst_refreshes += other.active_nonfirst_refreshes;
        self.switch_supersessions += other.switch_supersessions;
        self.stale_switch_settlements += other.stale_switch_settlements;
        self.current_switch_setups += other.current_switch_setups;
        self.known_commit_handoffs += other.known_commit_handoffs;
        self.identity_exhaustions += other.identity_exhaustions;
        add_counts(&mut self.later_failures, &other.later_failures);
        self.panel_cycles += other.panel_cycles;
        self.panel_admissions += other.panel_admissions;
        add_counts(&mut self.panel_kinds, &other.panel_kinds);
        self.panel_admission_refusals += other.panel_admission_refusals;
        self.panel_dup_refusals += other.panel_dup_refusals;
        self.panel_refresh_blocks += other.panel_refresh_blocks;
        self.panel_switch_blocks += other.panel_switch_blocks;
        self.panel_marks += other.panel_marks;
        self.panel_stale_marks += other.panel_stale_marks;
        add_counts(&mut self.panel_stages, &other.panel_stages);
        self.panel_publishes += other.panel_publishes;
        self.panel_replies += other.panel_replies;
        self.panel_stale_replies += other.panel_stale_replies;
        self.panel_prev_wait_stale += other.panel_prev_wait_stale;
        self.panel_post_settle_redeposits += other.panel_post_settle_redeposits;
        add_counts(&mut self.panel_decisions, &other.panel_decisions);
        add_counts(&mut self.panel_outcomes, &other.panel_outcomes);
        self.panel_membership_cleanups += other.panel_membership_cleanups;
        self.panel_membership_detachments += other.panel_membership_detachments;
        self.panel_membership_installs += other.panel_membership_installs;
        self.panel_ordering_probes += other.panel_ordering_probes;
        self.panel_attaches += other.panel_attaches;
        self.panel_recovery_arms += other.panel_recovery_arms;
        self.panel_recovery_clears += other.panel_recovery_clears;
        self.panel_publication_drains += other.panel_publication_drains;
        self.panel_negative_probes += other.panel_negative_probes;
        self.panel_terminal_probes += other.panel_terminal_probes;
        self.ambiguous_abandons += other.ambiguous_abandons;
        self.divergent_retires += other.divergent_retires;
        self.panel_retained_commits += other.panel_retained_commits;
        self.all_refused_cycles += other.all_refused_cycles;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ReplayFailure {
    Invalid(String),
    Violation(String),
}

impl fmt::Display for ReplayFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => write!(formatter, "invalid trace: {message}"),
            Self::Violation(message) => write!(formatter, "oracle violation: {message}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailureKind {
    Invalid,
    Violation,
    Panic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TraceFailure {
    step: Option<usize>,
    action: Option<TraceAction>,
    kind: FailureKind,
    oracle: String,
}

impl TraceFailure {
    fn from_replay(
        step: Option<usize>,
        action: Option<TraceAction>,
        failure: ReplayFailure,
    ) -> Self {
        let (kind, oracle) = match failure {
            ReplayFailure::Invalid(oracle) => (FailureKind::Invalid, oracle),
            ReplayFailure::Violation(oracle) => (FailureKind::Violation, oracle),
        };
        Self {
            step,
            action,
            kind,
            oracle,
        }
    }

    fn from_panic(
        step: Option<usize>,
        action: Option<TraceAction>,
        payload: Box<dyn std::any::Any + Send>,
    ) -> Self {
        let oracle = if let Some(message) = payload.downcast_ref::<&str>() {
            (*message).to_owned()
        } else if let Some(message) = payload.downcast_ref::<String>() {
            message.clone()
        } else {
            "non-string panic".to_owned()
        };
        Self {
            step,
            action,
            kind: FailureKind::Panic,
            oracle,
        }
    }

    fn same_fingerprint(&self, other: &Self) -> bool {
        self.action.map(TraceAction::class) == other.action.map(TraceAction::class)
            && self.kind == other.kind
            && self.oracle == other.oracle
    }
}

#[derive(Debug)]
struct FixedClock(Instant);

impl MailboxClock for FixedClock {
    fn now(&self) -> Instant {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpectedRecord {
    Input(u8),
    Poll,
    Create,
}

struct Trace {
    base: Instant,
    clock: FixedClock,
    mailbox: CommandMailbox,
    session: NativeSessionState,
    authority: Authority,
    next_request: u128,
    phase: TracePhase,
    first_operation: Option<CreateOperationKey>,
    last_panel_wait: Option<UiWaitKey>,
    retire_cause: Option<CarrierFailure>,
    coverage: Coverage,
}

#[test]
fn generated_persistent_mailbox_and_state_lifecycle_replays_and_shrinks() {
    if let Ok(encoded) = std::env::var("ORCHBEE_NATIVE_LIFECYCLE_REPLAY") {
        let actions = deserialize(&encoded)
            .unwrap_or_else(|error| panic!("invalid lifecycle replay: {error}"));
        replay(&actions).unwrap_or_else(|error| panic!("lifecycle replay failed: {error:?}"));
        return;
    }
    let mut total = Coverage::default();
    for (seed_index, seed) in SEEDS.into_iter().enumerate() {
        let actions = generate(seed, seed_index);
        assert_eq!(actions.len(), ACTIONS_PER_TRACE);
        let encoded = serialize(&actions);
        let decoded = deserialize(&encoded)
            .unwrap_or_else(|error| panic!("generated replay did not parse: {error}; {encoded}"));
        assert_eq!(decoded, actions);
        assert!(actions.iter().copied().all(|action| {
            shrink_values(action)
                .into_iter()
                .all(|candidate| candidate.class() == action.class())
        }));

        let coverage = match replay(&actions) {
            Ok(coverage) => coverage,
            Err(error) if error.kind != FailureKind::Invalid => {
                let minimized = shrink_failure(actions.clone(), &error);
                let minimized_error = replay(&minimized)
                    .expect_err("a minimized lifecycle trace must preserve its failure");
                assert!(error.same_fingerprint(&minimized_error));
                panic!(
                    "seed {seed:#x} failed: {error:?}; minimized={minimized_error:?}; \
                     rerun with ORCHBEE_NATIVE_LIFECYCLE_REPLAY={}",
                    serialize(&minimized)
                );
            }
            Err(error) => {
                panic!("seed {seed:#x} generated an invalid trace: {error:?}; replay={encoded}");
            }
        };
        let replayed = replay(&decoded).unwrap_or_else(|error| {
            panic!("serialized replay failed: {error:?}; replay={encoded}")
        });
        assert_eq!(replayed, coverage);
        eprintln!(
            "native-lifecycle seed={seed:#x} actions={} coverage={coverage:?} replay={encoded}",
            actions.len(),
        );
        total.add(&coverage);
    }

    assert_eq!(total.completed_traces, SEEDS.len());
    assert!(total.capacity_refusals >= SEEDS.len());
    assert!(total.no_burn_refusals >= SEEDS.len() * 2);
    assert_eq!(total.fifo_records, SEEDS.len() * COMMAND_CAPACITY);
    assert!(total.causal_before_marker >= SEEDS.len());
    assert!(total.causal_empty > 0 && total.causal_feedback > 0);
    assert!(total.precommit_not_submitted > 0 && total.precommit_returned > 0);
    assert!(total.precommit_reentries >= SEEDS.len());
    assert!(total.burned_create_pairs >= SEEDS.len());
    assert!(total.pending_commits >= SEEDS.len());
    assert!(
        total
            .refresh_stages
            .iter()
            .all(|count| *count >= SEEDS.len())
    );
    assert!(total.refresh_controls_held >= SEEDS.len());
    assert!(total.retained_provenance >= SEEDS.len() * 2);
    assert!(total.active_nonfirst_refreshes >= SEEDS.len());
    assert!(total.switch_supersessions >= SEEDS.len());
    assert!(total.stale_switch_settlements >= SEEDS.len());
    assert!(total.current_switch_setups >= SEEDS.len());
    // Terminal actions alternate: even seeds exhaust with a known commit, odd seeds run the
    // negative sweep; `completed_traces` above already proves every seed reached a terminal.
    assert!(total.known_commit_handoffs >= SEEDS.len() / 2);
    assert!(total.identity_exhaustions >= SEEDS.len() / 2);
    assert!(total.later_failures.iter().all(|count| *count > 0));

    assert_eq!(total.panel_cycles, SEEDS.len() * 4);
    assert!(
        total.panel_admissions >= SEEDS.len() * 3,
        "panel cycles degenerated into refusal-only traces: {total:?}"
    );
    assert!(total.panel_kinds.iter().all(|count| *count > 0));
    assert!(total.panel_admission_refusals > 0);
    assert!(total.panel_dup_refusals >= total.panel_admissions);
    assert!(total.panel_refresh_blocks >= total.panel_admissions);
    assert!(total.panel_switch_blocks >= total.panel_admissions);
    assert!(total.panel_marks >= total.panel_admissions);
    assert!(total.panel_stale_marks >= total.panel_admissions * 2);
    assert!(total.panel_stages.iter().all(|count| *count >= SEEDS.len()));
    assert!(total.panel_publishes >= total.panel_admissions);
    assert!(total.panel_replies >= total.panel_admissions);
    assert!(total.panel_stale_replies > 0);
    assert!(total.panel_prev_wait_stale > 0);
    assert!(total.panel_post_settle_redeposits > 0);
    assert!(
        total.panel_decisions[0] > 0,
        "no Continue decision: {total:?}"
    );
    assert!(
        total.panel_decisions[1] > 0,
        "no Cleanup decision: {total:?}"
    );
    assert!(
        total.panel_decisions[2] > 0,
        "no AttachRecovered decision: {total:?}"
    );
    assert!(
        total.panel_decisions[3] > 0,
        "no SettledWithoutAttachment decision: {total:?}"
    );
    assert!(
        total.panel_decisions[4] > 0,
        "no Retire decision: {total:?}"
    );
    assert!(
        total.panel_outcomes[OutcomeClass::CommittedKeep.index()] > 0
            && total.panel_outcomes[OutcomeClass::CommittedDrop.index()] > 0
            && total.panel_outcomes[OutcomeClass::CommittedNoSnapshot.index()] > 0
            && total.panel_outcomes[OutcomeClass::CloseAccepted.index()] > 0
            && total.panel_outcomes[OutcomeClass::CommittedTabGone.index()] > 0
            && total.panel_outcomes[OutcomeClass::NotSubmitted.index()] > 0
            && total.panel_outcomes[OutcomeClass::Rejected.index()] > 0,
        "outcome classes not covered: {total:?}"
    );
    assert!(total.panel_membership_cleanups > 0);
    assert!(total.panel_membership_detachments > 0);
    assert!(total.panel_membership_installs > 0);
    assert!(total.panel_ordering_probes > 0);
    assert!(total.panel_attaches > 0);
    assert!(total.panel_recovery_arms > 0);
    assert!(total.panel_recovery_clears >= total.panel_recovery_arms);
    assert!(total.panel_publication_drains > 0);
    assert!(
        total.panel_negative_probes >= 10 * (SEEDS.len() / 2),
        "negative sweep did not cover probes: {total:?}"
    );
    assert!(total.panel_terminal_probes >= SEEDS.len() / 2);
    assert!(total.ambiguous_abandons > 0);
    assert!(total.divergent_retires > 0);
    assert!(total.panel_retained_commits > 0);
}

/// Failure-cleanup coverage for the occupied-cell publisher scaffolding. Every
/// early exit out of the scope closure — a controlled UI-stage error, a waiter
/// observation that never lands, or a panic unwind — must close and wake the
/// publisher so the scope join completes, must preserve the original error,
/// and must never substitute the cleanup close for the measured production
/// verdict. `recv_timeout` on the publisher's own report proves the thread ran
/// to completion; a hung scope join would hang this test instead of passing.
#[test]
fn occupied_cell_publication_cleanup_covers_failure_paths() -> Result<(), ReplayFailure> {
    // Publisher parked on the occupied cell when the UI stage returns a
    // controlled error: the rescue close releases it before the scope joins,
    // and the original violation propagates unchanged.
    {
        let mut sandbox = PanelSandbox::new()?;
        let (operation, _lease, _wait) = sandbox.admit_split_awaiting()?;
        let (results, completed) = std::sync::mpsc::channel();
        let outcome = occupied_cell_publication(
            &sandbox.mailbox,
            WorkerEvent::PanelResult {
                operation: operation.clone(),
                outcome: PanelResult::NotSubmitted,
            },
            EventDeadline::PlacementOnly(operation.deadline()),
            &sandbox.clock,
            Duration::from_secs(2),
            Duration::from_secs(5),
            &results,
            &completed,
            || violation("injected UI-stage failure"),
        );
        let error = outcome.expect_err("injected UI-stage failure must propagate");
        assert!(
            matches!(&error, ReplayFailure::Violation(message) if message.contains("injected UI-stage failure")),
            "original error was replaced or swallowed: {error:?}"
        );
        let cleanup = completed
            .recv_timeout(Duration::from_secs(5))
            .expect("rescued publisher never reported");
        let failure = observed_err(cleanup, "rescued publication result")?
            .closed_failure()
            .expect("rescued publication was not closed");
        assert_eq!(
            failure,
            CarrierFailure::Closed,
            "cleanup close must be reported as cleanup, never as the measured verdict"
        );
        assert_eq!(sandbox.mailbox.is_closed(), Some(CarrierFailure::Closed));
    }
    // A publisher that never reaches the drain wait (its mailbox is already
    // closed) still fails the waiter observation, and the scope join still
    // completes because the rescue close is a no-op on an already-closed
    // mailbox.
    {
        let mut sandbox = PanelSandbox::new()?;
        let (operation, _lease, _wait) = sandbox.admit_split_awaiting()?;
        sandbox.mailbox.close(CarrierFailure::Transport);
        let (results, completed) = std::sync::mpsc::channel();
        let outcome = occupied_cell_publication(
            &sandbox.mailbox,
            WorkerEvent::PanelResult {
                operation: operation.clone(),
                outcome: PanelResult::NotSubmitted,
            },
            EventDeadline::PlacementOnly(operation.deadline()),
            &sandbox.clock,
            Duration::from_millis(250),
            Duration::from_secs(5),
            &results,
            &completed,
            || Ok(()),
        );
        let error = outcome.expect_err("unparked publisher must fail the observation");
        assert!(
            matches!(&error, ReplayFailure::Violation(message) if message.contains("never parked")),
            "original error was replaced or swallowed: {error:?}"
        );
        let cleanup = completed
            .recv_timeout(Duration::from_secs(5))
            .expect("closed-mailbox publisher never reported");
        let failure = observed_err(cleanup, "closed-mailbox publication result")?
            .closed_failure()
            .expect("closed-mailbox publication was not closed");
        assert_eq!(
            failure,
            CarrierFailure::Transport,
            "the pre-existing close must survive the rescue close"
        );
    }
    // A panic unwind inside the scope closure drops the rescue before the
    // scope's implicit join, so the parked publisher exits and the panic
    // payload reaches the caller unchanged.
    {
        let mut sandbox = PanelSandbox::new()?;
        let (operation, _lease, _wait) = sandbox.admit_split_awaiting()?;
        let (results, completed) = std::sync::mpsc::channel();
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = occupied_cell_publication(
                &sandbox.mailbox,
                WorkerEvent::PanelResult {
                    operation: operation.clone(),
                    outcome: PanelResult::NotSubmitted,
                },
                EventDeadline::PlacementOnly(operation.deadline()),
                &sandbox.clock,
                Duration::from_secs(2),
                Duration::from_secs(5),
                &results,
                &completed,
                || -> Result<(), ReplayFailure> { panic!("injected unwind") },
            );
        }));
        assert!(panicked.is_err(), "injected unwind must propagate");
        let cleanup = completed
            .recv_timeout(Duration::from_secs(5))
            .expect("rescued publisher never reported after unwind");
        let failure = observed_err(cleanup, "unwind-rescued publication result")?
            .closed_failure()
            .expect("unwind-rescued publication was not closed");
        assert_eq!(failure, CarrierFailure::Closed);
    }
    Ok(())
}

fn generate(seed: u64, seed_index: usize) -> Vec<TraceAction> {
    let mut random = seed;
    let mut next = || {
        random = random
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (random >> 48) as u8
    };
    let terminal = if seed_index.is_multiple_of(2) {
        TraceAction::ExhaustWithKnownCommit {
            later_failure: LaterFailure::from_index(seed_index / 2),
        }
    } else {
        TraceAction::PanelNegativeSweep {
            terminal_probe: next(),
        }
    };
    vec![
        TraceAction::PreludeCreate {
            split: next(),
            first_byte: next(),
            feedback: seed_index.is_multiple_of(2),
            returned: (seed_index / 2).is_multiple_of(2),
        },
        TraceAction::PanelCycle {
            kind: next(),
            outcome: next(),
            probe: next(),
        },
        TraceAction::CommitSecondCreate,
        // The session just settled without attachment on a single-leaf non-attachable
        // pending tab. Forcing Split here lets the setup steering commit a Keep that grows
        // it to two leaves — the precondition both post-refresh membership outcomes need
        // (Move admissibility on even seeds, a surviving sibling for leaf repair on odd
        // seeds).
        TraceAction::PanelCycle {
            kind: 0,
            outcome: next(),
            probe: next(),
        },
        TraceAction::RefreshAppend {
            extra_tabs: next(),
            control: next(),
        },
        TraceAction::PanelCycle {
            kind: next(),
            outcome: next(),
            probe: next(),
        },
        TraceAction::SupersedeSwitch {
            target_hint: next(),
        },
        // The post-refresh cycle is the only position where an unattached membership
        // result can still fire a decision: the session settled without attachment on the
        // pending tab after the second create, and the switch always re-attaches. The
        // forced intent alternates by seed parity so the preference order in
        // `build_panel_outcome` differs across seeds; which side lands depends on which
        // tab the refresh left selected, and across the fixed seeds both
        // AttachRecovered and SettledWithoutAttachment fire.
        TraceAction::PanelCycle {
            kind: 1 - (seed_index % 2) as u8,
            outcome: next(),
            probe: next(),
        },
        terminal,
    ]
}

fn replay(actions: &[TraceAction]) -> Result<Coverage, TraceFailure> {
    let mut trace = match std::panic::catch_unwind(Trace::new) {
        Ok(Ok(trace)) => trace,
        Ok(Err(error)) => return Err(TraceFailure::from_replay(None, None, error)),
        Err(payload) => return Err(TraceFailure::from_panic(None, None, payload)),
    };
    for (step, action) in actions.iter().copied().enumerate() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            trace.apply(action)?;
            require(
                trace.session.invariant_holds(),
                "NativeSessionState invariant failed",
            )?;
            trace.assert_pair_alignment()
        }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                return Err(TraceFailure::from_replay(Some(step), Some(action), error));
            }
            Err(payload) => {
                return Err(TraceFailure::from_panic(Some(step), Some(action), payload));
            }
        }
    }
    Ok(trace.coverage)
}

impl Trace {
    fn new() -> Result<Self, ReplayFailure> {
        let base = Instant::now();
        let authority = Authority::bootstrap();
        let cache = project_native_standalone_tabs(&authority.snapshot())
            .map_err(|error| violation_error(format!("initial projection: {error:?}")))?;
        let (mut session, _target) = NativeSessionState::starting(cache, 1, geometry())
            .ok_or_else(|| violation_error("initial state was rejected"))?;
        let desired = session.desired().unwrap().clone();
        require(
            session.install_projection_t(&desired),
            "initial projection install",
        )?;
        require(
            session.record_setup_configuration_t(&desired, controlling(), Some(geometry())),
            "initial setup configuration",
        )?;
        require(
            session.stage_drawn_frame_t(&desired, FrameIdentity::new(1)),
            "initial frame stage",
        )?;
        require(
            session.mark_feedback_accepted_t(&desired, FrameIdentity::new(1)),
            "initial frame feedback",
        )?;
        require(
            session.publish_ready_t(
                &desired,
                FrameIdentity::new(1),
                controlling(),
                Some(geometry()),
            ),
            "initial Ready publication",
        )?;

        let mailbox = CommandMailbox::new(IDENTITY_SEED, geometry());
        {
            let mut raw = mailbox.lock();
            raw.attachment_generation = desired.generation();
            raw.desired = MailboxMembership::Live {
                desired: Some(desired.clone()),
                panes: vec![session.focus_pane_ref().unwrap()],
            };
        }
        Ok(Self {
            base,
            clock: FixedClock(base),
            mailbox,
            session,
            authority,
            next_request: IDENTITY_SEED + 1,
            phase: TracePhase::Ready,
            first_operation: None,
            last_panel_wait: None,
            retire_cause: None,
            coverage: Coverage::default(),
        })
    }

    fn apply(&mut self, action: TraceAction) -> Result<(), ReplayFailure> {
        match (self.phase, action) {
            (
                TracePhase::Ready,
                TraceAction::PreludeCreate {
                    split,
                    first_byte,
                    feedback,
                    returned,
                },
            ) => self.prelude_create(split, first_byte, feedback, returned),
            (
                TracePhase::FirstRestored
                | TracePhase::SecondProjected
                | TracePhase::Refreshed
                | TracePhase::SwitchedReady,
                TraceAction::PanelCycle {
                    kind,
                    outcome,
                    probe,
                },
            ) => self.panel_cycle(kind, outcome, probe),
            (TracePhase::FirstRestored, TraceAction::CommitSecondCreate) => {
                self.commit_second_create()
            }
            (
                TracePhase::SecondProjected,
                TraceAction::RefreshAppend {
                    extra_tabs,
                    control,
                },
            ) => self.refresh_append(extra_tabs, control),
            (TracePhase::Refreshed, TraceAction::SupersedeSwitch { target_hint }) => {
                self.supersede_switch(target_hint)
            }
            (TracePhase::SwitchedReady, TraceAction::ExhaustWithKnownCommit { later_failure }) => {
                self.exhaust_with_known_commit(later_failure)
            }
            (TracePhase::SwitchedReady, TraceAction::PanelNegativeSweep { terminal_probe }) => {
                self.panel_negative_sweep(terminal_probe)
            }
            (_, action) => Err(ReplayFailure::Invalid(format!(
                "{action:?} is not admitted from {:?}",
                self.phase
            ))),
        }
    }

    fn assert_pair_alignment(&self) -> Result<(), ReplayFailure> {
        if self.phase == TracePhase::Retired {
            let cause = self
                .retire_cause
                .ok_or_else(|| violation_error("retired trace lost its cause"))?;
            require(
                self.mailbox.is_closed() == Some(cause),
                "retired mailbox lost its close cause",
            )?;
            return require(
                self.session.retired_failure() == Some(cause),
                "retired state lost its retirement cause",
            );
        }
        require(self.mailbox.is_closed().is_none(), "mailbox closed early")?;
        let mailbox_desired = self.mailbox.lock().desired.key().cloned();
        require(
            mailbox_desired.as_ref() == Some(self.session.desired().unwrap()),
            "mailbox and state desired attachment diverged",
        )
    }

    fn prelude_create(
        &mut self,
        split: u8,
        first_byte: u8,
        feedback: bool,
        returned: bool,
    ) -> Result<(), ReplayFailure> {
        const CONTROL_COUNT: usize = COMMAND_CAPACITY - 2;
        let before_poll = 1 + usize::from(split) % 3;
        let mut expected = Vec::with_capacity(COMMAND_CAPACITY);
        for offset in 0..before_poll {
            let byte = first_byte.wrapping_add(u8::try_from(offset).unwrap_or(0));
            self.enqueue_input(byte)?;
            expected.push(ExpectedRecord::Input(byte));
        }
        observed(
            self.mailbox.try_enqueue(poll_command(&self.mailbox)),
            "enqueue pre-Create Poll",
        )?;
        expected.push(ExpectedRecord::Poll);
        for offset in before_poll..CONTROL_COUNT {
            let byte = first_byte.wrapping_add(u8::try_from(offset).unwrap_or(0));
            self.enqueue_input(byte)?;
            expected.push(ExpectedRecord::Input(byte));
        }

        let base = self.base;
        let operation = observed(
            self.mailbox.try_enqueue_create_for(
                &mut self.session,
                |session, generation, request_id| {
                    let marker = create_marker(session, base, generation, request_id, "2")?;
                    if !session.begin_create(marker.operation().clone()) {
                        return Err("state rejected first Create");
                    }
                    Ok(marker)
                },
                |_session, _result| {},
            ),
            "admit first Create",
        )?;
        expected.push(ExpectedRecord::Create);
        require(
            operation.request_id().to_string() == id(self.next_request),
            "first Create skipped or reused a request identity",
        )?;
        self.next_request += 1;
        require(
            expected.len() == COMMAND_CAPACITY,
            "prelude did not fill FIFO",
        )?;
        require(
            self.mailbox.lock().queue.len() == COMMAND_CAPACITY,
            "mailbox did not reach real capacity",
        )?;

        let before_refusal = cursor_snapshot(&self.mailbox);
        let builder_ran = Cell::new(false);
        let refused = self.mailbox.try_enqueue_create_for(
            &mut self.session,
            |_session, _generation, _request_id| -> Result<CreateMarker, &'static str> {
                builder_ran.set(true);
                Err("full admission invoked builder")
            },
            |_session, _result| {},
        );
        require(
            matches!(refused, Err(AdmissionError::Full)),
            "full FIFO did not refuse Create",
        )?;
        require(!builder_ran.get(), "full refusal ran Create builder")?;
        require(
            cursor_snapshot(&self.mailbox) == before_refusal,
            "full refusal burned a cursor",
        )?;
        self.coverage.capacity_refusals += 1;
        self.coverage.no_burn_refusals += 1;

        let mut causal_settled = false;
        for record in expected {
            let mut lease = observed(self.mailbox.try_recv(), "take prelude record")?
                .ok_or_else(|| violation_error("prelude FIFO ended early"))?;
            match record {
                ExpectedRecord::Input(byte) => {
                    require(
                        matches!(lease.record(), QueuedCommand::Input(tagged)
                            if tagged.value().as_bytes() == [byte]),
                        "control FIFO order changed",
                    )?;
                }
                ExpectedRecord::Poll => {
                    self.settle_poll(&lease, feedback)?;
                    causal_settled = true;
                }
                ExpectedRecord::Create => {
                    require(
                        causal_settled,
                        "Create reached worker before Poll settlement",
                    )?;
                    let QueuedCommand::Create(marker) = lease.record_mut() else {
                        return violation("Create marker lost FIFO position");
                    };
                    let actual = marker.operation().key().clone();
                    require(actual == operation, "first Create operation changed")?;
                    require(marker.take_request().is_some(), "first request missing")?;
                    require(
                        marker.take_request().is_none(),
                        "first request was reusable",
                    )?;
                    self.settle_precommit(&actual, returned)?;
                }
            }
            observed(self.mailbox.finish_lease(lease), "finish prelude record")?;
            self.coverage.fifo_records += 1;
        }
        require(
            observed(self.mailbox.try_recv(), "verify drained prelude")?.is_none(),
            "prelude left queued records",
        )?;
        require(
            self.session.phase_kind() == SessionPhaseKind::Ready,
            "definite precommit did not restore Ready",
        )?;
        require(
            self.session.notice() == Some(SessionNotice::CreateNotCreated),
            "definite precommit notice missing",
        )?;
        self.first_operation = Some(operation);
        self.phase = TracePhase::FirstRestored;
        self.coverage.precommit_reentries += 1;
        Ok(())
    }

    fn enqueue_input(&self, byte: u8) -> Result<(), ReplayFailure> {
        observed(
            self.mailbox.try_enqueue(input_command(&self.mailbox, byte)),
            "enqueue control input",
        )
        .map(|_| ())
    }

    fn settle_poll(&mut self, lease: &CommandLease, feedback: bool) -> Result<(), ReplayFailure> {
        let poll_key = lease
            .poll_key()
            .ok_or_else(|| violation_error("expected Poll lease"))?;
        let record = feedback.then(|| vec![0x5a]);
        require(
            observed(
                self.mailbox.publish_poll_result_before_using(
                    poll_key.clone(),
                    record.clone(),
                    self.base + Duration::from_secs(30),
                    &self.clock,
                ),
                "publish Poll result",
            )? == MutationResult::Applied,
            "current Poll result was stale",
        )?;
        let taken = observed(
            self.mailbox.take_event_before_using(&self.clock),
            "take Poll event",
        )?
        .ok_or_else(|| violation_error("Poll event missing"))?;
        let (wait_key, event) = taken.into_parts();
        require(wait_key.is_none(), "Poll unexpectedly required UI reply")?;
        require(
            matches!(event, WorkerEvent::PollResult {
                poll_key: actual_key,
                record: actual_record,
            } if actual_key == poll_key && actual_record == record),
            "Poll payload or identity changed",
        )?;

        let expected = if feedback {
            let settlement = CausalSettlement::Feedback(TerminalViewerAction::Poll {
                attachment_id: fixture_viewer_id(),
            });
            require(
                observed(
                    self.mailbox.deposit_causal(&poll_key, settlement.clone()),
                    "deposit Poll feedback",
                )? == MutationResult::Applied,
                "Poll feedback was not applied",
            )?;
            self.coverage.causal_feedback += 1;
            settlement
        } else {
            require(
                observed(
                    self.mailbox.complete_empty_poll(&poll_key),
                    "settle empty Poll",
                )? == MutationResult::Applied,
                "empty Poll settlement was not applied",
            )?;
            self.coverage.causal_empty += 1;
            CausalSettlement::NoFeedback
        };
        let actual = observed(
            self.mailbox.take_causal_before_using(
                &poll_key,
                self.base + Duration::from_secs(30),
                &self.clock,
            ),
            "take Poll causal settlement",
        )?;
        require(actual == Some(expected), "causal Poll settlement changed")?;
        self.coverage.causal_before_marker += 1;
        Ok(())
    }

    fn settle_precommit(
        &mut self,
        operation: &CreateOperationKey,
        returned: bool,
    ) -> Result<(), ReplayFailure> {
        let outcome = if returned {
            CreatePrecommitOutcome::Returned(WorkbenchTabCreatePrecommit::Denied)
        } else {
            CreatePrecommitOutcome::NotSubmitted
        };
        let wait = observed(
            self.mailbox.publish_event_before_using(
                WorkerEvent::CreatePrecommit {
                    operation: operation.clone(),
                    outcome,
                    applied_geometry: Some(geometry()),
                },
                EventDeadline::Forward(self.base + Duration::from_secs(30)),
                &self.clock,
            ),
            "publish Create precommit",
        )?;
        let (taken_wait, event) = self.take_ui_event()?;
        require(taken_wait == wait, "precommit wait key changed")?;
        let event_matches = match event {
            WorkerEvent::CreatePrecommit {
                operation: actual,
                outcome: CreatePrecommitOutcome::Returned(WorkbenchTabCreatePrecommit::Denied),
                applied_geometry: Some(actual_geometry),
            } => returned && actual == *operation && actual_geometry == geometry(),
            WorkerEvent::CreatePrecommit {
                operation: actual,
                outcome: CreatePrecommitOutcome::NotSubmitted,
                applied_geometry: Some(actual_geometry),
            } => !returned && actual == *operation && actual_geometry == geometry(),
            _ => false,
        };
        require(event_matches, "Create precommit event changed")?;

        let session = &mut self.session;
        require(
            observed(
                self.mailbox
                    .deposit_ui_reply_before_using(&wait, &self.clock, |_transaction| {
                        if !session.restore_precommit(operation, Some(geometry())) {
                            return Err(CarrierFailure::Protocol);
                        }
                        Ok(UiReply::PrecommitRestored)
                    }),
                "deposit precommit reply",
            )? == MutationResult::Applied,
            "precommit reply was stale",
        )?;
        require(
            matches!(self.take_reply(&wait)?, UiReply::PrecommitRestored),
            "precommit reply kind changed",
        )?;
        if returned {
            self.coverage.precommit_returned += 1;
        } else {
            self.coverage.precommit_not_submitted += 1;
        }
        Ok(())
    }

    fn commit_second_create(&mut self) -> Result<(), ReplayFailure> {
        self.mailbox
            .apply_local_for(&mut self.session, NativeSessionState::clear_notice);
        let base = self.base;
        let operation = observed(
            self.mailbox.try_enqueue_create_for(
                &mut self.session,
                |session, generation, request_id| {
                    let marker = create_marker(session, base, generation, request_id, "2")?;
                    if !session.begin_create(marker.operation().clone()) {
                        return Err("state rejected second Create");
                    }
                    Ok(marker)
                },
                |_session, _result| {},
            ),
            "admit second Create",
        )?;
        let first = self
            .first_operation
            .as_ref()
            .ok_or_else(|| invalid_error("missing first operation"))?;
        require(
            operation.reserved_generation() > first.reserved_generation(),
            "second Create reused the first attachment generation",
        )?;
        require(
            operation.request_id() != first.request_id(),
            "second Create reused the first request identity",
        )?;
        require(
            operation.request_id().to_string() == id(self.next_request),
            "second Create skipped or reused a request identity",
        )?;
        self.next_request += 1;
        self.coverage.burned_create_pairs += 1;

        let mut lease = observed(self.mailbox.try_recv(), "take second Create")?
            .ok_or_else(|| violation_error("second Create marker missing"))?;
        let QueuedCommand::Create(marker) = lease.record_mut() else {
            return violation("second Create lost FIFO position");
        };
        require(
            marker.operation().key() == &operation,
            "second operation changed",
        )?;
        require(marker.take_request().is_some(), "second request missing")?;

        self.authority.append_tab();
        let candidate = self.authority.snapshot();
        let commit = pending_commit(&candidate)?;
        let wait = observed(
            self.mailbox.publish_event_before_using(
                WorkerEvent::CreateCommittedKnown {
                    operation: operation.clone(),
                    commit: commit.clone(),
                },
                EventDeadline::Forward(self.base + Duration::from_secs(30)),
                &self.clock,
            ),
            "publish committed Create",
        )?;
        let (taken_wait, event) = self.take_ui_event()?;
        require(taken_wait == wait, "commit wait key changed")?;
        require(
            matches!(event, WorkerEvent::CreateCommittedKnown {
                operation: actual_operation,
                commit: actual_commit,
            } if actual_operation == operation && actual_commit == commit),
            "committed Create event changed",
        )?;
        let session = &mut self.session;
        let retained_commit = commit.clone();
        require(
            observed(
                self.mailbox
                    .deposit_ui_reply_before_using(&wait, &self.clock, |_transaction| {
                        if !session.retain_create_commit(&operation, retained_commit) {
                            return Err(CarrierFailure::Protocol);
                        }
                        Ok(UiReply::CommitRetained)
                    }),
                "deposit committed Create reply",
            )? == MutationResult::Applied,
            "committed Create reply was stale",
        )?;
        require(
            matches!(self.take_reply(&wait)?, UiReply::CommitRetained),
            "committed Create reply kind changed",
        )?;

        let outcome = WorkbenchSnapshotOutcome::Available {
            snapshot: candidate.clone(),
        };
        let wait = observed(
            self.mailbox.publish_event_before_using(
                WorkerEvent::CreateSnapshotResult {
                    operation: operation.clone(),
                    outcome: outcome.clone(),
                },
                EventDeadline::PlacementOnly(self.base + Duration::from_secs(30)),
                &self.clock,
            ),
            "publish committed snapshot",
        )?;
        let (taken_wait, event) = self.take_ui_event()?;
        require(taken_wait == wait, "snapshot wait key changed")?;
        require(
            matches!(event, WorkerEvent::CreateSnapshotResult { operation: actual, outcome: actual_outcome }
            if actual == operation && actual_outcome == outcome),
            "snapshot identity changed",
        )?;
        let crate::terminal_session::state::PreparedCreateSnapshot::Available {
            baseline,
            committed,
            projection,
        } = self.session.prepare_create_snapshot(&operation, outcome)
        else {
            return violation("invalid committed candidate");
        };
        let prepared = observed(
            self.session
                .prepare_create_membership_candidate(&baseline, &committed, projection),
            "prepare Create membership",
        )?;
        let session = &mut self.session;
        require(
            observed(
                self.mailbox
                    .deposit_ui_reply_before_using(&wait, &self.clock, |_transaction| {
                        let cleanup = session.stage_create_membership_cleanup(
                            &operation,
                            prepared,
                            committed.operation().deadlines(),
                        )?;
                        Ok(UiReply::CreateProjection(
                            CreateProjectionDecision::Cleanup(cleanup),
                        ))
                    }),
                "stage Create cleanup",
            )? == MutationResult::Applied,
            "stale Create cleanup",
        )?;
        let UiReply::CreateProjection(CreateProjectionDecision::Cleanup(cleanup)) =
            self.take_reply(&wait)?
        else {
            return violation("cleanup decision missing");
        };
        let wait = observed(
            self.mailbox.publish_event_before_using(
                WorkerEvent::MembershipDetached {
                    cleanup: cleanup.clone(),
                    detached: cleanup.old().clone(),
                },
                EventDeadline::TotalCleanup(cleanup.deadlines().total()),
                &self.clock,
            ),
            "publish exact old cleanup",
        )?;
        let (taken_wait, event) = self.take_ui_event()?;
        require(taken_wait == wait, "cleanup wait key changed")?;
        require(
            matches!(event, WorkerEvent::MembershipDetached { cleanup: actual, detached }
            if actual == cleanup && &detached == cleanup.old()),
            "cleanup identity changed",
        )?;
        let prepared = observed(
            self.session
                .prepare_cleaned_membership_candidate(&cleanup, cleanup.old()),
            "prepare cleaned candidate",
        )?;
        let session = &mut self.session;
        require(observed(self.mailbox.deposit_ui_reply_before_using(&wait, &self.clock, |transaction| {
            let desired = transaction.prepare_desired_install(super::super::membership::DesiredInstall::ReuseReserved { key: committed.committed_key().clone(), panel: prepared.focus_panel_id().cloned().ok_or(CarrierFailure::Protocol)? })?;
            let plan = session.prepare_cleaned_membership_install(&cleanup, prepared, desired.desired().map(|pane| pane.anchor().clone()))?;
            let installed = plan.commit();
            desired.commit();
            if installed != crate::terminal_session::state::MembershipInstallDecision::SelectedWithoutAttachment { return Err(CarrierFailure::Protocol); }
            Ok(UiReply::Membership(crate::terminal_session::mailbox::RefreshSettlement::new(
                crate::terminal_session::state::RefreshDecision::SettledWithoutAttachment, session.desired().cloned(),
            )))
        }), "install committed membership")? == MutationResult::Applied, "stale Create install")?;
        require(
            matches!(self.take_reply(&wait)?, UiReply::Membership(_)),
            "membership reply missing",
        )?;
        observed(self.mailbox.finish_lease(lease), "finish second Create")?;

        require(
            self.session.phase_kind() == SessionPhaseKind::SelectedWithoutAttachment,
            "Pending Create selected the wrong phase",
        )?;
        let active = self
            .session
            .active()
            .ok_or_else(|| violation_error("Pending Create has no active tab"))?;
        require(
            active.index() == 1,
            "Pending Create did not select appended tab",
        )?;
        require(
            active.key() == self.session.desired().unwrap()
                && active.key().generation() == operation.reserved_generation(),
            "Pending Create selected the wrong identity",
        )?;
        require(
            self.session.provenance(0) == Some(&CreateTerminalProvenance::BindingOnly)
                && self.session.provenance(1) == Some(&CreateTerminalProvenance::Pending),
            "Pending Create provenance partition changed",
        )?;
        self.coverage.pending_commits += 1;
        self.phase = TracePhase::SecondProjected;
        Ok(())
    }

    fn refresh_append(&mut self, extra_tabs: u8, control: u8) -> Result<(), ReplayFailure> {
        let old_count = self.session.cache().tabs().len();
        let appended = 1 + usize::from(extra_tabs) % 3;
        for _ in 0..appended {
            self.authority.append_tab();
        }
        let refresh_revision = WorkbenchRevision::new(self.authority.revision);
        let candidate = self.authority.snapshot();
        let expected_tab_ids = candidate.terminal_contexts[0]
            .tabs
            .iter()
            .map(|tab| tab.tab_id.clone())
            .collect::<Vec<_>>();
        let old_active = self
            .session
            .active()
            .cloned()
            .ok_or_else(|| violation_error("refresh has no active tab"))?;
        require(
            old_active.index() > 0,
            "refresh active tab was not non-first",
        )?;
        let old_provenance = (0..old_count)
            .map(|index| {
                self.session
                    .provenance(index)
                    .cloned()
                    .ok_or_else(|| violation_error("missing pre-refresh provenance"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let deadline = self.base + Duration::from_secs(30);
        let generation = observed(
            self.mailbox.try_enqueue_refresh_for(
                &mut self.session,
                deadline,
                |session, generation| {
                    session
                        .start_pending_refresh(generation)
                        .then_some(())
                        .ok_or("state rejected Refresh")
                },
                |_session, _result| {},
            ),
            "admit Refresh",
        )?;
        require(
            self.mailbox.pending_refresh_stage() == Some((generation, PendingRefreshStage::Queued))
                && self.session.pending_refresh_generation() == Some(generation),
            "Refresh did not enter both queued states",
        )?;
        self.coverage.refresh_stages[0] += 1;

        let before_refusal = cursor_snapshot(&self.mailbox);
        let prepare_ran = Cell::new(false);
        let duplicate = self.mailbox.try_enqueue_refresh_for(
            &mut self.session,
            deadline,
            |_session, _generation| -> Result<(), &'static str> {
                prepare_ran.set(true);
                Err("duplicate Refresh invoked prepare")
            },
            |_session, _result| {},
        );
        require(
            matches!(duplicate, Err(AdmissionError::Full)),
            "second pending Refresh was admitted",
        )?;
        require(!prepare_ran.get(), "duplicate Refresh ran prepare")?;
        require(
            cursor_snapshot(&self.mailbox) == before_refusal,
            "duplicate Refresh burned a cursor",
        )?;
        self.coverage.no_burn_refusals += 1;

        self.enqueue_input(control)?;
        let lease = observed(self.mailbox.try_recv(), "take Refresh marker")?
            .ok_or_else(|| violation_error("Refresh marker missing"))?;
        require(
            matches!(lease.record(), QueuedCommand::Refresh(marker)
                if marker.generation() == generation),
            "Refresh lost FIFO position",
        )?;
        require(
            observed(
                self.mailbox.mark_refresh_in_flight(generation),
                "mark Refresh in flight",
            )? == MutationResult::Applied,
            "Refresh in-flight transition was stale",
        )?;
        require(
            self.mailbox.pending_refresh_stage()
                == Some((generation, PendingRefreshStage::InFlight)),
            "Refresh did not enter in-flight stage",
        )?;
        self.coverage.refresh_stages[1] += 1;

        let outcome = WorkbenchSnapshotOutcome::Available {
            snapshot: candidate,
        };
        let wait = observed(
            self.mailbox.publish_event_before_using(
                WorkerEvent::RefreshSnapshotResult {
                    generation,
                    outcome: outcome.clone(),
                },
                EventDeadline::PlacementOnly(deadline),
                &self.clock,
            ),
            "publish Refresh snapshot",
        )?;
        require(
            self.mailbox.pending_refresh_stage()
                == Some((generation, PendingRefreshStage::AwaitingUi)),
            "Refresh did not enter awaiting-UI stage",
        )?;
        self.coverage.refresh_stages[2] += 1;
        require(
            observed(self.mailbox.try_recv(), "probe held control")?.is_none(),
            "accepted control escaped while Refresh awaited UI",
        )?;
        let (taken_wait, event) = self.take_ui_event()?;
        require(taken_wait == wait, "Refresh wait key changed")?;
        require(
            matches!(event, WorkerEvent::RefreshSnapshotResult {
                generation: actual_generation,
                outcome: actual_outcome,
            } if actual_generation == generation && actual_outcome == outcome),
            "Refresh event changed",
        )?;
        let prepared = self.session.prepare_refresh(generation, outcome);
        let session = &mut self.session;
        require(
            observed(
                self.mailbox
                    .deposit_ui_reply_before_using(&wait, &self.clock, |_transaction| {
                        let decision =
                            session.apply_prepared_refresh(prepared, std::time::Instant::now());
                        Ok(UiReply::Refresh(RefreshSettlement::new(
                            decision,
                            session.desired().unwrap().clone(),
                        )))
                    }),
                "deposit Refresh reply",
            )? == MutationResult::Applied,
            "Refresh reply was stale",
        )?;
        require(
            observed(self.mailbox.try_recv(), "probe reply fence")?.is_none(),
            "control escaped before Refresh reply consumption",
        )?;
        require(
            matches!(self.take_reply(&wait)?, UiReply::Refresh(settlement)
                if settlement.decision() == &RefreshDecision::Continue
                    && settlement.desired() == self.session.desired()),
            "Refresh settlement changed",
        )?;
        observed(self.mailbox.finish_lease(lease), "finish Refresh lease")?;
        let control_lease = observed(self.mailbox.try_recv(), "take held control")?
            .ok_or_else(|| violation_error("held control disappeared"))?;
        require(
            matches!(control_lease.record(), QueuedCommand::Input(tagged)
                if tagged.value().as_bytes() == [control]),
            "held control payload changed",
        )?;
        observed(
            self.mailbox.finish_lease(control_lease),
            "finish held control",
        )?;

        require(
            self.session.active() == Some(&old_active),
            "append Refresh changed active selection",
        )?;
        require(
            self.session.cache().tabs().len() == old_count + appended,
            "append Refresh published the wrong tab count",
        )?;
        require(
            self.session
                .cache()
                .tabs()
                .iter()
                .map(|tab| tab.tab_id().clone())
                .eq(expected_tab_ids),
            "append Refresh changed the candidate tab identity order",
        )?;
        for (index, provenance) in old_provenance.iter().enumerate() {
            require(
                self.session.provenance(index) == Some(provenance),
                "append Refresh changed prior provenance",
            )?;
        }
        for index in old_count..self.session.cache().tabs().len() {
            require(
                self.session.provenance(index) == Some(&CreateTerminalProvenance::BindingOnly),
                "append Refresh assigned non-default provenance",
            )?;
        }
        require(
            self.session.refresh_status()
                == Some(NavigatorRefreshStatus::Success(refresh_revision))
                && self.session.pending_refresh_generation().is_none()
                && self.mailbox.pending_refresh_stage().is_none(),
            "Refresh did not settle both pending records",
        )?;
        self.coverage.refresh_controls_held += 1;
        self.coverage.retained_provenance += old_provenance.len();
        self.coverage.active_nonfirst_refreshes += 1;
        self.phase = TracePhase::Refreshed;
        Ok(())
    }

    fn supersede_switch(&mut self, target_hint: u8) -> Result<(), ReplayFailure> {
        let tab_count = self.session.cache().tabs().len();
        require(tab_count >= 2, "Switch trace needs a second tab")?;
        let first_target = 0;
        let second_target = if tab_count == 2 {
            1
        } else {
            2 + usize::from(target_hint) % (tab_count - 2)
        };
        let old_provenance = (0..tab_count)
            .map(|index| {
                self.session
                    .provenance(index)
                    .cloned()
                    .ok_or_else(|| violation_error("missing pre-Switch provenance"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let first = self.admit_switch(first_target)?;
        let first_lease = observed(self.mailbox.try_recv(), "take first Switch")?
            .ok_or_else(|| violation_error("first Switch marker missing"))?;
        require(
            matches!(first_lease.record(), QueuedCommand::Switch(marker)
                if marker.target() == &first),
            "first Switch lost FIFO order",
        )?;
        let current_fence = self.mailbox.execution_fence(&first_lease);
        require(
            current_fence.lease_current()
                && current_fence.desired().key() == Some(first.selected().key()),
            "first Switch was not current at its attempted-attachment cut",
        )?;
        let first = self.install_switch_candidate(&first, &first_lease)?;

        let second = self.admit_switch(second_target)?;
        require(
            first.selected().key().generation() < second.selected().key().generation(),
            "Switch generations did not advance",
        )?;
        require(
            first.selected().key() != second.selected().key()
                && self.session.desired().unwrap() == second.selected().key(),
            "second Switch did not supersede the first",
        )?;

        let stale_fence = self.mailbox.execution_fence(&first_lease);
        require(
            stale_fence.lease_current()
                && stale_fence.desired().key() == Some(second.selected().key()),
            "active first Switch lease did not observe newer desired generation",
        )?;
        self.settle_switch_setup(
            &first,
            CleanupContinuation::Superseded {
                desired: second.selected().key().clone(),
            },
        )?;
        observed(
            self.mailbox.finish_lease(first_lease),
            "finish stale Switch",
        )?;
        self.coverage.stale_switch_settlements += 1;

        let second_lease = observed(self.mailbox.try_recv(), "take current Switch")?
            .ok_or_else(|| violation_error("current Switch marker missing"))?;
        require(
            matches!(second_lease.record(), QueuedCommand::Switch(marker)
                if marker.target() == &second),
            "second Switch lost FIFO order",
        )?;
        require(
            self.mailbox.execution_fence(&second_lease).lease_current(),
            "current Switch fence was stale",
        )?;
        let second = self.install_switch_candidate(&second, &second_lease)?;
        self.drive_switch_ready(&second)?;
        observed(
            self.mailbox.finish_lease(second_lease),
            "finish current Switch",
        )?;
        for (index, provenance) in old_provenance.iter().enumerate() {
            require(
                self.session.provenance(index) == Some(provenance),
                "Switch supersession changed provenance",
            )?;
        }
        let active = self
            .session
            .active()
            .ok_or_else(|| violation_error("current Switch has no active tab"))?;
        require(
            active.key() == second.selected().key() && active.index() == second_target,
            "current Switch selected the wrong tab",
        )?;
        require(
            active.index() > 0,
            "current Switch did not keep a non-first selection",
        )?;
        require(
            self.session.phase_kind() == SessionPhaseKind::Ready,
            "current Switch did not reach Ready",
        )?;
        self.coverage.switch_supersessions += 1;
        self.coverage.current_switch_setups += 1;
        self.phase = TracePhase::SwitchedReady;
        Ok(())
    }

    fn admit_switch(&mut self, index: usize) -> Result<AttachTarget, ReplayFailure> {
        let base = self.base;
        let mut built = None;
        observed(
            self.mailbox.try_enqueue_switch_for(
                &mut self.session,
                |session, generation| {
                    let tab_id = session
                        .tab(index)
                        .ok_or("Switch target missing")?
                        .tab_id()
                        .clone();
                    let key = AttachmentKey::new(generation, tab_id);
                    let target = session
                        .target_for_switch(
                            index,
                            key,
                            OperationDeadlines::from_sample(base)
                                .ok_or("Switch deadline overflow")?,
                        )
                        .ok_or("Switch target rejected")?;
                    if !session.begin_switch(target.clone()) {
                        return Err("state rejected Switch");
                    }
                    built = Some(target.clone());
                    Ok(SwitchMarker::new(target))
                },
                |session, result, facts| {
                    if result.is_ok() {
                        session.finish_switch_membership_admission(
                            facts.discarded(),
                            facts.inherited(),
                            facts.authorized_cleanup(),
                        );
                    }
                },
            ),
            "admit Switch",
        )?;
        built.ok_or_else(|| violation_error("Switch builder did not return target"))
    }

    fn install_unattached_switch_candidate(
        &mut self,
        marker: &AttachTarget,
    ) -> Result<AttachTarget, ReplayFailure> {
        // This reducer trace models a validated Snapshot at the common installation cut.
        // The scripted executor suite owns Snapshot invocation and exact reply receipts.
        let prepared = observed(
            self.session.prepare_membership_candidate(
                self.session.cache().clone(),
                Some(marker.selected().key().tab_id()),
                Some(marker.selected().index()),
                None,
                None,
            ),
            "prepare Switch candidate",
        )?;
        let mut locked = self.mailbox.lock();
        let mut transaction = UiReplyTransaction {
            state: &mut locked,
            observed_at: self.base,
        };
        let desired = observed(
            transaction.prepare_desired_install(DesiredInstall::Fresh {
                tab_id: marker.selected().key().tab_id().clone(),
                panel: marker.selected().key().panel().clone(),
            }),
            "prepare fresh Switch successor",
        )?;
        let plan = observed(
            self.session.prepare_switch_membership_install_k(
                marker.selected().key(),
                prepared,
                desired.desired().map(|pane| pane.anchor().clone()),
            ),
            "prepare canonical Switch install",
        )?;
        let decision = plan.commit();
        desired.commit();
        let crate::terminal_session::state::MembershipInstallDecision::Attach(target) = decision
        else {
            return violation("Switch candidate must be attachable");
        };
        Ok(target)
    }

    /// Installs the switch target: an unattached session installs directly, while an
    /// attached session must first drain the authorized old-attachment cleanup that the
    /// switch admission reserved before the new candidate can be installed.
    fn install_switch_candidate(
        &mut self,
        marker: &AttachTarget,
        lease: &CommandLease,
    ) -> Result<AttachTarget, ReplayFailure> {
        if self.session.attached_key().is_none() {
            return self.install_unattached_switch_candidate(marker);
        }
        let prepared = observed(
            self.session.prepare_membership_candidate(
                self.session.cache().clone(),
                Some(marker.selected().key().tab_id()),
                Some(marker.selected().index()),
                None,
                None,
            ),
            "prepare attached Switch candidate",
        )?;
        let cleanup = observed(
            self.session.stage_switch_membership_cleanup_k(
                marker.selected().key(),
                prepared,
                marker.deadlines(),
            ),
            "stage Switch membership cleanup",
        )?;
        self.drive_switch_membership_cleanup(&cleanup, marker, lease)
    }

    /// Mirrors the worker's switch cleanup drive and `NativeUi::apply_membership_detached`
    /// for a `MembershipResultKey::Switch` cleanup: the lease authorizes the cleanup call,
    /// the answered receipt gates the `MembershipDetached` publication, and the settlement
    /// reply kind is `UiReply::Switch`, not `UiReply::Membership`.
    fn drive_switch_membership_cleanup(
        &mut self,
        cleanup: &MembershipCleanup,
        marker: &AttachTarget,
        lease: &CommandLease,
    ) -> Result<AttachTarget, ReplayFailure> {
        require(
            cleanup.installs_candidate(),
            "switch cleanup lost its candidate",
        )?;
        let cleanup = cleanup.clone();
        // The cleanup authorization samples a real monotonic clock; the frozen trace clock
        // only feeds the mailbox's `MailboxClock` seam.
        let operation_clock = crate::local_client::test_server::adjustable_operation_clock(
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        );
        let permit = observed(
            self.mailbox
                .authorize_switch_cleanup(lease, &cleanup, cleanup.old(), &operation_clock),
            "authorize switch cleanup",
        )?
        .ok_or_else(|| violation_error("switch cleanup permit lost currency"))?;
        // The authorized cleanup call burns one workbench identity from the shared cursor.
        self.next_request += 1;
        require(
            observed(
                self.mailbox.answer_switch_cleanup(permit, true),
                "answer switch cleanup",
            )?,
            "switch cleanup answer did not retain the detached receipt",
        )?;
        let wait = observed(
            self.mailbox.publish_event_before_using(
                WorkerEvent::MembershipDetached {
                    cleanup: cleanup.clone(),
                    detached: cleanup.old().clone(),
                },
                EventDeadline::TotalCleanup(cleanup.deadlines().total()),
                &self.clock,
            ),
            "publish switch MembershipDetached",
        )?;
        require(
            matches!(&wait.correlation, UiCorrelation::Membership(result, old)
                if result == cleanup.result() && old == cleanup.old())
                && wait.reply_kind == UiReplyKind::Switch,
            "switch-detached wait key correlation changed",
        )?;
        let (taken_wait, event) = self.take_ui_event()?;
        require(taken_wait == wait, "switch-detached wait key changed")?;
        require(
            matches!(event, WorkerEvent::MembershipDetached { cleanup: actual, detached }
                if actual == cleanup && detached == *cleanup.old()),
            "switch-detached payload changed",
        )?;
        self.coverage.panel_membership_detachments += 1;

        let prepared = observed(
            self.session
                .prepare_cleaned_membership_candidate(&cleanup, cleanup.old()),
            "prepare cleaned switch candidate",
        )?;
        let session = &mut self.session;
        let marker_key = marker.selected().key().clone();
        require(
            observed(
                self.mailbox
                    .deposit_ui_reply_before_using(&wait, &self.clock, |transaction| {
                        if !session.switch_marker_is_current_k(&marker_key) {
                            session.settle_switch_membership_cleanup(&cleanup, true)?;
                            return Ok(UiReply::Switch(RefreshSettlement::new(
                                RefreshDecision::Continue,
                                session.desired().cloned(),
                            )));
                        }
                        let install =
                            prepared
                                .selected_tab_id()
                                .map_or(DesiredInstall::Empty, |tab| DesiredInstall::Fresh {
                                    tab_id: tab.clone(),
                                    panel: marker_key.panel().clone(),
                                });
                        let desired = transaction.prepare_desired_install(install)?;
                        let plan = session.prepare_cleaned_membership_install(
                            &cleanup,
                            prepared,
                            desired.desired().map(|pane| pane.anchor().clone()),
                        )?;
                        let installed = plan.commit();
                        desired.commit();
                        let decision = match installed {
                            MembershipInstallDecision::Attach(target) => {
                                RefreshDecision::AttachRecovered(target)
                            }
                            MembershipInstallDecision::SelectedWithoutAttachment
                            | MembershipInstallDecision::Empty => {
                                RefreshDecision::SettledWithoutAttachment
                            }
                            MembershipInstallDecision::Preserved => RefreshDecision::Continue,
                        };
                        Ok(UiReply::Switch(RefreshSettlement::new(
                            decision,
                            session.desired().cloned(),
                        )))
                    }),
                "deposit switch-detached reply",
            )? == MutationResult::Applied,
            "switch-detached reply was stale",
        )?;
        let UiReply::Switch(settlement) = self.take_reply(&wait)? else {
            return violation("switch-detached reply kind changed");
        };
        self.coverage.panel_membership_installs += 1;
        match settlement.decision() {
            RefreshDecision::AttachRecovered(target) => Ok(target.clone()),
            other => violation(format!(
                "attached Switch install did not recover an attachment: {other:?}"
            )),
        }
    }

    fn settle_switch_setup(
        &mut self,
        target: &AttachTarget,
        expected: CleanupContinuation,
    ) -> Result<(), ReplayFailure> {
        // This internal worker event represents an exact, definite no-attachment result from the
        // attempted Attach. The real carrier classification is covered by the scripted executor
        // suite; this lifecycle trace exercises its atomic mailbox/reducer settlement.
        let wait = observed(
            self.mailbox.publish_event_before_using(
                WorkerEvent::SetupWithoutAttachment {
                    key: target.pane_ref(),
                    target_index: target.selected().index(),
                    target_provenance: target.provenance().clone(),
                },
                EventDeadline::Forward(self.base + Duration::from_secs(30)),
                &self.clock,
            ),
            "publish stale Switch setup",
        )?;
        let (taken_wait, event) = self.take_ui_event()?;
        require(taken_wait == wait, "Switch setup wait key changed")?;
        require(
            matches!(event, WorkerEvent::SetupWithoutAttachment {
                key,
                target_index,
                target_provenance,
            } if key == *target.selected().key()
                && target_index == target.selected().index()
                && target_provenance == *target.provenance()),
            "Switch setup event changed",
        )?;
        let session = &mut self.session;
        let key = target.selected().key().clone();
        let index = target.selected().index();
        let provenance = target.provenance().clone();
        require(
            observed(
                self.mailbox
                    .deposit_ui_reply_before_using(&wait, &self.clock, |_transaction| {
                        let continuation = session
                            .settle_unattached_setup_t(&key, index, &provenance)
                            .ok_or(CarrierFailure::Protocol)?;
                        Ok(UiReply::SetupWithoutAttachmentApplied(continuation))
                    }),
                "deposit Switch setup reply",
            )? == MutationResult::Applied,
            "Switch setup reply was stale",
        )?;
        require(
            matches!(self.take_reply(&wait)?, UiReply::SetupWithoutAttachmentApplied(actual)
                if actual == expected),
            "Switch setup continuation changed",
        )
    }

    fn drive_switch_ready(&mut self, target: &AttachTarget) -> Result<(), ReplayFailure> {
        let key = target.selected().key();
        require(
            self.session.install_projection_t(key),
            "Switch projection install",
        )?;
        require(
            self.session
                .record_setup_configuration_t(key, controlling(), Some(geometry())),
            "Switch setup configuration",
        )?;
        let frame = FrameIdentity::new(2);
        require(
            self.session.stage_drawn_frame_t(key, frame),
            "Switch frame stage",
        )?;
        require(
            self.session.mark_feedback_accepted_t(key, frame),
            "Switch frame feedback",
        )?;
        require(
            self.session
                .publish_ready_t(key, frame, controlling(), Some(geometry())),
            "Switch Ready publication",
        )
    }

    fn exhaust_with_known_commit(
        &mut self,
        later_failure: LaterFailure,
    ) -> Result<(), ReplayFailure> {
        let tab_count = self.session.cache().tabs().len();
        let label = (tab_count + 1).to_string();
        let base = self.base;
        let operation = observed(
            self.mailbox.try_enqueue_create_for(
                &mut self.session,
                |session, generation, request_id| {
                    let marker = create_marker(session, base, generation, request_id, &label)?;
                    if !session.begin_create(marker.operation().clone()) {
                        return Err("state rejected retirement Create");
                    }
                    Ok(marker)
                },
                |_session, _result| {},
            ),
            "admit retirement Create",
        )?;
        self.enqueue_input(0xfe)?;
        let mut lease = observed(self.mailbox.try_recv(), "take retirement Create")?
            .ok_or_else(|| violation_error("retirement Create marker missing"))?;
        let QueuedCommand::Create(marker) = lease.record_mut() else {
            return violation("retirement Create lost FIFO position");
        };
        require(
            marker.operation().key() == &operation,
            "retirement operation changed",
        )?;
        require(
            marker.take_request().is_some(),
            "retirement request missing",
        )?;

        self.authority.append_tab();
        let committed_snapshot = self.authority.snapshot();
        let commit = pending_commit(&committed_snapshot)?;
        let pending = PendingCreateCommit::new(marker.operation().clone(), commit.clone());
        require(
            pending.operation().key() == &operation,
            "worker-known commit lost its operation",
        )?;

        {
            let mut raw = self.mailbox.lock();
            raw.identity_cursor = u128::MAX;
        }
        let before_exhaustion = cursor_snapshot(&self.mailbox);
        let closed = self
            .mailbox
            .allocate_workbench_request_id()
            .expect_err("identity exhaustion must close mailbox");
        require(
            closed.failure() == CarrierFailure::IdentityExhausted,
            "identity exhaustion reported the wrong cause",
        )?;
        require(
            cursor_snapshot(&self.mailbox) == before_exhaustion,
            "identity exhaustion burned another cursor",
        )?;
        require(
            self.mailbox.lock().queue.is_empty(),
            "close retained queued control storage",
        )?;
        require(
            self.session
                .finalize_retirement(CarrierFailure::IdentityExhausted, Some(pending)),
            "retirement rejected exact worker-known commit",
        )?;
        let retained = self
            .session
            .committed_target()
            .ok_or_else(|| violation_error("retirement dropped worker-known commit"))?;
        require(
            retained.operation().key() == &operation
                && retained.provenance() == &CreateTerminalProvenance::Pending,
            "retirement changed worker-known commit identity or provenance",
        )?;
        require(
            self.session
                .finalize_retirement(later_failure.failure(), None),
            "repeated retirement finalization rejected empty handoff",
        )?;
        require(
            self.mailbox.close(later_failure.failure()).failure()
                == CarrierFailure::IdentityExhausted,
            "mailbox replaced its first close cause",
        )?;
        require(
            self.session.retired_failure() == Some(CarrierFailure::IdentityExhausted),
            "state replaced its first retirement cause",
        )?;
        require(
            matches!(self.mailbox.try_recv(), Err(closed)
                if closed.failure() == CarrierFailure::IdentityExhausted),
            "closed mailbox admitted more work",
        )?;
        drop(lease);
        self.coverage.known_commit_handoffs += 1;
        self.coverage.identity_exhaustions += 1;
        self.coverage.later_failures[later_failure.index()] += 1;
        self.coverage.completed_traces += 1;
        self.retire_cause = Some(CarrierFailure::IdentityExhausted);
        self.phase = TracePhase::Retired;
        Ok(())
    }

    /// One generated panel lifecycle: admission, blocked-peer probes, queueing, the in-flight
    /// mark, a real result publication anchored to this lease, stale-reply probes, the UI
    /// settlement deposit, and any membership cleanup/install or publication-drain follow-up.
    fn panel_cycle(
        &mut self,
        kind_sel: u8,
        outcome_sel: u8,
        probe: u8,
    ) -> Result<(), ReplayFailure> {
        self.coverage.panel_cycles += 1;
        let deadline = self.base + Duration::from_secs(30);
        let active_index = self.session.active().map(|active| active.index());
        let requested = panel_kind(kind_sel);
        let mut operation = None;
        let mut kind = requested;
        let mut seen = Vec::new();
        for candidate in [
            requested,
            PanelRequestKind::Split {
                direction: PanelDirection::Right,
            },
            PanelRequestKind::Move {
                direction: PanelDirection::Right,
            },
            PanelRequestKind::Close,
        ] {
            if operation.is_some() || seen.contains(&candidate) {
                continue;
            }
            seen.push(candidate);
            if matches!(candidate, PanelRequestKind::Close)
                && active_index.is_some_and(|index| self.authority.leaf_count(index) < 2)
            {
                continue;
            }
            let before = cursor_snapshot(&self.mailbox);
            match self.mailbox.try_enqueue_panel_for_using(
                &mut self.session,
                deadline,
                &self.clock,
                |session, request_id| {
                    session
                        .plan_panel(request_id, candidate, deadline)
                        .ok_or("inadmissible")
                },
                |session, planned| {
                    session
                        .begin_panel(planned.clone())
                        .then_some(())
                        .ok_or("begin refused")
                },
            ) {
                Ok(planned) => {
                    kind = candidate;
                    operation = Some(planned);
                }
                Err(MoveAdmissionError::Build("inadmissible")) => {
                    require(
                        cursor_snapshot(&self.mailbox) == before,
                        "refused Panel admission burned a cursor",
                    )?;
                    self.coverage.panel_admission_refusals += 1;
                    self.coverage.no_burn_refusals += 1;
                }
                Err(error) => {
                    return violation(format!("panel admission failed unexpectedly: {error:?}"));
                }
            }
        }
        let Some(operation) = operation else {
            self.coverage.all_refused_cycles += 1;
            return Ok(());
        };
        require(
            operation.request_id().to_string() == id(self.next_request),
            "Panel admission skipped or reused a request identity",
        )?;
        self.next_request += 1;
        self.coverage.panel_admissions += 1;
        self.coverage.panel_kinds[kind_index(kind)] += 1;
        require(
            self.session.pending_panel() == Some(&operation),
            "state did not record the admitted Panel operation",
        )?;
        let flight_serial = {
            let raw = self.mailbox.lock();
            match &raw.structural_mutation_pending {
                Some(StructuralMutationFlight::Panel(flight))
                    if flight.operation == operation
                        && flight.stage == PendingRefreshStage::Queued =>
                {
                    flight.serial
                }
                _ => return violation("panel flight did not enter the queued stage"),
            }
        };
        self.coverage.panel_stages[0] += 1;

        let tail = probe.wrapping_add(0x61);
        self.enqueue_input(tail)?;

        let before = cursor_snapshot(&self.mailbox);
        let duplicate = self.mailbox.try_enqueue_panel_for_using(
            &mut self.session,
            deadline,
            &self.clock,
            |session, request_id| session.plan_panel(request_id, kind, deadline).ok_or("dup"),
            |session, planned| {
                session
                    .begin_panel(planned.clone())
                    .then_some(())
                    .ok_or("dup")
            },
        );
        require(
            matches!(duplicate, Err(MoveAdmissionError::Busy)),
            "a second Panel was admitted while the first was pending",
        )?;
        let refresh = self.mailbox.try_enqueue_refresh_for(
            &mut self.session,
            deadline,
            |session, generation| {
                session
                    .start_pending_refresh(generation)
                    .then_some(())
                    .ok_or("refresh prepare ran during Panel flight")
            },
            |_session, _result| {},
        );
        require(
            matches!(refresh, Err(AdmissionError::Full)),
            "Refresh was admitted while a Panel mutation was pending",
        )?;
        let switch = self.mailbox.try_enqueue_switch_for(
            &mut self.session,
            |_, _| -> Result<SwitchMarker, &'static str> {
                Err("switch built during Panel flight")
            },
            |_, _, _| {},
        );
        require(
            matches!(switch, Err(AdmissionError::Full)),
            "Switch was admitted while a Panel mutation was pending",
        )?;
        require(
            cursor_snapshot(&self.mailbox) == before,
            "blocked admissions burned a cursor",
        )?;
        self.coverage.panel_dup_refusals += 1;
        self.coverage.panel_refresh_blocks += 1;
        self.coverage.panel_switch_blocks += 1;
        self.coverage.no_burn_refusals += 3;

        let lease = observed(self.mailbox.try_recv(), "take Panel marker")?
            .ok_or_else(|| violation_error("Panel marker missing"))?;
        require(
            matches!(lease.record(), QueuedCommand::Panel(marker)
                if marker.operation() == &operation),
            "Panel marker lost its operation",
        )?;
        require(
            lease.serial() == flight_serial,
            "Panel lease serial diverged from the flight",
        )?;

        let forged = PanelOperation::new(
            self.fixture_request(),
            operation.target().clone(),
            operation.panel_id().clone(),
            operation.baseline().clone(),
            kind,
            operation.intent().clone(),
            operation.deadline(),
        );
        require(
            observed(
                self.mailbox.mark_panel_in_flight(&forged),
                "stale Panel in-flight mark",
            )? == MutationResult::Stale,
            "a foreign Panel operation reached in-flight",
        )?;
        require(
            observed(
                self.mailbox.mark_panel_in_flight(&operation),
                "mark Panel in flight",
            )? == MutationResult::Applied,
            "the current Panel mark was stale",
        )?;
        require(
            observed(
                self.mailbox.mark_panel_in_flight(&operation),
                "re-mark Panel in flight",
            )? == MutationResult::Stale,
            "an in-flight Panel accepted a second mark",
        )?;
        self.coverage.panel_marks += 1;
        self.coverage.panel_stale_marks += 2;
        {
            let raw = self.mailbox.lock();
            require(
                matches!(&raw.structural_mutation_pending,
                    Some(StructuralMutationFlight::Panel(flight))
                        if flight.stage == PendingRefreshStage::InFlight),
                "Panel flight did not reach in-flight stage",
            )?;
        }
        self.coverage.panel_stages[1] += 1;
        require(
            observed(self.mailbox.try_recv(), "probe held input")?.is_none(),
            "queued input escaped while the Panel lease was active",
        )?;

        let (outcome, class, expected_decision) =
            self.build_panel_outcome(&operation, kind, outcome_sel)?;
        self.coverage.panel_outcomes[class.index()] += 1;
        if matches!(
            class,
            OutcomeClass::CommittedKeep
                | OutcomeClass::CommittedDrop
                | OutcomeClass::CommittedTabGone
        ) {
            self.allocate_read()?;
        }

        let wait = observed(
            self.mailbox.publish_event_before_using(
                WorkerEvent::PanelResult {
                    operation: operation.clone(),
                    outcome: outcome.clone(),
                },
                EventDeadline::PlacementOnly(deadline),
                &self.clock,
            ),
            "publish Panel result",
        )?;
        require(
            wait.correlation == UiCorrelation::Panel(operation.request_id().clone())
                && wait.reply_kind == UiReplyKind::Panel,
            "Panel result wait key correlation changed",
        )?;
        {
            let raw = self.mailbox.lock();
            require(
                matches!(&raw.structural_mutation_pending,
                    Some(StructuralMutationFlight::Panel(flight))
                        if flight.stage == PendingRefreshStage::AwaitingUi),
                "Panel result did not move the flight to awaiting-UI",
            )?;
            require(
                raw.event_anchor_serial == Some(flight_serial),
                "Panel result was not anchored to its own lease",
            )?;
        }
        self.coverage.panel_stages[2] += 1;
        self.coverage.panel_publishes += 1;

        if probe & 1 != 0 {
            let stale = UiWaitKey {
                serial: wait.serial().saturating_sub(1),
                correlation: wait.correlation.clone(),
                reply_kind: wait.reply_kind,
            };
            let ran = Cell::new(false);
            require(
                observed(
                    self.mailbox.deposit_ui_reply_before_using(
                        &stale,
                        &self.clock,
                        |_transaction| {
                            ran.set(true);
                            Ok(UiReply::PrecommitRestored)
                        },
                    ),
                    "deposit earlier-serial Panel reply",
                )? == MutationResult::Stale,
                "an older-serial Panel reply was applied",
            )?;
            require(!ran.get(), "stale Panel reply ran the settlement closure")?;
            self.coverage.panel_stale_replies += 1;
        }
        if probe & 4 != 0
            && let Some(previous) = self.last_panel_wait.clone()
        {
            let ran = Cell::new(false);
            require(
                observed(
                    self.mailbox.deposit_ui_reply_before_using(
                        &previous,
                        &self.clock,
                        |_transaction| {
                            ran.set(true);
                            Ok(UiReply::PrecommitRestored)
                        },
                    ),
                    "deposit previous-cycle Panel reply",
                )? == MutationResult::Stale,
                "an old Panel reply settled under a newer request",
            )?;
            require(!ran.get(), "old Panel reply ran the settlement closure")?;
            self.coverage.panel_prev_wait_stale += 1;
        }

        let (taken_wait, event) = self.take_ui_event()?;
        require(taken_wait == wait, "Panel event wait key changed")?;
        require(
            matches!(event, WorkerEvent::PanelResult { operation: actual, outcome: actual_outcome }
                if actual == operation && actual_outcome == outcome),
            "Panel event payload changed",
        )?;

        // Mirrors `NativeUi::apply_panel_event`: prepare outside the reply mutex, then settle
        // inside the exact reply transaction with the same membership cleanup/install pairing.
        let prepared = self.session.prepare_panel(&operation, outcome);
        let membership = prepared.membership_candidate().is_some();
        let deadlines = membership
            .then(|| OperationDeadlines::from_sample(self.base))
            .flatten();
        let session = &mut self.session;
        require(
            observed(
                self.mailbox
                    .deposit_ui_reply_before_using(&wait, &self.clock, |transaction| {
                        let decision = if membership {
                            let (operation, prepared) =
                                session.retain_prepared_panel_membership(prepared)?;
                            let deadlines = deadlines.ok_or(CarrierFailure::Deadline)?;
                            if transaction.observed_at() >= deadlines.forward_cutoff() {
                                return Err(CarrierFailure::Deadline);
                            }
                            if session.has_attachments() {
                                PanelDecision::Cleanup(session.stage_panel_membership_cleanup(
                                    operation.request_id(),
                                    prepared,
                                    deadlines,
                                )?)
                            } else {
                                let install = prepared.selected_tab_id().map_or(
                                    DesiredInstall::Empty,
                                    |tab| DesiredInstall::Fresh {
                                        tab_id: tab.clone(),
                                        panel: prepared
                                            .focus_panel_id()
                                            .cloned()
                                            .unwrap_or_else(panel),
                                    },
                                );
                                let desired = transaction.prepare_desired_install(install)?;
                                let plan = session.prepare_panel_membership_install(
                                    operation.request_id(),
                                    prepared,
                                    desired.desired().map(|pane| pane.anchor().clone()),
                                    Some(deadlines),
                                )?;
                                let installed = plan.commit();
                                desired.commit();
                                match installed {
                                    MembershipInstallDecision::Attach(target) => {
                                        PanelDecision::AttachRecovered(target)
                                    }
                                    MembershipInstallDecision::SelectedWithoutAttachment
                                    | MembershipInstallDecision::Empty => {
                                        PanelDecision::SettledWithoutAttachment
                                    }
                                    MembershipInstallDecision::Preserved => PanelDecision::Continue,
                                }
                            }
                        } else {
                            session.apply_prepared_panel(prepared)
                        };
                        Ok(UiReply::Panel(PanelSettlement::new(
                            decision,
                            session.desired().cloned(),
                        )))
                    }),
                "deposit Panel reply",
            )? == MutationResult::Applied,
            "the current Panel reply was stale",
        )?;
        let UiReply::Panel(settlement) = self.take_reply(&wait)? else {
            return violation("Panel reply kind changed");
        };
        self.coverage.panel_replies += 1;
        self.last_panel_wait = Some(wait.clone());

        if probe & 2 != 0 {
            let ran = Cell::new(false);
            require(
                observed(
                    self.mailbox.deposit_ui_reply_before_using(
                        &wait,
                        &self.clock,
                        |_transaction| {
                            ran.set(true);
                            Ok(UiReply::PrecommitRestored)
                        },
                    ),
                    "re-deposit consumed Panel reply",
                )? == MutationResult::Stale,
                "a consumed Panel reply was applied again",
            )?;
            require(
                !ran.get(),
                "consumed Panel reply ran the settlement closure",
            )?;
            self.coverage.panel_post_settle_redeposits += 1;
        }

        let mut needs_drain = false;
        let decision_index = match settlement.decision() {
            PanelDecision::Continue => 0,
            PanelDecision::Cleanup(_) => 1,
            PanelDecision::AttachRecovered(_) => 2,
            PanelDecision::SettledWithoutAttachment => 3,
            PanelDecision::Retire => 4,
        };
        // The decision the settlement returned is checked against the expectation derived
        // from the input class and the pre-settlement facts — never echoed back — so a
        // misclassified membership result cannot hide behind the Continue catch-all.
        if decision_index != expected_decision {
            return violation(format!(
                "settlement decision diverged from the input-fact oracle: class={class:?} expected={expected_decision} got={decision_index}"
            ));
        }
        self.coverage.panel_decisions[decision_index] += 1;
        // Side facts each input class must leave behind, asserted independently of the
        // returned decision and before any drive mutates the session further.
        match class {
            OutcomeClass::NotSubmitted
            | OutcomeClass::Rejected
            | OutcomeClass::Denied
            | OutcomeClass::Unavailable => {
                require(
                    self.session.notice() == Some(SessionNotice::PanelNotApplied),
                    "non-committed Panel lost its PanelNotApplied notice",
                )?;
                require(
                    self.session
                        .retained_panel_for_test()
                        .is_none_or(|pending| pending.operation() != &operation),
                    "non-committed Panel retained a commit",
                )?;
                if class == OutcomeClass::Rejected {
                    // The peer commit that justified the RevisionMismatch left the
                    // authority ahead of the session cache: resync through the drain
                    // refresh before any further fresh outcome is honest.
                    needs_drain = true;
                }
            }
            OutcomeClass::Unchanged => {
                require(
                    self.session.notice().is_none(),
                    "Unchanged Panel left a notice",
                )?;
                require(
                    self.session
                        .retained_panel_for_test()
                        .is_none_or(|pending| pending.operation() != &operation),
                    "Unchanged Panel retained a commit",
                )?;
            }
            OutcomeClass::CommittedKeep => {
                require(
                    self.session.notice().is_none(),
                    "committed Panel left a notice",
                )?;
                require(
                    self.session
                        .retained_panel_for_test()
                        .is_some_and(|pending| pending.operation() == &operation),
                    "committed Panel did not retain its commit",
                )?;
            }
            OutcomeClass::CommittedNoSnapshot => {
                require(
                    self.session.notice().is_none(),
                    "snapshot-less commit left a notice",
                )?;
                require(
                    self.session
                        .retained_panel_for_test()
                        .is_some_and(|pending| pending.operation() == &operation),
                    "snapshot-less commit did not retain its commit",
                )?;
                require(
                    self.session.panel_recovery_for_test().is_some(),
                    "snapshot-less commit did not arm panel recovery",
                )?;
                self.coverage.panel_recovery_arms += 1;
                needs_drain = true;
            }
            OutcomeClass::CloseAccepted => {
                require(
                    self.session.notice().is_none(),
                    "accepted close left a notice",
                )?;
                require(
                    self.session.publication_debt_pending(),
                    "CloseAccepted did not arm publication debt",
                )?;
                // The receipt latches session-side debt only; the mailbox's own
                // publication latch is a separate ingress set by poll-result hints.
                require(
                    !self.mailbox.lock().publication_hint_pending,
                    "CloseAccepted latched the mailbox publication hint",
                )?;
                self.coverage.panel_publication_drains += 1;
                needs_drain = true;
            }
            OutcomeClass::CommittedDrop | OutcomeClass::CommittedTabGone => {
                // The membership path retains the commit's pending record; the notice
                // stays clear.
                require(
                    self.session.notice().is_none(),
                    "membership commit left a notice",
                )?;
                require(
                    self.session
                        .retained_panel_for_test()
                        .is_some_and(|pending| pending.operation() == &operation),
                    "membership commit did not retain its commit",
                )?;
            }
        }
        match settlement.decision() {
            PanelDecision::Continue => {}
            PanelDecision::Cleanup(cleanup) => {
                self.coverage.panel_membership_cleanups += 1;
                let cleanup = cleanup.clone();
                self.drive_membership_cleanup(&cleanup)?;
            }
            PanelDecision::AttachRecovered(target) => {
                let target = target.clone();
                self.drive_attach(&target)?;
            }
            PanelDecision::SettledWithoutAttachment => {
                require(
                    self.session.attached_key().is_none(),
                    "SettledWithoutAttachment left an attachment",
                )?;
            }
            PanelDecision::Retire => {
                return violation("generated Panel cycle retired mid-trace");
            }
        }

        observed(self.mailbox.finish_lease(lease), "finish Panel lease")?;
        let tail_lease = observed(self.mailbox.try_recv(), "take held input")?
            .ok_or_else(|| violation_error("input queued behind Panel disappeared"))?;
        require(
            matches!(tail_lease.record(), QueuedCommand::Input(tagged)
                if tagged.value().as_bytes() == [tail]),
            "held input payload changed",
        )?;
        observed(self.mailbox.finish_lease(tail_lease), "finish held input")?;

        if needs_drain {
            self.drain_refresh()?;
        }
        require(
            self.mailbox.lock().structural_mutation_pending.is_none(),
            "Panel cycle left a structural mutation pending",
        )?;
        require(
            self.session.pending_panel().is_none(),
            "Panel cycle left a pending operation",
        )?;
        Ok(())
    }

    /// Mirrors `NativeUi::apply_membership_detached` for the Panel/Refresh result keys: the old
    /// attachment is fully detached and settled before the membership install commits a new one.
    fn drive_membership_cleanup(
        &mut self,
        cleanup: &MembershipCleanup,
    ) -> Result<(), ReplayFailure> {
        require(
            cleanup.installs_candidate(),
            "membership cleanup lost its candidate",
        )?;
        let cleanup = cleanup.clone();
        // Ordering witnesses on the real session: a second install path and a mismatched
        // detachment must both refuse while the cleanup is still pending.
        if let MembershipResultKey::Panel(request_id) = cleanup.result() {
            let prepared = observed(
                self.session
                    .prepare_cleaned_membership_candidate(&cleanup, cleanup.old()),
                "duplicate cleaned candidate",
            )?;
            require(
                self.session
                    .prepare_panel_membership_install(request_id, prepared, None, None)
                    .is_err(),
                "panel membership install was admitted before cleanup settled",
            )?;
            self.coverage.panel_ordering_probes += 1;
        }
        let wrong_detached = AttachmentKey::new(
            cleanup.old().generation() + 0x1000,
            cleanup.old().tab_id().clone(),
        );
        require(
            self.session
                .prepare_cleaned_membership_candidate(&cleanup, &wrong_detached)
                .is_err(),
            "cleaned candidate accepted a mismatched detachment",
        )?;
        self.coverage.panel_ordering_probes += 1;

        let wait = observed(
            self.mailbox.publish_event_before_using(
                WorkerEvent::MembershipDetached {
                    cleanup: cleanup.clone(),
                    detached: cleanup.old().clone(),
                },
                EventDeadline::TotalCleanup(cleanup.deadlines().total()),
                &self.clock,
            ),
            "publish MembershipDetached",
        )?;
        require(
            matches!(&wait.correlation, UiCorrelation::Membership(result, old)
                if result == cleanup.result() && old == cleanup.old())
                && wait.reply_kind == UiReplyKind::Membership,
            "membership-detached wait key correlation changed",
        )?;
        let (taken_wait, event) = self.take_ui_event()?;
        require(taken_wait == wait, "membership-detached wait key changed")?;
        require(
            matches!(event, WorkerEvent::MembershipDetached { cleanup: actual, detached }
                if actual == cleanup && detached == *cleanup.old()),
            "membership-detached payload changed",
        )?;
        self.coverage.panel_membership_detachments += 1;

        let prepared = observed(
            self.session
                .prepare_cleaned_membership_candidate(&cleanup, cleanup.old()),
            "prepare cleaned membership candidate",
        )?;
        let session = &mut self.session;
        let detached_cleanup = cleanup.clone();
        require(
            observed(
                self.mailbox
                    .deposit_ui_reply_before_using(&wait, &self.clock, |transaction| {
                        let install = match prepared.selected_tab_id() {
                            Some(tab) => {
                                let reserved =
                                    session.create_membership_commit().and_then(|committed| {
                                        (committed.committed_key().tab_id() == tab)
                                            .then(|| committed.committed_key().clone())
                                    });
                                let focus_panel =
                                    prepared.focus_panel_id().cloned().unwrap_or_else(panel);
                                reserved.map_or(
                                    DesiredInstall::Fresh {
                                        tab_id: tab.clone(),
                                        panel: focus_panel.clone(),
                                    },
                                    |key| DesiredInstall::ReuseReserved {
                                        key,
                                        panel: focus_panel,
                                    },
                                )
                            }
                            None => DesiredInstall::Empty,
                        };
                        let desired = transaction.prepare_desired_install(install)?;
                        let plan = session.prepare_cleaned_membership_install(
                            &detached_cleanup,
                            prepared,
                            desired.desired().map(|pane| pane.anchor().clone()),
                        )?;
                        let installed = plan.commit();
                        desired.commit();
                        let decision = match installed {
                            MembershipInstallDecision::Attach(target) => {
                                RefreshDecision::AttachRecovered(target)
                            }
                            MembershipInstallDecision::SelectedWithoutAttachment
                            | MembershipInstallDecision::Empty => {
                                RefreshDecision::SettledWithoutAttachment
                            }
                            MembershipInstallDecision::Preserved => RefreshDecision::Continue,
                        };
                        Ok(UiReply::Membership(RefreshSettlement::new(
                            decision,
                            session.desired().cloned(),
                        )))
                    }),
                "deposit membership-detached reply",
            )? == MutationResult::Applied,
            "membership-detached reply was stale",
        )?;
        let UiReply::Membership(settlement) = self.take_reply(&wait)? else {
            return violation("membership reply kind changed");
        };
        self.coverage.panel_membership_installs += 1;
        match settlement.decision() {
            RefreshDecision::AttachRecovered(target) => {
                let target = target.clone();
                self.drive_attach(&target)?;
            }
            RefreshDecision::SettledWithoutAttachment => {
                require(
                    self.session.attached_key().is_none(),
                    "membership install left an attachment after parking",
                )?;
            }
            RefreshDecision::Continue => {}
            other => {
                return violation(format!("unexpected membership decision: {other:?}"));
            }
        }
        Ok(())
    }

    /// Drives the four-event attach settlement for a recovered attachment, exactly as the
    /// worker publishes them while the panel lease is still active.
    fn drive_attach(&mut self, target: &AttachTarget) -> Result<(), ReplayFailure> {
        let key = target.selected().key().clone();
        let cutoff = self.base + Duration::from_secs(30);
        let frame = FrameIdentity::new(self.coverage.panel_attaches as u64 + 2);

        let wait = observed(
            self.mailbox.publish_event_before_using(
                WorkerEvent::Attached {
                    key: target.pane_ref(),
                    projection: TerminalProjection::new(
                        AttachmentId::parse(&id(self.authority.fresh()))
                            .expect("attachment fixture"),
                        HostEpoch::parse(&id(self.authority.fresh())).expect("epoch fixture"),
                    ),
                },
                EventDeadline::Forward(cutoff),
                &self.clock,
            ),
            "publish Attached",
        )?;
        let (taken_wait, event) = self.take_ui_event()?;
        require(
            taken_wait == wait
                && matches!(event, WorkerEvent::Attached { key: actual, .. } if actual == key),
            "Attached event changed",
        )?;
        let session = &mut self.session;
        let settled_key = key.clone();
        require(
            observed(
                self.mailbox
                    .deposit_ui_reply_before_using(&wait, &self.clock, |_transaction| {
                        session
                            .settle_attached_event_t(&settled_key)
                            .map(UiReply::AttachedSettled)
                            .ok_or(CarrierFailure::Protocol)
                    }),
                "deposit Attached reply",
            )? == MutationResult::Applied,
            "Attached reply was stale",
        )?;
        require(
            matches!(
                self.take_reply(&wait)?,
                UiReply::AttachedSettled(SetupEventDisposition::Applied)
            ),
            "Attached disposition changed",
        )?;

        let wait = observed(
            self.mailbox.publish_event_before_using(
                WorkerEvent::SetupConfigured {
                    key: target.pane_ref(),
                    mode: controlling(),
                    applied_geometry: Some(geometry()),
                },
                EventDeadline::Forward(cutoff),
                &self.clock,
            ),
            "publish SetupConfigured",
        )?;
        let (taken_wait, event) = self.take_ui_event()?;
        require(
            taken_wait == wait
                && matches!(event, WorkerEvent::SetupConfigured { key: actual, .. }
                    if actual == key),
            "SetupConfigured event changed",
        )?;
        let session = &mut self.session;
        let settled_key = key.clone();
        require(
            observed(
                self.mailbox
                    .deposit_ui_reply_before_using(&wait, &self.clock, |_transaction| {
                        session
                            .settle_setup_configuration_event_t(
                                &settled_key,
                                controlling(),
                                Some(geometry()),
                            )
                            .map(UiReply::SetupConfigurationSettled)
                            .ok_or(CarrierFailure::Protocol)
                    }),
                "deposit SetupConfigured reply",
            )? == MutationResult::Applied,
            "SetupConfigured reply was stale",
        )?;
        require(
            matches!(
                self.take_reply(&wait)?,
                UiReply::SetupConfigurationSettled(SetupEventDisposition::Applied)
            ),
            "SetupConfigured disposition changed",
        )?;

        require(
            self.session.stage_drawn_frame_t(&key, frame),
            "attach frame did not stage",
        )?;
        let wait = observed(
            self.mailbox.publish_event_before_using(
                WorkerEvent::FeedbackAccepted {
                    key: target.pane_ref(),
                    frame,
                },
                EventDeadline::Forward(cutoff),
                &self.clock,
            ),
            "publish FeedbackAccepted",
        )?;
        let (taken_wait, event) = self.take_ui_event()?;
        require(
            taken_wait == wait
                && matches!(event, WorkerEvent::FeedbackAccepted { key: actual, frame: actual_frame }
                    if actual == key && actual_frame == frame),
            "FeedbackAccepted event changed",
        )?;
        let session = &mut self.session;
        let settled_key = key.clone();
        require(
            observed(
                self.mailbox
                    .deposit_ui_reply_before_using(&wait, &self.clock, |_transaction| {
                        session
                            .settle_feedback_event_t(&settled_key, frame)
                            .map(UiReply::FeedbackSettled)
                            .ok_or(CarrierFailure::Protocol)
                    }),
                "deposit FeedbackAccepted reply",
            )? == MutationResult::Applied,
            "FeedbackAccepted reply was stale",
        )?;
        require(
            matches!(
                self.take_reply(&wait)?,
                UiReply::FeedbackSettled(SetupEventDisposition::Applied)
            ),
            "FeedbackAccepted disposition changed",
        )?;

        let wait = observed(
            self.mailbox.publish_event_before_using(
                WorkerEvent::ReadyCandidate {
                    key: target.pane_ref(),
                    mode: controlling(),
                    last_accepted_geometry: Some(geometry()),
                    frame,
                },
                EventDeadline::Forward(cutoff),
                &self.clock,
            ),
            "publish ReadyCandidate",
        )?;
        let (taken_wait, event) = self.take_ui_event()?;
        require(
            taken_wait == wait
                && matches!(event, WorkerEvent::ReadyCandidate { key: actual, .. }
                    if actual == key),
            "ReadyCandidate event changed",
        )?;
        let session = &mut self.session;
        let settled_key = key.clone();
        require(
            observed(
                self.mailbox
                    .deposit_ui_reply_before_using(&wait, &self.clock, |_transaction| {
                        session
                            .settle_ready_candidate_t(
                                &settled_key,
                                frame,
                                controlling(),
                                Some(geometry()),
                            )
                            .map(UiReply::Ready)
                    }),
                "deposit ReadyCandidate reply",
            )? == MutationResult::Applied,
            "ReadyCandidate reply was stale",
        )?;
        require(
            matches!(
                self.take_reply(&wait)?,
                UiReply::Ready(ReadyDecision::Ready)
            ),
            "ReadyCandidate decision changed",
        )?;
        require(
            self.session.attached_key() == Some(&key),
            "attach drive did not settle the new attachment",
        )?;
        self.coverage.panel_attaches += 1;
        Ok(())
    }

    /// A committed publication or snapshot-less commit leaves mailbox/session state that only a
    /// refresh reconciles: publication debt drains through a real refresh flight, and a missing
    /// snapshot is recovered by validating the authority's next candidate.
    fn drain_refresh(&mut self) -> Result<(), ReplayFailure> {
        let deadline = self.base + Duration::from_secs(30);
        let generation = observed(
            self.mailbox.try_enqueue_refresh_for(
                &mut self.session,
                deadline,
                |session, generation| {
                    session
                        .start_pending_refresh(generation)
                        .then_some(())
                        .ok_or("drain Refresh rejected")
                },
                |_session, _result| {},
            ),
            "admit drain Refresh",
        )?;
        let lease = observed(self.mailbox.try_recv(), "take drain Refresh")?
            .ok_or_else(|| violation_error("drain Refresh marker missing"))?;
        require(
            matches!(lease.record(), QueuedCommand::Refresh(marker)
                if marker.generation() == generation),
            "drain Refresh lost its generation",
        )?;
        require(
            observed(
                self.mailbox.mark_refresh_in_flight(generation),
                "mark drain Refresh",
            )? == MutationResult::Applied,
            "drain Refresh mark was stale",
        )?;
        self.allocate_read()?;
        let armed_recovery = self.session.panel_recovery_for_test().is_some();
        let outcome = WorkbenchSnapshotOutcome::Available {
            snapshot: self.authority.snapshot(),
        };
        let wait = observed(
            self.mailbox.publish_event_before_using(
                WorkerEvent::RefreshSnapshotResult {
                    generation,
                    outcome: outcome.clone(),
                },
                EventDeadline::PlacementOnly(deadline),
                &self.clock,
            ),
            "publish drain Refresh result",
        )?;
        let (taken_wait, event) = self.take_ui_event()?;
        require(
            taken_wait == wait
                && matches!(event, WorkerEvent::RefreshSnapshotResult { generation: actual, .. }
                    if actual == generation),
            "drain Refresh event changed",
        )?;

        // Mirrors `NativeUi::apply_refresh_event`'s membership arms.
        let prepared = self.session.prepare_refresh(generation, outcome);
        let recovery_deadlines =
            match &prepared {
                crate::terminal_session::state::PreparedRefresh::Membership {
                    prepared, ..
                } if !prepared.preserves_selection() => Some(
                    OperationDeadlines::from_sample(self.base)
                        .ok_or_else(|| violation_error("drain deadline overflow"))?,
                ),
                _ => None,
            };
        let session = &mut self.session;
        require(
            observed(
                self.mailbox
                    .deposit_ui_reply_before_using(&wait, &self.clock, |transaction| {
                        let decision =
                            if let crate::terminal_session::state::PreparedRefresh::Membership {
                                generation,
                                prepared,
                            } = prepared
                            {
                                if !prepared.preserves_selection() && recovery_deadlines.is_none() {
                                    return Err(CarrierFailure::Deadline);
                                }
                                if recovery_deadlines.is_some_and(|deadlines| {
                                    transaction.observed_at() >= deadlines.forward_cutoff()
                                }) {
                                    return Err(CarrierFailure::Deadline);
                                }
                                if session.has_attachments() && !prepared.preserves_selection() {
                                    let cleanup = session.stage_refresh_membership_cleanup(
                                        generation,
                                        prepared,
                                        recovery_deadlines.ok_or(CarrierFailure::Deadline)?,
                                    )?;
                                    RefreshDecision::Cleanup(cleanup)
                                } else {
                                    let install = if prepared.preserves_selection() {
                                        DesiredInstall::Preserve
                                    } else {
                                        prepared.selected_tab_id().map_or(
                                            DesiredInstall::Empty,
                                            |tab| DesiredInstall::Fresh {
                                                tab_id: tab.clone(),
                                                panel: prepared
                                                    .focus_panel_id()
                                                    .cloned()
                                                    .unwrap_or_else(panel),
                                            },
                                        )
                                    };
                                    let desired = transaction.prepare_desired_install(install)?;
                                    let plan = session.prepare_refresh_membership_install(
                                        generation,
                                        prepared,
                                        desired.desired().map(|pane| pane.anchor().clone()),
                                        recovery_deadlines,
                                    )?;
                                    let installed = plan.commit();
                                    desired.commit();
                                    match installed {
                                        MembershipInstallDecision::Preserved => {
                                            RefreshDecision::Continue
                                        }
                                        MembershipInstallDecision::Attach(target) => {
                                            RefreshDecision::AttachRecovered(target)
                                        }
                                        MembershipInstallDecision::SelectedWithoutAttachment
                                        | MembershipInstallDecision::Empty => {
                                            RefreshDecision::SettledWithoutAttachment
                                        }
                                    }
                                }
                            } else {
                                session.apply_prepared_refresh(prepared, std::time::Instant::now())
                            };
                        Ok(UiReply::Refresh(RefreshSettlement::new(
                            decision,
                            session.desired().cloned(),
                        )))
                    }),
                "deposit drain Refresh reply",
            )? == MutationResult::Applied,
            "drain Refresh reply was stale",
        )?;
        let UiReply::Refresh(settlement) = self.take_reply(&wait)? else {
            return violation("drain Refresh reply kind changed");
        };
        match settlement.decision() {
            RefreshDecision::Cleanup(cleanup) => {
                let cleanup = cleanup.clone();
                self.drive_membership_cleanup(&cleanup)?;
            }
            RefreshDecision::AttachRecovered(target) => {
                let target = target.clone();
                self.drive_attach(&target)?;
            }
            RefreshDecision::Continue | RefreshDecision::SettledWithoutAttachment => {}
            RefreshDecision::Retire => {
                return violation("drain Refresh retired mid-trace");
            }
        }
        observed(self.mailbox.finish_lease(lease), "finish drain Refresh")?;
        if armed_recovery {
            require(
                self.session.panel_recovery_for_test().is_none(),
                "drain Refresh left an armed panel recovery",
            )?;
            self.coverage.panel_recovery_clears += 1;
        }
        require(
            !self.session.publication_debt_pending(),
            "drain Refresh left publication debt pending",
        )?;
        Ok(())
    }

    /// Fixture-lane request identity: never consumed from the mailbox cursor, so forged
    /// operations stay disjoint from real admission identities.
    fn fixture_request(&mut self) -> WorkbenchRequestId {
        WorkbenchRequestId::parse(&id(self.authority.fresh())).expect("fixture request")
    }

    /// Models the workbench snapshot-read identity a worker burns when a result carries a
    /// snapshot (or when a drain refresh reads): allocates from the real mailbox cursor.
    fn allocate_read(&mut self) -> Result<(), ReplayFailure> {
        let allocated = observed(
            self.mailbox.allocate_workbench_request_id(),
            "allocate snapshot read identity",
        )?;
        require(
            allocated.to_string() == id(self.next_request),
            "snapshot read skipped or reused a request identity",
        )?;
        self.next_request += 1;
        Ok(())
    }

    /// True when removing `operation`'s whole tab reselects a tab whose recorded provenance
    /// cannot attach — the only reachable `SettledWithoutAttachment` shape mid-trace. The
    /// reselection rule mirrors `prepare_membership_candidate`: the candidate keeps the
    /// removed tab's ordinal, clamped to the last surviving index.
    fn tabgone_retargets_unattachable(&self, operation: &PanelOperation) -> bool {
        let count = self.authority.tabs.len();
        let Some(index) = self
            .authority
            .tabs
            .iter()
            .position(|tab| tab.tab_id == *operation.target().tab_id())
        else {
            return false;
        };
        if count < 2 {
            return false;
        }
        let reselected = if index < count - 1 {
            index + 1
        } else {
            index - 1
        };
        self.session
            .provenance(reselected)
            .is_some_and(|provenance| !provenance.attachable())
    }

    /// Builds the worker-visible `PanelResult` for the admitted operation and mutates the
    /// authority model so the snapshot the result carries is the real committed state. The
    /// third tuple element is the settlement decision this exact input must produce —
    /// computed from the request class and pre-settlement session/authority facts before
    /// the authority mutates, so the caller can check the SUT's decision against an
    /// independent expectation rather than echoing it.
    fn build_panel_outcome(
        &mut self,
        operation: &PanelOperation,
        kind: PanelRequestKind,
        outcome_sel: u8,
    ) -> Result<(PanelResult, OutcomeClass, usize), ReplayFailure> {
        let table = outcome_table(kind);
        // A tab-gone refusal needs enough surviving tabs for the rest of the trace, and a
        // selected-leaf close needs a sibling to keep the tab alive.
        let target_index = self
            .authority
            .tabs
            .iter()
            .position(|tab| tab.tab_id == *operation.target().tab_id());
        let target_attachable = target_index
            .and_then(|index| self.session.provenance(index))
            .is_none_or(|provenance| provenance.attachable());
        let legal = |class: OutcomeClass| match class {
            // `Unchanged` is adapter-legal on Move but unreachable on this finite
            // Native-planned fixture: `plan_panel` always picks the same-direction
            // neighbor, while a server `Unchanged` requires the source already
            // sitting at the target's side of that direction (R6
            // state-final-delta25). Keep it in the table for the white-box
            // boundary, but never generate it as a normal outcome.
            OutcomeClass::Unchanged => false,
            OutcomeClass::CommittedTabGone => self.authority.tabs.len() >= 3,
            OutcomeClass::CommittedDrop | OutcomeClass::CloseAccepted => {
                target_index.is_some_and(|index| self.authority.leaf_count(index) >= 2)
            }
            _ => true,
        };
        let index = (usize::from(outcome_sel) + self.coverage.panel_cycles - 1) % table.len();
        let mut class = table[index];
        if !legal(class) {
            class = match kind {
                PanelRequestKind::Close => OutcomeClass::CloseAccepted,
                _ => OutcomeClass::CommittedKeep,
            };
        }
        // Deterministic coverage: membership classes drive the attached session to a staged
        // Cleanup decision and the unattached session to a direct install. CommittedTabGone
        // reselects another tab (which may be non-attachable → SettledWithoutAttachment);
        // CommittedDrop repairs the selected leaf in place. While a decision is still
        // uncovered this trace, prefer the class that produces it; otherwise prefer the
        // recovery-arming class once, then fall back to the first supported class that
        // never fired.
        let supported = |candidate: OutcomeClass| table.contains(&candidate) && legal(candidate);
        let mut steered = None;
        if self.session.has_attachments() {
            if self.coverage.panel_decisions[1] == 0 {
                steered = [OutcomeClass::CommittedDrop, OutcomeClass::CommittedTabGone]
                    .into_iter()
                    .find(|candidate| supported(*candidate));
            }
        } else {
            // The unattached membership install either reattaches (AttachRecovered) or
            // settles without an attachment (SettledWithoutAttachment), decided by the
            // selected candidate tab's provenance. A trace reaches at most one unattached
            // cycle where a membership class is legal, so the generator splits the two
            // decisions across seeds: the post-refresh request is forced to Move on even
            // seeds and Split on odd seeds, and the preference follows the intent rather
            // than this trace's own coverage (which is always zero on arrival).
            let drop_legal = supported(OutcomeClass::CommittedDrop);
            let gone_legal = supported(OutcomeClass::CommittedTabGone);
            let gone_unattachable = gone_legal && self.tabgone_retargets_unattachable(operation);
            let settle_choice = if gone_unattachable {
                Some(OutcomeClass::CommittedTabGone)
            } else if !target_attachable && drop_legal {
                Some(OutcomeClass::CommittedDrop)
            } else {
                None
            };
            let recover_choice = if target_attachable && drop_legal {
                Some(OutcomeClass::CommittedDrop)
            } else if gone_legal && !gone_unattachable {
                Some(OutcomeClass::CommittedTabGone)
            } else {
                None
            };
            steered = if matches!(kind, PanelRequestKind::Move { .. }) {
                recover_choice.or(settle_choice)
            } else {
                settle_choice.or(recover_choice)
            };
            // A split commit on a single-leaf non-attachable selected tab is the only way
            // to grow it toward what later membership coverage needs: leaf repair needs a
            // surviving sibling, and the forced Move request at the post-refresh position
            // only admits a multi-leaf active tab.
            if steered.is_none()
                && (self.coverage.panel_decisions[2] == 0 || self.coverage.panel_decisions[3] == 0)
                && !target_attachable
                && target_index.is_some_and(|index| self.authority.leaf_count(index) < 2)
                && supported(OutcomeClass::CommittedKeep)
            {
                steered = Some(OutcomeClass::CommittedKeep);
            }
        }
        if let Some(candidate) = steered {
            class = candidate;
        } else if self.coverage.panel_outcomes[OutcomeClass::CommittedNoSnapshot.index()] == 0
            && supported(OutcomeClass::CommittedNoSnapshot)
        {
            class = OutcomeClass::CommittedNoSnapshot;
        } else if let Some(candidate) = (0..table.len())
            .map(|offset| table[(index + offset) % table.len()])
            .find(|candidate| {
                self.coverage.panel_outcomes[candidate.index()] == 0 && supported(*candidate)
            })
        {
            class = candidate;
        }
        require(
            adapter_legal_outcome(kind, class),
            "generated outcome class is unreachable for this intent at the adapter",
        )?;
        // The decision the settlement must produce for this input, derived from the
        // request facts and the pre-settlement session/authority state: non-committed
        // refusals and no-ops Continue; membership classes stage cleanup while attached
        // or install directly while unattached, attaching only when the reselected tab's
        // provenance allows it.
        let expected_decision = match class {
            OutcomeClass::CommittedDrop | OutcomeClass::CommittedTabGone => {
                if self.session.has_attachments() {
                    1
                } else {
                    let attachable = if class == OutcomeClass::CommittedDrop {
                        target_attachable
                    } else {
                        !self.tabgone_retargets_unattachable(operation)
                    };
                    if attachable { 2 } else { 3 }
                }
            }
            _ => 0,
        };
        let baseline_revision = operation.baseline().revision();
        // Fresh-only precondition: every normal outcome is built while the authority
        // still sits at the revision the operation was planned against. A peer advance
        // between admission and the reply would make a committed result or a later
        // `Unavailable` dishonest; the `Rejected` arm below is the single exception —
        // it constructs that peer advance itself before answering.
        require(
            self.authority.revision == baseline_revision.value(),
            "normal Panel outcome built on a stale baseline",
        )?;
        let result = match class {
            OutcomeClass::NotSubmitted => PanelResult::NotSubmitted,
            OutcomeClass::Rejected => {
                // A legal peer commit lands between our admission and the reply: the
                // authority revision advances past the baseline, so the honest
                // first-check rejection is `RevisionMismatch` carrying the server's
                // current revision (revision checks precede Tab/Panel checks). A
                // `RequestIdentityConflict` at equal revision would claim a ledger
                // history — a different command already occupying this request ID —
                // that the generator never constructed. The peer commit is a tab
                // rename: it advances the revision without consuming fixture IDs or
                // disturbing the finite fixture's tab/leaf geometry, and the
                // following `drain_refresh` resynchronizes the session to it.
                self.authority.rename_first_tab();
                let current = self.authority.revision;
                require(
                    current > baseline_revision.value(),
                    "rejected reply did not carry the advanced authority revision",
                )?;
                PanelResult::Precommit(PanelPrecommit::Rejected {
                    revision: WorkbenchRevision::new(current),
                    reason: WorkbenchRejectionReason::RevisionMismatch,
                })
            }
            OutcomeClass::Denied => PanelResult::Precommit(PanelPrecommit::Denied),
            OutcomeClass::Unavailable => PanelResult::Precommit(PanelPrecommit::Unavailable {
                // Per-intent honest transient: Split has no runtime dependency of its own
                // (`TerminalRuntimeUnavailable` only ever arrives embedded in a committed
                // `PanelSplitCreated`), Move never touches a runtime at all, and only Close
                // resolves a retained runtime target. `ServiceUnavailable` is honest for
                // every intent but the split gets its own reachable coordinator answer.
                reason: match kind {
                    PanelRequestKind::Split { .. } => {
                        WorkbenchCommandUnavailableReason::NoCompatibleWorker
                    }
                    PanelRequestKind::Move { .. } => {
                        WorkbenchCommandUnavailableReason::TerminalBusy
                    }
                    PanelRequestKind::Close => {
                        WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable
                    }
                },
            }),
            OutcomeClass::Unchanged => PanelResult::Unchanged(baseline_revision),
            OutcomeClass::CommittedKeep
            | OutcomeClass::CommittedDrop
            | OutcomeClass::CommittedNoSnapshot => {
                let commit = self.authority.apply_commit(operation)?;
                let outcome = if class == OutcomeClass::CommittedNoSnapshot {
                    WorkbenchSnapshotOutcome::Unavailable {
                        reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                    }
                } else {
                    if class == OutcomeClass::CommittedDrop {
                        require(
                            self.authority
                                .close_leaf(operation.target().tab_id(), operation.panel_id()),
                            "committed-drop close missed its leaf",
                        )?;
                    }
                    WorkbenchSnapshotOutcome::Available {
                        snapshot: self.authority.snapshot(),
                    }
                };
                PanelResult::Committed {
                    commit: Box::new(commit),
                    outcome,
                }
            }
            OutcomeClass::CloseAccepted => {
                require(
                    self.authority
                        .close_leaf(operation.target().tab_id(), operation.panel_id()),
                    "accepted close missed its leaf",
                )?;
                PanelResult::CloseAccepted(WorkbenchPanelCloseAccepted {
                    close_request_id: operation.request_id().clone(),
                    terminal_context_id: operation.baseline().terminal_context_id().clone(),
                    tab_id: operation.target().tab_id().clone(),
                    panel_id: operation.panel_id().clone(),
                    accepted_revision: baseline_revision,
                })
            }
            OutcomeClass::CommittedTabGone => {
                // Our commit lands at baseline+1; a peer whole-tab close then folds into
                // the superseding snapshot the adapter reads — revision > committed, so
                // projection validation skips the exact graft check and the missing target
                // routes to the membership retarget path with the commit still retained.
                let commit = self.authority.apply_commit(operation)?;
                self.authority.remove_tab(operation.target().tab_id());
                PanelResult::Committed {
                    commit: Box::new(commit),
                    outcome: WorkbenchSnapshotOutcome::Available {
                        snapshot: self.authority.snapshot(),
                    },
                }
            }
        };
        Ok((result, class, expected_decision))
    }

    /// Terminal action: runs every protocol-negative probe on fresh sandbox pairs (each probe
    /// must close the mailbox with the expected failure while the session keeps the pending
    /// operation), then fires one destructive probe on this trace's own pair and retires.
    fn panel_negative_sweep(&mut self, terminal_probe: u8) -> Result<(), ReplayFailure> {
        self.run_sandbox_probes()?;
        self.terminal_panel_probe(terminal_probe % 4)?;
        self.coverage.completed_traces += 1;
        self.phase = TracePhase::Retired;
        Ok(())
    }

    fn run_sandbox_probes(&mut self) -> Result<(), ReplayFailure> {
        // Publishing a result for a flight that was never marked in-flight is a protocol fault.
        {
            let mut sandbox = PanelSandbox::new()?;
            let (operation, _lease) = sandbox.admit_split()?;
            let commit = sandbox.authority.apply_commit(&operation)?;
            let outcome = PanelResult::Committed {
                commit: Box::new(commit),
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: sandbox.authority.snapshot(),
                },
            };
            let error = self.expect_protocol_close(
                sandbox.mailbox.publish_event_before_using(
                    WorkerEvent::PanelResult {
                        operation: operation.clone(),
                        outcome,
                    },
                    EventDeadline::PlacementOnly(operation.deadline()),
                    &sandbox.clock,
                ),
                "publish Panel result before in-flight mark",
            )?;
            require(error == CarrierFailure::Protocol, "wrong close failure")?;
            require(
                sandbox.session.pending_panel() == Some(&operation),
                "queued-stage publish probe disturbed the pending operation",
            )?;
            self.coverage.panel_negative_probes += 1;
        }
        // A result carrying a forged operation identity must close the mailbox.
        {
            let mut sandbox = PanelSandbox::new()?;
            let (operation, _lease) = sandbox.admit_split_inflight()?;
            let forged = PanelOperation::new(
                sandbox.fixture_request(),
                operation.target().clone(),
                operation.panel_id().clone(),
                operation.baseline().clone(),
                PanelRequestKind::Split {
                    direction: PanelDirection::Right,
                },
                operation.intent().clone(),
                operation.deadline(),
            );
            let error = self.expect_protocol_close(
                sandbox.mailbox.publish_event_before_using(
                    WorkerEvent::PanelResult {
                        operation: forged,
                        outcome: PanelResult::NotSubmitted,
                    },
                    EventDeadline::PlacementOnly(operation.deadline()),
                    &sandbox.clock,
                ),
                "publish forged Panel result",
            )?;
            require(
                error == CarrierFailure::Protocol,
                "forged publish wrong failure",
            )?;
            require(
                sandbox.session.pending_panel() == Some(&operation),
                "forged publish disturbed the pending operation",
            )?;
            self.coverage.panel_negative_probes += 1;
        }
        // A result published under the wrong deadline shape must close the mailbox.
        {
            let mut sandbox = PanelSandbox::new()?;
            let (operation, _lease) = sandbox.admit_split_inflight()?;
            let error = self.expect_protocol_close(
                sandbox.mailbox.publish_event_before_using(
                    WorkerEvent::PanelResult {
                        operation: operation.clone(),
                        outcome: PanelResult::NotSubmitted,
                    },
                    EventDeadline::Forward(operation.deadline()),
                    &sandbox.clock,
                ),
                "publish Panel result under forward deadline",
            )?;
            require(
                error == CarrierFailure::Protocol,
                "deadline probe wrong failure",
            )?;
            self.coverage.panel_negative_probes += 1;
        }
        // A second publication for a settled flight is a protocol fault: the pending
        // operation was already consumed by the UI settlement, so re-placing a result
        // must close the mailbox instead of mutating anything.
        {
            let mut sandbox = PanelSandbox::new()?;
            let (operation, _lease, wait) = sandbox.admit_split_awaiting()?;
            sandbox.settle_published(&operation, &wait)?;
            let error = self.expect_protocol_close(
                sandbox.mailbox.publish_event_before_using(
                    WorkerEvent::PanelResult {
                        operation: operation.clone(),
                        outcome: PanelResult::NotSubmitted,
                    },
                    EventDeadline::PlacementOnly(operation.deadline()),
                    &sandbox.clock,
                ),
                "post-settlement Panel re-publication",
            )?;
            require(
                error == CarrierFailure::Protocol,
                "double publish wrong failure",
            )?;
            self.coverage.panel_negative_probes += 1;
        }
        // An occupied event cell forces a second publication to wait for the consumer to
        // drain it in real order — deposit the reply and take the receipt. Once the cell
        // empties the publication is validated against state at placement: the flight was
        // already consumed by the UI settlement, so it must close with Protocol rather
        // than mutating anything. The scoped publisher runs under the rescue guard so an
        // early failure or unwind closes and wakes it before the scope joins.
        {
            let mut sandbox = PanelSandbox::new()?;
            let (operation, _lease, wait) = sandbox.admit_split_awaiting()?;
            let published = sandbox
                .published
                .clone()
                .ok_or_else(|| violation_error("sandbox outcome missing"))?;
            let mailbox = &sandbox.mailbox;
            let session = &mut sandbox.session;
            let clock = &sandbox.clock;
            let publisher_operation = operation.clone();
            let deadline = EventDeadline::PlacementOnly(publisher_operation.deadline());
            let (results, completed) = std::sync::mpsc::channel();
            let joined = occupied_cell_publication(
                mailbox,
                WorkerEvent::PanelResult {
                    operation: publisher_operation,
                    outcome: PanelResult::NotSubmitted,
                },
                deadline,
                clock,
                Duration::from_secs(2),
                Duration::from_secs(5),
                &results,
                &completed,
                || {
                    require(
                        observed(
                            mailbox.deposit_ui_reply_before_using(&wait, clock, |_transaction| {
                                let prepared = session.prepare_panel(&operation, published);
                                Ok(UiReply::Panel(PanelSettlement::new(
                                    session.apply_prepared_panel(prepared),
                                    session.desired().cloned(),
                                )))
                            }),
                            "drain occupied cell reply",
                        )? == MutationResult::Applied,
                        "occupied cell reply was stale",
                    )?;
                    observed(
                        mailbox.take_deposited_reply(&wait),
                        "drain occupied cell reply receipt",
                    )?
                    .ok_or_else(|| violation_error("occupied cell reply receipt missing"))?;
                    Ok(())
                },
            )?;
            let error =
                self.expect_protocol_close(joined, "occupied-cell publication after drain")?;
            require(
                error == CarrierFailure::Protocol,
                "occupied-cell drain publication wrong failure",
            )?;
            self.coverage.panel_negative_probes += 1;
        }
        // A publication parked on an occupied event cell must honor its own placement
        // deadline: an advancing test clock pushes the re-sampled instant past the
        // deadline so the drain wait refuses with Deadline instead of waiting without
        // bound (the hang the frozen `FixedClock` produced). The scoped publisher runs
        // under the rescue guard so an early failure or unwind closes and wakes it
        // before the scope joins.
        {
            let mut sandbox = PanelSandbox::new()?;
            let (operation, _lease, _wait) = sandbox.admit_split_awaiting()?;
            let elapsed = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
            let adjustable =
                crate::local_client::test_server::adjustable_operation_clock(elapsed.clone());
            let mailbox = &sandbox.mailbox;
            let publisher_operation = operation.clone();
            let deadline = EventDeadline::PlacementOnly(publisher_operation.deadline());
            let (results, completed) = std::sync::mpsc::channel();
            let joined = occupied_cell_publication(
                mailbox,
                WorkerEvent::PanelResult {
                    operation: publisher_operation,
                    outcome: PanelResult::NotSubmitted,
                },
                deadline,
                &adjustable,
                Duration::from_secs(2),
                Duration::from_secs(5),
                &results,
                &completed,
                || {
                    elapsed.store(120_000, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                },
            )?;
            let error = self.expect_protocol_close(joined, "occupied-cell publication deadline")?;
            require(
                error == CarrierFailure::Deadline,
                "occupied-cell expiry wrong failure",
            )?;
            self.coverage.panel_negative_probes += 1;
        }
        // A reply deposited before its event was taken is a protocol fault.
        {
            let mut sandbox = PanelSandbox::new()?;
            let (_operation, _lease, wait) = sandbox.admit_split_awaiting_published()?;
            let error = self.expect_protocol_close(
                sandbox.mailbox.deposit_ui_reply_before_using(
                    &wait,
                    &sandbox.clock,
                    |_transaction| Ok(UiReply::PrecommitRestored),
                ),
                "reply deposited before event consumption",
            )?;
            require(
                error == CarrierFailure::Protocol,
                "early reply wrong failure",
            )?;
            self.coverage.panel_negative_probes += 1;
        }
        // A reply deposited with a valid wait key after the mailbox closed must surface
        // the recorded close as the first cause: `ensure_open` precedes admission and
        // settlement, so the closure never runs and no new state lands — a mismatched
        // wait-key fault cannot be re-derived in place of the close.
        {
            let mut sandbox = PanelSandbox::new()?;
            let (_operation, _lease, wait) = sandbox.admit_split_awaiting()?;
            sandbox.mailbox.close(CarrierFailure::Closed);
            let ran = Cell::new(false);
            let error = observed_err(
                sandbox.mailbox.deposit_ui_reply_before_using(
                    &wait,
                    &sandbox.clock,
                    |_transaction| {
                        ran.set(true);
                        Ok(UiReply::Panel(PanelSettlement::new(
                            PanelDecision::Continue,
                            None,
                        )))
                    },
                ),
                "deposit Panel reply on a closed mailbox",
            )?;
            require(
                error.closed_failure() == Some(CarrierFailure::Closed),
                "closed-mailbox deposit lost the first cause",
            )?;
            require(
                !ran.get(),
                "closed-mailbox deposit ran the settlement closure",
            )?;
            require(
                sandbox.mailbox.is_closed() == Some(CarrierFailure::Closed),
                "closed-mailbox deposit changed the recorded close",
            )?;
            self.coverage.panel_negative_probes += 1;
        }
        // A reply under the same serial but a foreign correlation must close the mailbox.
        {
            let mut sandbox = PanelSandbox::new()?;
            let (_operation, _lease, wait) = sandbox.admit_split_awaiting()?;
            let forged = UiWaitKey {
                serial: wait.serial(),
                correlation: UiCorrelation::Panel(sandbox.fixture_request()),
                reply_kind: wait.reply_kind,
            };
            let error = self.expect_protocol_close(
                sandbox.mailbox.deposit_ui_reply_before_using(
                    &forged,
                    &sandbox.clock,
                    |_transaction| Ok(UiReply::PrecommitRestored),
                ),
                "same-serial foreign-correlation reply",
            )?;
            require(
                error == CarrierFailure::Protocol,
                "forged reply wrong failure",
            )?;
            self.coverage.panel_negative_probes += 1;
        }
        // A reply under a future serial must close the mailbox.
        {
            let mut sandbox = PanelSandbox::new()?;
            let (_operation, _lease, wait) = sandbox.admit_split_awaiting()?;
            let forged = UiWaitKey {
                serial: wait.serial() + 1,
                correlation: wait.correlation.clone(),
                reply_kind: wait.reply_kind,
            };
            let error = self.expect_protocol_close(
                sandbox.mailbox.deposit_ui_reply_before_using(
                    &forged,
                    &sandbox.clock,
                    |_transaction| Ok(UiReply::PrecommitRestored),
                ),
                "future-serial Panel reply",
            )?;
            require(
                error == CarrierFailure::Protocol,
                "future serial wrong failure",
            )?;
            self.coverage.panel_negative_probes += 1;
        }
        // A reply under an older serial is merely stale: it is ignored and the mailbox stays open.
        {
            let mut sandbox = PanelSandbox::new()?;
            let (_operation, _lease, wait) = sandbox.admit_split_awaiting()?;
            let stale = UiWaitKey {
                serial: wait.serial().saturating_sub(1),
                correlation: wait.correlation.clone(),
                reply_kind: wait.reply_kind,
            };
            let ran = Cell::new(false);
            require(
                observed(
                    sandbox.mailbox.deposit_ui_reply_before_using(
                        &stale,
                        &sandbox.clock,
                        |_transaction| {
                            ran.set(true);
                            Ok(UiReply::PrecommitRestored)
                        },
                    ),
                    "stale-serial Panel reply",
                )? == MutationResult::Stale,
                "stale-serial reply was applied",
            )?;
            require(!ran.get(), "stale reply ran the settlement closure")?;
            require(
                sandbox.mailbox.is_closed().is_none(),
                "stale-serial reply closed the mailbox",
            )?;
            self.coverage.panel_negative_probes += 1;
        }
        // An ambiguous (OutcomeUnknown) abandon freezes the queue without publishing a result.
        {
            let mut sandbox = PanelSandbox::new()?;
            let (operation, lease) = sandbox.admit_split_inflight()?;
            sandbox.session.retire(CarrierFailure::OutcomeUnknown);
            require(
                sandbox
                    .mailbox
                    .close(CarrierFailure::OutcomeUnknown)
                    .failure()
                    == CarrierFailure::OutcomeUnknown,
                "ambiguous abandon recorded the wrong close cause",
            )?;
            require(
                sandbox.session.pending_panel() == Some(&operation),
                "ambiguous abandon dropped the pending operation",
            )?;
            require(
                matches!(
                    sandbox.mailbox.mark_panel_in_flight(&operation),
                    Err(closed) if closed.failure() == CarrierFailure::OutcomeUnknown
                ),
                "abandoned mailbox admitted a later mark",
            )?;
            drop(lease);
            self.coverage.panel_negative_probes += 1;
            self.coverage.ambiguous_abandons += 1;
        }
        // A divergent committed result retires the session while retaining the exact commit.
        {
            let mut sandbox = PanelSandbox::new()?;
            let (operation, _lease, wait) = sandbox.admit_split_awaiting_divergent()?;
            let settlement = sandbox.settle_published(&operation, &wait)?;
            require(
                settlement == PanelDecision::Retire,
                "divergent commit did not retire",
            )?;
            require(
                sandbox.session.retired_failure() == Some(CarrierFailure::AuthorityDivergence),
                "divergent commit recorded the wrong retirement",
            )?;
            require(
                sandbox
                    .session
                    .retained_panel_for_test()
                    .is_some_and(|pending| pending.operation() == &operation),
                "divergent commit dropped the retained commit",
            )?;
            self.coverage.panel_negative_probes += 1;
            self.coverage.panel_decisions[4] += 1;
            self.coverage.divergent_retires += 1;
            self.coverage.panel_retained_commits += 1;
        }
        // An Unchanged report at a foreign revision is divergent, not a no-op.
        {
            let mut sandbox = PanelSandbox::new()?;
            let (operation, _lease) = sandbox.admit_split_inflight()?;
            let outcome = PanelResult::Unchanged(WorkbenchRevision::new(
                operation.baseline().revision().value() + 1,
            ));
            let wait = observed(
                sandbox.mailbox.publish_event_before_using(
                    WorkerEvent::PanelResult {
                        operation: operation.clone(),
                        outcome: outcome.clone(),
                    },
                    EventDeadline::PlacementOnly(operation.deadline()),
                    &sandbox.clock,
                ),
                "publish foreign-revision Unchanged",
            )?;
            sandbox.published = Some(outcome);
            let taken = observed(
                sandbox.mailbox.take_event_before_using(&sandbox.clock),
                "take foreign-Unchanged event",
            )?
            .ok_or_else(|| violation_error("foreign-Unchanged event missing"))?;
            let _ = taken.into_parts();
            let settlement = sandbox.settle_published(&operation, &wait)?;
            require(
                settlement == PanelDecision::Retire,
                "foreign-revision Unchanged did not retire",
            )?;
            require(
                sandbox.session.retired_failure() == Some(CarrierFailure::AuthorityDivergence),
                "foreign Unchanged recorded the wrong retirement",
            )?;
            require(
                sandbox.session.retained_panel_for_test().is_none(),
                "uncommitted divergence retained a commit",
            )?;
            self.coverage.panel_negative_probes += 1;
            self.coverage.panel_decisions[4] += 1;
            self.coverage.divergent_retires += 1;
        }
        Ok(())
    }

    /// Fires one destructive probe against this trace's own pair: the mailbox closes with
    /// Protocol and the session retires on the same cause.
    fn terminal_panel_probe(&mut self, select: u8) -> Result<(), ReplayFailure> {
        let deadline = self.base + Duration::from_secs(30);
        let operation = match self.mailbox.try_enqueue_panel_for_using(
            &mut self.session,
            deadline,
            &self.clock,
            |session, request_id| {
                session
                    .plan_panel(
                        request_id,
                        PanelRequestKind::Split {
                            direction: PanelDirection::Right,
                        },
                        deadline,
                    )
                    .ok_or("terminal split inadmissible")
            },
            |session, planned| {
                session
                    .begin_panel(planned.clone())
                    .then_some(())
                    .ok_or("terminal begin refused")
            },
        ) {
            Ok(operation) => operation,
            Err(MoveAdmissionError::Busy) | Err(MoveAdmissionError::Build(_)) => {
                // No desired attachment: a reply under a foreign wait key still must close the
                // mailbox, keeping the probe destructive without a pending operation.
                let serial = self.mailbox.lock().next_ui_wait_serial + 1;
                let correlation = UiCorrelation::Panel(self.fixture_request());
                let forged = UiWaitKey {
                    serial,
                    correlation,
                    reply_kind: UiReplyKind::Panel,
                };
                let error = self.expect_protocol_close(
                    self.mailbox.deposit_ui_reply_before_using(
                        &forged,
                        &self.clock,
                        |_transaction| Ok(UiReply::PrecommitRestored),
                    ),
                    "terminal unattached reply probe",
                )?;
                require(
                    error == CarrierFailure::Protocol,
                    "unattached reply probe closed with the wrong cause",
                )?;
                self.session.retire(CarrierFailure::Protocol);
                self.retire_cause = Some(CarrierFailure::Protocol);
                self.coverage.panel_terminal_probes += 1;
                return Ok(());
            }
            Err(error) => return violation(format!("terminal Panel admission failed: {error:?}")),
        };
        let lease = observed(self.mailbox.try_recv(), "take terminal Panel")?
            .ok_or_else(|| violation_error("terminal Panel marker missing"))?;
        require(
            observed(
                self.mailbox.mark_panel_in_flight(&operation),
                "mark terminal Panel",
            )? == MutationResult::Applied,
            "terminal Panel mark was stale",
        )?;
        match select {
            // Publish under a deadline shape other than the placement-only operation deadline.
            0 => {
                let error = self.expect_protocol_close(
                    self.mailbox.publish_event_before_using(
                        WorkerEvent::PanelResult {
                            operation: operation.clone(),
                            outcome: PanelResult::NotSubmitted,
                        },
                        EventDeadline::TotalCleanup(deadline),
                        &self.clock,
                    ),
                    "terminal wrong-deadline publish",
                )?;
                require(
                    error == CarrierFailure::Protocol,
                    "wrong-deadline publish closed with the wrong cause",
                )?;
            }
            // Publish a result for a foreign operation identity.
            1 => {
                let forged = PanelOperation::new(
                    self.fixture_request(),
                    operation.target().clone(),
                    operation.panel_id().clone(),
                    operation.baseline().clone(),
                    PanelRequestKind::Split {
                        direction: PanelDirection::Right,
                    },
                    operation.intent().clone(),
                    operation.deadline(),
                );
                let error = self.expect_protocol_close(
                    self.mailbox.publish_event_before_using(
                        WorkerEvent::PanelResult {
                            operation: forged,
                            outcome: PanelResult::NotSubmitted,
                        },
                        EventDeadline::PlacementOnly(deadline),
                        &self.clock,
                    ),
                    "terminal forged-operation publish",
                )?;
                require(
                    error == CarrierFailure::Protocol,
                    "forged-operation publish closed with the wrong cause",
                )?;
            }
            // Deposit a reply under the current serial but a foreign correlation.
            2 => {
                let wait = observed(
                    self.mailbox.publish_event_before_using(
                        WorkerEvent::PanelResult {
                            operation: operation.clone(),
                            outcome: PanelResult::NotSubmitted,
                        },
                        EventDeadline::PlacementOnly(deadline),
                        &self.clock,
                    ),
                    "publish terminal Panel result",
                )?;
                let _ = self.take_ui_event()?;
                let forged = UiWaitKey {
                    serial: wait.serial(),
                    correlation: UiCorrelation::Panel(self.fixture_request()),
                    reply_kind: wait.reply_kind,
                };
                let error = self.expect_protocol_close(
                    self.mailbox.deposit_ui_reply_before_using(
                        &forged,
                        &self.clock,
                        |_transaction| Ok(UiReply::PrecommitRestored),
                    ),
                    "terminal foreign-correlation reply",
                )?;
                require(
                    error == CarrierFailure::Protocol,
                    "foreign-correlation reply closed with the wrong cause",
                )?;
            }
            // Deposit a reply under a future serial.
            _ => {
                let wait = observed(
                    self.mailbox.publish_event_before_using(
                        WorkerEvent::PanelResult {
                            operation: operation.clone(),
                            outcome: PanelResult::NotSubmitted,
                        },
                        EventDeadline::PlacementOnly(deadline),
                        &self.clock,
                    ),
                    "publish terminal Panel result",
                )?;
                let _ = self.take_ui_event()?;
                let forged = UiWaitKey {
                    serial: wait.serial() + 1,
                    correlation: wait.correlation.clone(),
                    reply_kind: wait.reply_kind,
                };
                let error = self.expect_protocol_close(
                    self.mailbox.deposit_ui_reply_before_using(
                        &forged,
                        &self.clock,
                        |_transaction| Ok(UiReply::PrecommitRestored),
                    ),
                    "terminal future-serial reply",
                )?;
                require(
                    error == CarrierFailure::Protocol,
                    "future-serial reply closed with the wrong cause",
                )?;
            }
        }
        require(
            self.mailbox.is_closed() == Some(CarrierFailure::Protocol),
            "terminal probe mailbox did not close with Protocol",
        )?;
        require(
            self.session.pending_panel() == Some(&operation),
            "terminal probe dropped the pending operation",
        )?;
        self.session.retire(CarrierFailure::Protocol);
        drop(lease);
        self.retire_cause = Some(CarrierFailure::Protocol);
        self.coverage.panel_terminal_probes += 1;
        Ok(())
    }

    /// Asserts a publish/deposit failed by closing the mailbox, returning the close failure.
    fn expect_protocol_close<T>(
        &self,
        result: Result<T, EventError>,
        context: &str,
    ) -> Result<CarrierFailure, ReplayFailure> {
        let error = observed_err(result, context)?;
        error
            .closed_failure()
            .ok_or_else(|| violation_error(format!("{context}: expected a close")))
    }

    fn take_ui_event(&self) -> Result<(UiWaitKey, WorkerEvent), ReplayFailure> {
        let taken = observed(
            self.mailbox.take_event_before_using(&self.clock),
            "take UI event",
        )?
        .ok_or_else(|| violation_error("UI event missing"))?;
        let (wait, event) = taken.into_parts();
        let wait = wait.ok_or_else(|| violation_error("UI event had no wait key"))?;
        Ok((wait, event))
    }

    fn take_reply(&self, wait: &UiWaitKey) -> Result<UiReply, ReplayFailure> {
        observed(
            self.mailbox.take_deposited_reply(wait),
            "take deposited UI reply",
        )?
        .ok_or_else(|| violation_error("deposited UI reply missing"))
    }
}

fn create_marker(
    session: &NativeSessionState,
    sampled_at: Instant,
    generation: u64,
    request_id: WorkbenchRequestId,
    label: &str,
) -> Result<CreateMarker, &'static str> {
    let label = WorkbenchLabel::parse(label).map_err(|_| "invalid Create label")?;
    let workbench_geometry =
        WorkbenchTerminalGeometry::new(80, 24, 8, 16).map_err(|_| "invalid Workbench geometry")?;
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: request_id.clone(),
        site_id: session.cache().site_id().clone(),
        expected_revision: session.cache().revision(),
        action: WorkbenchAction::TabCreate {
            terminal_context_id: session.cache().terminal_context_id().clone(),
            label: label.clone(),
            geometry: workbench_geometry,
            launch: None,
            profile: None,
            options: Default::default(),
        },
    };
    let request = WorkbenchTabCreateRequest::try_from(command)
        .map_err(|_| "Create command did not convert")?;
    let deadlines =
        OperationDeadlines::from_sample(sampled_at).ok_or("Create deadline overflow")?;
    let operation = CreateOperation::new(
        CreateOperationKey::new(request_id, generation),
        session.cache().site_id().clone(),
        session.cache().revision(),
        session.cache().terminal_context_id().clone(),
        label,
        geometry(),
        deadlines,
    );
    Ok(CreateMarker::new(request, operation))
}

fn pending_commit(snapshot: &Snapshot) -> Result<WorkbenchTabCreateCommit, ReplayFailure> {
    let context = snapshot
        .terminal_contexts
        .first()
        .ok_or_else(|| violation_error("fixture snapshot has no context"))?;
    let created = context
        .tabs
        .last()
        .ok_or_else(|| violation_error("fixture snapshot has no created tab"))?;
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &created.root
    else {
        return violation("fixture created tab is not a single Panel");
    };
    Ok(WorkbenchTabCreateCommit::Pending {
        revision: snapshot.revision,
        terminal_context_id: context.terminal_context_id.clone(),
        tab_id: created.tab_id.clone(),
        panel_id: panel_id.clone(),
        presentation_id: primary_presentation.presentation_id.clone(),
    })
}

fn input_command(mailbox: &CommandMailbox, byte: u8) -> QueuedCommand {
    let desired = mailbox
        .lock()
        .desired
        .key()
        .cloned()
        .expect("test mailbox is bootstrapped");
    QueuedCommand::Input(Tagged::new(
        PaneRef::new(desired, 1),
        ControlInput::new(vec![byte]).expect("one-byte input is valid"),
    ))
}

fn poll_command(mailbox: &CommandMailbox) -> QueuedCommand {
    let desired = mailbox
        .lock()
        .desired
        .key()
        .cloned()
        .expect("test mailbox is bootstrapped");
    QueuedCommand::Poll(Tagged::new(PaneRef::new(desired, 1), ()))
}

fn controlling() -> ControllerMode {
    ControllerMode::Controlling(ControllerId::parse(&id(8)).expect("controller fixture"))
}

fn geometry() -> Geometry {
    Geometry::new(80, 24, 8, 16).expect("geometry fixture")
}

fn site_id(value: u128) -> SiteId {
    SiteId::parse(&id(value)).expect("site fixture")
}

fn context_id(value: u128) -> WorkbenchTerminalContextId {
    WorkbenchTerminalContextId::parse(&id(value)).expect("context fixture")
}

fn tab_id(value: u128) -> WorkbenchTabId {
    WorkbenchTabId::parse(&id(value)).expect("tab fixture")
}

fn panel_id(value: u128) -> WorkbenchPanelId {
    WorkbenchPanelId::parse(&id(value)).expect("panel fixture")
}

fn slot_id(value: u128) -> WorkbenchPanelSlotId {
    WorkbenchPanelSlotId::parse(&id(value)).expect("slot fixture")
}

fn split_id(value: u128) -> WorkbenchSplitId {
    WorkbenchSplitId::parse(&id(value)).expect("split fixture")
}

fn presentation_id(value: u128) -> TerminalPresentationId {
    TerminalPresentationId::parse(&id(value)).expect("presentation fixture")
}

fn worker_id(value: u128) -> WorkerId {
    WorkerId::parse(&id(value)).expect("worker fixture")
}

fn terminal_id(value: u128) -> TerminalId {
    TerminalId::parse(&id(value)).expect("terminal fixture")
}

fn id(value: u128) -> String {
    format!("{value:032x}")
}

/// Mailbox fixtures fabricate one panel identity per anchor — the mailbox only
/// needs a consistent logical pane, never a real workbench panel.
fn panel() -> WorkbenchPanelId {
    WorkbenchPanelId::parse(&id(0xF1)).unwrap()
}

fn cursor_snapshot(mailbox: &CommandMailbox) -> (u64, u64, u64, u128) {
    let raw = mailbox.lock();
    (
        raw.next_command_serial,
        raw.attachment_generation,
        raw.refresh_generation,
        raw.identity_cursor,
    )
}

fn observed<T, E: fmt::Debug>(result: Result<T, E>, context: &str) -> Result<T, ReplayFailure> {
    result.map_err(|error| violation_error(format!("{context}: {error:?}")))
}

fn require(condition: bool, message: &str) -> Result<(), ReplayFailure> {
    if condition {
        Ok(())
    } else {
        violation(message)
    }
}

fn invalid_error(message: impl Into<String>) -> ReplayFailure {
    ReplayFailure::Invalid(message.into())
}

fn violation<T>(message: impl Into<String>) -> Result<T, ReplayFailure> {
    Err(violation_error(message))
}

fn violation_error(message: impl Into<String>) -> ReplayFailure {
    ReplayFailure::Violation(message.into())
}

/// Drop-order cleanup for a scoped publisher parked on an occupied event cell.
/// `std::thread::scope` joins its spawned threads only after the scope closure
/// exits, so any early `?` or unwind out of the closure would otherwise join a
/// publisher still parked in `publish_event_before_using` — under a frozen
/// `FixedClock` with an undrained cell that join never returns. Arming this
/// guard first closes the sandbox mailbox (waking every condition waiter) on
/// the way out, so the publisher exits and the scope join completes. The
/// rescue close is cleanup only: it is never the measured production verdict,
/// and the original error still propagates.
struct OccupiedCellRescue<'a> {
    mailbox: &'a CommandMailbox,
    armed: bool,
}

impl<'a> OccupiedCellRescue<'a> {
    fn arm(mailbox: &'a CommandMailbox) -> Self {
        Self {
            mailbox,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for OccupiedCellRescue<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.mailbox.close(CarrierFailure::Closed);
        }
    }
}

/// Publishes `event` on a scoped thread while the sandbox event cell is
/// occupied, then runs `drive` (the real drain or the controlled clock
/// advance) on the test thread. The publisher reports its own result over
/// `results`; the test thread receives it within `result_bound` before
/// joining, so completion is proven by the publisher's report rather than
/// inferred from waiter counters. Any early error or unwind drops the rescue
/// guard before the scope's implicit join, so the parked publisher is closed
/// and woken instead of blocking the test forever.
#[allow(clippy::too_many_arguments)]
fn occupied_cell_publication(
    mailbox: &CommandMailbox,
    event: WorkerEvent,
    deadline: EventDeadline,
    clock: &(dyn MailboxClock + Sync),
    waiter_bound: Duration,
    result_bound: Duration,
    results: &std::sync::mpsc::Sender<Result<UiWaitKey, EventError>>,
    completed: &std::sync::mpsc::Receiver<Result<UiWaitKey, EventError>>,
    drive: impl FnOnce() -> Result<(), ReplayFailure>,
) -> Result<Result<UiWaitKey, EventError>, ReplayFailure> {
    std::thread::scope(|scope| {
        let mut rescue = OccupiedCellRescue::arm(mailbox);
        let publisher = {
            let results = results.clone();
            scope.spawn(move || {
                let _ = results.send(mailbox.publish_event_before_using(event, deadline, clock));
            })
        };
        require(
            mailbox.await_condition_waiter_before(Instant::now() + waiter_bound),
            "occupied-cell publisher never parked on the drain wait",
        )?;
        drive()?;
        let publication = completed.recv_timeout(result_bound).map_err(|error| {
            violation_error(format!("occupied-cell publisher did not report: {error}"))
        })?;
        publisher
            .join()
            .map_err(|_| violation_error("occupied-cell publisher panicked"))?;
        rescue.disarm();
        Ok(publication)
    })
}

fn add_counts<const N: usize>(total: &mut [usize; N], other: &[usize; N]) {
    for (total, count) in total.iter_mut().zip(other) {
        *total += count;
    }
}

fn serialize(actions: &[TraceAction]) -> String {
    actions
        .iter()
        .map(|action| match action {
            TraceAction::PreludeCreate {
                split,
                first_byte,
                feedback,
                returned,
            } => format!(
                "P:{split}:{first_byte}:{}:{}",
                u8::from(*feedback),
                u8::from(*returned),
            ),
            TraceAction::PanelCycle {
                kind,
                outcome,
                probe,
            } => format!("N:{kind}:{outcome}:{probe}"),
            TraceAction::CommitSecondCreate => "C".to_owned(),
            TraceAction::RefreshAppend {
                extra_tabs,
                control,
            } => format!("R:{extra_tabs}:{control}"),
            TraceAction::SupersedeSwitch { target_hint } => format!("S:{target_hint}"),
            TraceAction::ExhaustWithKnownCommit { later_failure } => {
                format!("X:{}", later_failure.index())
            }
            TraceAction::PanelNegativeSweep { terminal_probe } => {
                format!("G:{terminal_probe}")
            }
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn deserialize(encoded: &str) -> Result<Vec<TraceAction>, String> {
    if encoded.is_empty() {
        return Err("empty replay".to_owned());
    }
    encoded.split(';').map(parse_action).collect()
}

fn parse_action(encoded: &str) -> Result<TraceAction, String> {
    let fields = encoded.split(':').collect::<Vec<_>>();
    match fields.as_slice() {
        ["P", split, first_byte, feedback, returned] => Ok(TraceAction::PreludeCreate {
            split: parse_u8(split)?,
            first_byte: parse_u8(first_byte)?,
            feedback: parse_bool(feedback)?,
            returned: parse_bool(returned)?,
        }),
        ["N", kind, outcome, probe] => Ok(TraceAction::PanelCycle {
            kind: parse_u8(kind)?,
            outcome: parse_u8(outcome)?,
            probe: parse_u8(probe)?,
        }),
        ["C"] => Ok(TraceAction::CommitSecondCreate),
        ["R", extra_tabs, control] => Ok(TraceAction::RefreshAppend {
            extra_tabs: parse_u8(extra_tabs)?,
            control: parse_u8(control)?,
        }),
        ["S", target_hint] => Ok(TraceAction::SupersedeSwitch {
            target_hint: parse_u8(target_hint)?,
        }),
        ["X", later_failure] => {
            let index = parse_u8(later_failure)?;
            if usize::from(index) >= LaterFailure::COUNT {
                return Err(format!("unknown later failure {index}"));
            }
            Ok(TraceAction::ExhaustWithKnownCommit {
                later_failure: LaterFailure::from_index(usize::from(index)),
            })
        }
        ["G", terminal_probe] => Ok(TraceAction::PanelNegativeSweep {
            terminal_probe: parse_u8(terminal_probe)?,
        }),
        _ => Err(format!("unknown action {encoded}")),
    }
}

fn parse_u8(encoded: &str) -> Result<u8, String> {
    encoded
        .parse()
        .map_err(|error| format!("invalid u8 {encoded}: {error}"))
}

fn parse_bool(encoded: &str) -> Result<bool, String> {
    match encoded {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(format!("invalid bool {encoded}")),
    }
}

fn shrink_failure(mut actions: Vec<TraceAction>, failure: &TraceFailure) -> Vec<TraceAction> {
    let mut chunk = actions.len().next_power_of_two() / 2;
    while chunk > 0 {
        let mut start = 0;
        while start < actions.len() {
            let end = (start + chunk).min(actions.len());
            let mut candidate = actions.clone();
            candidate.drain(start..end);
            if !candidate.is_empty()
                && replay(&candidate).is_err_and(|candidate| failure.same_fingerprint(&candidate))
            {
                actions = candidate;
            } else {
                start += chunk;
            }
        }
        chunk /= 2;
    }
    for index in 0..actions.len() {
        for smaller in shrink_values(actions[index]) {
            let mut candidate = actions.clone();
            candidate[index] = smaller;
            if replay(&candidate).is_err_and(|candidate| failure.same_fingerprint(&candidate)) {
                actions = candidate;
            }
        }
    }
    actions
}

fn shrink_values(action: TraceAction) -> Vec<TraceAction> {
    match action {
        TraceAction::PreludeCreate {
            split,
            first_byte,
            feedback,
            returned,
        } => vec![
            TraceAction::PreludeCreate {
                split: split / 2,
                first_byte,
                feedback,
                returned,
            },
            TraceAction::PreludeCreate {
                split: 0,
                first_byte,
                feedback,
                returned,
            },
            TraceAction::PreludeCreate {
                split,
                first_byte: first_byte / 2,
                feedback,
                returned,
            },
            TraceAction::PreludeCreate {
                split,
                first_byte: 0,
                feedback,
                returned,
            },
        ],
        TraceAction::PanelCycle {
            kind,
            outcome,
            probe,
        } => vec![
            TraceAction::PanelCycle {
                kind,
                outcome: outcome / 2,
                probe,
            },
            TraceAction::PanelCycle {
                kind,
                outcome,
                probe: probe / 2,
            },
        ],
        TraceAction::RefreshAppend {
            extra_tabs,
            control,
        } => vec![TraceAction::RefreshAppend {
            extra_tabs: extra_tabs / 2,
            control: control / 2,
        }],
        TraceAction::SupersedeSwitch { target_hint } => vec![TraceAction::SupersedeSwitch {
            target_hint: target_hint / 2,
        }],
        TraceAction::PanelNegativeSweep { terminal_probe } => {
            vec![TraceAction::PanelNegativeSweep {
                terminal_probe: terminal_probe / 2,
            }]
        }
        TraceAction::ExhaustWithKnownCommit { .. } => Vec::new(),
        TraceAction::CommitSecondCreate => Vec::new(),
    }
}

/// Worker-visible panel outcome classes the generated cycle can deliver. Every entry maps to
/// a real `PanelResult` shape; committed classes apply the commit to the authority model first.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutcomeClass {
    NotSubmitted,
    Rejected,
    Denied,
    Unavailable,
    /// `Unchanged` at exactly the operation baseline revision: a legal no-op.
    Unchanged,
    /// Committed, and the delivered snapshot still contains the selected leaf.
    CommittedKeep,
    /// Committed, but a peer close folded the selected leaf out of the same tab before the
    /// snapshot arrived: the membership repair path must retarget the selection.
    CommittedDrop,
    /// Committed with no snapshot: arms `panel_recovery` for the next refresh to validate.
    CommittedNoSnapshot,
    /// `panel.close` accepted with the close deferred to a later revision: arms publication debt.
    CloseAccepted,
    /// Committed, but a peer whole-tab close folded into the snapshot between our commit
    /// and the read: the only honest whole-tab-removal shape (`PanelResult::TabUnavailable`
    /// is a defensive-only variant — a real `Rejected{TabUnavailable}` always carries
    /// `revision == baseline`, which can never describe a tab that was present there).
    CommittedTabGone,
}

impl OutcomeClass {
    const COUNT: usize = 10;

    const fn index(self) -> usize {
        match self {
            Self::NotSubmitted => 0,
            Self::Rejected => 1,
            Self::Denied => 2,
            Self::Unavailable => 3,
            Self::Unchanged => 4,
            Self::CommittedKeep => 5,
            Self::CommittedDrop => 6,
            Self::CommittedNoSnapshot => 7,
            Self::CloseAccepted => 8,
            Self::CommittedTabGone => 9,
        }
    }
}

/// The adapter-legality boundary (`connection/native/panel.rs`): which `PanelResult` shapes
/// the native adapter can return for each intent, crossed with the server-side honesty tier
/// (`report-server-panel-outcomes-18` §5–§7): `Unchanged` exists only on Move,
/// `CloseAccepted` only on Close, Close never returns `Committed`, and
/// `PanelResult::TabUnavailable` is a defensive-only variant no honest normal trajectory
/// produces for any intent — so no `OutcomeClass` models it and none may be added back.
const fn adapter_legal_outcome(kind: PanelRequestKind, class: OutcomeClass) -> bool {
    !matches!(
        (kind, class),
        (
            PanelRequestKind::Split { .. } | PanelRequestKind::Close,
            OutcomeClass::Unchanged,
        ) | (
            PanelRequestKind::Split { .. } | PanelRequestKind::Move { .. },
            OutcomeClass::CloseAccepted,
        ) | (
            PanelRequestKind::Close,
            OutcomeClass::CommittedKeep
                | OutcomeClass::CommittedDrop
                | OutcomeClass::CommittedNoSnapshot
                | OutcomeClass::CommittedTabGone,
        )
    )
}

/// Per-request-kind outcome tables — entries restricted to `adapter_legal_outcome`.
fn outcome_table(kind: PanelRequestKind) -> &'static [OutcomeClass] {
    const SPLIT: [OutcomeClass; 8] = [
        OutcomeClass::NotSubmitted,
        OutcomeClass::Rejected,
        OutcomeClass::Denied,
        OutcomeClass::Unavailable,
        OutcomeClass::CommittedKeep,
        OutcomeClass::CommittedDrop,
        OutcomeClass::CommittedNoSnapshot,
        OutcomeClass::CommittedTabGone,
    ];
    const MOVE: [OutcomeClass; 9] = [
        OutcomeClass::NotSubmitted,
        OutcomeClass::Rejected,
        OutcomeClass::Denied,
        OutcomeClass::Unavailable,
        OutcomeClass::Unchanged,
        OutcomeClass::CommittedKeep,
        OutcomeClass::CommittedDrop,
        OutcomeClass::CommittedNoSnapshot,
        OutcomeClass::CommittedTabGone,
    ];
    const CLOSE: [OutcomeClass; 5] = [
        OutcomeClass::NotSubmitted,
        OutcomeClass::Rejected,
        OutcomeClass::Denied,
        OutcomeClass::Unavailable,
        OutcomeClass::CloseAccepted,
    ];
    match kind {
        PanelRequestKind::Split { .. } => &SPLIT,
        PanelRequestKind::Move { .. } => &MOVE,
        PanelRequestKind::Close => &CLOSE,
    }
}

const fn panel_kind(selector: u8) -> PanelRequestKind {
    match selector % 3 {
        0 => PanelRequestKind::Split {
            direction: PanelDirection::Right,
        },
        1 => PanelRequestKind::Move {
            direction: PanelDirection::Right,
        },
        _ => PanelRequestKind::Close,
    }
}

const fn kind_index(kind: PanelRequestKind) -> usize {
    match kind {
        PanelRequestKind::Split { .. } => 0,
        PanelRequestKind::Move { .. } => 1,
        PanelRequestKind::Close => 2,
    }
}

/// The honest workbench the generated trace answers for: every snapshot a result carries is a
/// real committed revision of this model, so graft/candidate validation sees legal shapes only.
struct Authority {
    revision: u64,
    tabs: Vec<Tab>,
    fixture: u128,
}

impl Authority {
    /// One two-leaf tab at revision 1, matching the bootstrapped session projection.
    fn bootstrap() -> Self {
        let mut authority = Self {
            revision: 1,
            tabs: Vec::new(),
            fixture: 0,
        };
        let first = leaf_node(panel_id(5), "1", 4, 7, 6);
        let second = authority.fresh_leaf("1");
        let first_slot_id = slot_id(authority.fresh());
        let second_slot_id = slot_id(authority.fresh());
        let root_split_id = split_id(authority.fresh());
        authority.tabs.push(Tab {
            group_id: None,
            label: "1".to_owned(),
            pinned: false,
            root: PanelNode::Split {
                children: Box::new([
                    PanelSlot {
                        node: first,
                        slot_id: first_slot_id,
                    },
                    PanelSlot {
                        node: second,
                        slot_id: second_slot_id,
                    },
                ]),
                orientation: PanelOrientation::Horizontal,
                split_id: root_split_id,
            },
            tab_id: tab_id(1),
        });
        authority
    }

    fn fresh(&mut self) -> u128 {
        self.fixture += 1;
        FIXTURE_SEED + self.fixture
    }

    fn fresh_leaf(&mut self, label: &str) -> PanelNode {
        leaf_node(
            panel_id(self.fresh()),
            label,
            self.fresh(),
            self.fresh(),
            self.fresh(),
        )
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            navigation: Navigation {
                projects: Vec::new(),
                standalone_terminal_context_id: context_id(3),
            },
            protocol_version: ProtocolVersion,
            revision: WorkbenchRevision::new(self.revision),
            site_id: site_id(2),
            terminal_contexts: vec![TerminalContext {
                groups: Vec::new(),
                tabs: self.tabs.clone(),
                terminal_context_id: context_id(3),
            }],
        }
    }

    fn append_tab(&mut self) {
        let label = (self.tabs.len() + 1).to_string();
        let root = self.fresh_leaf(&label);
        let new_tab_id = tab_id(self.fresh());
        self.tabs.push(Tab {
            group_id: None,
            label,
            pinned: false,
            root,
            tab_id: new_tab_id,
        });
        self.revision += 1;
    }

    /// A legal peer commit that only changes a tab's label: the revision
    /// advances past every in-flight baseline without consuming fixture IDs or
    /// touching tab order, leaf geometry, or admission-time expectations.
    fn rename_first_tab(&mut self) {
        let tab = self.tabs.first_mut().expect("bootstrap tab exists");
        tab.label = format!("peer-{}", self.revision);
        self.revision += 1;
    }

    fn leaf_count(&self, tab_index: usize) -> usize {
        self.tabs
            .get(tab_index)
            .map(|tab| count_leaves(&tab.root))
            .unwrap_or(0)
    }

    /// Applies the operation's committed form and returns the exact commit the worker reports.
    fn apply_commit(&mut self, operation: &PanelOperation) -> Result<PanelCommit, ReplayFailure> {
        let revision = WorkbenchRevision::new(self.revision + 1);
        let tab_id = operation.target().tab_id().clone();
        match operation.intent() {
            PanelIntent::Split {
                direction, label, ..
            } => {
                let (split_id, first_slot_id, second_slot_id, new_panel_id, presentation_id) =
                    self.apply_split(&tab_id, operation.panel_id(), *direction, label)?;
                Ok(PanelCommit::Split(WorkbenchPanelSplitCommit::Pending {
                    revision,
                    terminal_context_id: context_id(3),
                    tab_id,
                    panel_id: operation.panel_id().clone(),
                    new_panel_id,
                    split_id,
                    first_slot_id,
                    second_slot_id,
                    presentation_id,
                }))
            }
            PanelIntent::Move {
                target_panel_id,
                direction,
            } => {
                self.apply_move(&tab_id, operation.panel_id(), target_panel_id, *direction)?;
                Ok(PanelCommit::Move(WorkbenchPanelMoveCommit {
                    revision,
                    terminal_context_id: context_id(3),
                    tab_id,
                    panel_id: operation.panel_id().clone(),
                    target_panel_id: target_panel_id.clone(),
                }))
            }
            PanelIntent::Close => violation("panel close has no commit"),
        }
    }

    /// `panel.split` graft: the source leaf becomes a Split whose first child is the source and
    /// whose second child is the new leaf (for rightward splits).
    fn apply_split(
        &mut self,
        tab_id: &WorkbenchTabId,
        source: &WorkbenchPanelId,
        direction: WorkbenchPanelSplitDirection,
        label: &WorkbenchLabel,
    ) -> Result<
        (
            WorkbenchSplitId,
            WorkbenchPanelSlotId,
            WorkbenchPanelSlotId,
            WorkbenchPanelId,
            TerminalPresentationId,
        ),
        ReplayFailure,
    > {
        let split_id = split_id(self.fresh());
        let first_slot_id = slot_id(self.fresh());
        let second_slot_id = slot_id(self.fresh());
        let new_panel_id = panel_id(self.fresh());
        let presentation_id = presentation_id(self.fresh());
        let terminal = terminal_id(self.fresh());
        let worker = worker_id(self.fresh());
        let tab = self
            .tabs
            .iter_mut()
            .find(|tab| &tab.tab_id == tab_id)
            .ok_or_else(|| violation_error("authority split missed its tab"))?;
        let new_leaf = PanelNode::Panel {
            child_presentations: Vec::new(),
            panel_id: new_panel_id.clone(),
            primary_presentation: Presentation {
                custom_label: None,
                binding: Some(PresentationBinding {
                    terminal_id: terminal,
                    worker_id: worker,
                }),
                label: label.as_str().to_owned(),
                presentation_id: presentation_id.clone(),
            },
        };
        require(
            graft_split(
                &mut tab.root,
                source,
                new_leaf,
                direction,
                split_id.clone(),
                first_slot_id.clone(),
                second_slot_id.clone(),
            ),
            "authority split missed its leaf",
        )?;
        self.revision += 1;
        Ok((
            split_id,
            first_slot_id,
            second_slot_id,
            new_panel_id,
            presentation_id,
        ))
    }

    /// `panel.move` graft: the moved leaf detaches (collapsing its parent split) and regrafts
    /// beside the target leaf under a fresh split.
    fn apply_move(
        &mut self,
        tab_id: &WorkbenchTabId,
        moved: &WorkbenchPanelId,
        target: &WorkbenchPanelId,
        direction: WorkbenchPanelSplitDirection,
    ) -> Result<(), ReplayFailure> {
        let split_id = split_id(self.fresh());
        let first_slot_id = slot_id(self.fresh());
        let second_slot_id = slot_id(self.fresh());
        let tab = self
            .tabs
            .iter_mut()
            .find(|tab| &tab.tab_id == tab_id)
            .ok_or_else(|| violation_error("authority move missed its tab"))?;
        let moved_node = remove_leaf(&mut tab.root, moved)
            .ok_or_else(|| violation_error("authority move missed its leaf"))?;
        require(
            graft_move(
                &mut tab.root,
                target,
                moved_node,
                direction,
                split_id,
                first_slot_id,
                second_slot_id,
            ),
            "authority move missed its target leaf",
        )?;
        self.revision += 1;
        Ok(())
    }

    /// `panel.close` on one leaf: removes it and collapses the parent split to the sibling.
    fn close_leaf(&mut self, tab_id: &WorkbenchTabId, leaf: &WorkbenchPanelId) -> bool {
        let Some(tab) = self.tabs.iter_mut().find(|tab| &tab.tab_id == tab_id) else {
            return false;
        };
        if remove_leaf(&mut tab.root, leaf).is_none() {
            return false;
        }
        self.revision += 1;
        true
    }

    /// A peer closed the whole tab.
    fn remove_tab(&mut self, tab_id: &WorkbenchTabId) {
        self.tabs.retain(|tab| &tab.tab_id != tab_id);
        self.revision += 1;
    }
}

fn leaf_node(
    panel: WorkbenchPanelId,
    label: &str,
    presentation: u128,
    terminal: u128,
    worker: u128,
) -> PanelNode {
    PanelNode::Panel {
        child_presentations: Vec::new(),
        panel_id: panel,
        primary_presentation: Presentation {
            custom_label: None,
            binding: Some(PresentationBinding {
                terminal_id: terminal_id(terminal),
                worker_id: worker_id(worker),
            }),
            label: label.to_owned(),
            presentation_id: presentation_id(presentation),
        },
    }
}

fn placeholder_leaf() -> PanelNode {
    leaf_node(panel_id(0xdead), "x", 0xdead, 0xdead, 0xdead)
}

fn count_leaves(node: &PanelNode) -> usize {
    match node {
        PanelNode::Panel { .. } => 1,
        PanelNode::Split { children, .. } => {
            children.iter().map(|slot| count_leaves(&slot.node)).sum()
        }
    }
}

/// Replaces the leaf carrying `source` with a Split whose children order matches `direction`:
/// for Right/Below the source stays first and the new leaf is second.
fn graft_split(
    node: &mut PanelNode,
    source: &WorkbenchPanelId,
    new_leaf: PanelNode,
    direction: WorkbenchPanelSplitDirection,
    split_id: WorkbenchSplitId,
    first_slot_id: WorkbenchPanelSlotId,
    second_slot_id: WorkbenchPanelSlotId,
) -> bool {
    match node {
        PanelNode::Panel { panel_id, .. } if panel_id == source => {
            let old = std::mem::replace(node, placeholder_leaf());
            let (first, second) = match direction {
                WorkbenchPanelSplitDirection::Right | WorkbenchPanelSplitDirection::Below => {
                    (old, new_leaf)
                }
                WorkbenchPanelSplitDirection::Left | WorkbenchPanelSplitDirection::Above => {
                    (new_leaf, old)
                }
            };
            *node =
                PanelNode::Split {
                    children: Box::new([
                        PanelSlot {
                            node: first,
                            slot_id: first_slot_id,
                        },
                        PanelSlot {
                            node: second,
                            slot_id: second_slot_id,
                        },
                    ]),
                    orientation: match direction {
                        WorkbenchPanelSplitDirection::Right
                        | WorkbenchPanelSplitDirection::Left => PanelOrientation::Horizontal,
                        WorkbenchPanelSplitDirection::Above
                        | WorkbenchPanelSplitDirection::Below => PanelOrientation::Vertical,
                    },
                    split_id,
                };
            true
        }
        PanelNode::Panel { .. } => false,
        PanelNode::Split { children, .. } => children.iter_mut().any(|slot| {
            graft_split(
                &mut slot.node,
                source,
                new_leaf.clone(),
                direction,
                split_id.clone(),
                first_slot_id.clone(),
                second_slot_id.clone(),
            )
        }),
    }
}

/// Removes the leaf carrying `panel_id` and collapses its parent Split into the surviving
/// sibling subtree. Returns the removed leaf node.
fn remove_leaf(node: &mut PanelNode, panel_id: &WorkbenchPanelId) -> Option<PanelNode> {
    let PanelNode::Split { children, .. } = node else {
        return None;
    };
    for index in 0..2 {
        if matches!(&children[index].node, PanelNode::Panel { panel_id: actual, .. }
            if actual == panel_id)
        {
            let removed = std::mem::replace(&mut children[index].node, placeholder_leaf());
            *node = std::mem::replace(&mut children[1 - index].node, placeholder_leaf());
            return Some(removed);
        }
        if let Some(removed) = remove_leaf(&mut children[index].node, panel_id) {
            return Some(removed);
        }
    }
    None
}

/// Regrafts `moved` beside the leaf carrying `target` under a fresh Split; for Right/Below the
/// target stays first and the moved leaf is second.
fn graft_move(
    node: &mut PanelNode,
    target: &WorkbenchPanelId,
    moved: PanelNode,
    direction: WorkbenchPanelSplitDirection,
    split_id: WorkbenchSplitId,
    first_slot_id: WorkbenchPanelSlotId,
    second_slot_id: WorkbenchPanelSlotId,
) -> bool {
    match node {
        PanelNode::Panel { panel_id, .. } if panel_id == target => {
            let old = std::mem::replace(node, placeholder_leaf());
            let (first, second) = match direction {
                WorkbenchPanelSplitDirection::Right | WorkbenchPanelSplitDirection::Below => {
                    (old, moved)
                }
                WorkbenchPanelSplitDirection::Left | WorkbenchPanelSplitDirection::Above => {
                    (moved, old)
                }
            };
            *node =
                PanelNode::Split {
                    children: Box::new([
                        PanelSlot {
                            node: first,
                            slot_id: first_slot_id,
                        },
                        PanelSlot {
                            node: second,
                            slot_id: second_slot_id,
                        },
                    ]),
                    orientation: match direction {
                        WorkbenchPanelSplitDirection::Right
                        | WorkbenchPanelSplitDirection::Left => PanelOrientation::Horizontal,
                        WorkbenchPanelSplitDirection::Above
                        | WorkbenchPanelSplitDirection::Below => PanelOrientation::Vertical,
                    },
                    split_id,
                };
            true
        }
        PanelNode::Panel { .. } => false,
        PanelNode::Split { children, .. } => children.iter_mut().any(|slot| {
            graft_move(
                &mut slot.node,
                target,
                moved.clone(),
                direction,
                split_id.clone(),
                first_slot_id.clone(),
                second_slot_id.clone(),
            )
        }),
    }
}

/// A second mailbox/session pair for protocol-negative probes: each probe runs on a fresh pair
/// so a protocol close never bleeds into the live trace.
struct PanelSandbox {
    base: Instant,
    clock: FixedClock,
    mailbox: CommandMailbox,
    session: NativeSessionState,
    authority: Authority,
    published: Option<PanelResult>,
}

impl PanelSandbox {
    fn new() -> Result<Self, ReplayFailure> {
        let base = Instant::now();
        let authority = Authority::bootstrap();
        let cache = project_native_standalone_tabs(&authority.snapshot())
            .map_err(|error| violation_error(format!("sandbox projection: {error:?}")))?;
        let (mut session, _target) = NativeSessionState::starting(cache, 1, geometry())
            .ok_or_else(|| violation_error("sandbox state was rejected"))?;
        let desired = session.desired().unwrap().clone();
        require(
            session.install_projection_t(&desired),
            "sandbox projection install",
        )?;
        require(
            session.record_setup_configuration_t(&desired, controlling(), Some(geometry())),
            "sandbox setup configuration",
        )?;
        require(
            session.stage_drawn_frame_t(&desired, FrameIdentity::new(1)),
            "sandbox frame stage",
        )?;
        require(
            session.mark_feedback_accepted_t(&desired, FrameIdentity::new(1)),
            "sandbox frame feedback",
        )?;
        require(
            session.publish_ready_t(
                &desired,
                FrameIdentity::new(1),
                controlling(),
                Some(geometry()),
            ),
            "sandbox Ready publication",
        )?;
        let mailbox = CommandMailbox::new(IDENTITY_SEED, geometry());
        {
            let mut raw = mailbox.lock();
            raw.attachment_generation = desired.generation();
            raw.desired = MailboxMembership::Live {
                desired: Some(desired.clone()),
                panes: vec![session.focus_pane_ref().unwrap()],
            };
        }
        Ok(Self {
            base,
            clock: FixedClock(base),
            mailbox,
            session,
            authority,
            published: None,
        })
    }

    fn fixture_request(&mut self) -> WorkbenchRequestId {
        WorkbenchRequestId::parse(&id(self.authority.fresh())).expect("fixture request")
    }

    /// Admits a split on the two-leaf bootstrap tab and takes its queued marker.
    fn admit_split(&mut self) -> Result<(PanelOperation, CommandLease), ReplayFailure> {
        let deadline = self.base + Duration::from_secs(30);
        let operation = observed(
            self.mailbox.try_enqueue_panel_for_using(
                &mut self.session,
                deadline,
                &self.clock,
                |session, request_id| {
                    session
                        .plan_panel(
                            request_id,
                            PanelRequestKind::Split {
                                direction: PanelDirection::Right,
                            },
                            deadline,
                        )
                        .ok_or("sandbox split inadmissible")
                },
                |session, planned| {
                    session
                        .begin_panel(planned.clone())
                        .then_some(())
                        .ok_or("sandbox begin refused")
                },
            ),
            "admit sandbox split",
        )?;
        let lease = observed(self.mailbox.try_recv(), "take sandbox split")?
            .ok_or_else(|| violation_error("sandbox split marker missing"))?;
        Ok((operation, lease))
    }

    fn admit_split_inflight(&mut self) -> Result<(PanelOperation, CommandLease), ReplayFailure> {
        let (operation, lease) = self.admit_split()?;
        require(
            observed(
                self.mailbox.mark_panel_in_flight(&operation),
                "mark sandbox split",
            )? == MutationResult::Applied,
            "sandbox split mark was stale",
        )?;
        Ok((operation, lease))
    }

    /// Admits a split, applies its commit to the authority, and publishes the committed result.
    /// The event is left in the `Event` cell (not yet taken) when `take` is false.
    fn admit_split_published(
        &mut self,
        take: bool,
    ) -> Result<(PanelOperation, CommandLease, UiWaitKey), ReplayFailure> {
        let (operation, lease) = self.admit_split_inflight()?;
        let commit = self.authority.apply_commit(&operation)?;
        let outcome = PanelResult::Committed {
            commit: Box::new(commit),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: self.authority.snapshot(),
            },
        };
        let wait = observed(
            self.mailbox.publish_event_before_using(
                WorkerEvent::PanelResult {
                    operation: operation.clone(),
                    outcome: outcome.clone(),
                },
                EventDeadline::PlacementOnly(operation.deadline()),
                &self.clock,
            ),
            "publish sandbox committed result",
        )?;
        self.published = Some(outcome);
        if take {
            let taken = observed(
                self.mailbox.take_event_before_using(&self.clock),
                "take sandbox Panel event",
            )?
            .ok_or_else(|| violation_error("sandbox Panel event missing"))?;
            let (taken_wait, event) = taken.into_parts();
            require(
                taken_wait.as_ref() == Some(&wait)
                    && matches!(event, WorkerEvent::PanelResult { operation: actual, .. }
                        if actual == operation),
                "sandbox Panel event changed",
            )?;
        }
        Ok((operation, lease, wait))
    }

    fn admit_split_awaiting(
        &mut self,
    ) -> Result<(PanelOperation, CommandLease, UiWaitKey), ReplayFailure> {
        self.admit_split_published(true)
    }

    fn admit_split_awaiting_published(
        &mut self,
    ) -> Result<(PanelOperation, CommandLease, UiWaitKey), ReplayFailure> {
        self.admit_split_published(false)
    }

    /// Admits a split and publishes a committed result whose snapshot fails projection
    /// validation (no standalone terminal context), reaching `AwaitingUi` with a divergent
    /// outcome that must retire while retaining the commit.
    fn admit_split_awaiting_divergent(
        &mut self,
    ) -> Result<(PanelOperation, CommandLease, UiWaitKey), ReplayFailure> {
        let (operation, lease) = self.admit_split_inflight()?;
        let commit = self.authority.apply_commit(&operation)?;
        let mut snapshot = self.authority.snapshot();
        snapshot.terminal_contexts.clear();
        let outcome = PanelResult::Committed {
            commit: Box::new(commit),
            outcome: WorkbenchSnapshotOutcome::Available { snapshot },
        };
        let wait = observed(
            self.mailbox.publish_event_before_using(
                WorkerEvent::PanelResult {
                    operation: operation.clone(),
                    outcome: outcome.clone(),
                },
                EventDeadline::PlacementOnly(operation.deadline()),
                &self.clock,
            ),
            "publish sandbox divergent result",
        )?;
        self.published = Some(outcome);
        let taken = observed(
            self.mailbox.take_event_before_using(&self.clock),
            "take divergent Panel event",
        )?
        .ok_or_else(|| violation_error("divergent Panel event missing"))?;
        let (taken_wait, _event) = taken.into_parts();
        require(
            taken_wait.as_ref() == Some(&wait),
            "divergent Panel wait key changed",
        )?;
        Ok((operation, lease, wait))
    }

    /// Deposits the UI reply for a published result and returns the panel decision.
    fn settle_published(
        &mut self,
        operation: &PanelOperation,
        wait: &UiWaitKey,
    ) -> Result<PanelDecision, ReplayFailure> {
        let outcome = self
            .published
            .take()
            .ok_or_else(|| violation_error("sandbox has no published outcome"))?;
        let session = &mut self.session;
        require(
            observed(
                self.mailbox
                    .deposit_ui_reply_before_using(wait, &self.clock, |_transaction| {
                        let prepared = session.prepare_panel(operation, outcome);
                        Ok(UiReply::Panel(PanelSettlement::new(
                            session.apply_prepared_panel(prepared),
                            session.desired().cloned(),
                        )))
                    }),
                "deposit sandbox reply",
            )? == MutationResult::Applied,
            "sandbox reply was stale",
        )?;
        let taken = observed(
            self.mailbox.take_deposited_reply(wait),
            "take sandbox reply",
        )?
        .ok_or_else(|| violation_error("sandbox reply missing"))?;
        let UiReply::Panel(settlement) = taken else {
            return violation("sandbox reply kind changed");
        };
        Ok(settlement.decision().clone())
    }
}

fn observed_err<T, E>(result: Result<T, E>, context: &str) -> Result<E, ReplayFailure> {
    result.map_or_else(Ok, |_| {
        violation(format!("{context}: unexpectedly succeeded"))
    })
}
