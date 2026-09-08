use super::super::{
    AdmissionError, COMMAND_CAPACITY, CausalSettlement, CommandLease, CommandMailbox, CreateMarker,
    CreatePrecommitOutcome, DesiredInstall, EventDeadline, MailboxClock, MailboxMembership,
    MutationResult, QueuedCommand, RefreshSettlement, SwitchMarker, Tagged, UiReply,
    UiReplyTransaction, UiWaitKey, WorkerEvent,
};
use crate::{
    local_client::workbench::{
        WorkbenchTabCreateCommit, WorkbenchTabCreatePrecommit, WorkbenchTabCreateRequest,
    },
    terminal_session::{
        state::{
            AttachTarget, AttachmentKey, CarrierFailure, CleanupContinuation, ControllerMode,
            CreateOperation, CreateOperationKey, CreateProjectionDecision,
            CreateTerminalProvenance, FrameIdentity, NativeSessionState, NavigatorRefreshStatus,
            OperationDeadlines, PendingCreateCommit, PendingRefreshStage, RefreshDecision,
            SessionNotice, SessionPhaseKind,
        },
        workbench_projection::project_native_standalone_tabs,
    },
};
use orchbee_client_server_protocol::{ControlInput, ControllerId, Geometry, TerminalViewerAction};
use orchbee_workbench_protocol::{
    Navigation, PanelNode, Presentation, PresentationBinding, ProtocolVersion, SiteId, Snapshot,
    Tab, TerminalContext, TerminalId, TerminalPresentationId, WorkbenchAction, WorkbenchCommand,
    WorkbenchLabel, WorkbenchPanelId, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchSnapshotOutcome, WorkbenchTabId, WorkbenchTerminalContextId,
    WorkbenchTerminalGeometry, WorkerId,
};
use std::{
    cell::Cell,
    fmt,
    time::{Duration, Instant},
};

const IDENTITY_SEED: u128 = 0x400;
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
const ACTIONS_PER_TRACE: usize = 5;

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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActionClass {
    PreludeCreate { feedback: bool, returned: bool },
    CommitSecondCreate,
    RefreshAppend,
    SupersedeSwitch,
    ExhaustWithKnownCommit(LaterFailure),
}

impl TraceAction {
    const fn class(self) -> ActionClass {
        match self {
            Self::PreludeCreate {
                feedback, returned, ..
            } => ActionClass::PreludeCreate { feedback, returned },
            Self::CommitSecondCreate => ActionClass::CommitSecondCreate,
            Self::RefreshAppend { .. } => ActionClass::RefreshAppend,
            Self::SupersedeSwitch { .. } => ActionClass::SupersedeSwitch,
            Self::ExhaustWithKnownCommit { later_failure } => {
                ActionClass::ExhaustWithKnownCommit(later_failure)
            }
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
    phase: TracePhase,
    first_operation: Option<CreateOperationKey>,
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
    assert!(total.known_commit_handoffs >= SEEDS.len());
    assert!(total.identity_exhaustions >= SEEDS.len());
    assert!(total.later_failures.iter().all(|count| *count > 0));
}

fn generate(seed: u64, seed_index: usize) -> Vec<TraceAction> {
    let mut random = seed;
    let mut next = || {
        random = random
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (random >> 48) as u8
    };
    vec![
        TraceAction::PreludeCreate {
            split: next(),
            first_byte: next(),
            feedback: seed_index.is_multiple_of(2),
            returned: (seed_index / 2).is_multiple_of(2),
        },
        TraceAction::CommitSecondCreate,
        TraceAction::RefreshAppend {
            extra_tabs: next(),
            control: next(),
        },
        TraceAction::SupersedeSwitch {
            target_hint: next(),
        },
        TraceAction::ExhaustWithKnownCommit {
            later_failure: LaterFailure::from_index(seed_index),
        },
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
        let cache = project_native_standalone_tabs(&snapshot(1, 1))
            .map_err(|error| violation_error(format!("initial projection: {error:?}")))?;
        let (mut session, _target) = NativeSessionState::starting(cache, 1, geometry())
            .ok_or_else(|| violation_error("initial state was rejected"))?;
        let desired = session.desired().unwrap().clone();
        require(
            session.install_projection(&desired),
            "initial projection install",
        )?;
        require(
            session.record_setup_configuration(&desired, controlling(), Some(geometry())),
            "initial setup configuration",
        )?;
        require(
            session.stage_drawn_frame(&desired, FrameIdentity::new(1)),
            "initial frame stage",
        )?;
        require(
            session.mark_feedback_accepted(&desired, FrameIdentity::new(1)),
            "initial frame feedback",
        )?;
        require(
            session.publish_ready(
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
                desired: Some(desired),
            };
        }
        Ok(Self {
            base,
            clock: FixedClock(base),
            mailbox,
            session,
            phase: TracePhase::Ready,
            first_operation: None,
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
            (_, action) => Err(ReplayFailure::Invalid(format!(
                "{action:?} is not admitted from {:?}",
                self.phase
            ))),
        }
    }

    fn assert_pair_alignment(&self) -> Result<(), ReplayFailure> {
        if self.phase == TracePhase::Retired {
            require(
                self.mailbox.is_closed() == Some(CarrierFailure::IdentityExhausted),
                "retired mailbox lost its identity-exhaustion cause",
            )?;
            return require(
                self.session.retired_failure() == Some(CarrierFailure::IdentityExhausted),
                "retired state lost its identity-exhaustion cause",
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
            let settlement = CausalSettlement::Feedback(TerminalViewerAction::Poll {});
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
            operation.reserved_generation() == first.reserved_generation() + 1,
            "second Create reused the first attachment generation",
        )?;
        require(
            operation.request_id() != first.request_id(),
            "second Create reused the first request identity",
        )?;
        require(
            operation.request_id().to_string() == id(IDENTITY_SEED + 2),
            "second Create skipped or reused a request identity",
        )?;
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

        let candidate = snapshot(2, 2);
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
            let desired = transaction.prepare_desired_install(super::super::membership::DesiredInstall::ReuseReserved(committed.committed_key().clone()))?;
            let plan = session.prepare_cleaned_membership_install(&cleanup, prepared, desired.desired().cloned())?;
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
        let candidate = snapshot(old_count + appended, 3);
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
                        let decision = session.apply_prepared_refresh(prepared);
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
                == Some(NavigatorRefreshStatus::Success(WorkbenchRevision::new(3)))
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
        require(tab_count >= 3, "Switch trace needs an appended third tab")?;
        let first_target = 0;
        let second_target = 2 + usize::from(target_hint) % (tab_count - 2);
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
        let first = self.install_unattached_switch_candidate(&first)?;

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
        let second = self.install_unattached_switch_candidate(&second)?;
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
            ),
            "prepare Switch candidate",
        )?;
        let mut locked = self.mailbox.lock();
        let mut transaction = UiReplyTransaction {
            state: &mut locked,
            observed_at: self.base,
        };
        let desired = observed(
            transaction.prepare_desired_install(DesiredInstall::Fresh(
                marker.selected().key().tab_id().clone(),
            )),
            "prepare fresh Switch successor",
        )?;
        let plan = observed(
            self.session.prepare_switch_membership_install(
                marker.selected().key(),
                prepared,
                desired.desired().cloned(),
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
                    key: target.selected().key().clone(),
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
                            .settle_unattached_setup(&key, index, &provenance)
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
            self.session.install_projection(key),
            "Switch projection install",
        )?;
        require(
            self.session
                .record_setup_configuration(key, controlling(), Some(geometry())),
            "Switch setup configuration",
        )?;
        let frame = FrameIdentity::new(2);
        require(
            self.session.stage_drawn_frame(key, frame),
            "Switch frame stage",
        )?;
        require(
            self.session.mark_feedback_accepted(key, frame),
            "Switch frame feedback",
        )?;
        require(
            self.session
                .publish_ready(key, frame, controlling(), Some(geometry())),
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

        let committed_snapshot = snapshot(tab_count + 1, 4);
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
        self.phase = TracePhase::Retired;
        Ok(())
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
        desired,
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
    QueuedCommand::Poll(Tagged::new(desired, ()))
}

fn snapshot(tab_count: usize, revision: u64) -> Snapshot {
    Snapshot {
        navigation: Navigation {
            projects: Vec::new(),
            standalone_terminal_context_id: context_id(3),
        },
        protocol_version: ProtocolVersion,
        revision: WorkbenchRevision::new(revision),
        site_id: site_id(2),
        terminal_contexts: vec![TerminalContext {
            groups: Vec::new(),
            tabs: (0..tab_count).map(tab).collect(),
            terminal_context_id: context_id(3),
        }],
    }
}

fn tab(index: usize) -> Tab {
    let offset = u128::try_from(index).unwrap_or(0) * 10;
    let label = (index + 1).to_string();
    Tab {
        group_id: None,
        label: label.clone(),
        pinned: false,
        root: PanelNode::Panel {
            child_presentations: Vec::new(),
            panel_id: panel_id(5 + offset),
            primary_presentation: Presentation {
                binding: Some(PresentationBinding {
                    terminal_id: terminal_id(7 + offset),
                    worker_id: worker_id(6 + offset),
                }),
                label,
                presentation_id: presentation_id(4 + offset),
            },
        },
        tab_id: tab_id(1 + offset),
    }
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
            TraceAction::CommitSecondCreate => "C".to_owned(),
            TraceAction::RefreshAppend {
                extra_tabs,
                control,
            } => format!("R:{extra_tabs}:{control}"),
            TraceAction::SupersedeSwitch { target_hint } => format!("S:{target_hint}"),
            TraceAction::ExhaustWithKnownCommit { later_failure } => {
                format!("X:{}", later_failure.index())
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
        TraceAction::ExhaustWithKnownCommit { .. } => Vec::new(),
        TraceAction::CommitSecondCreate => Vec::new(),
    }
}
