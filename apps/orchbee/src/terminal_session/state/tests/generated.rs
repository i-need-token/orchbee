use super::*;

const SEEDS: [u64; 8] = [
    0x41c6_4e6d,
    0x9e37_79b9,
    0xd1b5_4a32,
    0x94d0_49bb,
    0x8538_ebad,
    0xda94_2042,
    0xa409_3822,
    0x1319_8a2e,
];
const GENERATED_STEPS_PER_SEED: usize = 96;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CreatedClass {
    Pending,
    Started,
    Unavailable,
    Failed,
    OutcomeUnknown,
}

impl CreatedClass {
    const COUNT: usize = 5;

    const fn index(self) -> usize {
        match self {
            Self::Pending => 0,
            Self::Started => 1,
            Self::Unavailable => 2,
            Self::Failed => 3,
            Self::OutcomeUnknown => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReadyFrameCut {
    StagedAwaitingFeedback,
    ProjectionProofInvalidated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CleanupClass {
    CurrentInstalled,
    CurrentBeforeUiInstall,
    Superseded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RetirementClass {
    Deadline,
    Ambiguous,
    AuthorityDivergence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TraceAction {
    ObserveGeometry(u8),
    SelectPosition(u8),
    SwitchReady {
        target_hint: u8,
        generation_step: u8,
    },
    SwitchUnavailable {
        target_hint: u8,
        generation_step: u8,
    },
    CreatePrecommit {
        generation_step: u8,
    },
    CreateCommitted {
        class: CreatedClass,
        generation_step: u8,
    },
    RejectReusedCommittedTuple,
    RefreshAppend {
        generation_step: u8,
    },
    RefreshStaleThenCurrent {
        generation_step: u8,
    },
    RefreshFailure,
    HistoricalReadyCreate(ReadyFrameCut),
    ControllerBusyReadOnly,
    Cleanup(CleanupClass),
    Retire(RetirementClass),
    ClearNotice,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActionClass {
    ObserveGeometry,
    SelectPosition,
    SwitchReady,
    SwitchUnavailable,
    CreatePrecommit,
    CreateCommitted(CreatedClass),
    RejectReusedCommittedTuple,
    RefreshAppend,
    RefreshStaleThenCurrent,
    RefreshFailure,
    HistoricalReadyCreate(ReadyFrameCut),
    ControllerBusyReadOnly,
    Cleanup(CleanupClass),
    Retire(RetirementClass),
    ClearNotice,
}

impl TraceAction {
    const fn class(self) -> ActionClass {
        match self {
            Self::ObserveGeometry(_) => ActionClass::ObserveGeometry,
            Self::SelectPosition(_) => ActionClass::SelectPosition,
            Self::SwitchReady { .. } => ActionClass::SwitchReady,
            Self::SwitchUnavailable { .. } => ActionClass::SwitchUnavailable,
            Self::CreatePrecommit { .. } => ActionClass::CreatePrecommit,
            Self::CreateCommitted { class, .. } => ActionClass::CreateCommitted(class),
            Self::RejectReusedCommittedTuple => ActionClass::RejectReusedCommittedTuple,
            Self::RefreshAppend { .. } => ActionClass::RefreshAppend,
            Self::RefreshStaleThenCurrent { .. } => ActionClass::RefreshStaleThenCurrent,
            Self::RefreshFailure => ActionClass::RefreshFailure,
            Self::HistoricalReadyCreate(cut) => ActionClass::HistoricalReadyCreate(cut),
            Self::ControllerBusyReadOnly => ActionClass::ControllerBusyReadOnly,
            Self::Cleanup(class) => ActionClass::Cleanup(class),
            Self::Retire(class) => ActionClass::Retire(class),
            Self::ClearNotice => ActionClass::ClearNotice,
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
    creates: usize,
    created_classes: [usize; CreatedClass::COUNT],
    precommit_restores: usize,
    repeated_create_sequences: usize,
    reused_committed_tuple_rejections: usize,
    refresh_successes: usize,
    refresh_failures: usize,
    stale_refreshes: usize,
    retryable_settlements: usize,
    switches: usize,
    valid_indices: usize,
    out_of_range_indices: usize,
    staged_frame_creates: usize,
    invalidated_frame_creates: usize,
    controller_busy_read_only: usize,
    cleanup_classes: [usize; 3],
    retirements: [usize; 3],
}

impl Coverage {
    fn add(&mut self, other: Self) {
        self.creates += other.creates;
        for (total, count) in self.created_classes.iter_mut().zip(other.created_classes) {
            *total += count;
        }
        self.precommit_restores += other.precommit_restores;
        self.repeated_create_sequences += other.repeated_create_sequences;
        self.reused_committed_tuple_rejections += other.reused_committed_tuple_rejections;
        self.refresh_successes += other.refresh_successes;
        self.refresh_failures += other.refresh_failures;
        self.stale_refreshes += other.stale_refreshes;
        self.retryable_settlements += other.retryable_settlements;
        self.switches += other.switches;
        self.valid_indices += other.valid_indices;
        self.out_of_range_indices += other.out_of_range_indices;
        self.staged_frame_creates += other.staged_frame_creates;
        self.invalidated_frame_creates += other.invalidated_frame_creates;
        self.controller_busy_read_only += other.controller_busy_read_only;
        for (total, count) in self.cleanup_classes.iter_mut().zip(other.cleanup_classes) {
            *total += count;
        }
        for (total, count) in self.retirements.iter_mut().zip(other.retirements) {
            *total += count;
        }
    }
}

struct TraceState {
    session: NativeSessionState,
    generation: u64,
    request: u128,
    frame: u64,
    refresh: u64,
    geometry_columns: u16,
    human_creates: usize,
    committed_tabs: Vec<WorkbenchTabId>,
    coverage: Coverage,
}

#[test]
fn stateful_generated_traces_replay_and_cover_barrier_reentry() {
    if let Ok(encoded) = std::env::var("ORCHBEE_NATIVE_STATE_REPLAY") {
        let actions =
            deserialize(&encoded).unwrap_or_else(|error| panic!("invalid replay: {error}"));
        replay(&actions).unwrap_or_else(|error| panic!("replay failed: {error:?}"));
        return;
    }
    let mut total = Coverage::default();
    let mut randomized = Coverage::default();
    for seed in SEEDS {
        let mut random = seed;
        let mut actions = vec![
            TraceAction::CreatePrecommit { generation_step: 1 },
            TraceAction::CreateCommitted {
                class: CreatedClass::Pending,
                generation_step: 1,
            },
            TraceAction::CreateCommitted {
                class: CreatedClass::Started,
                generation_step: 1,
            },
            TraceAction::CreateCommitted {
                class: CreatedClass::Unavailable,
                generation_step: 2,
            },
            TraceAction::CreateCommitted {
                class: CreatedClass::Failed,
                generation_step: 1,
            },
            TraceAction::CreateCommitted {
                class: CreatedClass::OutcomeUnknown,
                generation_step: 3,
            },
            TraceAction::RejectReusedCommittedTuple,
            TraceAction::SelectPosition(0),
            TraceAction::SelectPosition(1),
            TraceAction::SelectPosition(u8::MAX),
            TraceAction::SwitchUnavailable {
                target_hint: 1,
                generation_step: 1,
            },
            TraceAction::SwitchReady {
                target_hint: 0,
                generation_step: 1,
            },
            TraceAction::RefreshAppend { generation_step: 1 },
            TraceAction::RefreshStaleThenCurrent { generation_step: 2 },
            TraceAction::RefreshFailure,
            TraceAction::HistoricalReadyCreate(ReadyFrameCut::StagedAwaitingFeedback),
            TraceAction::HistoricalReadyCreate(ReadyFrameCut::ProjectionProofInvalidated),
            TraceAction::ControllerBusyReadOnly,
            TraceAction::Cleanup(CleanupClass::CurrentInstalled),
            TraceAction::Cleanup(CleanupClass::CurrentBeforeUiInstall),
            TraceAction::Cleanup(CleanupClass::Superseded),
            TraceAction::Retire(RetirementClass::Deadline),
            TraceAction::Retire(RetirementClass::Ambiguous),
            TraceAction::Retire(RetirementClass::AuthorityDivergence),
        ];
        let mandatory_count = actions.len();
        for _ in 0..GENERATED_STEPS_PER_SEED {
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let byte = (random >> 48) as u8;
            actions.push(match (random >> 32) % 16 {
                0 => TraceAction::ObserveGeometry((random >> 48) as u8),
                1 => TraceAction::SelectPosition(byte),
                2 => TraceAction::SwitchReady {
                    target_hint: byte,
                    generation_step: byte,
                },
                3 => TraceAction::SwitchUnavailable {
                    target_hint: byte,
                    generation_step: byte,
                },
                4 => TraceAction::CreatePrecommit {
                    generation_step: byte,
                },
                5 => TraceAction::CreateCommitted {
                    class: match byte % 5 {
                        0 => CreatedClass::Pending,
                        1 => CreatedClass::Started,
                        2 => CreatedClass::Unavailable,
                        3 => CreatedClass::Failed,
                        _ => CreatedClass::OutcomeUnknown,
                    },
                    generation_step: byte,
                },
                6 => TraceAction::RefreshAppend {
                    generation_step: byte,
                },
                7 => TraceAction::RefreshStaleThenCurrent {
                    generation_step: byte,
                },
                8 => TraceAction::RefreshFailure,
                9 => TraceAction::HistoricalReadyCreate(if byte & 1 == 0 {
                    ReadyFrameCut::StagedAwaitingFeedback
                } else {
                    ReadyFrameCut::ProjectionProofInvalidated
                }),
                10 => TraceAction::ControllerBusyReadOnly,
                11 => TraceAction::Cleanup(match byte % 3 {
                    0 => CleanupClass::CurrentInstalled,
                    1 => CleanupClass::CurrentBeforeUiInstall,
                    _ => CleanupClass::Superseded,
                }),
                12 => TraceAction::Retire(match byte % 3 {
                    0 => RetirementClass::Deadline,
                    1 => RetirementClass::Ambiguous,
                    _ => RetirementClass::AuthorityDivergence,
                }),
                13 => TraceAction::RejectReusedCommittedTuple,
                _ => TraceAction::ClearNotice,
            });
        }
        match replay(&actions) {
            Ok(coverage) => {
                eprintln!(
                    "native-state-trace seed={seed:#x} max_steps={} coverage={coverage:?} replay={}",
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
                let minimized = shrink_failure(actions, &error);
                let minimized_error = replay(&minimized)
                    .expect_err("a minimized failure must retain the original failure");
                assert!(error.same_fingerprint(&minimized_error));
                panic!(
                    "seed {seed:#x} failed: {error:?}; minimized={minimized_error:?}; \
                     rerun with ORCHBEE_NATIVE_STATE_REPLAY={}",
                    serialize(&minimized)
                );
            }
        }
    }
    assert!(total.creates >= SEEDS.len() * 2);
    assert!(
        total
            .created_classes
            .iter()
            .all(|count| *count >= SEEDS.len())
    );
    assert!(total.precommit_restores >= SEEDS.len());
    assert!(total.repeated_create_sequences >= SEEDS.len());
    assert!(total.reused_committed_tuple_rejections >= SEEDS.len());
    assert!(total.refresh_successes >= SEEDS.len());
    assert!(total.refresh_failures >= SEEDS.len());
    assert!(total.stale_refreshes >= SEEDS.len());
    assert!(total.retryable_settlements >= SEEDS.len());
    assert!(total.switches >= SEEDS.len());
    assert!(total.valid_indices >= SEEDS.len());
    assert!(total.out_of_range_indices >= SEEDS.len());
    assert!(total.staged_frame_creates >= SEEDS.len());
    assert!(total.invalidated_frame_creates >= SEEDS.len());
    assert!(total.controller_busy_read_only >= SEEDS.len());
    assert!(
        total
            .cleanup_classes
            .iter()
            .all(|count| *count >= SEEDS.len())
    );
    assert!(total.retirements.iter().all(|count| *count >= SEEDS.len()));
    assert!(randomized.creates > 0);
    assert!(randomized.precommit_restores > 0);
    assert!(randomized.refresh_successes > 0);
    assert!(randomized.refresh_failures > 0);
    assert!(randomized.stale_refreshes > 0);
    assert!(randomized.switches > 0);
    assert!(randomized.valid_indices > 0);
    assert!(randomized.out_of_range_indices > 0);
    assert!(randomized.staged_frame_creates > 0);
    assert!(randomized.invalidated_frame_creates > 0);
    assert!(randomized.controller_busy_read_only > 0);
    assert!(randomized.cleanup_classes.iter().all(|count| *count > 0));
    assert!(randomized.retirements.iter().all(|count| *count > 0));
}

fn replay(actions: &[TraceAction]) -> Result<Coverage, TraceFailure> {
    let mut trace = TraceState {
        session: ready(3),
        generation: 1,
        request: 10_000,
        frame: 1,
        refresh: 0,
        geometry_columns: 80,
        human_creates: 0,
        committed_tabs: Vec::new(),
        coverage: Coverage::default(),
    };
    for (step, action) in actions.iter().copied().enumerate() {
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| apply(&mut trace, action)));
        let oracle = match result {
            Ok(Ok(())) if trace.session.invariant_holds() => continue,
            Ok(Ok(())) => "invariant failed".to_owned(),
            Ok(Err(error)) => error.to_owned(),
            Err(payload) => {
                let message = if let Some(message) = payload.downcast_ref::<&str>() {
                    (*message).to_owned()
                } else if let Some(message) = payload.downcast_ref::<String>() {
                    message.clone()
                } else {
                    "non-string panic".to_owned()
                };
                format!("panic: {message}")
            }
        };
        return Err(TraceFailure {
            step,
            action,
            oracle,
        });
    }
    Ok(trace.coverage)
}

fn apply(trace: &mut TraceState, action: TraceAction) -> Result<(), &'static str> {
    match action {
        TraceAction::ObserveGeometry(delta) => {
            trace.geometry_columns = 40 + u16::from(delta % 81);
            trace
                .session
                .observe_geometry(geometry(trace.geometry_columns));
        }
        TraceAction::SelectPosition(position) => exercise_selection(trace, position),
        TraceAction::SwitchReady {
            target_hint,
            generation_step,
        } => switch_to_attachable(trace, target_hint, generation_step)?,
        TraceAction::SwitchUnavailable {
            target_hint,
            generation_step,
        } => settle_switch_unavailable(trace, target_hint, generation_step)?,
        TraceAction::CreatePrecommit { generation_step } => {
            create_precommit(trace, generation_step)?
        }
        TraceAction::CreateCommitted {
            class,
            generation_step,
        } => create_committed_then_reselect(trace, class, generation_step)?,
        TraceAction::RejectReusedCommittedTuple => reject_reused_committed_tuple(trace)?,
        TraceAction::RefreshAppend { generation_step } => refresh(trace, true, generation_step)?,
        TraceAction::RefreshStaleThenCurrent { generation_step } => {
            refresh_stale_then_current(trace, generation_step)?
        }
        TraceAction::RefreshFailure => refresh(trace, false, 1)?,
        TraceAction::HistoricalReadyCreate(cut) => historical_ready_create(trace, cut)?,
        TraceAction::ControllerBusyReadOnly => exercise_controller_busy_read_only(trace)?,
        TraceAction::Cleanup(class) => exercise_cleanup(trace, class)?,
        TraceAction::Retire(class) => exercise_retirement(trace, class)?,
        TraceAction::ClearNotice => trace.session.clear_notice(),
    }
    Ok(())
}

fn advance_generation(trace: &mut TraceState, step: u8) -> Result<u64, &'static str> {
    trace.generation = trace
        .generation
        .checked_add(u64::from(step.max(1)))
        .ok_or("generation")?;
    Ok(trace.generation)
}

fn switch_to_attachable(
    trace: &mut TraceState,
    target_hint: u8,
    generation_step: u8,
) -> Result<(), &'static str> {
    if !matches!(
        trace.session.phase_kind(),
        SessionPhaseKind::Ready
            | SessionPhaseKind::SelectedWithoutAttachment
            | SessionPhaseKind::RetryableAttachUnavailable
    ) {
        return Ok(());
    }
    let active_index = trace.session.active().ok_or("missing active")?.index();
    let mut candidates: Vec<_> = (0..trace.session.cache().tabs().len())
        .filter(|index| {
            *index != active_index
                && trace
                    .session
                    .provenance(*index)
                    .is_some_and(CreateTerminalProvenance::attachable)
        })
        .collect();
    if candidates.is_empty()
        && trace
            .session
            .provenance(active_index)
            .is_some_and(CreateTerminalProvenance::attachable)
    {
        candidates.push(active_index);
    }
    let target_index = *candidates
        .get(usize::from(target_hint) % candidates.len().max(1))
        .ok_or("no attachable target")?;
    let generation = advance_generation(trace, generation_step)?;
    let key = AttachmentKey::new(
        generation,
        trace
            .session
            .tab(target_index)
            .ok_or("target")?
            .tab_id()
            .clone(),
    );
    let target = trace
        .session
        .target_for_switch(
            target_index,
            key.clone(),
            OperationDeadlines::from_sample(Instant::now()).ok_or("deadline")?,
        )
        .ok_or("switch target")?;
    if !trace.session.begin_switch(target) {
        return Err("begin switch");
    }
    let key = prepare_and_install_switch_candidate(&mut trace.session, &key)
        .map_err(|_| "Switch candidate install")?;
    trace.generation = key.generation();
    if !trace.session.install_projection(&key)
        || !trace.session.record_setup_configuration(
            &key,
            controlling(),
            Some(trace.session.latest_geometry()),
        )
    {
        return Err("setup");
    }
    trace.frame = trace.frame.checked_add(1).ok_or("frame")?;
    let frame = FrameIdentity::new(trace.frame);
    if !trace.session.stage_drawn_frame(&key, frame)
        || !trace.session.mark_feedback_accepted(&key, frame)
        || trace
            .session
            .settle_ready_candidate(
                &key,
                frame,
                controlling(),
                Some(trace.session.latest_geometry()),
            )
            .map_err(|_| "ready")?
            != ReadyDecision::Ready
    {
        return Err("ready");
    }
    trace.coverage.switches += 1;
    Ok(())
}

fn settle_switch_unavailable(
    trace: &mut TraceState,
    target_hint: u8,
    generation_step: u8,
) -> Result<(), &'static str> {
    if trace.session.phase_kind() != SessionPhaseKind::Ready {
        return Ok(());
    }
    let active = trace.session.active().ok_or("active")?.index();
    let candidates = (0..trace.session.cache().tabs().len())
        .filter(|index| {
            *index != active
                && trace
                    .session
                    .provenance(*index)
                    .is_some_and(CreateTerminalProvenance::attachable)
        })
        .collect::<Vec<_>>();
    let target_index = *candidates
        .get(usize::from(target_hint) % candidates.len().max(1))
        .ok_or("no unavailable target")?;
    let provenance = trace
        .session
        .provenance(target_index)
        .ok_or("unavailable provenance")?
        .clone();
    let generation = advance_generation(trace, generation_step)?;
    let key = AttachmentKey::new(
        generation,
        trace
            .session
            .tab(target_index)
            .ok_or("target")?
            .tab_id()
            .clone(),
    );
    let target = trace
        .session
        .target_for_switch(
            target_index,
            key.clone(),
            OperationDeadlines::from_sample(Instant::now()).ok_or("deadline")?,
        )
        .ok_or("target")?;
    if !trace.session.begin_switch(target) {
        return Err("begin unavailable switch");
    }
    let key = prepare_and_install_switch_candidate(&mut trace.session, &key)
        .map_err(|_| "unavailable Switch candidate install")?;
    trace.generation = key.generation();
    if trace
        .session
        .settle_unattached_setup(&key, target_index, &provenance)
        != Some(CleanupContinuation::Stable)
    {
        return Err("unavailable settlement");
    }
    trace.coverage.retryable_settlements += 1;
    Ok(())
}

fn note_human_create(trace: &mut TraceState) {
    trace.human_creates += 1;
    if trace.human_creates == 2 {
        trace.coverage.repeated_create_sequences += 1;
    }
}

fn create_precommit(trace: &mut TraceState, generation_step: u8) -> Result<(), &'static str> {
    if trace.session.phase_kind() != SessionPhaseKind::Ready {
        return Ok(());
    }
    trace.session.clear_notice();
    let generation = advance_generation(trace, generation_step)?;
    trace.request = trace.request.checked_add(1).ok_or("request")?;
    let label = (trace.session.cache().tabs().len() + 1).to_string();
    let operation = create_operation(&trace.session, generation, trace.request, &label);
    let key = operation.key().clone();
    let applied = trace
        .session
        .last_accepted_geometry(trace.session.desired().unwrap());
    if !trace.session.begin_create(operation) || !trace.session.restore_precommit(&key, applied) {
        return Err("precommit");
    }
    note_human_create(trace);
    trace.coverage.creates += 1;
    trace.coverage.precommit_restores += 1;
    Ok(())
}

fn create_committed_then_reselect(
    trace: &mut TraceState,
    class: CreatedClass,
    generation_step: u8,
) -> Result<(), &'static str> {
    if trace.session.phase_kind() != SessionPhaseKind::Ready
        || trace.session.cache().tabs().len() >= 64
    {
        return Ok(());
    }
    trace.session.clear_notice();
    let generation = advance_generation(trace, generation_step)?;
    trace.request = trace.request.checked_add(1).ok_or("request")?;
    let prior_provenance = provenance_snapshot(&trace.session)?;
    let new_count = trace.session.cache().tabs().len() + 1;
    let label = new_count.to_string();
    let operation = create_operation(&trace.session, generation, trace.request, &label);
    let operation_key = operation.key().clone();
    if !trace.session.begin_create(operation) {
        return Err("pending begin");
    }
    let candidate = snapshot(
        new_count,
        trace
            .session
            .cache()
            .revision()
            .value()
            .checked_add(1)
            .ok_or("revision")?,
        Some(&label),
    );
    let created = candidate.terminal_contexts[0]
        .tabs
        .last()
        .ok_or("created")?;
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &created.root
    else {
        return Err("panel");
    };
    let created_tab_id = created.tab_id.clone();
    if trace.committed_tabs.contains(&created_tab_id) {
        return Err("committed Tab tuple reused");
    }
    let (commit, expected_provenance) = match class {
        CreatedClass::Pending => (
            WorkbenchTabCreateCommit::Pending {
                revision: candidate.revision,
                terminal_context_id: candidate.terminal_contexts[0].terminal_context_id.clone(),
                tab_id: created.tab_id.clone(),
                panel_id: panel_id.clone(),
                presentation_id: primary_presentation.presentation_id.clone(),
            },
            CreateTerminalProvenance::Pending,
        ),
        CreatedClass::Started => (
            WorkbenchTabCreateCommit::Created {
                revision: candidate.revision,
                terminal_context_id: candidate.terminal_contexts[0].terminal_context_id.clone(),
                tab_id: created.tab_id.clone(),
                panel_id: panel_id.clone(),
                presentation_id: primary_presentation.presentation_id.clone(),
                terminal_outcome: WorkbenchTerminalOutcome::Started {
                    geometry: workbench_geometry(trace.geometry_columns),
                },
            },
            CreateTerminalProvenance::Started,
        ),
        CreatedClass::Unavailable => (
            WorkbenchTabCreateCommit::Created {
                revision: candidate.revision,
                terminal_context_id: candidate.terminal_contexts[0].terminal_context_id.clone(),
                tab_id: created.tab_id.clone(),
                panel_id: panel_id.clone(),
                presentation_id: primary_presentation.presentation_id.clone(),
                terminal_outcome: WorkbenchTerminalOutcome::Unavailable {
                    reason: WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
                },
            },
            CreateTerminalProvenance::Unavailable(
                WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
            ),
        ),
        CreatedClass::Failed => (
            WorkbenchTabCreateCommit::Created {
                revision: candidate.revision,
                terminal_context_id: candidate.terminal_contexts[0].terminal_context_id.clone(),
                tab_id: created.tab_id.clone(),
                panel_id: panel_id.clone(),
                presentation_id: primary_presentation.presentation_id.clone(),
                terminal_outcome: WorkbenchTerminalOutcome::Failed {
                    reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
                },
            },
            CreateTerminalProvenance::Failed(WorkbenchTerminalFailureReason::TerminalCreateFailed),
        ),
        CreatedClass::OutcomeUnknown => (
            WorkbenchTabCreateCommit::Created {
                revision: candidate.revision,
                terminal_context_id: candidate.terminal_contexts[0].terminal_context_id.clone(),
                tab_id: created.tab_id.clone(),
                panel_id: panel_id.clone(),
                presentation_id: primary_presentation.presentation_id.clone(),
                terminal_outcome: WorkbenchTerminalOutcome::OutcomeUnknown {},
            },
            CreateTerminalProvenance::OutcomeUnknown,
        ),
    };
    if !trace.session.retain_create_commit(&operation_key, commit)
        || !trace.session.publish_old_detached(&operation_key)
    {
        return Err("commit retention/detach");
    }
    let decision = trace.session.settle_create_snapshot(
        &operation_key,
        WorkbenchSnapshotOutcome::Available {
            snapshot: candidate,
        },
    );
    require_provenance_prefix(&trace.session, &prior_provenance)?;
    if trace.session.provenance(new_count - 1) != Some(&expected_provenance) {
        return Err("created provenance");
    }
    trace.committed_tabs.push(created_tab_id);
    note_human_create(trace);
    trace.coverage.creates += 1;
    trace.coverage.created_classes[class.index()] += 1;

    if class == CreatedClass::Started {
        if !matches!(decision, CreateProjectionDecision::Attach(_)) {
            return Err("started create did not attach");
        }
        let key = trace.session.desired().unwrap().clone();
        if !trace.session.install_projection(&key)
            || !trace.session.record_setup_configuration(
                &key,
                controlling(),
                Some(trace.session.latest_geometry()),
            )
        {
            return Err("created setup");
        }
        trace.frame = trace.frame.checked_add(1).ok_or("frame")?;
        let frame = FrameIdentity::new(trace.frame);
        if !trace.session.stage_drawn_frame(&key, frame)
            || !trace.session.mark_feedback_accepted(&key, frame)
            || trace
                .session
                .settle_ready_candidate(
                    &key,
                    frame,
                    controlling(),
                    Some(trace.session.latest_geometry()),
                )
                .map_err(|_| "created ready")?
                != ReadyDecision::Ready
        {
            return Err("created ready");
        }
        return Ok(());
    }
    if decision != CreateProjectionDecision::SettledWithoutAttachment {
        return Err("nonattachable create attached");
    }
    switch_to_attachable(trace, 0, 1)
}

fn reject_reused_committed_tuple(trace: &mut TraceState) -> Result<(), &'static str> {
    let mut session = ready(1);
    let first_operation = create_operation(&session, 2, 20, "2");
    let first_key = first_operation.key().clone();
    let first_candidate = snapshot(2, 2, Some("2"));
    let first_created = first_candidate.terminal_contexts[0]
        .tabs
        .last()
        .ok_or("first created")?;
    let PanelNode::Panel {
        panel_id: first_panel_id,
        primary_presentation: first_presentation,
        ..
    } = &first_created.root
    else {
        return Err("first created panel");
    };
    if !session.begin_create(first_operation)
        || !session.retain_create_commit(
            &first_key,
            WorkbenchTabCreateCommit::Created {
                revision: first_candidate.revision,
                terminal_context_id: first_candidate.terminal_contexts[0]
                    .terminal_context_id
                    .clone(),
                tab_id: first_created.tab_id.clone(),
                panel_id: first_panel_id.clone(),
                presentation_id: first_presentation.presentation_id.clone(),
                terminal_outcome: WorkbenchTerminalOutcome::Started {
                    geometry: workbench_geometry(80),
                },
            },
        )
        || !session.publish_old_detached(&first_key)
        || !matches!(
            session.settle_create_snapshot(
                &first_key,
                WorkbenchSnapshotOutcome::Available {
                    snapshot: first_candidate,
                },
            ),
            CreateProjectionDecision::Attach(_)
        )
    {
        return Err("first committed create");
    }
    let first_created_key = session.desired().unwrap().clone();
    let first_frame = FrameIdentity::new(2);
    if !session.install_projection(&first_created_key)
        || !session.record_setup_configuration(
            &first_created_key,
            controlling(),
            Some(session.latest_geometry()),
        )
        || !session.stage_drawn_frame(&first_created_key, first_frame)
        || !session.mark_feedback_accepted(&first_created_key, first_frame)
        || !session.publish_ready(
            &first_created_key,
            first_frame,
            controlling(),
            Some(session.latest_geometry()),
        )
    {
        return Err("first committed ready");
    }

    let preserved_provenance = provenance_snapshot(&session)?;
    let reused = session.tab(1).ok_or("reused member")?.clone();
    let second_operation = create_operation(&session, 3, 21, "3");
    let second_key = second_operation.key().clone();
    if !session.begin_create(second_operation)
        || !session.retain_create_commit(
            &second_key,
            WorkbenchTabCreateCommit::Pending {
                revision: WorkbenchRevision::new(3),
                terminal_context_id: session.cache().terminal_context_id().clone(),
                tab_id: reused.tab_id().clone(),
                panel_id: reused.panel_id().clone(),
                presentation_id: reused.presentation_id().clone(),
            },
        )
        || !session.publish_old_detached(&second_key)
    {
        return Err("reused committed retention");
    }
    if session.settle_create_snapshot(
        &second_key,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(3, 3, Some("3")),
        },
    ) != CreateProjectionDecision::Retire
        || session.phase_kind() != SessionPhaseKind::Retired
        || session.retired_failure() != Some(CarrierFailure::AuthorityDivergence)
        || session.committed_target().is_none()
    {
        return Err("reused committed tuple accepted");
    }
    require_provenance_prefix(&session, &preserved_provenance)?;
    trace.coverage.reused_committed_tuple_rejections += 1;
    Ok(())
}

fn provenance_snapshot(
    session: &NativeSessionState,
) -> Result<Vec<CreateTerminalProvenance>, &'static str> {
    (0..session.cache().tabs().len())
        .map(|index| session.provenance(index).cloned().ok_or("provenance"))
        .collect()
}

fn require_provenance_prefix(
    session: &NativeSessionState,
    expected: &[CreateTerminalProvenance],
) -> Result<(), &'static str> {
    if expected
        .iter()
        .enumerate()
        .all(|(index, provenance)| session.provenance(index) == Some(provenance))
    {
        Ok(())
    } else {
        Err("prior provenance changed")
    }
}

fn refresh(
    trace: &mut TraceState,
    available: bool,
    generation_step: u8,
) -> Result<(), &'static str> {
    if !trace.session.phase_allows_refresh() {
        return Ok(());
    }
    trace.refresh = trace
        .refresh
        .checked_add(u64::from(generation_step.max(1)))
        .ok_or("refresh")?;
    if !trace.session.start_pending_refresh(trace.refresh) {
        return Err("refresh admission");
    }
    let old_count = trace.session.cache().tabs().len();
    let prior_provenance = provenance_snapshot(&trace.session)?;
    let active = trace.session.active().cloned();
    let desired = trace.session.desired().unwrap().clone();
    let attached = trace.session.attached_key().cloned();
    let projection = trace.session.projection_key().cloned();
    let mode = trace.session.controller_mode().cloned();
    let outcome = if available {
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(
                old_count + 1,
                trace
                    .session
                    .cache()
                    .revision()
                    .value()
                    .checked_add(1)
                    .ok_or("revision")?,
                None,
            ),
        }
    } else {
        WorkbenchSnapshotOutcome::Denied {}
    };
    let prepared = trace.session.prepare_refresh(trace.refresh, outcome);
    let decision = trace.session.apply_prepared_refresh(prepared, None);
    if decision != RefreshDecision::Continue {
        return Err("refresh settlement");
    }
    if available {
        require_provenance_prefix(&trace.session, &prior_provenance)?;
        if trace.session.cache().tabs().len() != old_count + 1
            || trace.session.provenance(old_count) != Some(&CreateTerminalProvenance::BindingOnly)
            || trace.session.active() != active.as_ref()
            || trace.session.desired().unwrap() != &desired
            || trace.session.attached_key() != attached.as_ref()
            || trace.session.projection_key() != projection.as_ref()
            || trace.session.controller_mode() != mode.as_ref()
        {
            return Err("refresh append did not preserve live state");
        }
        trace.coverage.refresh_successes += 1;
    } else {
        trace.coverage.refresh_failures += 1;
    }
    Ok(())
}

fn refresh_stale_then_current(
    trace: &mut TraceState,
    generation_step: u8,
) -> Result<(), &'static str> {
    if !trace.session.phase_allows_refresh() {
        return Ok(());
    }
    let current = trace
        .refresh
        .checked_add(u64::from(generation_step.max(2)))
        .ok_or("refresh")?;
    if !trace.session.start_pending_refresh(current) {
        return Err("stale refresh admission");
    }
    let stale = current.checked_sub(1).ok_or("stale generation")?;
    let prepared = trace
        .session
        .prepare_refresh(stale, WorkbenchSnapshotOutcome::Denied {});
    if trace.session.apply_prepared_refresh(prepared, None) != RefreshDecision::Continue
        || trace.session.pending_refresh_generation() != Some(current)
    {
        return Err("stale refresh changed current");
    }
    let count = trace.session.cache().tabs().len();
    let prior_provenance = provenance_snapshot(&trace.session)?;
    let active = trace.session.active().cloned();
    let desired = trace.session.desired().unwrap().clone();
    let revision = trace
        .session
        .cache()
        .revision()
        .value()
        .checked_add(1)
        .ok_or("revision")?;
    let prepared = trace.session.prepare_refresh(
        current,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(count + 1, revision, None),
        },
    );
    if trace.session.apply_prepared_refresh(prepared, None) != RefreshDecision::Continue
        || trace.session.cache().tabs().len() != count + 1
        || trace.session.provenance(count) != Some(&CreateTerminalProvenance::BindingOnly)
        || trace.session.active() != active.as_ref()
        || trace.session.desired().unwrap() != &desired
    {
        return Err("current refresh after stale");
    }
    require_provenance_prefix(&trace.session, &prior_provenance)?;
    trace.refresh = current;
    trace.coverage.stale_refreshes += 1;
    trace.coverage.refresh_successes += 1;
    Ok(())
}

fn historical_ready_create(trace: &mut TraceState, cut: ReadyFrameCut) -> Result<(), &'static str> {
    if trace.session.phase_kind() != SessionPhaseKind::Ready {
        return Ok(());
    }
    let key = trace.session.desired().unwrap().clone();
    let active = trace.session.active().cloned().ok_or("active")?;
    let mode = trace.session.controller_mode().cloned().ok_or("mode")?;
    let provenance = trace
        .session
        .provenance(active.index())
        .cloned()
        .ok_or("provenance")?;
    match cut {
        ReadyFrameCut::StagedAwaitingFeedback => {
            trace.frame = trace.frame.checked_add(1).ok_or("frame")?;
            if !trace
                .session
                .stage_drawn_frame(&key, FrameIdentity::new(trace.frame))
            {
                return Err("stage newer frame");
            }
            trace.coverage.staged_frame_creates += 1;
        }
        ReadyFrameCut::ProjectionProofInvalidated => {
            if !trace.session.invalidate_projection_frame(&key) {
                return Err("invalidate projection proof");
            }
            trace.coverage.invalidated_frame_creates += 1;
        }
    }
    if trace.session.feedback_accepted() {
        return Err("transient frame cut retained current proof");
    }
    let generation = advance_generation(trace, 1)?;
    trace.request = trace.request.checked_add(1).ok_or("request")?;
    let operation = create_operation(&trace.session, generation, trace.request, "probe");
    let operation_key = operation.key().clone();
    let applied_geometry = trace.session.last_accepted_geometry(&key);
    if !trace.session.begin_create(operation)
        || trace.session.phase_kind() != SessionPhaseKind::Creating
        || trace.session.active() != Some(&active)
        || trace.session.controller_mode() != Some(&mode)
        || trace.session.provenance(active.index()) != Some(&provenance)
        || !trace
            .session
            .restore_precommit(&operation_key, applied_geometry)
    {
        return Err("historical Ready Create/precommit");
    }
    note_human_create(trace);
    trace.coverage.creates += 1;
    trace.coverage.precommit_restores += 1;
    trace.frame = trace.frame.checked_add(1).ok_or("frame")?;
    let frame = FrameIdentity::new(trace.frame);
    if !trace.session.stage_drawn_frame(&key, frame)
        || !trace.session.mark_feedback_accepted(&key, frame)
    {
        return Err("recover frame");
    }
    Ok(())
}

fn exercise_selection(trace: &mut TraceState, position: u8) {
    match trace
        .session
        .plan_selection(ClientAction::SelectTab(position))
    {
        SelectionPlan::NoOp | SelectionPlan::Switch { .. } => {
            trace.coverage.valid_indices += 1;
        }
        SelectionPlan::OutOfRange(_) => trace.coverage.out_of_range_indices += 1,
        SelectionPlan::Busy => {}
    }
}

fn exercise_controller_busy_read_only(trace: &mut TraceState) -> Result<(), &'static str> {
    let mut session = state(2);
    let key = session.desired().unwrap().clone();
    let frame = FrameIdentity::new(trace.frame.checked_add(1).ok_or("frame")?);
    if !session.install_projection(&key)
        || !session.record_setup_configuration(&key, ControllerMode::ReadOnly, None)
        || !session.stage_drawn_frame(&key, frame)
        || !session.mark_feedback_accepted(&key, frame)
        || !session.publish_ready(&key, frame, ControllerMode::ReadOnly, None)
        || session.record_admitted_geometry(&key, geometry(99))
        || !session.invariant_holds()
    {
        return Err("controller-busy read-only");
    }
    trace.coverage.controller_busy_read_only += 1;
    Ok(())
}

fn exercise_cleanup(trace: &mut TraceState, class: CleanupClass) -> Result<(), &'static str> {
    let mut session = ready(3);
    let old_key = AttachmentKey::new(2, session.tab(1).ok_or("cleanup tab")?.tab_id().clone());
    let deadlines = OperationDeadlines::from_sample(Instant::now()).ok_or("deadline")?;
    let target = session
        .target_for_switch(1, old_key.clone(), deadlines)
        .ok_or("cleanup target")?;
    if !session.begin_switch(target) {
        return Err("cleanup handoff");
    }
    let old_key = prepare_and_install_switch_candidate(&mut session, &old_key)
        .map_err(|_| "cleanup Switch candidate install")?;
    let continuation = match class {
        CleanupClass::CurrentInstalled => {
            if !session.install_projection(&old_key) {
                return Err("cleanup install");
            }
            session.settle_cleanup(&old_key, 1, &CreateTerminalProvenance::BindingOnly)
        }
        CleanupClass::CurrentBeforeUiInstall => {
            session.settle_cleanup(&old_key, 1, &CreateTerminalProvenance::BindingOnly)
        }
        CleanupClass::Superseded => {
            if !session.install_projection(&old_key) {
                return Err("superseded install");
            }
            let newer_key = AttachmentKey::new(
                old_key.generation() + 1,
                session.tab(2).ok_or("newer tab")?.tab_id().clone(),
            );
            let newer = session
                .target_for_switch(2, newer_key.clone(), deadlines)
                .ok_or("newer target")?;
            if !session.begin_switch(newer) {
                return Err("newer switch");
            }
            let continuation =
                session.settle_cleanup(&old_key, 1, &CreateTerminalProvenance::BindingOnly);
            if continuation != Some(CleanupContinuation::Superseded { desired: newer_key }) {
                return Err("superseded cleanup");
            }
            continuation
        }
    };
    if !matches!(class, CleanupClass::Superseded)
        && (continuation != Some(CleanupContinuation::Stable)
            || session.phase_kind() != SessionPhaseKind::RetryableAttachUnavailable)
    {
        return Err("stable cleanup");
    }
    if !session.invariant_holds() {
        return Err("cleanup invariant");
    }
    trace.coverage.cleanup_classes[match class {
        CleanupClass::CurrentInstalled => 0,
        CleanupClass::CurrentBeforeUiInstall => 1,
        CleanupClass::Superseded => 2,
    }] += 1;
    if !matches!(class, CleanupClass::Superseded) {
        trace.coverage.retryable_settlements += 1;
    }
    Ok(())
}

fn exercise_retirement(trace: &mut TraceState, class: RetirementClass) -> Result<(), &'static str> {
    let mut session = ready(2);
    let diagnostic_active = session.active().cloned();
    let failure = match class {
        RetirementClass::Deadline => CarrierFailure::Deadline,
        RetirementClass::Ambiguous => CarrierFailure::Transport,
        RetirementClass::AuthorityDivergence => CarrierFailure::AuthorityDivergence,
    };
    session.retire(failure);
    if session.phase_kind() != SessionPhaseKind::Retired
        || session.active() != diagnostic_active.as_ref()
        || session.retired_failure() != Some(failure)
        || session.committed_target().is_some()
        || session.attached_key().is_some()
        || session.projection_key().is_some()
        || !session.invariant_holds()
    {
        return Err("retirement");
    }
    trace.coverage.retirements[match class {
        RetirementClass::Deadline => 0,
        RetirementClass::Ambiguous => 1,
        RetirementClass::AuthorityDivergence => 2,
    }] += 1;
    Ok(())
}

/*
 * The generated harness above deliberately keeps one live Session for re-entry and cache/provenance
 * checks. Cleanup and retirement actions exercise destructive terminal branches on fresh anchors so
 * a later random action can continue checking that same live Session.
 */

fn shrink_failure(mut actions: Vec<TraceAction>, failure: &TraceFailure) -> Vec<TraceAction> {
    let mut chunk = actions.len() / 2;
    while chunk != 0 {
        let mut start = 0;
        let mut reduced = false;
        while start + chunk <= actions.len() {
            let mut candidate = actions.clone();
            candidate.drain(start..start + chunk);
            if replay(&candidate).is_err_and(|candidate| failure.same_fingerprint(&candidate)) {
                actions = candidate;
                reduced = true;
                break;
            }
            start += 1;
        }
        if !reduced {
            chunk /= 2;
        }
    }
    for index in 0..actions.len() {
        for replacement in shrink_values(actions[index]) {
            let mut candidate = actions.clone();
            candidate[index] = replacement;
            if replay(&candidate).is_err_and(|candidate| failure.same_fingerprint(&candidate)) {
                actions = candidate;
            }
        }
    }
    actions
}

fn shrink_values(action: TraceAction) -> Vec<TraceAction> {
    match action {
        TraceAction::ObserveGeometry(value) => shrink_u8(value)
            .into_iter()
            .map(TraceAction::ObserveGeometry)
            .collect(),
        TraceAction::SelectPosition(position) => shrink_u8(position)
            .into_iter()
            .map(TraceAction::SelectPosition)
            .collect(),
        TraceAction::SwitchReady {
            target_hint,
            generation_step,
        } => shrink_pair(target_hint, generation_step)
            .into_iter()
            .map(|(target_hint, generation_step)| TraceAction::SwitchReady {
                target_hint,
                generation_step,
            })
            .collect(),
        TraceAction::SwitchUnavailable {
            target_hint,
            generation_step,
        } => shrink_pair(target_hint, generation_step)
            .into_iter()
            .map(
                |(target_hint, generation_step)| TraceAction::SwitchUnavailable {
                    target_hint,
                    generation_step,
                },
            )
            .collect(),
        TraceAction::CreatePrecommit { generation_step } => shrink_u8(generation_step)
            .into_iter()
            .map(|generation_step| TraceAction::CreatePrecommit { generation_step })
            .collect(),
        TraceAction::CreateCommitted {
            class,
            generation_step,
        } => shrink_u8(generation_step)
            .into_iter()
            .map(|generation_step| TraceAction::CreateCommitted {
                class,
                generation_step,
            })
            .collect(),
        TraceAction::RefreshAppend { generation_step }
        | TraceAction::RefreshStaleThenCurrent { generation_step } => shrink_u8(generation_step)
            .into_iter()
            .map(|generation_step| match action {
                TraceAction::RefreshAppend { .. } => TraceAction::RefreshAppend { generation_step },
                TraceAction::RefreshStaleThenCurrent { .. } => {
                    TraceAction::RefreshStaleThenCurrent { generation_step }
                }
                _ => unreachable!(),
            })
            .collect(),
        TraceAction::RefreshFailure
        | TraceAction::RejectReusedCommittedTuple
        | TraceAction::HistoricalReadyCreate(_)
        | TraceAction::ControllerBusyReadOnly
        | TraceAction::Cleanup(_)
        | TraceAction::Retire(_)
        | TraceAction::ClearNotice => Vec::new(),
    }
}

fn shrink_u8(value: u8) -> Vec<u8> {
    match value {
        0 => Vec::new(),
        1 => vec![0],
        _ => vec![value / 2, 1, 0],
    }
}

fn shrink_pair(first: u8, second: u8) -> Vec<(u8, u8)> {
    shrink_u8(first)
        .into_iter()
        .map(|value| (value, second))
        .chain(shrink_u8(second).into_iter().map(|value| (first, value)))
        .collect()
}

fn serialize(actions: &[TraceAction]) -> String {
    actions
        .iter()
        .map(|action| match action {
            TraceAction::ObserveGeometry(value) => format!("g:{value}"),
            TraceAction::SelectPosition(position) => format!("p:{position}"),
            TraceAction::SwitchReady {
                target_hint,
                generation_step,
            } => format!("sr:{target_hint}:{generation_step}"),
            TraceAction::SwitchUnavailable {
                target_hint,
                generation_step,
            } => format!("su:{target_hint}:{generation_step}"),
            TraceAction::CreatePrecommit { generation_step } => {
                format!("cp:{generation_step}")
            }
            TraceAction::CreateCommitted {
                class,
                generation_step,
            } => format!("cc:{}:{generation_step}", created_class_token(*class)),
            TraceAction::RejectReusedCommittedTuple => "cr".to_owned(),
            TraceAction::RefreshAppend { generation_step } => {
                format!("ra:{generation_step}")
            }
            TraceAction::RefreshStaleThenCurrent { generation_step } => {
                format!("rs:{generation_step}")
            }
            TraceAction::RefreshFailure => "rf".to_owned(),
            TraceAction::HistoricalReadyCreate(cut) => {
                format!("hc:{}", ready_frame_cut_token(*cut))
            }
            TraceAction::ControllerBusyReadOnly => "cb".to_owned(),
            TraceAction::Cleanup(class) => format!("cl:{}", cleanup_class_token(*class)),
            TraceAction::Retire(class) => format!("rt:{}", retirement_class_token(*class)),
            TraceAction::ClearNotice => "n".to_owned(),
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
        ["g", _] => Ok(TraceAction::ObserveGeometry(number(1)?)),
        ["p", _] => Ok(TraceAction::SelectPosition(number(1)?)),
        ["sr", _, _] => Ok(TraceAction::SwitchReady {
            target_hint: number(1)?,
            generation_step: number(2)?,
        }),
        ["su", _, _] => Ok(TraceAction::SwitchUnavailable {
            target_hint: number(1)?,
            generation_step: number(2)?,
        }),
        ["cp", _] => Ok(TraceAction::CreatePrecommit {
            generation_step: number(1)?,
        }),
        ["cc", class, _] => Ok(TraceAction::CreateCommitted {
            class: parse_created_class(class)?,
            generation_step: number(2)?,
        }),
        ["cr"] => Ok(TraceAction::RejectReusedCommittedTuple),
        ["ra", _] => Ok(TraceAction::RefreshAppend {
            generation_step: number(1)?,
        }),
        ["rs", _] => Ok(TraceAction::RefreshStaleThenCurrent {
            generation_step: number(1)?,
        }),
        ["rf"] => Ok(TraceAction::RefreshFailure),
        ["hc", cut] => Ok(TraceAction::HistoricalReadyCreate(parse_ready_frame_cut(
            cut,
        )?)),
        ["cb"] => Ok(TraceAction::ControllerBusyReadOnly),
        ["cl", class] => Ok(TraceAction::Cleanup(parse_cleanup_class(class)?)),
        ["rt", class] => Ok(TraceAction::Retire(parse_retirement_class(class)?)),
        ["n"] => Ok(TraceAction::ClearNotice),
        _ => Err("invalid action"),
    }
}

const fn created_class_token(class: CreatedClass) -> &'static str {
    match class {
        CreatedClass::Pending => "p",
        CreatedClass::Started => "s",
        CreatedClass::Unavailable => "u",
        CreatedClass::Failed => "f",
        CreatedClass::OutcomeUnknown => "o",
    }
}

fn parse_created_class(token: &str) -> Result<CreatedClass, &'static str> {
    match token {
        "p" => Ok(CreatedClass::Pending),
        "s" => Ok(CreatedClass::Started),
        "u" => Ok(CreatedClass::Unavailable),
        "f" => Ok(CreatedClass::Failed),
        "o" => Ok(CreatedClass::OutcomeUnknown),
        _ => Err("invalid created class"),
    }
}

const fn ready_frame_cut_token(cut: ReadyFrameCut) -> &'static str {
    match cut {
        ReadyFrameCut::StagedAwaitingFeedback => "s",
        ReadyFrameCut::ProjectionProofInvalidated => "i",
    }
}

fn parse_ready_frame_cut(token: &str) -> Result<ReadyFrameCut, &'static str> {
    match token {
        "s" => Ok(ReadyFrameCut::StagedAwaitingFeedback),
        "i" => Ok(ReadyFrameCut::ProjectionProofInvalidated),
        _ => Err("invalid Ready frame cut"),
    }
}

const fn cleanup_class_token(class: CleanupClass) -> &'static str {
    match class {
        CleanupClass::CurrentInstalled => "i",
        CleanupClass::CurrentBeforeUiInstall => "u",
        CleanupClass::Superseded => "s",
    }
}

fn parse_cleanup_class(token: &str) -> Result<CleanupClass, &'static str> {
    match token {
        "i" => Ok(CleanupClass::CurrentInstalled),
        "u" => Ok(CleanupClass::CurrentBeforeUiInstall),
        "s" => Ok(CleanupClass::Superseded),
        _ => Err("invalid cleanup class"),
    }
}

const fn retirement_class_token(class: RetirementClass) -> &'static str {
    match class {
        RetirementClass::Deadline => "d",
        RetirementClass::Ambiguous => "a",
        RetirementClass::AuthorityDivergence => "v",
    }
}

fn parse_retirement_class(token: &str) -> Result<RetirementClass, &'static str> {
    match token {
        "d" => Ok(RetirementClass::Deadline),
        "a" => Ok(RetirementClass::Ambiguous),
        "v" => Ok(RetirementClass::AuthorityDivergence),
        _ => Err("invalid retirement class"),
    }
}

#[test]
fn serialized_state_trace_round_trips_for_direct_replay() {
    let actions = vec![
        TraceAction::ObserveGeometry(255),
        TraceAction::SelectPosition(0),
        TraceAction::SwitchReady {
            target_hint: 1,
            generation_step: 2,
        },
        TraceAction::SwitchUnavailable {
            target_hint: 3,
            generation_step: 4,
        },
        TraceAction::CreatePrecommit { generation_step: 5 },
        TraceAction::CreateCommitted {
            class: CreatedClass::Pending,
            generation_step: 6,
        },
        TraceAction::CreateCommitted {
            class: CreatedClass::Started,
            generation_step: 7,
        },
        TraceAction::CreateCommitted {
            class: CreatedClass::Unavailable,
            generation_step: 8,
        },
        TraceAction::CreateCommitted {
            class: CreatedClass::Failed,
            generation_step: 9,
        },
        TraceAction::CreateCommitted {
            class: CreatedClass::OutcomeUnknown,
            generation_step: 10,
        },
        TraceAction::RejectReusedCommittedTuple,
        TraceAction::RefreshAppend {
            generation_step: 11,
        },
        TraceAction::RefreshStaleThenCurrent {
            generation_step: 12,
        },
        TraceAction::RefreshFailure,
        TraceAction::HistoricalReadyCreate(ReadyFrameCut::StagedAwaitingFeedback),
        TraceAction::HistoricalReadyCreate(ReadyFrameCut::ProjectionProofInvalidated),
        TraceAction::ControllerBusyReadOnly,
        TraceAction::Cleanup(CleanupClass::CurrentInstalled),
        TraceAction::Cleanup(CleanupClass::CurrentBeforeUiInstall),
        TraceAction::Cleanup(CleanupClass::Superseded),
        TraceAction::Retire(RetirementClass::Deadline),
        TraceAction::Retire(RetirementClass::Ambiguous),
        TraceAction::Retire(RetirementClass::AuthorityDivergence),
        TraceAction::ClearNotice,
    ];
    assert_eq!(deserialize(&serialize(&actions)), Ok(actions.clone()));
    assert!(deserialize("").is_err());
    assert!(deserialize("cc:x:1").is_err());
    for action in actions {
        assert!(
            shrink_values(action)
                .into_iter()
                .all(|candidate| candidate.class() == action.class())
        );
    }
}
