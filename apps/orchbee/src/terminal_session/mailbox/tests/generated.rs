use super::*;

const SEEDS: [u64; 6] = [
    0x243f_6a88,
    0x85a3_08d3,
    0x1319_8a2e,
    0x0370_7344,
    0xa409_3822,
    0x299f_31d0,
];
const GENERATED_STEPS_PER_SEED: usize = 48;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdmissionKind {
    Input,
    Switch,
    Create,
    Refresh,
}

impl AdmissionKind {
    const COUNT: usize = 4;

    const fn index(self) -> usize {
        match self {
            Self::Input => 0,
            Self::Switch => 1,
            Self::Create => 2,
            Self::Refresh => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SupersessionCut {
    BeforePollHandoff,
    AfterPollHandoff,
    AfterFrameDelivery,
    AfterEventTake,
    AfterDraw,
    AfterCausalTake,
}

impl SupersessionCut {
    const COUNT: usize = 6;

    const fn index(self) -> usize {
        match self {
            Self::BeforePollHandoff => 0,
            Self::AfterPollHandoff => 1,
            Self::AfterFrameDelivery => 2,
            Self::AfterEventTake => 3,
            Self::AfterDraw => 4,
            Self::AfterCausalTake => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeadlineCut {
    ForwardPlacement,
    ForwardTake,
    ForwardReply,
    TotalPlacement,
    TotalTake,
    TotalReply,
    DepositedReplyWins,
}

impl DeadlineCut {
    const COUNT: usize = 7;

    const fn index(self) -> usize {
        match self {
            Self::ForwardPlacement => 0,
            Self::ForwardTake => 1,
            Self::ForwardReply => 2,
            Self::TotalPlacement => 3,
            Self::TotalTake => 4,
            Self::TotalReply => 5,
            Self::DepositedReplyWins => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ForwardEventKind {
    SetupConfigured,
    FeedbackAccepted,
}

impl ForwardEventKind {
    const COUNT: usize = 2;

    const fn index(self) -> usize {
        match self {
            Self::SetupConfigured => 0,
            Self::FeedbackAccepted => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UiBoundary {
    Placement,
    Take,
    Reply,
}

impl UiBoundary {
    const COUNT: usize = 3;

    const fn index(self) -> usize {
        match self {
            Self::Placement => 0,
            Self::Take => 1,
            Self::Reply => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExhaustionKind {
    CommandSerial,
    AttachmentGeneration,
    RefreshGeneration,
    RequestIdentity,
}

impl ExhaustionKind {
    const COUNT: usize = 4;

    const fn index(self) -> usize {
        match self {
            Self::CommandSerial => 0,
            Self::AttachmentGeneration => 1,
            Self::RefreshGeneration => 2,
            Self::RequestIdentity => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CloseKind {
    Deadline,
    OutcomeUnknown,
    Protocol,
    Transport,
}

impl CloseKind {
    const COUNT: usize = 4;

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
    Admission {
        occupancy: u8,
        kind: AdmissionKind,
    },
    FifoDrain {
        occupancy: u8,
    },
    PreMarkerPoll {
        occupancy: u8,
        feedback: bool,
    },
    RefreshUiFence,
    SwitchSupersession(SupersessionCut),
    Deadline(DeadlineCut),
    ForwardBarrier {
        event: ForwardEventKind,
        boundary: UiBoundary,
    },
    Exhaustion(ExhaustionKind),
    GeometryRetry {
        columns: u8,
    },
    CloseFirstCause(CloseKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActionClass {
    Admission(AdmissionKind),
    FifoDrain,
    PreMarkerPoll { feedback: bool },
    RefreshUiFence,
    SwitchSupersession(SupersessionCut),
    Deadline(DeadlineCut),
    ForwardBarrier(ForwardEventKind, UiBoundary),
    Exhaustion(ExhaustionKind),
    GeometryRetry,
    CloseFirstCause(CloseKind),
}

impl TraceAction {
    const fn class(self) -> ActionClass {
        match self {
            Self::Admission { kind, .. } => ActionClass::Admission(kind),
            Self::FifoDrain { .. } => ActionClass::FifoDrain,
            Self::PreMarkerPoll { feedback, .. } => ActionClass::PreMarkerPoll { feedback },
            Self::RefreshUiFence => ActionClass::RefreshUiFence,
            Self::SwitchSupersession(cut) => ActionClass::SwitchSupersession(cut),
            Self::Deadline(cut) => ActionClass::Deadline(cut),
            Self::ForwardBarrier { event, boundary } => {
                ActionClass::ForwardBarrier(event, boundary)
            }
            Self::Exhaustion(kind) => ActionClass::Exhaustion(kind),
            Self::GeometryRetry { .. } => ActionClass::GeometryRetry,
            Self::CloseFirstCause(kind) => ActionClass::CloseFirstCause(kind),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TraceFailure {
    step: usize,
    action: TraceAction,
    oracle: String,
}

impl TraceFailure {
    fn same_fingerprint(&self, other: &Self) -> bool {
        self.action.class() == other.action.class() && self.oracle == other.oracle
    }
}

#[derive(Clone, Debug, Default)]
struct Coverage {
    admission_kinds: [usize; AdmissionKind::COUNT],
    full_refusals: [usize; AdmissionKind::COUNT],
    occupancy_values: [usize; COMMAND_CAPACITY + 1],
    fifo_drains: usize,
    causal_empty: usize,
    causal_feedback: usize,
    refresh_stages: [usize; 3],
    refresh_duplicate_refusals: [usize; 3],
    refresh_control_release: usize,
    refresh_rearms: usize,
    supersession_cuts: [usize; SupersessionCut::COUNT],
    deadline_cuts: [usize; DeadlineCut::COUNT],
    forward_event_cuts: [[usize; UiBoundary::COUNT]; ForwardEventKind::COUNT],
    exhaustions: [usize; ExhaustionKind::COUNT],
    geometry_retries: usize,
    close_kinds: [usize; CloseKind::COUNT],
}

impl Coverage {
    fn add(&mut self, other: Self) {
        add_array(&mut self.admission_kinds, other.admission_kinds);
        add_array(&mut self.full_refusals, other.full_refusals);
        add_array(&mut self.occupancy_values, other.occupancy_values);
        self.fifo_drains += other.fifo_drains;
        self.causal_empty += other.causal_empty;
        self.causal_feedback += other.causal_feedback;
        add_array(&mut self.refresh_stages, other.refresh_stages);
        add_array(
            &mut self.refresh_duplicate_refusals,
            other.refresh_duplicate_refusals,
        );
        self.refresh_control_release += other.refresh_control_release;
        self.refresh_rearms += other.refresh_rearms;
        add_array(&mut self.supersession_cuts, other.supersession_cuts);
        add_array(&mut self.deadline_cuts, other.deadline_cuts);
        for (total, counts) in self
            .forward_event_cuts
            .iter_mut()
            .zip(other.forward_event_cuts)
        {
            add_array(total, counts);
        }
        add_array(&mut self.exhaustions, other.exhaustions);
        self.geometry_retries += other.geometry_retries;
        add_array(&mut self.close_kinds, other.close_kinds);
    }
}

#[test]
fn generated_mailbox_traces_replay_and_cover_fifo_barrier_boundaries() {
    if let Ok(encoded) = std::env::var("ORCHBEE_NATIVE_MAILBOX_REPLAY") {
        let actions =
            deserialize(&encoded).unwrap_or_else(|error| panic!("invalid replay: {error}"));
        replay(&actions).unwrap_or_else(|error| panic!("replay failed: {error:?}"));
        return;
    }
    let mut total = Coverage::default();
    let mut randomized = Coverage::default();
    for seed in SEEDS {
        let mut actions = mandatory_actions();
        let mandatory_count = actions.len();
        let mut random = seed;
        for _ in 0..GENERATED_STEPS_PER_SEED {
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            actions.push(random_action(random));
        }
        match replay(&actions) {
            Ok(coverage) => {
                eprintln!(
                    "native-mailbox-trace seed={seed:#x} max_steps={} coverage={coverage:?} replay={}",
                    actions.len(),
                    serialize(&actions)
                );
                total.add(coverage);
                randomized.add(
                    replay(&actions[mandatory_count..])
                        .unwrap_or_else(|error| panic!("random replay failed: {error:?}")),
                );
            }
            Err(error) => {
                let minimized = shrink_failure(&actions, &error);
                let minimized_error = replay(&minimized)
                    .expect_err("a minimized failure must retain the original failure");
                assert!(error.same_fingerprint(&minimized_error));
                panic!(
                    "seed {seed:#x} failed: {error:?}; minimized={minimized_error:?}; \
                     rerun with ORCHBEE_NATIVE_MAILBOX_REPLAY={}",
                    serialize(&minimized)
                );
            }
        }
    }

    assert!(
        total
            .admission_kinds
            .iter()
            .all(|count| *count >= SEEDS.len())
    );
    assert!(
        total
            .full_refusals
            .iter()
            .all(|count| *count >= SEEDS.len())
    );
    assert!(
        total
            .occupancy_values
            .iter()
            .all(|count| *count >= SEEDS.len())
    );
    assert!(total.fifo_drains >= SEEDS.len());
    assert!(total.causal_empty >= SEEDS.len());
    assert!(total.causal_feedback >= SEEDS.len());
    assert!(
        total
            .refresh_stages
            .iter()
            .all(|count| *count >= SEEDS.len())
    );
    assert!(
        total
            .refresh_duplicate_refusals
            .iter()
            .all(|count| *count >= SEEDS.len())
    );
    assert!(total.refresh_control_release >= SEEDS.len());
    assert!(total.refresh_rearms >= SEEDS.len());
    assert!(
        total
            .supersession_cuts
            .iter()
            .all(|count| *count >= SEEDS.len())
    );
    assert!(
        total
            .deadline_cuts
            .iter()
            .all(|count| *count >= SEEDS.len())
    );
    assert!(
        total
            .forward_event_cuts
            .iter()
            .flatten()
            .all(|count| *count >= SEEDS.len())
    );
    assert!(total.exhaustions.iter().all(|count| *count >= SEEDS.len()));
    assert!(total.geometry_retries >= SEEDS.len());
    assert!(total.close_kinds.iter().all(|count| *count >= SEEDS.len()));
    assert!(randomized.admission_kinds.iter().all(|count| *count > 0));
    assert!(randomized.fifo_drains > 0);
    assert!(randomized.causal_empty > 0 && randomized.causal_feedback > 0);
    assert!(randomized.refresh_stages.iter().all(|count| *count > 0));
    assert!(
        randomized
            .refresh_duplicate_refusals
            .iter()
            .all(|count| *count > 0)
    );
    assert!(randomized.refresh_control_release > 0);
    assert!(randomized.refresh_rearms > 0);
    assert!(randomized.supersession_cuts.iter().all(|count| *count > 0));
    assert!(randomized.deadline_cuts.iter().all(|count| *count > 0));
    assert!(
        randomized
            .forward_event_cuts
            .iter()
            .flatten()
            .all(|count| *count > 0)
    );
    assert!(randomized.exhaustions.iter().all(|count| *count > 0));
    assert!(randomized.geometry_retries > 0);
    assert!(randomized.close_kinds.iter().all(|count| *count > 0));
}

fn mandatory_actions() -> Vec<TraceAction> {
    let mut actions = Vec::new();
    for occupancy in 0..=COMMAND_CAPACITY {
        actions.push(TraceAction::Admission {
            occupancy: u8::try_from(occupancy).unwrap(),
            kind: AdmissionKind::Input,
        });
    }
    for kind in [
        AdmissionKind::Switch,
        AdmissionKind::Create,
        AdmissionKind::Refresh,
    ] {
        actions.push(TraceAction::Admission {
            occupancy: u8::try_from(COMMAND_CAPACITY).unwrap(),
            kind,
        });
    }
    actions.extend([
        TraceAction::FifoDrain {
            occupancy: u8::try_from(COMMAND_CAPACITY).unwrap(),
        },
        TraceAction::PreMarkerPoll {
            occupancy: 0,
            feedback: false,
        },
        TraceAction::PreMarkerPoll {
            occupancy: 6,
            feedback: true,
        },
        TraceAction::RefreshUiFence,
        TraceAction::GeometryRetry { columns: 91 },
    ]);
    for cut in [
        SupersessionCut::BeforePollHandoff,
        SupersessionCut::AfterPollHandoff,
        SupersessionCut::AfterFrameDelivery,
        SupersessionCut::AfterEventTake,
        SupersessionCut::AfterDraw,
        SupersessionCut::AfterCausalTake,
    ] {
        actions.push(TraceAction::SwitchSupersession(cut));
    }
    for cut in [
        DeadlineCut::ForwardPlacement,
        DeadlineCut::ForwardTake,
        DeadlineCut::ForwardReply,
        DeadlineCut::TotalPlacement,
        DeadlineCut::TotalTake,
        DeadlineCut::TotalReply,
        DeadlineCut::DepositedReplyWins,
    ] {
        actions.push(TraceAction::Deadline(cut));
    }
    for event in [
        ForwardEventKind::SetupConfigured,
        ForwardEventKind::FeedbackAccepted,
    ] {
        for boundary in [UiBoundary::Placement, UiBoundary::Take, UiBoundary::Reply] {
            actions.push(TraceAction::ForwardBarrier { event, boundary });
        }
    }
    for kind in [
        ExhaustionKind::CommandSerial,
        ExhaustionKind::AttachmentGeneration,
        ExhaustionKind::RefreshGeneration,
        ExhaustionKind::RequestIdentity,
    ] {
        actions.push(TraceAction::Exhaustion(kind));
    }
    for kind in [
        CloseKind::Deadline,
        CloseKind::OutcomeUnknown,
        CloseKind::Protocol,
        CloseKind::Transport,
    ] {
        actions.push(TraceAction::CloseFirstCause(kind));
    }
    actions
}

fn random_action(random: u64) -> TraceAction {
    let byte = (random >> 48) as u8;
    match (random >> 32) % 10 {
        0 => TraceAction::Admission {
            occupancy: byte % 9,
            kind: admission_kind(byte),
        },
        1 => TraceAction::FifoDrain {
            occupancy: 1 + byte % 8,
        },
        2 => TraceAction::PreMarkerPoll {
            occupancy: byte % 7,
            feedback: byte & 1 != 0,
        },
        3 => TraceAction::RefreshUiFence,
        4 => TraceAction::SwitchSupersession(supersession_cut(byte)),
        5 => TraceAction::Deadline(deadline_cut(byte)),
        6 => TraceAction::ForwardBarrier {
            event: if byte & 1 == 0 {
                ForwardEventKind::SetupConfigured
            } else {
                ForwardEventKind::FeedbackAccepted
            },
            boundary: ui_boundary(byte),
        },
        7 => TraceAction::Exhaustion(exhaustion_kind(byte)),
        8 => TraceAction::GeometryRetry { columns: byte },
        _ => TraceAction::CloseFirstCause(close_kind(byte)),
    }
}

fn replay(actions: &[TraceAction]) -> Result<Coverage, TraceFailure> {
    let mut coverage = Coverage::default();
    for (step, action) in actions.iter().copied().enumerate() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            apply(action, &mut coverage)
        }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                return Err(TraceFailure {
                    step,
                    action,
                    oracle: error.to_owned(),
                });
            }
            Err(payload) => {
                let oracle = if let Some(message) = payload.downcast_ref::<&str>() {
                    (*message).to_owned()
                } else if let Some(message) = payload.downcast_ref::<String>() {
                    message.clone()
                } else {
                    "non-string panic".to_owned()
                };
                return Err(TraceFailure {
                    step,
                    action,
                    oracle: format!("panic: {oracle}"),
                });
            }
        }
    }
    Ok(coverage)
}

fn apply(action: TraceAction, coverage: &mut Coverage) -> Result<(), &'static str> {
    match action {
        TraceAction::Admission { occupancy, kind } => {
            exercise_admission(occupancy, kind, coverage)?
        }
        TraceAction::FifoDrain { occupancy } => exercise_fifo(occupancy, coverage)?,
        TraceAction::PreMarkerPoll {
            occupancy,
            feedback,
        } => exercise_pre_marker_poll(occupancy, feedback, coverage)?,
        TraceAction::RefreshUiFence => exercise_refresh_ui_fence(coverage)?,
        TraceAction::SwitchSupersession(cut) => exercise_supersession(cut, coverage)?,
        TraceAction::Deadline(cut) => exercise_deadline(cut, coverage)?,
        TraceAction::ForwardBarrier { event, boundary } => {
            exercise_forward_barrier(event, boundary, coverage)?
        }
        TraceAction::Exhaustion(kind) => exercise_exhaustion(kind, coverage)?,
        TraceAction::GeometryRetry { columns } => exercise_geometry_retry(columns, coverage)?,
        TraceAction::CloseFirstCause(kind) => exercise_close(kind, coverage)?,
    }
    Ok(())
}

fn exercise_admission(
    occupancy: u8,
    kind: AdmissionKind,
    coverage: &mut Coverage,
) -> Result<(), &'static str> {
    let count = usize::from(occupancy).min(COMMAND_CAPACITY);
    let base = Instant::now();
    let mailbox = bootstrapped_mailbox(1, 10);
    for index in 0..count {
        mailbox
            .try_enqueue(input(&mailbox, u8::try_from(index).unwrap()))
            .map_err(|_| "prefill")?;
    }
    let before = cursor_snapshot(&mailbox);
    let refused_as_full = match kind {
        AdmissionKind::Input => admission_was_full(mailbox.try_enqueue(input(&mailbox, 0xff)))?,
        AdmissionKind::Switch => admission_was_full(enqueue_switch(&mailbox, base))?,
        AdmissionKind::Create => {
            admission_was_full(mailbox.try_enqueue_create(|generation, request_id| {
                Ok::<_, ()>(create_marker(base, generation, request_id))
            }))?
        }
        AdmissionKind::Refresh => admission_was_full(
            mailbox.try_enqueue_refresh(base + Duration::from_secs(50), |_| Ok::<_, ()>(())),
        )?,
    };
    coverage.admission_kinds[kind.index()] += 1;
    coverage.occupancy_values[count] += 1;
    if count == COMMAND_CAPACITY {
        if !refused_as_full {
            return Err("full admission was not refused");
        }
        if cursor_snapshot(&mailbox) != before {
            return Err("full admission burned a cursor");
        }
        coverage.full_refusals[kind.index()] += 1;
    } else {
        if refused_as_full {
            return Err("non-full admission was refused");
        }
        let after = cursor_snapshot(&mailbox);
        if after.0 != before.0.checked_add(1).ok_or("serial overflow")? {
            return Err("command serial did not advance once");
        }
        match kind {
            AdmissionKind::Input => {
                if after.1 != before.1 || after.2 != before.2 || after.3 != before.3 {
                    return Err("input changed a reserved cursor");
                }
            }
            AdmissionKind::Switch => {
                if after.1 != before.1 + 1 || after.2 != before.2 || after.3 != before.3 {
                    return Err("switch cursor change");
                }
            }
            AdmissionKind::Create => {
                if after.1 != before.1 + 1 || after.2 != before.2 || after.3 != before.3 + 1 {
                    return Err("create cursor change");
                }
            }
            AdmissionKind::Refresh => {
                if after.1 != before.1 || after.2 != before.2 + 1 || after.3 != before.3 {
                    return Err("refresh cursor change");
                }
            }
        }
    }
    Ok(())
}

fn exercise_fifo(occupancy: u8, coverage: &mut Coverage) -> Result<(), &'static str> {
    let count = usize::from(occupancy).clamp(1, COMMAND_CAPACITY);
    let mailbox = bootstrapped_mailbox(1, 10);
    for index in 0..count {
        mailbox
            .try_enqueue(input(&mailbox, u8::try_from(index).unwrap()))
            .map_err(|_| "fifo enqueue")?;
    }
    for index in 0..count {
        let lease = mailbox
            .try_recv()
            .map_err(|_| "fifo recv")?
            .ok_or("fifo empty")?;
        let QueuedCommand::Input(tagged) = lease.record() else {
            return Err("fifo kind");
        };
        if tagged.value().as_bytes() != [u8::try_from(index).unwrap()] {
            return Err("fifo order");
        }
        mailbox.finish_lease(lease).map_err(|_| "fifo finish")?;
    }
    if mailbox.try_recv().map_err(|_| "fifo final recv")?.is_some() {
        return Err("fifo retained a record");
    }
    coverage.fifo_drains += 1;
    Ok(())
}

fn exercise_pre_marker_poll(
    occupancy: u8,
    feedback: bool,
    coverage: &mut Coverage,
) -> Result<(), &'static str> {
    let base = Instant::now();
    let cutoff = base + Duration::from_secs(45);
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 10);
    mailbox.try_enqueue(poll(&mailbox)).map_err(|_| "poll")?;
    let controls = usize::from(occupancy).min(COMMAND_CAPACITY - 2);
    for index in 0..controls {
        mailbox
            .try_enqueue(input(&mailbox, u8::try_from(index).unwrap()))
            .map_err(|_| "control")?;
    }
    mailbox
        .try_enqueue_create(|generation, request_id| {
            Ok::<_, ()>(create_marker(base, generation, request_id))
        })
        .map_err(|_| "create marker")?;
    let poll_lease = mailbox
        .try_recv()
        .map_err(|_| "recv")?
        .ok_or("missing poll")?;
    let poll_key = poll_lease.poll_key().ok_or("poll key")?;
    while mailbox.lock().queue.len() < COMMAND_CAPACITY {
        mailbox
            .try_enqueue(input(&mailbox, 0xee))
            .map_err(|_| "fill")?;
    }
    mailbox
        .publish_poll_result_before_using(
            poll_key.clone(),
            feedback.then(|| vec![1]),
            cutoff,
            &clock,
        )
        .map_err(|_| "publish poll")?;
    let event = mailbox
        .take_event_before_using(&clock)
        .map_err(|_| "take poll")?
        .ok_or("missing event")?;
    if !matches!(event.into_event(), WorkerEvent::PollResult { .. }) {
        return Err("poll event kind");
    }
    let settlement = if feedback {
        CausalSettlement::Feedback(TerminalViewerAction::Poll {
            attachment_id: fixture_viewer_id(),
        })
    } else {
        CausalSettlement::NoFeedback
    };
    mailbox
        .deposit_causal(&poll_key, settlement.clone())
        .map_err(|_| "deposit causal")?;
    if mailbox
        .take_causal_before_using(&poll_key, cutoff, &clock)
        .map_err(|_| "take causal")?
        != Some(settlement)
    {
        return Err("causal identity");
    }
    mailbox
        .finish_lease(poll_lease)
        .map_err(|_| "finish poll")?;
    for _ in 0..controls {
        let lease = mailbox
            .try_recv()
            .map_err(|_| "control recv")?
            .ok_or("control missing")?;
        if !matches!(lease.record(), QueuedCommand::Input(_)) {
            return Err("control did not precede marker");
        }
        mailbox.finish_lease(lease).map_err(|_| "control finish")?;
    }
    let marker = mailbox
        .try_recv()
        .map_err(|_| "marker recv")?
        .ok_or("marker missing")?;
    if !matches!(marker.record(), QueuedCommand::Create(_)) {
        return Err("create overtook causal drain");
    }
    mailbox.finish_lease(marker).map_err(|_| "marker finish")?;
    if feedback {
        coverage.causal_feedback += 1;
    } else {
        coverage.causal_empty += 1;
    }
    Ok(())
}

fn exercise_refresh_ui_fence(coverage: &mut Coverage) -> Result<(), &'static str> {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(50);
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 10);
    let generation = mailbox
        .try_enqueue_refresh(deadline, |_| Ok::<_, ()>(()))
        .map_err(|_| "refresh enqueue")?;
    if mailbox.pending_refresh_stage() != Some((generation, PendingRefreshStage::Queued)) {
        return Err("refresh queued stage");
    }
    coverage.refresh_stages[0] += 1;
    let before_duplicate = cursor_snapshot(&mailbox);
    if !matches!(
        mailbox.try_enqueue_refresh(deadline, |_| Ok::<_, ()>(())),
        Err(AdmissionError::Full)
    ) || cursor_snapshot(&mailbox) != before_duplicate
    {
        return Err("second queued refresh");
    }
    coverage.refresh_duplicate_refusals[0] += 1;
    mailbox
        .try_enqueue(input(&mailbox, 9))
        .map_err(|_| "queued control")?;
    let lease = mailbox
        .try_recv()
        .map_err(|_| "refresh recv")?
        .ok_or("refresh missing")?;
    if mailbox
        .mark_refresh_in_flight(generation - 1)
        .map_err(|_| "stale refresh")?
        != MutationResult::Stale
    {
        return Err("old refresh generation applied");
    }
    if mailbox
        .mark_refresh_in_flight(generation)
        .map_err(|_| "refresh in flight")?
        != MutationResult::Applied
        || mailbox.pending_refresh_stage() != Some((generation, PendingRefreshStage::InFlight))
    {
        return Err("refresh in-flight stage");
    }
    coverage.refresh_stages[1] += 1;
    let before_duplicate = cursor_snapshot(&mailbox);
    if !matches!(
        mailbox.try_enqueue_refresh(deadline, |_| Ok::<_, ()>(())),
        Err(AdmissionError::Full)
    ) || cursor_snapshot(&mailbox) != before_duplicate
    {
        return Err("second in-flight refresh");
    }
    coverage.refresh_duplicate_refusals[1] += 1;
    let wait = mailbox
        .publish_event_before_using(
            WorkerEvent::RefreshSnapshotResult {
                generation,
                outcome: WorkbenchSnapshotOutcome::Denied {},
            },
            EventDeadline::PlacementOnly(deadline),
            &clock,
        )
        .map_err(|_| "refresh event")?;
    if mailbox.pending_refresh_stage() != Some((generation, PendingRefreshStage::AwaitingUi)) {
        return Err("refresh awaiting UI stage");
    }
    coverage.refresh_stages[2] += 1;
    let before_duplicate = cursor_snapshot(&mailbox);
    if !matches!(
        mailbox.try_enqueue_refresh(deadline, |_| Ok::<_, ()>(())),
        Err(AdmissionError::Full)
    ) || cursor_snapshot(&mailbox) != before_duplicate
    {
        return Err("second awaiting-UI refresh");
    }
    coverage.refresh_duplicate_refusals[2] += 1;
    if mailbox.try_recv().map_err(|_| "blocked recv")?.is_some() {
        return Err("control ran before UI settlement");
    }
    let taken = mailbox
        .take_event_before_using(&clock)
        .map_err(|_| "refresh take")?
        .ok_or("refresh event missing")?;
    if taken.wait_key() != Some(&wait) {
        return Err("refresh wait key");
    }
    let desired = key(1, 1);
    mailbox
        .deposit_ui_reply_before_using(&wait, &clock, |_| {
            Ok(UiReply::Refresh(RefreshSettlement::new(
                RefreshDecision::Continue,
                pane_key(&desired),
            )))
        })
        .map_err(|_| "refresh reply")?;
    if mailbox.pending_refresh_stage().is_some() {
        return Err("refresh remained pending");
    }
    if mailbox.try_recv().map_err(|_| "reply recv")?.is_some() {
        return Err("control ran before worker consumed reply");
    }
    if !matches!(
        mailbox.take_deposited_reply(&wait),
        Ok(Some(UiReply::Refresh(_)))
    ) {
        return Err("refresh reply missing");
    }
    mailbox.finish_lease(lease).map_err(|_| "refresh finish")?;
    let control = mailbox
        .try_recv()
        .map_err(|_| "control recv")?
        .ok_or("control missing")?;
    if !matches!(control.record(), QueuedCommand::Input(_)) {
        return Err("post-refresh control kind");
    }
    mailbox
        .finish_lease(control)
        .map_err(|_| "control finish")?;
    let before_rearm = cursor_snapshot(&mailbox);
    let next_generation = mailbox
        .try_enqueue_refresh(deadline, |_| Ok::<_, ()>(()))
        .map_err(|_| "refresh rearm")?;
    let after_rearm = cursor_snapshot(&mailbox);
    if next_generation != generation + 1
        || after_rearm.0 != before_rearm.0 + 1
        || after_rearm.1 != before_rearm.1
        || after_rearm.2 != before_rearm.2 + 1
        || after_rearm.3 != before_rearm.3
        || mailbox.pending_refresh_stage() != Some((next_generation, PendingRefreshStage::Queued))
    {
        return Err("refresh did not rearm with fresh generation");
    }
    coverage.refresh_control_release += 1;
    coverage.refresh_rearms += 1;
    Ok(())
}

fn exercise_supersession(
    cut: SupersessionCut,
    coverage: &mut Coverage,
) -> Result<(), &'static str> {
    let base = Instant::now();
    let cutoff = base + Duration::from_secs(50);
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 10);
    mailbox.try_enqueue(poll(&mailbox)).map_err(|_| "poll")?;
    if cut == SupersessionCut::BeforePollHandoff {
        enqueue_switch(&mailbox, base).map_err(|_| "switch")?;
    }
    let lease = mailbox
        .try_recv()
        .map_err(|_| "poll recv")?
        .ok_or("poll missing")?;
    let poll_key = lease.poll_key().ok_or("poll key")?;
    if cut == SupersessionCut::AfterPollHandoff {
        enqueue_switch(&mailbox, base).map_err(|_| "switch")?;
        if mailbox
            .publish_poll_result_before_using(poll_key.clone(), Some(vec![1]), cutoff, &clock)
            .map_err(|_| "stale publish")?
            != MutationResult::Stale
        {
            return Err("superseded poll published");
        }
    } else if cut != SupersessionCut::BeforePollHandoff {
        mailbox
            .publish_poll_result_before_using(poll_key.clone(), Some(vec![1]), cutoff, &clock)
            .map_err(|_| "publish")?;
        if cut == SupersessionCut::AfterFrameDelivery {
            enqueue_switch(&mailbox, base).map_err(|_| "switch")?;
            if mailbox
                .take_event_before_using(&clock)
                .map_err(|_| "take")?
                .is_some()
            {
                return Err("stale delivered frame survived");
            }
        } else {
            mailbox
                .take_event_before_using(&clock)
                .map_err(|_| "event take")?
                .ok_or("event missing")?;
            if cut == SupersessionCut::AfterEventTake {
                enqueue_switch(&mailbox, base).map_err(|_| "switch")?;
                if mailbox
                    .deposit_causal(&poll_key, CausalSettlement::NoFeedback)
                    .map_err(|_| "late deposit")?
                    != MutationResult::Stale
                {
                    return Err("late event settlement mutated");
                }
            } else {
                mailbox
                    .deposit_causal(
                        &poll_key,
                        CausalSettlement::Feedback(TerminalViewerAction::Poll {
                            attachment_id: fixture_viewer_id(),
                        }),
                    )
                    .map_err(|_| "causal deposit")?;
                if cut == SupersessionCut::AfterDraw {
                    enqueue_switch(&mailbox, base).map_err(|_| "switch")?;
                    if mailbox
                        .take_causal_before_using(&poll_key, cutoff, &clock)
                        .map_err(|_| "causal take")?
                        .is_some()
                    {
                        return Err("stale drawn frame survived switch");
                    }
                } else {
                    if mailbox
                        .take_causal_before_using(&poll_key, cutoff, &clock)
                        .map_err(|_| "causal take")?
                        .is_none()
                    {
                        return Err("exact causal settlement missing");
                    }
                    enqueue_switch(&mailbox, base).map_err(|_| "switch")?;
                }
            }
        }
    }
    let desired = mailbox.execution_fence(&lease).desired().key().cloned();
    if desired.as_ref().is_none_or(|key| key.generation() != 2) {
        return Err("new desired generation missing");
    }
    if cut == SupersessionCut::BeforePollHandoff
        && mailbox
            .publish_poll_result_before_using(poll_key.clone(), Some(vec![1]), cutoff, &clock)
            .map_err(|_| "before handoff stale")?
            != MutationResult::Stale
    {
        return Err("pre-handoff stale Poll published");
    }
    if mailbox
        .take_causal_before_using(&poll_key, cutoff, &clock)
        .map_err(|_| "final causal")?
        .is_some()
    {
        return Err("superseded causal remained");
    }
    mailbox.finish_lease(lease).map_err(|_| "finish")?;
    coverage.supersession_cuts[cut.index()] += 1;
    Ok(())
}

fn exercise_deadline(cut: DeadlineCut, coverage: &mut Coverage) -> Result<(), &'static str> {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(1);
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 10);
    let total = matches!(
        cut,
        DeadlineCut::TotalPlacement | DeadlineCut::TotalTake | DeadlineCut::TotalReply
    );
    let event_deadline = if total {
        EventDeadline::TotalCleanup(deadline)
    } else {
        EventDeadline::Forward(deadline)
    };
    if matches!(
        cut,
        DeadlineCut::ForwardPlacement | DeadlineCut::TotalPlacement
    ) {
        clock.set(deadline);
        expect_expiry(
            mailbox.publish_event_before_using(cleanup_event(key(1, 1)), event_deadline, &clock),
            total,
        )?;
    } else {
        let wait = mailbox
            .publish_event_before_using(cleanup_event(key(1, 1)), event_deadline, &clock)
            .map_err(|_| "publish")?;
        if matches!(cut, DeadlineCut::ForwardTake | DeadlineCut::TotalTake) {
            clock.set(deadline);
            expect_expiry(mailbox.take_event_before_using(&clock).map(|_| ()), total)?;
            if !total {
                expect_forward_expiry(
                    mailbox
                        .await_ui_reply_before_using(&wait, deadline, &clock)
                        .map(|_| ()),
                )?;
            }
        } else {
            mailbox
                .take_event_before_using(&clock)
                .map_err(|_| "take")?
                .ok_or("event")?;
            if cut == DeadlineCut::DepositedReplyWins {
                clock.set(deadline - Duration::from_nanos(1));
                mailbox
                    .deposit_ui_reply_before_using(&wait, &clock, |_| {
                        Ok(UiReply::CleanupApplied(CleanupContinuation::Stable))
                    })
                    .map_err(|_| "deposit")?;
                clock.set(deadline + Duration::from_secs(1));
                if !matches!(
                    mailbox.take_deposited_reply(&wait),
                    Ok(Some(UiReply::CleanupApplied(CleanupContinuation::Stable)))
                ) {
                    return Err("deposited reply did not win");
                }
            } else {
                clock.set(deadline);
                expect_expiry(
                    mailbox
                        .deposit_ui_reply_before_using(&wait, &clock, |_| {
                            Ok(UiReply::CleanupApplied(CleanupContinuation::Stable))
                        })
                        .map(|_| ()),
                    total,
                )?;
                if !total {
                    expect_forward_expiry(
                        mailbox
                            .await_ui_reply_before_using(&wait, deadline, &clock)
                            .map(|_| ()),
                    )?;
                }
            }
        }
    }
    if total {
        if mailbox.is_closed() != Some(CarrierFailure::Deadline) {
            return Err("total expiry did not retire");
        }
    } else if mailbox.is_closed().is_some() {
        return Err("forward expiry retired mailbox");
    }
    coverage.deadline_cuts[cut.index()] += 1;
    Ok(())
}

fn exercise_forward_barrier(
    event: ForwardEventKind,
    boundary: UiBoundary,
    coverage: &mut Coverage,
) -> Result<(), &'static str> {
    let base = Instant::now();
    let ordinary_cutoff = base + Duration::from_secs(50);
    let barrier_cutoff = OperationDeadlines::from_sample(base)
        .ok_or("deadlines")?
        .forward_cutoff();
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 10);
    mailbox
        .try_enqueue(input(&mailbox, 1))
        .map_err(|_| "input")?;
    let lease = mailbox.try_recv().map_err(|_| "recv")?.ok_or("lease")?;
    let worker_event = || match event {
        ForwardEventKind::SetupConfigured => WorkerEvent::SetupConfigured {
            key: pane_ref(&key(1, 1)),
            mode: ControllerMode::ReadOnly,
            applied_geometry: None,
        },
        ForwardEventKind::FeedbackAccepted => WorkerEvent::FeedbackAccepted {
            key: pane_ref(&key(1, 1)),
            frame: FrameIdentity::new(7),
        },
    };
    let mut wait = None;
    if boundary == UiBoundary::Placement {
        enqueue_create(&mailbox, base)?;
        clock.set(barrier_cutoff);
        expect_forward_expiry(mailbox.publish_event_before_using(
            worker_event(),
            EventDeadline::Forward(ordinary_cutoff),
            &clock,
        ))?;
    } else {
        let published = mailbox
            .publish_event_before_using(
                worker_event(),
                EventDeadline::Forward(ordinary_cutoff),
                &clock,
            )
            .map_err(|_| "publish")?;
        wait = Some(published.clone());
        enqueue_create(&mailbox, base)?;
        clock.set(barrier_cutoff);
        if boundary == UiBoundary::Take {
            expect_forward_expiry(mailbox.take_event_before_using(&clock).map(|_| ()))?;
        } else {
            mailbox
                .take_event_before_using(&FakeClock::new(base))
                .map_err(|_| "take before cutoff")?
                .ok_or("event")?;
            let reply = match event {
                ForwardEventKind::SetupConfigured => {
                    UiReply::SetupConfigurationSettled(SetupEventDisposition::Applied)
                }
                ForwardEventKind::FeedbackAccepted => {
                    UiReply::FeedbackSettled(SetupEventDisposition::Applied)
                }
            };
            expect_forward_expiry(
                mailbox
                    .deposit_ui_reply_before_using(&published, &clock, |_| Ok(reply))
                    .map(|_| ()),
            )?;
        }
    }
    if let Some(wait) = wait {
        expect_forward_expiry(
            mailbox
                .await_ui_reply_before_using(&wait, barrier_cutoff, &clock)
                .map(|_| ()),
        )?;
    }
    if mailbox.is_closed().is_some() || mailbox.cells() != ("empty", "empty") {
        return Err("forward clamp damaged mailbox or retained cell");
    }
    mailbox.finish_lease(lease).map_err(|_| "finish")?;
    coverage.forward_event_cuts[event.index()][boundary.index()] += 1;
    Ok(())
}

fn exercise_exhaustion(kind: ExhaustionKind, coverage: &mut Coverage) -> Result<(), &'static str> {
    let base = Instant::now();
    match kind {
        ExhaustionKind::CommandSerial => {
            let mailbox = bootstrapped_mailbox(1, 10);
            mailbox.lock().next_command_serial = u64::MAX;
            let before = cursor_snapshot(&mailbox);
            if !matches!(
                mailbox.try_enqueue(input(&mailbox, 1)),
                Err(AdmissionError::Closed(MailboxClosed {
                    failure: CarrierFailure::IdentityExhausted
                }))
            ) || cursor_snapshot(&mailbox) != before
            {
                return Err("command serial exhaustion");
            }
        }
        ExhaustionKind::AttachmentGeneration => {
            let mailbox = bootstrapped_mailbox(u64::MAX, 10);
            let before = cursor_snapshot(&mailbox);
            if !matches!(
                mailbox.try_enqueue_create(|generation, request_id| {
                    Ok::<_, ()>(create_marker(base, generation, request_id))
                }),
                Err(AdmissionError::AttachmentGenerationExhausted)
            ) || cursor_snapshot(&mailbox) != before
                || mailbox.is_closed().is_some()
            {
                return Err("attachment generation exhaustion");
            }
        }
        ExhaustionKind::RefreshGeneration => {
            let mailbox = bootstrapped_mailbox(1, 10);
            mailbox.lock().refresh_generation = u64::MAX;
            let before = cursor_snapshot(&mailbox);
            if !matches!(
                mailbox.try_enqueue_refresh(base + Duration::from_secs(1), |_| Ok::<_, ()>(())),
                Err(AdmissionError::RefreshGenerationExhausted)
            ) || cursor_snapshot(&mailbox) != before
                || mailbox.is_closed().is_some()
            {
                return Err("refresh generation exhaustion");
            }
        }
        ExhaustionKind::RequestIdentity => {
            let mailbox = bootstrapped_mailbox(1, u128::MAX);
            let before = cursor_snapshot(&mailbox);
            if !matches!(
                mailbox.try_enqueue_create(|generation, request_id| {
                    Ok::<_, ()>(create_marker(base, generation, request_id))
                }),
                Err(AdmissionError::Closed(MailboxClosed {
                    failure: CarrierFailure::IdentityExhausted
                }))
            ) || cursor_snapshot(&mailbox) != before
            {
                return Err("request identity exhaustion");
            }
        }
    }
    coverage.exhaustions[kind.index()] += 1;
    Ok(())
}

fn exercise_geometry_retry(columns: u8, coverage: &mut Coverage) -> Result<(), &'static str> {
    let mailbox = bootstrapped_mailbox(1, 10);
    let observed = geometry(40 + u16::from(columns % 81));
    for index in 0..COMMAND_CAPACITY {
        mailbox
            .try_enqueue(input(&mailbox, u8::try_from(index).unwrap()))
            .map_err(|_| "geometry prefill")?;
    }
    mailbox.observe_geometry(observed).map_err(|_| "observe")?;
    if !matches!(
        mailbox.try_enqueue(QueuedCommand::Resize(Tagged::new(
            pane_ref(&key(1, 1)),
            observed
        ))),
        Err(AdmissionError::Full)
    ) {
        return Err("full geometry admission");
    }
    let first = mailbox
        .try_recv()
        .map_err(|_| "first recv")?
        .ok_or("first")?;
    mailbox.finish_lease(first).map_err(|_| "first finish")?;
    mailbox
        .observe_geometry(observed)
        .map_err(|_| "repeat observe")?;
    mailbox
        .try_enqueue(QueuedCommand::Resize(Tagged::new(
            pane_ref(&key(1, 1)),
            observed,
        )))
        .map_err(|_| "retry resize")?;
    for _ in 1..COMMAND_CAPACITY {
        let lease = mailbox
            .try_recv()
            .map_err(|_| "drain")?
            .ok_or("queued input")?;
        if !matches!(lease.record(), QueuedCommand::Input(_)) {
            return Err("resize overtook input");
        }
        mailbox.finish_lease(lease).map_err(|_| "drain finish")?;
    }
    let resize = mailbox
        .try_recv()
        .map_err(|_| "resize recv")?
        .ok_or("resize missing")?;
    if !matches!(
        resize.record(),
        QueuedCommand::Resize(tagged) if *tagged.value() == observed
    ) {
        return Err("same-size retry missing");
    }
    mailbox.finish_lease(resize).map_err(|_| "resize finish")?;
    if mailbox.latest_geometry().map_err(|_| "latest")? != observed {
        return Err("latest geometry changed");
    }
    coverage.geometry_retries += 1;
    Ok(())
}

fn exercise_close(kind: CloseKind, coverage: &mut Coverage) -> Result<(), &'static str> {
    let mailbox = bootstrapped_mailbox(1, 10);
    let first = kind.failure();
    if mailbox.close(first).failure() != first
        || mailbox.close(CarrierFailure::WorkerExited).failure() != first
        || mailbox.is_closed() != Some(first)
        || mailbox.cells() != ("empty", "empty")
    {
        return Err("first close cause changed");
    }
    coverage.close_kinds[kind.index()] += 1;
    Ok(())
}

fn enqueue_switch(
    mailbox: &CommandMailbox,
    sampled_at: Instant,
) -> Result<u64, AdmissionError<()>> {
    let mut state = ready_state(2);
    mailbox.try_enqueue_switch(|generation| {
        let target_key = AttachmentKey::new(generation, state.tab(1).unwrap().tab_id().clone());
        let target = state
            .target_for_switch(
                1,
                target_key,
                OperationDeadlines::from_sample(sampled_at).unwrap(),
            )
            .unwrap();
        Ok::<_, ()>(SwitchMarker::new(target))
    })
}

fn enqueue_create(mailbox: &CommandMailbox, sampled_at: Instant) -> Result<(), &'static str> {
    mailbox
        .try_enqueue_create(|generation, request_id| {
            Ok::<_, ()>(create_marker(sampled_at, generation, request_id))
        })
        .map(|_| ())
        .map_err(|_| "create admission")
}

fn cursor_snapshot(mailbox: &CommandMailbox) -> (u64, u64, u64, u128) {
    let state = mailbox.lock();
    (
        state.next_command_serial,
        state.attachment_generation,
        state.refresh_generation,
        state.identity_cursor,
    )
}

fn expect_expiry<T>(result: Result<T, EventError>, total: bool) -> Result<(), &'static str> {
    match (result, total) {
        (Err(EventError::ForwardExpired), false)
        | (
            Err(EventError::Closed(MailboxClosed {
                failure: CarrierFailure::Deadline,
            })),
            true,
        ) => Ok(()),
        _ => Err("wrong deadline result"),
    }
}

fn expect_forward_expiry<T>(result: Result<T, EventError>) -> Result<(), &'static str> {
    if matches!(result, Err(EventError::ForwardExpired)) {
        Ok(())
    } else {
        Err("forward boundary did not expire")
    }
}

fn admission_was_full<T, BuildError>(
    result: Result<T, AdmissionError<BuildError>>,
) -> Result<bool, &'static str> {
    match result {
        Ok(_) => Ok(false),
        Err(AdmissionError::Full) => Ok(true),
        Err(
            AdmissionError::Closed(_)
            | AdmissionError::BootstrapPending
            | AdmissionError::Busy
            | AdmissionError::StaleAttachment
            | AdmissionError::AttachmentGenerationExhausted
            | AdmissionError::RefreshGenerationExhausted
            | AdmissionError::Build(_),
        ) => Err("unexpected admission result"),
    }
}

fn admission_kind(value: u8) -> AdmissionKind {
    match value % 4 {
        0 => AdmissionKind::Input,
        1 => AdmissionKind::Switch,
        2 => AdmissionKind::Create,
        _ => AdmissionKind::Refresh,
    }
}

fn supersession_cut(value: u8) -> SupersessionCut {
    match value % 6 {
        0 => SupersessionCut::BeforePollHandoff,
        1 => SupersessionCut::AfterPollHandoff,
        2 => SupersessionCut::AfterFrameDelivery,
        3 => SupersessionCut::AfterEventTake,
        4 => SupersessionCut::AfterDraw,
        _ => SupersessionCut::AfterCausalTake,
    }
}

fn deadline_cut(value: u8) -> DeadlineCut {
    match value % 7 {
        0 => DeadlineCut::ForwardPlacement,
        1 => DeadlineCut::ForwardTake,
        2 => DeadlineCut::ForwardReply,
        3 => DeadlineCut::TotalPlacement,
        4 => DeadlineCut::TotalTake,
        5 => DeadlineCut::TotalReply,
        _ => DeadlineCut::DepositedReplyWins,
    }
}

fn ui_boundary(value: u8) -> UiBoundary {
    match value % 3 {
        0 => UiBoundary::Placement,
        1 => UiBoundary::Take,
        _ => UiBoundary::Reply,
    }
}

fn exhaustion_kind(value: u8) -> ExhaustionKind {
    match value % 4 {
        0 => ExhaustionKind::CommandSerial,
        1 => ExhaustionKind::AttachmentGeneration,
        2 => ExhaustionKind::RefreshGeneration,
        _ => ExhaustionKind::RequestIdentity,
    }
}

fn close_kind(value: u8) -> CloseKind {
    match value % 4 {
        0 => CloseKind::Deadline,
        1 => CloseKind::OutcomeUnknown,
        2 => CloseKind::Protocol,
        _ => CloseKind::Transport,
    }
}

fn add_array<const N: usize>(total: &mut [usize; N], counts: [usize; N]) {
    for (total, count) in total.iter_mut().zip(counts) {
        *total += count;
    }
}

fn shrink_failure(actions: &[TraceAction], failure: &TraceFailure) -> Vec<TraceAction> {
    // Every action deliberately constructs its own fixture, so isolate the exact failing
    // scenario before shrinking only its non-semantic numeric values.
    let mut action = actions[failure.step];
    for candidate in shrink_values(action) {
        let Err(candidate_failure) = replay(&[candidate]) else {
            continue;
        };
        if failure.same_fingerprint(&candidate_failure) {
            action = candidate;
        }
    }
    vec![action]
}

fn shrink_values(action: TraceAction) -> Vec<TraceAction> {
    match action {
        TraceAction::Admission { occupancy, kind } => vec![
            TraceAction::Admission {
                occupancy: occupancy / 2,
                kind,
            },
            TraceAction::Admission { occupancy: 0, kind },
        ],
        TraceAction::FifoDrain { occupancy } => vec![TraceAction::FifoDrain {
            occupancy: 1.max(occupancy / 2),
        }],
        TraceAction::PreMarkerPoll {
            occupancy,
            feedback,
        } => vec![
            TraceAction::PreMarkerPoll {
                occupancy: occupancy / 2,
                feedback,
            },
            TraceAction::PreMarkerPoll {
                occupancy: 0,
                feedback,
            },
        ],
        TraceAction::GeometryRetry { columns } => vec![TraceAction::GeometryRetry {
            columns: columns / 2,
        }],
        TraceAction::RefreshUiFence
        | TraceAction::SwitchSupersession(_)
        | TraceAction::Deadline(_)
        | TraceAction::ForwardBarrier { .. }
        | TraceAction::Exhaustion(_)
        | TraceAction::CloseFirstCause(_) => Vec::new(),
    }
}

fn serialize(actions: &[TraceAction]) -> String {
    actions
        .iter()
        .map(|action| match action {
            TraceAction::Admission { occupancy, kind } => {
                format!("a:{occupancy}:{}", kind.index())
            }
            TraceAction::FifoDrain { occupancy } => format!("f:{occupancy}"),
            TraceAction::PreMarkerPoll {
                occupancy,
                feedback,
            } => format!("p:{occupancy}:{}", u8::from(*feedback)),
            TraceAction::RefreshUiFence => "r".to_owned(),
            TraceAction::SwitchSupersession(cut) => format!("s:{}", cut.index()),
            TraceAction::Deadline(cut) => format!("d:{}", cut.index()),
            TraceAction::ForwardBarrier { event, boundary } => {
                format!("b:{}:{}", event.index(), boundary.index())
            }
            TraceAction::Exhaustion(kind) => format!("e:{}", kind.index()),
            TraceAction::GeometryRetry { columns } => format!("g:{columns}"),
            TraceAction::CloseFirstCause(kind) => format!("c:{}", kind.index()),
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn deserialize(encoded: &str) -> Result<Vec<TraceAction>, &'static str> {
    if encoded.is_empty() {
        return Err("empty trace");
    }
    encoded.split(',').map(deserialize_action).collect()
}

fn deserialize_action(encoded: &str) -> Result<TraceAction, &'static str> {
    let fields = encoded.split(':').collect::<Vec<_>>();
    let number = |index: usize| -> Result<u8, &'static str> {
        fields
            .get(index)
            .ok_or("missing field")?
            .parse()
            .map_err(|_| "invalid number")
    };
    match fields.as_slice() {
        ["a", _, _] if number(2)? < AdmissionKind::COUNT as u8 => Ok(TraceAction::Admission {
            occupancy: number(1)?,
            kind: admission_kind(number(2)?),
        }),
        ["f", _] => Ok(TraceAction::FifoDrain {
            occupancy: number(1)?,
        }),
        ["p", _, _] if number(2)? <= 1 => Ok(TraceAction::PreMarkerPoll {
            occupancy: number(1)?,
            feedback: number(2)? == 1,
        }),
        ["r"] => Ok(TraceAction::RefreshUiFence),
        ["s", _] if number(1)? < SupersessionCut::COUNT as u8 => Ok(
            TraceAction::SwitchSupersession(supersession_cut(number(1)?)),
        ),
        ["d", _] if number(1)? < DeadlineCut::COUNT as u8 => {
            Ok(TraceAction::Deadline(deadline_cut(number(1)?)))
        }
        ["b", _, _]
            if number(1)? < ForwardEventKind::COUNT as u8
                && number(2)? < UiBoundary::COUNT as u8 =>
        {
            Ok(TraceAction::ForwardBarrier {
                event: if number(1)? == 0 {
                    ForwardEventKind::SetupConfigured
                } else {
                    ForwardEventKind::FeedbackAccepted
                },
                boundary: ui_boundary(number(2)?),
            })
        }
        ["e", _] if number(1)? < ExhaustionKind::COUNT as u8 => {
            Ok(TraceAction::Exhaustion(exhaustion_kind(number(1)?)))
        }
        ["g", _] => Ok(TraceAction::GeometryRetry {
            columns: number(1)?,
        }),
        ["c", _] if number(1)? < CloseKind::COUNT as u8 => {
            Ok(TraceAction::CloseFirstCause(close_kind(number(1)?)))
        }
        _ => Err("invalid action"),
    }
}

#[test]
fn serialized_mailbox_trace_round_trips_for_direct_replay() {
    let mut actions = mandatory_actions();
    actions.extend(SEEDS.into_iter().map(random_action));
    assert_eq!(deserialize(&serialize(&actions)), Ok(actions.clone()));
    assert!(deserialize("").is_err());
    assert!(deserialize("s:99").is_err());
    for action in actions {
        assert!(
            shrink_values(action)
                .into_iter()
                .all(|candidate| candidate.class() == action.class())
        );
    }
}
