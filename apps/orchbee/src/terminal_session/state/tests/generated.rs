use super::super::panel::{
    PanelCommit, PanelDecision, PanelIntent, PanelOperation, PanelPrecommit, PanelRequestKind,
    PanelResult, PendingPanelCommit, PreparedPanel,
};
use super::*;
use crate::client_view::PanelDirection;
use crate::local_client::workbench::{
    panel_close::WorkbenchPanelCloseAccepted, panel_move::WorkbenchPanelMoveCommit,
    panel_split::WorkbenchPanelSplitCommit,
};
use orchbee_workbench_protocol::{
    WorkbenchCommandUnavailableReason, WorkbenchPanelSplitDirection, WorkbenchRejectionReason,
    WorkbenchTerminalOutcome,
};

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
enum PanelSettleClass {
    NotSubmitted,
    // Synthetic defensive input only: see the construction site in `panel_settle` — no
    // reachable Server history produces a fresh-baseline rejection for these fixtures,
    // so the class verifies the NotApplied mapping, not a real peer edit.
    RejectedFresh,
    RejectedStale,
    Denied,
    Unavailable,
    Unchanged,
    CloseAccepted,
    CommittedMerge,
    CommittedRecovery,
    TabUnavailableGone,
    CommittedLeafRepair,
}

impl PanelSettleClass {
    const COUNT: usize = 11;

    const fn index(self) -> usize {
        match self {
            Self::NotSubmitted => 0,
            Self::RejectedFresh => 1,
            Self::RejectedStale => 2,
            Self::Denied => 3,
            Self::Unavailable => 4,
            Self::Unchanged => 5,
            Self::CloseAccepted => 6,
            Self::CommittedMerge => 7,
            Self::CommittedRecovery => 8,
            Self::TabUnavailableGone => 9,
            Self::CommittedLeafRepair => 10,
        }
    }
}

/// Whether a settle input models an honest Server trajectory or a deliberate
/// defensive forgery. A `Normal` input must be a wire shape a real Server can
/// emit for the operation's intent AND the adapter can produce; a `Defensive`
/// input is a forged shape no honest trajectory delivers — it exercises reducer
/// leniency only. The two tiers are counted separately so a forged input can
/// never masquerade as normal coverage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SettleTier {
    Normal,
    Defensive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PanelAnchorClass {
    DivergentCommit,
    StaleOperation,
    ForeignRetain,
    MembershipGates,
    ApplyMembershipRetires,
    RefreshMembershipRetires,
    RetireWithPending,
}

impl PanelAnchorClass {
    const COUNT: usize = 7;

    const fn index(self) -> usize {
        match self {
            Self::DivergentCommit => 0,
            Self::StaleOperation => 1,
            Self::ForeignRetain => 2,
            Self::MembershipGates => 3,
            Self::ApplyMembershipRetires => 4,
            Self::RefreshMembershipRetires => 5,
            Self::RetireWithPending => 6,
        }
    }
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
    PanelFocus {
        direction: u8,
        generation_step: u8,
    },
    PanelSubmit {
        kind: u8,
        direction: u8,
    },
    PanelSettle {
        class: PanelSettleClass,
        tier: SettleTier,
    },
    PanelRecoveryRefresh {
        generation_step: u8,
    },
    RefreshLeafRepair {
        generation_step: u8,
    },
    PanelAnchor {
        class: PanelAnchorClass,
    },
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
    PanelFocus,
    PanelSubmit,
    PanelSettle(PanelSettleClass),
    PanelRecoveryRefresh,
    RefreshLeafRepair,
    PanelAnchor(PanelAnchorClass),
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
            Self::PanelFocus { .. } => ActionClass::PanelFocus,
            Self::PanelSubmit { .. } => ActionClass::PanelSubmit,
            Self::PanelSettle { class, .. } => ActionClass::PanelSettle(class),
            Self::PanelRecoveryRefresh { .. } => ActionClass::PanelRecoveryRefresh,
            Self::RefreshLeafRepair { .. } => ActionClass::RefreshLeafRepair,
            Self::PanelAnchor { class } => ActionClass::PanelAnchor(class),
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
    panel_focuses: usize,
    panel_submissions: usize,
    panel_plan_refusals: usize,
    panel_structure_exclusions: usize,
    panel_settlements: [usize; PanelSettleClass::COUNT],
    /// Tier split of `panel_settlements`: a settle is counted under exactly
    /// one tier, so honest-trajectory coverage is visible apart from forged
    /// inputs.
    panel_settlements_normal: [usize; PanelSettleClass::COUNT],
    panel_settlements_defensive: [usize; PanelSettleClass::COUNT],
    panel_merges: usize,
    panel_membership_installs: usize,
    panel_membership_cleanups: usize,
    panel_recovery_entries: usize,
    panel_recovery_resolutions: usize,
    refresh_leaf_repairs: usize,
    panel_anchor_witnesses: [usize; PanelAnchorClass::COUNT],
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
        self.panel_focuses += other.panel_focuses;
        self.panel_submissions += other.panel_submissions;
        self.panel_plan_refusals += other.panel_plan_refusals;
        self.panel_structure_exclusions += other.panel_structure_exclusions;
        for (total, count) in self
            .panel_settlements
            .iter_mut()
            .zip(other.panel_settlements)
        {
            *total += count;
        }
        for (total, count) in self
            .panel_settlements_normal
            .iter_mut()
            .zip(other.panel_settlements_normal)
        {
            *total += count;
        }
        for (total, count) in self
            .panel_settlements_defensive
            .iter_mut()
            .zip(other.panel_settlements_defensive)
        {
            *total += count;
        }
        self.panel_merges += other.panel_merges;
        self.panel_membership_installs += other.panel_membership_installs;
        self.panel_membership_cleanups += other.panel_membership_cleanups;
        self.panel_recovery_entries += other.panel_recovery_entries;
        self.panel_recovery_resolutions += other.panel_recovery_resolutions;
        self.refresh_leaf_repairs += other.refresh_leaf_repairs;
        for (total, count) in self
            .panel_anchor_witnesses
            .iter_mut()
            .zip(other.panel_anchor_witnesses)
        {
            *total += count;
        }
    }
}

/// The wire fields a delivered settle input carried, summarized so the
/// input-fidelity witness can audit what the generator actually produced —
/// not just what the reducer did with it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SettleWire {
    NotSubmitted,
    Rejected {
        revision: u64,
        reason: WorkbenchRejectionReason,
    },
    Denied,
    Unavailable {
        reason: WorkbenchCommandUnavailableReason,
    },
    Unchanged {
        revision: u64,
    },
    CloseAccepted,
    Committed,
    TabUnavailable {
        revision: u64,
    },
}

/// One recorded settle input: which intent it targeted, which tier produced it,
/// the wire summary, and the authority/baseline revisions at construction.
#[derive(Clone, Copy, Debug)]
struct SettleWitness {
    intent: IntentTag,
    class: PanelSettleClass,
    tier: SettleTier,
    wire: SettleWire,
    baseline_revision: u64,
    server_revision: u64,
}

/// Which of the three panel intents a settle input targeted — `PanelIntent`
/// carries payloads, so the witness stores this plain tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IntentTag {
    Split,
    Move,
    Close,
}

fn intent_tag(intent: &PanelIntent) -> IntentTag {
    match intent {
        PanelIntent::Split { .. } => IntentTag::Split,
        PanelIntent::Move { .. } => IntentTag::Move,
        PanelIntent::Close => IntentTag::Close,
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
    /// Every settle input the driver delivered, in order — the audit trail the
    /// legality witness inspects.
    settle_log: Vec<SettleWitness>,
    /// Authoritative wire history the emulated Server has committed; every delivered
    /// candidate is a projection of some prefix of this, so the Session cache is always
    /// re-derivable from a real Snapshot.
    server_tabs: Vec<Tab>,
    server_revision: u64,
    /// Wire tabs of the candidate the Session last adopted; equals `session.cache()`.
    delivered_tabs: Vec<Tab>,
    /// `server_revision` while a committed panel op is still unobserved (`panel_recovery`).
    recovery_revision: Option<u64>,
    /// Next `tab()` seed; kept monotonic so appended tabs never reuse removed ids.
    tab_seed: usize,
    /// Fresh identity allocator for panel commits (kept far from `tab()` seeds).
    identity: u128,
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
            // Panel prefix: every reachability class is forced once per seed before the
            // random tail, so coverage claims never depend on a lucky draw.
            TraceAction::PanelSubmit {
                kind: 0,
                direction: 1,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::CommittedMerge,
                tier: SettleTier::Normal,
            },
            TraceAction::PanelFocus {
                direction: 1,
                generation_step: 1,
            },
            TraceAction::PanelSubmit {
                kind: 0,
                direction: 3,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::CommittedMerge,
                tier: SettleTier::Normal,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 0,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::CommittedMerge,
                tier: SettleTier::Normal,
            },
            TraceAction::PanelSubmit {
                kind: 0,
                direction: 2,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::CommittedRecovery,
                tier: SettleTier::Normal,
            },
            TraceAction::PanelRecoveryRefresh { generation_step: 1 },
            TraceAction::PanelSubmit {
                kind: 1,
                direction: 0,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::CloseAccepted,
                tier: SettleTier::Normal,
            },
            // Every multi-leaf Tab has a move target in some direction, so fanning all
            // four leaves a pending Move (later submissions just count as refusals).
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 0,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 1,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 2,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 3,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::Unchanged,
                tier: SettleTier::Defensive,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 0,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 1,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 2,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 3,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::TabUnavailableGone,
                tier: SettleTier::Defensive,
            },
            TraceAction::SwitchReady {
                target_hint: 0,
                generation_step: 1,
            },
            TraceAction::PanelSubmit {
                kind: 0,
                direction: 1,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::CommittedMerge,
                tier: SettleTier::Normal,
            },
            TraceAction::PanelFocus {
                direction: 1,
                generation_step: 1,
            },
            TraceAction::RefreshLeafRepair { generation_step: 1 },
            TraceAction::PanelSubmit {
                kind: 0,
                direction: 1,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::CommittedMerge,
                tier: SettleTier::Normal,
            },
            TraceAction::PanelFocus {
                direction: 1,
                generation_step: 1,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 0,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 1,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 2,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 3,
            },
            // The committed candidate folds a peer close of the selected leaf — the real
            // selected-leaf-loss route for a panel result (paired membership + retained
            // commit fact).
            TraceAction::PanelSettle {
                class: PanelSettleClass::CommittedLeafRepair,
                tier: SettleTier::Normal,
            },
            // The repair folded the selected leaf away, leaving a single-leaf Tab — split
            // once more so a Move has a target again.
            TraceAction::PanelSubmit {
                kind: 0,
                direction: 1,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::CommittedMerge,
                tier: SettleTier::Normal,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 0,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 1,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 2,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 3,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::Unchanged,
                tier: SettleTier::Defensive,
            },
            TraceAction::PanelSubmit {
                kind: 1,
                direction: 0,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::NotSubmitted,
                tier: SettleTier::Normal,
            },
            TraceAction::PanelSubmit {
                kind: 1,
                direction: 0,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::RejectedFresh,
                tier: SettleTier::Defensive,
            },
            TraceAction::PanelSubmit {
                kind: 1,
                direction: 0,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::RejectedStale,
                tier: SettleTier::Normal,
            },
            // The preserved forged edge: a rejection carrying a revision below
            // the baseline — no monotone Server emits it.
            TraceAction::PanelSubmit {
                kind: 1,
                direction: 0,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::RejectedStale,
                tier: SettleTier::Defensive,
            },
            TraceAction::PanelSubmit {
                kind: 1,
                direction: 0,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::Denied,
                tier: SettleTier::Normal,
            },
            TraceAction::PanelSubmit {
                kind: 1,
                direction: 0,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::Unavailable,
                tier: SettleTier::Normal,
            },
            // The preserved forged edge: ServerBusy on a Move — outside Move's
            // real wire-compatible reason set, so no adapter could deliver it.
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 0,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 1,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 2,
            },
            TraceAction::PanelSubmit {
                kind: 2,
                direction: 3,
            },
            TraceAction::PanelSettle {
                class: PanelSettleClass::Unavailable,
                tier: SettleTier::Defensive,
            },
            TraceAction::PanelAnchor {
                class: PanelAnchorClass::DivergentCommit,
            },
            TraceAction::PanelAnchor {
                class: PanelAnchorClass::StaleOperation,
            },
            TraceAction::PanelAnchor {
                class: PanelAnchorClass::ForeignRetain,
            },
            TraceAction::PanelAnchor {
                class: PanelAnchorClass::MembershipGates,
            },
            TraceAction::PanelAnchor {
                class: PanelAnchorClass::ApplyMembershipRetires,
            },
            TraceAction::PanelAnchor {
                class: PanelAnchorClass::RefreshMembershipRetires,
            },
            TraceAction::PanelAnchor {
                class: PanelAnchorClass::RetireWithPending,
            },
        ];
        let mandatory_count = actions.len();
        for _ in 0..GENERATED_STEPS_PER_SEED {
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let byte = (random >> 48) as u8;
            actions.push(match (random >> 32) % 22 {
                0 => TraceAction::ObserveGeometry((random >> 48) as u8),
                // Bias positions into a range that exercises both in-range and
                // out-of-range selection against the fixture's small Tab counts.
                1 => TraceAction::SelectPosition(byte % 8),
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
                14 => TraceAction::ClearNotice,
                15 => TraceAction::PanelFocus {
                    direction: byte,
                    generation_step: byte,
                },
                16 => TraceAction::PanelSubmit {
                    kind: byte,
                    direction: byte,
                },
                17 => TraceAction::PanelSettle {
                    class: match byte % 11 {
                        0 => PanelSettleClass::NotSubmitted,
                        1 => PanelSettleClass::RejectedFresh,
                        2 => PanelSettleClass::RejectedStale,
                        3 => PanelSettleClass::Denied,
                        4 => PanelSettleClass::Unavailable,
                        5 => PanelSettleClass::Unchanged,
                        6 => PanelSettleClass::CloseAccepted,
                        7 => PanelSettleClass::CommittedMerge,
                        8 => PanelSettleClass::CommittedRecovery,
                        9 => PanelSettleClass::TabUnavailableGone,
                        _ => PanelSettleClass::CommittedLeafRepair,
                    },
                    // An independent bit picks the tier; combinations the
                    // tier gate rejects are skipped like illegal classes.
                    tier: if random & 0x100 == 0 {
                        SettleTier::Normal
                    } else {
                        SettleTier::Defensive
                    },
                },
                18 => TraceAction::PanelRecoveryRefresh {
                    generation_step: byte,
                },
                19 => TraceAction::RefreshLeafRepair {
                    generation_step: byte,
                },
                _ => TraceAction::PanelAnchor {
                    class: match byte % 7 {
                        0 => PanelAnchorClass::DivergentCommit,
                        1 => PanelAnchorClass::StaleOperation,
                        2 => PanelAnchorClass::ForeignRetain,
                        3 => PanelAnchorClass::MembershipGates,
                        4 => PanelAnchorClass::ApplyMembershipRetires,
                        5 => PanelAnchorClass::RefreshMembershipRetires,
                        _ => PanelAnchorClass::RetireWithPending,
                    },
                },
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
    assert!(total.panel_focuses >= SEEDS.len());
    assert!(total.panel_submissions >= SEEDS.len());
    assert!(total.panel_plan_refusals >= SEEDS.len());
    assert!(total.panel_structure_exclusions >= SEEDS.len());
    assert!(
        total
            .panel_settlements
            .iter()
            .all(|count| *count >= SEEDS.len())
    );
    // Tier split: every class with an honest wire shape exercised it per seed,
    // and every class carrying a forged edge exercised that edge per seed — a
    // defensive input can never substitute for the normal one, or vice versa.
    const NORMAL_SETTLE_CLASSES: [PanelSettleClass; 8] = [
        PanelSettleClass::NotSubmitted,
        PanelSettleClass::RejectedStale,
        PanelSettleClass::Denied,
        PanelSettleClass::Unavailable,
        PanelSettleClass::CloseAccepted,
        PanelSettleClass::CommittedMerge,
        PanelSettleClass::CommittedRecovery,
        PanelSettleClass::CommittedLeafRepair,
    ];
    const DEFENSIVE_SETTLE_CLASSES: [PanelSettleClass; 5] = [
        PanelSettleClass::RejectedFresh,
        PanelSettleClass::RejectedStale,
        PanelSettleClass::Unavailable,
        PanelSettleClass::Unchanged,
        PanelSettleClass::TabUnavailableGone,
    ];
    assert!(
        NORMAL_SETTLE_CLASSES
            .iter()
            .all(|class| total.panel_settlements_normal[class.index()] >= SEEDS.len())
    );
    assert!(
        DEFENSIVE_SETTLE_CLASSES
            .iter()
            .all(|class| total.panel_settlements_defensive[class.index()] >= SEEDS.len())
    );
    assert!(total.panel_merges >= SEEDS.len());
    assert!(total.panel_membership_installs >= SEEDS.len());
    assert!(total.panel_membership_cleanups >= SEEDS.len());
    assert!(total.panel_recovery_entries >= SEEDS.len());
    assert!(total.panel_recovery_resolutions >= SEEDS.len());
    assert!(total.refresh_leaf_repairs >= SEEDS.len());
    assert!(
        total
            .panel_anchor_witnesses
            .iter()
            .all(|count| *count >= SEEDS.len())
    );
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

fn new_trace() -> TraceState {
    let bootstrap = snapshot(3, 1, None).terminal_contexts[0].tabs.clone();
    TraceState {
        session: ready(3),
        generation: 1,
        request: 10_000,
        frame: 1,
        refresh: 0,
        geometry_columns: 80,
        human_creates: 0,
        committed_tabs: Vec::new(),
        settle_log: Vec::new(),
        server_tabs: bootstrap.clone(),
        server_revision: 1,
        delivered_tabs: bootstrap,
        recovery_revision: None,
        tab_seed: 3,
        identity: 0xA5E0_0000,
        coverage: Coverage::default(),
    }
}

fn replay(actions: &[TraceAction]) -> Result<Coverage, TraceFailure> {
    let mut trace = new_trace();
    for (step, action) in actions.iter().copied().enumerate() {
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| apply(&mut trace, action)));
        let oracle = match result {
            Ok(Ok(())) if trace.session.invariant_holds() && shadow_holds(&trace) => continue,
            Ok(Ok(())) => "invariant or shadow failed".to_owned(),
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
        TraceAction::PanelFocus {
            direction,
            generation_step,
        } => panel_focus(trace, direction, generation_step)?,
        TraceAction::PanelSubmit { kind, direction } => panel_submit(trace, kind, direction)?,
        TraceAction::PanelSettle { class, tier } => panel_settle(trace, class, tier)?,
        TraceAction::PanelRecoveryRefresh { generation_step } => {
            panel_recovery_refresh(trace, generation_step)?
        }
        TraceAction::RefreshLeafRepair { generation_step } => {
            refresh_leaf_repair(trace, generation_step)?
        }
        TraceAction::PanelAnchor { class } => panel_anchor(trace, class)?,
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
    ) || trace.session.mutation_blocks_structure()
    {
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
    if trace.session.phase_kind() != SessionPhaseKind::Ready
        || trace.session.mutation_blocks_structure()
    {
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
    if trace.session.phase_kind() != SessionPhaseKind::Ready
        || trace.session.mutation_blocks_structure()
    {
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
    // The freshness clause: a stale-baseline create is honestly rejected with
    // RevisionMismatch before admission — the commit path is only exercised
    // while the session is current (any refresh re-syncs and reopens it).
    if trace.session.phase_kind() != SessionPhaseKind::Ready
        || trace.session.mutation_blocks_structure()
        || trace.session.cache().tabs().len() >= 64
        || trace.session.cache().revision().value() < trace.server_revision
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
    // The Server appends exactly one fresh Tab to the committed history; identities that
    // were removed earlier can never reappear, so the appended Tab uses a new seed.
    let mut created_tab = fresh_tab(trace);
    created_tab.label = label.clone();
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut created_tab.root
    else {
        return Err("created root");
    };
    primary_presentation.label = label.clone();
    trace.server_tabs.push(created_tab);
    trace.server_revision = trace.server_revision.checked_add(1).ok_or("revision")?;
    let candidate = snapshot_with_tabs(trace.server_tabs.clone(), trace.server_revision);
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
    if !trace.session.retain_create_commit(&operation_key, commit) {
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
    trace.delivered_tabs = trace.server_tabs.clone();
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
        // The Server folds one appended Tab into the next revision; during
        // panel_recovery the same candidate also carries the still-unobserved graft.
        let appended = fresh_tab(trace);
        trace.server_tabs.push(appended);
        trace.server_revision = trace.server_revision.checked_add(1).ok_or("revision")?;
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot_with_tabs(trace.server_tabs.clone(), trace.server_revision),
        }
    } else {
        WorkbenchSnapshotOutcome::Denied {}
    };
    let prepared = trace.session.prepare_refresh(trace.refresh, outcome);
    let decision = trace.session.apply_prepared_refresh(prepared);
    if decision != RefreshDecision::Continue {
        return Err("refresh settlement");
    }
    if available {
        require_provenance_prefix(&trace.session, &prior_provenance)?;
        // A recovery-folding merge may re-index the selected leaf; the identity and Tab
        // slot must still be preserved. Unobserved peer commits (stale-rejection
        // paths) may already sit in `server_tabs`, so adoption is checked against
        // the delivered Server state — not the old count plus this append alone.
        let active_after = trace
            .session
            .active()
            .map(|selected| (selected.key(), selected.index()));
        let active_before = active
            .as_ref()
            .map(|selected| (selected.key(), selected.index()));
        if trace.session.cache().tabs().len() != trace.server_tabs.len()
            || trace.session.provenance(old_count) != Some(&CreateTerminalProvenance::BindingOnly)
            || active_after != active_before
            || trace.session.desired().unwrap() != &desired
            || trace.session.attached_key() != attached.as_ref()
            || trace.session.projection_key() != projection.as_ref()
            || trace.session.controller_mode() != mode.as_ref()
        {
            return Err("refresh append did not preserve live state");
        }
        trace.delivered_tabs = trace.server_tabs.clone();
        if trace.recovery_revision.take().is_some() {
            trace.coverage.panel_recovery_resolutions += 1;
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
    if trace.session.apply_prepared_refresh(prepared) != RefreshDecision::Continue
        || trace.session.pending_refresh_generation() != Some(current)
    {
        return Err("stale refresh changed current");
    }
    let count = trace.session.cache().tabs().len();
    let prior_provenance = provenance_snapshot(&trace.session)?;
    let active = trace.session.active().cloned();
    let desired = trace.session.desired().unwrap().clone();
    let appended = fresh_tab(trace);
    trace.server_tabs.push(appended);
    trace.server_revision = trace.server_revision.checked_add(1).ok_or("revision")?;
    let prepared = trace.session.prepare_refresh(
        current,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot_with_tabs(trace.server_tabs.clone(), trace.server_revision),
        },
    );
    // Unobserved peer commits (stale-rejection paths) may already sit in
    // `server_tabs`; adoption is checked against the delivered Server state.
    if trace.session.apply_prepared_refresh(prepared) != RefreshDecision::Continue
        || trace.session.cache().tabs().len() != trace.server_tabs.len()
        || trace.session.provenance(count) != Some(&CreateTerminalProvenance::BindingOnly)
        || trace
            .session
            .active()
            .map(|selected| (selected.key(), selected.index()))
            != active
                .as_ref()
                .map(|selected| (selected.key(), selected.index()))
        || trace.session.desired().unwrap() != &desired
    {
        return Err("current refresh after stale");
    }
    require_provenance_prefix(&trace.session, &prior_provenance)?;
    trace.delivered_tabs = trace.server_tabs.clone();
    if trace.recovery_revision.take().is_some() {
        trace.coverage.panel_recovery_resolutions += 1;
    }
    trace.refresh = current;
    trace.coverage.stale_refreshes += 1;
    trace.coverage.refresh_successes += 1;
    Ok(())
}

fn historical_ready_create(trace: &mut TraceState, cut: ReadyFrameCut) -> Result<(), &'static str> {
    if trace.session.phase_kind() != SessionPhaseKind::Ready
        || trace.session.mutation_blocks_structure()
    {
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

/*
 * Panel witness helpers. `TraceState.server_tabs` is the authoritative wire history the
 * emulated Server has committed, so every candidate the Session sees projects a history a
 * real Server could produce: tree edits only graft one fresh leaf under an existing leaf
 * or collapse one leaf into its sibling — never a root wrap and never a multi-operation
 * rewrite. `delivered_tabs` pins the last candidate the Session adopted, so
 * `session.cache()` must always re-derive from it; `replay` enforces that every step.
 */

fn fresh_tab(trace: &mut TraceState) -> Tab {
    let tab = tab(trace.tab_seed);
    trace.tab_seed += 1;
    tab
}

fn snapshot_with_tabs(tabs: Vec<Tab>, revision: u64) -> Snapshot {
    let mut snapshot = snapshot(1, revision, None);
    snapshot.terminal_contexts[0].tabs = tabs;
    snapshot
}

fn shadow_holds(trace: &TraceState) -> bool {
    let snapshot = snapshot_with_tabs(
        trace.delivered_tabs.clone(),
        trace.session.cache().revision().value(),
    );
    project_native_standalone_tabs(&snapshot)
        .is_ok_and(|projected| projected == *trace.session.cache())
}

fn next_identity(trace_identity: &mut u128) -> u128 {
    *trace_identity = trace_identity.saturating_add(1);
    *trace_identity
}

fn split_id(value: u128) -> WorkbenchSplitId {
    WorkbenchSplitId::parse(&id(value)).unwrap()
}

fn slot_id(value: u128) -> WorkbenchPanelSlotId {
    WorkbenchPanelSlotId::parse(&id(value)).unwrap()
}

fn panel_direction(hint: u8) -> PanelDirection {
    match hint % 4 {
        0 => PanelDirection::Left,
        1 => PanelDirection::Right,
        2 => PanelDirection::Up,
        _ => PanelDirection::Down,
    }
}

fn split_orientation(direction: WorkbenchPanelSplitDirection) -> PanelOrientation {
    match direction {
        WorkbenchPanelSplitDirection::Left | WorkbenchPanelSplitDirection::Right => {
            PanelOrientation::Horizontal
        }
        WorkbenchPanelSplitDirection::Above | WorkbenchPanelSplitDirection::Below => {
            PanelOrientation::Vertical
        }
    }
}

fn leaf_node<'a>(node: &'a PanelNode, panel_id: &WorkbenchPanelId) -> Option<&'a PanelNode> {
    match node {
        PanelNode::Panel { panel_id: id, .. } => (id == panel_id).then_some(node),
        PanelNode::Split { children, .. } => children
            .iter()
            .find_map(|slot| leaf_node(&slot.node, panel_id)),
    }
}

/// A real leaf-level split graft: `panel_id`'s leaf is replaced by one fresh Split whose
/// children place `added` on the direction-implied side — the only shape
/// `validate_split_panel` and a Server `replace_panel` both accept.
fn graft_split(
    node: &mut PanelNode,
    panel_id: &WorkbenchPanelId,
    direction: WorkbenchPanelSplitDirection,
    added: &PanelNode,
    split_id: &WorkbenchSplitId,
    first_slot_id: &WorkbenchPanelSlotId,
    second_slot_id: &WorkbenchPanelSlotId,
) -> bool {
    match node {
        PanelNode::Panel { panel_id: id, .. } if id == panel_id => {
            let source = node.clone();
            let (first_node, second_node) = match direction {
                WorkbenchPanelSplitDirection::Right | WorkbenchPanelSplitDirection::Below => {
                    (source, added.clone())
                }
                WorkbenchPanelSplitDirection::Left | WorkbenchPanelSplitDirection::Above => {
                    (added.clone(), source)
                }
            };
            *node = PanelNode::Split {
                children: Box::new([
                    PanelSlot {
                        node: first_node,
                        slot_id: first_slot_id.clone(),
                    },
                    PanelSlot {
                        node: second_node,
                        slot_id: second_slot_id.clone(),
                    },
                ]),
                orientation: split_orientation(direction),
                split_id: split_id.clone(),
            };
            true
        }
        PanelNode::Panel { .. } => false,
        PanelNode::Split { children, .. } => {
            graft_split(
                &mut children[0].node,
                panel_id,
                direction,
                added,
                split_id,
                first_slot_id,
                second_slot_id,
            ) || graft_split(
                &mut children[1].node,
                panel_id,
                direction,
                added,
                split_id,
                first_slot_id,
                second_slot_id,
            )
        }
    }
}

/// A real leaf-level close: `panel_id`'s leaf is removed and its parent Split collapses
/// into the surviving sibling subtree. A root leaf is never removed.
fn collapse_leaf(node: &mut PanelNode, panel_id: &WorkbenchPanelId) -> bool {
    let PanelNode::Split { children, .. } = node else {
        return false;
    };
    let [first, second] = children.as_mut();
    let remove_first =
        matches!(&first.node, PanelNode::Panel { panel_id: id, .. } if id == panel_id);
    let remove_second = !remove_first
        && matches!(&second.node, PanelNode::Panel { panel_id: id, .. } if id == panel_id);
    if remove_first || remove_second {
        *node = if remove_first {
            second.node.clone()
        } else {
            first.node.clone()
        };
        return true;
    }
    collapse_leaf(&mut first.node, panel_id) || collapse_leaf(&mut second.node, panel_id)
}

/// A real leaf-level move graft: `moved` re-appears beside `target_panel_id`'s leaf under
/// one fresh Split — the caller collapses the old leaf first so the candidate reduces to
/// the baseline by ungrafting exactly one fresh split (matching `move_graft_matches`).
fn graft_move(
    node: &mut PanelNode,
    target_panel_id: &WorkbenchPanelId,
    moved: &PanelNode,
    direction: WorkbenchPanelSplitDirection,
    split_id: &WorkbenchSplitId,
    first_slot_id: &WorkbenchPanelSlotId,
    second_slot_id: &WorkbenchPanelSlotId,
) -> bool {
    match node {
        PanelNode::Panel { panel_id: id, .. } if id == target_panel_id => {
            let target = node.clone();
            let (first_node, second_node) = match direction {
                WorkbenchPanelSplitDirection::Right | WorkbenchPanelSplitDirection::Below => {
                    (target, moved.clone())
                }
                WorkbenchPanelSplitDirection::Left | WorkbenchPanelSplitDirection::Above => {
                    (moved.clone(), target)
                }
            };
            *node = PanelNode::Split {
                children: Box::new([
                    PanelSlot {
                        node: first_node,
                        slot_id: first_slot_id.clone(),
                    },
                    PanelSlot {
                        node: second_node,
                        slot_id: second_slot_id.clone(),
                    },
                ]),
                orientation: split_orientation(direction),
                split_id: split_id.clone(),
            };
            true
        }
        PanelNode::Panel { .. } => false,
        PanelNode::Split { children, .. } => {
            graft_move(
                &mut children[0].node,
                target_panel_id,
                moved,
                direction,
                split_id,
                first_slot_id,
                second_slot_id,
            ) || graft_move(
                &mut children[1].node,
                target_panel_id,
                moved,
                direction,
                split_id,
                first_slot_id,
                second_slot_id,
            )
        }
    }
}

/// The Server-side committed edit for a pending Split/Move: graft/collapse exactly one
/// leaf in the wire history and return the commit record plus the new wire tabs.
fn commit_edit(
    identity: &mut u128,
    server_tabs: &[Tab],
    operation: &PanelOperation,
    geometry_columns: u16,
) -> Result<(PanelCommit, Vec<Tab>, Option<WorkbenchPanelId>), &'static str> {
    let baseline = operation.baseline();
    let revision = baseline
        .revision()
        .value()
        .checked_add(1)
        .ok_or("commit revision")?;
    let terminal_context_id = baseline.terminal_context_id().clone();
    let tab_id = operation.target().tab_id().clone();
    let mut tabs = server_tabs.to_vec();
    let Some(tab) = tabs.iter_mut().find(|tab| tab.tab_id == tab_id) else {
        return Err("commit tab missing");
    };
    match operation.intent() {
        PanelIntent::Split {
            direction, label, ..
        } => {
            let new_panel_id = panel_id(next_identity(identity));
            let presentation_id = presentation_id(next_identity(identity));
            let new_split = split_id(next_identity(identity));
            let first_slot = slot_id(next_identity(identity));
            let second_slot = slot_id(next_identity(identity));
            let added = PanelNode::Panel {
                child_presentations: Vec::new(),
                panel_id: new_panel_id.clone(),
                primary_presentation: Presentation {
                    binding: Some(PresentationBinding {
                        terminal_id: terminal_id(next_identity(identity)),
                        worker_id: worker_id(next_identity(identity)),
                    }),
                    label: label.as_str().to_owned(),
                    presentation_id: presentation_id.clone(),
                },
            };
            if !graft_split(
                &mut tab.root,
                operation.panel_id(),
                *direction,
                &added,
                &new_split,
                &first_slot,
                &second_slot,
            ) {
                return Err("split graft");
            }
            Ok((
                PanelCommit::Split(WorkbenchPanelSplitCommit::Created {
                    revision: WorkbenchRevision::new(revision),
                    terminal_context_id,
                    tab_id,
                    panel_id: operation.panel_id().clone(),
                    new_panel_id: new_panel_id.clone(),
                    split_id: new_split,
                    first_slot_id: first_slot,
                    second_slot_id: second_slot,
                    presentation_id,
                    terminal_outcome: WorkbenchTerminalOutcome::Started {
                        geometry: workbench_geometry(geometry_columns),
                    },
                }),
                tabs,
                Some(new_panel_id),
            ))
        }
        PanelIntent::Move {
            target_panel_id,
            direction,
        } => {
            let moved = leaf_node(&tab.root, operation.panel_id())
                .ok_or("moved leaf")?
                .clone();
            if !collapse_leaf(&mut tab.root, operation.panel_id()) {
                return Err("move collapse");
            }
            let new_split = split_id(next_identity(identity));
            let first_slot = slot_id(next_identity(identity));
            let second_slot = slot_id(next_identity(identity));
            if !graft_move(
                &mut tab.root,
                target_panel_id,
                &moved,
                *direction,
                &new_split,
                &first_slot,
                &second_slot,
            ) {
                return Err("move graft");
            }
            Ok((
                PanelCommit::Move(WorkbenchPanelMoveCommit {
                    revision: WorkbenchRevision::new(revision),
                    terminal_context_id,
                    tab_id,
                    panel_id: operation.panel_id().clone(),
                    target_panel_id: target_panel_id.clone(),
                }),
                tabs,
                None,
            ))
        }
        PanelIntent::Close => Err("close has no commit"),
    }
}

/// The settle classes a given intent can really produce on the wire: `Unchanged` and the
/// `TabUnavailable` refusal only exist for `panel.move`, `CloseAccepted` only for
/// `panel.close`, and only Split/Move ever commit. A committed candidate that folds the
/// selected leaf must also keep at least one baseline member in the Tab, or the Server
/// history is not canonical.
fn settle_class_legal(
    operation: &PanelOperation,
    server_revision: u64,
    class: PanelSettleClass,
    tier: SettleTier,
) -> bool {
    let intent = operation.intent();
    // Tier gate: a Normal settle must be a wire shape a real Server can emit
    // for this intent AND the adapter can produce. A Defensive settle is a
    // preserved forged edge, and the marker is only legal where this driver
    // actually constructs a forged shape.
    let tier_legal = match tier {
        SettleTier::Normal => match class {
            // RejectedFresh is always synthetic (no rejection reason is
            // reachable-consistent at the operation's own baseline), and a real
            // peer removal advances the authority revision so admission yields
            // RevisionMismatch before membership — TabUnavailableGone has no
            // honest variant. In this bounded fixture the native planner
            // builds Move through a same-direction geometric neighbor, so the
            // source is never already the target-direction sibling a Server
            // Unchanged would require; Unchanged stays a Move-only
            // adapter/reducer white-box edge, not normal Server history. (No
            // claim is made here about constructions outside this bounded
            // fixture.)
            PanelSettleClass::RejectedFresh
            | PanelSettleClass::TabUnavailableGone
            | PanelSettleClass::Unchanged => false,
            // Honest at any baseline: NotSubmitted never reaches the Server,
            // Denied is the authorization refusal and Unavailable the
            // transport/ledger preflight — both precede the revision check —
            // and RejectedStale IS the RevisionMismatch path.
            PanelSettleClass::NotSubmitted
            | PanelSettleClass::RejectedStale
            | PanelSettleClass::Denied
            | PanelSettleClass::Unavailable => true,
            // Everything else requires the op's baseline to still be the
            // current authority revision: admission rejects a stale baseline
            // with RevisionMismatch before consulting membership.
            _ => operation.baseline().revision().value() == server_revision,
        },
        SettleTier::Defensive => match class {
            PanelSettleClass::RejectedFresh
            | PanelSettleClass::RejectedStale
            | PanelSettleClass::TabUnavailableGone => true,
            // The forged edge is ServerBusy on a Move — outside Move's
            // wire-compatible reason set. On Split/Close ServerBusy is an
            // honest reason, so the defensive marker would be a false label.
            PanelSettleClass::Unavailable => matches!(intent, PanelIntent::Move { .. }),
            // Unchanged keeps its Move-only adapter/reducer white-box edge;
            // the intent legality below still confines it to Move.
            PanelSettleClass::Unchanged => true,
            _ => false,
        },
    };
    if !tier_legal {
        return false;
    }
    match class {
        PanelSettleClass::CommittedMerge | PanelSettleClass::CommittedRecovery => {
            matches!(intent, PanelIntent::Split { .. } | PanelIntent::Move { .. })
        }
        PanelSettleClass::CommittedLeafRepair => {
            matches!(intent, PanelIntent::Split { .. } | PanelIntent::Move { .. })
                && operation
                    .baseline()
                    .tab_index(operation.target().tab_id())
                    .and_then(|index| operation.baseline().tabs().get(index))
                    .is_some_and(|tab| tab.panel_count() >= 2)
        }
        PanelSettleClass::Unchanged | PanelSettleClass::TabUnavailableGone => {
            matches!(intent, PanelIntent::Move { .. })
        }
        PanelSettleClass::CloseAccepted => matches!(intent, PanelIntent::Close),
        _ => true,
    }
}

fn panel_submit(trace: &mut TraceState, kind: u8, direction: u8) -> Result<(), &'static str> {
    let kind = match kind % 3 {
        0 => PanelRequestKind::Split {
            direction: panel_direction(direction),
        },
        1 => PanelRequestKind::Close,
        _ => PanelRequestKind::Move {
            direction: panel_direction(direction),
        },
    };
    trace.request = trace.request.checked_add(1).ok_or("request")?;
    match trace
        .session
        .plan_panel(workbench_request_id(trace.request), kind, Instant::now())
    {
        Some(operation) => {
            if !trace.session.begin_panel(operation) {
                return Err("panel begin");
            }
            trace.coverage.panel_submissions += 1;
        }
        None => trace.coverage.panel_plan_refusals += 1,
    }
    Ok(())
}

fn panel_settle(
    trace: &mut TraceState,
    class: PanelSettleClass,
    tier: SettleTier,
) -> Result<(), &'static str> {
    let Some(operation) = trace.session.pending_panel().cloned() else {
        return Ok(());
    };
    if !settle_class_legal(&operation, trace.server_revision, class, tier) {
        return Ok(());
    }
    // While a panel operation is in flight every other structural admission must refuse:
    // this is the pending-op/refresh/switch/panel interleave the witness must reach.
    let deadlines = OperationDeadlines::from_sample(Instant::now()).ok_or("deadline")?;
    if trace
        .session
        .plan_panel(
            workbench_request_id(u128::from(u64::MAX)),
            PanelRequestKind::Close,
            Instant::now(),
        )
        .is_some()
        || trace.session.begin_panel(operation.clone())
        || trace.session.phase_allows_refresh()
        || trace
            .session
            .plan_panel_focus(
                PanelDirection::Left,
                trace.generation.saturating_add(1),
                deadlines,
            )
            .is_some()
        || !matches!(
            trace.session.plan_selection(ClientAction::SelectTab(1)),
            SelectionPlan::Busy
        )
    {
        return Err("pending panel did not exclude structure");
    }
    trace.coverage.panel_structure_exclusions += 1;

    let baseline_revision = operation.baseline().revision().value();
    // The authority revision is monotone and always at or above the op's
    // baseline (peer commits may already have advanced it), so any commit or
    // removal the Server performs lands at server_revision + 1 — not merely
    // baseline + 1, which could collide with an existing peer revision.
    let commit_revision = trace.server_revision.checked_add(1).ok_or("revision")?;
    let terminal_context_id = operation.baseline().terminal_context_id().clone();
    let tab_id = operation.target().tab_id().clone();
    let result = match class {
        PanelSettleClass::NotSubmitted => PanelResult::NotSubmitted,
        // A Rejected precommit evaluated at the operation's own baseline. No existing
        // WorkbenchRejectionReason is reachable-consistent here: tree-derived reasons
        // (PanelUnavailable/TabUnavailable/ResourceLimit) contradict a baseline snapshot
        // where the leaf still exists, and ledger/allocator reasons
        // (RequestIdentityConflict/IdentityExhausted) contradict the trace's unique
        // request ids or later commits. This is therefore a deliberately synthetic
        // defensive input — it exercises only the fresh-rejection → PanelNotApplied
        // mapping (pending cleared, no commit retained) and is not counted as real
        // Server-history evidence.
        PanelSettleClass::RejectedFresh => PanelResult::Precommit(PanelPrecommit::Rejected {
            revision: WorkbenchRevision::new(baseline_revision),
            reason: WorkbenchRejectionReason::PanelUnavailable,
        }),
        PanelSettleClass::RejectedStale => match tier {
            // Honest path: a peer commit advanced the authority past the
            // operation's baseline before admission, so the rejection cites
            // the CURRENT authority revision — the real RevisionMismatch shape.
            // The peer commit is a tab.create: a pure append is the only
            // member change a later canonical refresh can still deliver —
            // retained member tuples pin every adopted panel's label/binding.
            SettleTier::Normal => {
                let peer_tab = fresh_tab(trace);
                trace.server_tabs.push(peer_tab);
                trace.server_revision = trace.server_revision.checked_add(1).ok_or("revision")?;
                PanelResult::Precommit(PanelPrecommit::Rejected {
                    revision: WorkbenchRevision::new(trace.server_revision),
                    reason: WorkbenchRejectionReason::RevisionMismatch,
                })
            }
            // Forged edge: a rejection below the baseline — unreachable under
            // a monotone authority revision.
            SettleTier::Defensive => PanelResult::Precommit(PanelPrecommit::Rejected {
                revision: WorkbenchRevision::new(baseline_revision.saturating_sub(1)),
                reason: WorkbenchRejectionReason::RevisionMismatch,
            }),
        },
        PanelSettleClass::Denied => PanelResult::Precommit(PanelPrecommit::Denied),
        PanelSettleClass::Unavailable => {
            let reason = match tier {
                // Rotate across the intent's real wire-compatible reason set;
                // on a stale baseline only the pre-admission reason survives.
                SettleTier::Normal => {
                    let honest = honest_unavailable_reasons(
                        intent_tag(operation.intent()),
                        baseline_revision == trace.server_revision,
                    );
                    honest[usize::try_from(trace.request).unwrap_or(0) % honest.len()]
                }
                // Forged edge: ServerBusy, outside Move's wire-compatible set
                // (the tier gate confines this construction to Move).
                SettleTier::Defensive => WorkbenchCommandUnavailableReason::ServerBusy,
            };
            PanelResult::Precommit(PanelPrecommit::Unavailable { reason })
        }
        // Defensive-only (Move white-box edge): the baseline-stamped no-op a
        // real Server cannot emit for this fixture's planned Moves.
        PanelSettleClass::Unchanged => {
            PanelResult::Unchanged(WorkbenchRevision::new(baseline_revision))
        }
        PanelSettleClass::CloseAccepted => {
            PanelResult::CloseAccepted(WorkbenchPanelCloseAccepted {
                close_request_id: operation.request_id().clone(),
                terminal_context_id,
                tab_id,
                panel_id: operation.panel_id().clone(),
                accepted_revision: WorkbenchRevision::new(baseline_revision),
            })
        }
        PanelSettleClass::CommittedMerge | PanelSettleClass::CommittedRecovery => {
            let (commit, tabs, _added) = commit_edit(
                &mut trace.identity,
                &trace.server_tabs,
                &operation,
                trace.geometry_columns,
            )?;
            trace.server_tabs = tabs;
            trace.server_revision = commit_revision;
            if class == PanelSettleClass::CommittedRecovery {
                trace.recovery_revision = Some(commit_revision);
            }
            PanelResult::Committed {
                commit: Box::new(commit),
                outcome: if class == PanelSettleClass::CommittedMerge {
                    WorkbenchSnapshotOutcome::Available {
                        snapshot: snapshot_with_tabs(trace.server_tabs.clone(), commit_revision),
                    }
                } else {
                    WorkbenchSnapshotOutcome::Denied {}
                },
            }
        }
        PanelSettleClass::TabUnavailableGone => {
            // Defensive-only input (the tier gate admits no Normal variant): a
            // real peer removal advances the authority revision, so a stale
            // baseline fails admission's revision-equality check and emits
            // Rejected{RevisionMismatch} BEFORE membership is consulted — this
            // post-removal-revision TabUnavailable can never come off the
            // wire. It survives solely as a forged exercise of the membership
            // retarget path. The Tab is removed at the revision the forged
            // Server evaluated against; the refusal carries the observed
            // revision where it is absent. A TabUnavailable whose candidate
            // still contains the Tab is not a real Server history either, so
            // it is deliberately not generated.
            trace.server_tabs.retain(|tab| tab.tab_id != tab_id);
            trace.server_revision = commit_revision;
            PanelResult::TabUnavailable {
                revision: WorkbenchRevision::new(commit_revision),
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot_with_tabs(trace.server_tabs.clone(), commit_revision),
                },
            }
        }
        PanelSettleClass::CommittedLeafRepair => {
            // The commit lands at `commit_revision`; a peer close then folds the leaf the
            // commit leaves selected (the source leaf) away in the next revision, so the
            // delivered candidate still contains the committed graft minus that leaf.
            // The result must route through the paired membership path with the commit
            // fact retained — the real selected-leaf-loss route for a panel result.
            let (commit, mut tabs, _added) = commit_edit(
                &mut trace.identity,
                &trace.server_tabs,
                &operation,
                trace.geometry_columns,
            )?;
            let Some(wire) = tabs.iter_mut().find(|tab| tab.tab_id == tab_id) else {
                return Err("leaf-repair tab missing");
            };
            if !collapse_leaf(&mut wire.root, operation.panel_id()) {
                return Err("leaf-repair collapse");
            }
            trace.server_tabs = tabs;
            trace.server_revision = commit_revision.checked_add(1).ok_or("repair revision")?;
            PanelResult::Committed {
                commit: Box::new(commit),
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot_with_tabs(trace.server_tabs.clone(), trace.server_revision),
                },
            }
        }
    };
    trace.coverage.panel_settlements[class.index()] += 1;
    match tier {
        SettleTier::Normal => {
            assert_normal_wire_honest(&operation, &result, trace.server_revision);
            trace.coverage.panel_settlements_normal[class.index()] += 1;
        }
        SettleTier::Defensive => {
            trace.coverage.panel_settlements_defensive[class.index()] += 1;
        }
    }
    trace.settle_log.push(SettleWitness {
        intent: intent_tag(operation.intent()),
        class,
        tier,
        wire: settle_wire(&result),
        baseline_revision,
        server_revision: trace.server_revision,
    });
    apply_panel_result(trace, &operation, result, class)
}

/// The wire-compatible unavailable reasons for each panel intent, mirroring
/// the adapters' compatibility arms: Split accepts `RequestLedgerFull`,
/// `NoCompatibleWorker`, `TerminalBusy`, `ServerBusy`, `ServiceUnavailable`;
/// Move accepts `RequestLedgerFull`, `TerminalBusy`, `ServiceUnavailable`;
/// Close accepts `RequestLedgerFull`, `TerminalBusy`,
/// `TerminalRuntimeUnavailable`, `ServerBusy`, `ServiceUnavailable`.
///
/// The set is stage-sensitive: on a stale baseline the Server's revision check
/// fires before admission ever reaches the reasons a fuller set contains —
/// TerminalBusy (post-admission aggregate close), NoCompatibleWorker and
/// ServerBusy (coordinator/runtime preparation) can never outrun a
/// RevisionMismatch. Only the pre-admission transport/ledger failure is honest
/// there, so the stale set is exactly `ServiceUnavailable` — preflight,
/// wire-compatible with every intent.
fn honest_unavailable_reasons(
    intent: IntentTag,
    fresh: bool,
) -> &'static [WorkbenchCommandUnavailableReason] {
    if !fresh {
        return &[WorkbenchCommandUnavailableReason::ServiceUnavailable];
    }
    match intent {
        IntentTag::Split => &[
            WorkbenchCommandUnavailableReason::RequestLedgerFull,
            WorkbenchCommandUnavailableReason::NoCompatibleWorker,
            WorkbenchCommandUnavailableReason::TerminalBusy,
            WorkbenchCommandUnavailableReason::ServerBusy,
            WorkbenchCommandUnavailableReason::ServiceUnavailable,
        ],
        IntentTag::Move => &[
            WorkbenchCommandUnavailableReason::RequestLedgerFull,
            WorkbenchCommandUnavailableReason::TerminalBusy,
            WorkbenchCommandUnavailableReason::ServiceUnavailable,
        ],
        IntentTag::Close => &[
            WorkbenchCommandUnavailableReason::RequestLedgerFull,
            WorkbenchCommandUnavailableReason::TerminalBusy,
            WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable,
            WorkbenchCommandUnavailableReason::ServerBusy,
            WorkbenchCommandUnavailableReason::ServiceUnavailable,
        ],
    }
}

/// Summarizes the wire fields of a constructed settle input for the audit log.
fn settle_wire(result: &PanelResult) -> SettleWire {
    match result {
        PanelResult::NotSubmitted => SettleWire::NotSubmitted,
        PanelResult::Precommit(PanelPrecommit::Rejected { revision, reason }) => {
            SettleWire::Rejected {
                revision: revision.value(),
                reason: *reason,
            }
        }
        PanelResult::Precommit(PanelPrecommit::Denied) => SettleWire::Denied,
        PanelResult::Precommit(PanelPrecommit::Unavailable { reason }) => {
            SettleWire::Unavailable { reason: *reason }
        }
        PanelResult::Unchanged(revision) => SettleWire::Unchanged {
            revision: revision.value(),
        },
        PanelResult::CloseAccepted(_) => SettleWire::CloseAccepted,
        PanelResult::Committed { .. } => SettleWire::Committed,
        PanelResult::TabUnavailable { revision, .. } => SettleWire::TabUnavailable {
            revision: revision.value(),
        },
    }
}

/// The necessary conditions for a settle input to be an honest Server
/// trajectory rather than a forgery: a rejection must cite a revision the
/// monotone authority counter can produce (and RevisionMismatch cites the
/// current one), and an unavailable reason must be in the intent's real
/// wire-compatible set. Runs inside the driver so a violation fails the trace
/// immediately, and the witness test audits the same properties across the
/// recorded log.
fn assert_normal_wire_honest(
    operation: &PanelOperation,
    result: &PanelResult,
    server_revision: u64,
) {
    match result {
        PanelResult::Precommit(PanelPrecommit::Rejected { revision, reason }) => {
            assert_eq!(
                *reason,
                WorkbenchRejectionReason::RevisionMismatch,
                "normal rejection must be the stale RevisionMismatch shape"
            );
            assert_eq!(
                revision.value(),
                server_revision,
                "normal rejection must cite the current authority revision"
            );
            assert!(
                revision.value() > operation.baseline().revision().value(),
                "normal stale rejection must follow a peer advance"
            );
        }
        PanelResult::Precommit(PanelPrecommit::Unavailable { reason }) => assert!(
            honest_unavailable_reasons(
                intent_tag(operation.intent()),
                operation.baseline().revision().value() == server_revision
            )
            .contains(reason),
            "unavailable reason outside the intent's wire-compatible set for this revision stage"
        ),
        // The planner's same-direction Move never already sits at the target
        // sibling in this fixture, so a Normal Unchanged is a mislabeled edge.
        PanelResult::Unchanged(_) => {
            panic!("Unchanged has no honest variant in this fixture")
        }
        _ => {}
    }
}

/// The real consume order, mirroring `ui/panel.rs`: membership candidates go through
/// `retain_prepared_panel_membership` + the paired cleanup/install — never through
/// `apply_prepared_panel`.
fn apply_panel_result(
    trace: &mut TraceState,
    operation: &PanelOperation,
    result: PanelResult,
    class: PanelSettleClass,
) -> Result<(), &'static str> {
    let baseline_tab = operation
        .baseline()
        .tab_index(operation.target().tab_id())
        .and_then(|index| operation.baseline().tabs().get(index))
        .ok_or("baseline tab")?;
    let baseline_count = baseline_tab.panel_count();
    let baseline_leaf_index = baseline_tab.panel_index(operation.panel_id());
    let prepared = trace.session.prepare_panel(operation, result);
    if prepared.membership_candidate().is_some() {
        return install_panel_membership(
            trace,
            operation,
            prepared,
            class == PanelSettleClass::CommittedLeafRepair,
        );
    }
    let decision = trace.session.apply_prepared_panel(prepared);
    if decision != PanelDecision::Continue {
        return Err("panel settlement");
    }
    match class {
        PanelSettleClass::NotSubmitted
        | PanelSettleClass::RejectedFresh
        | PanelSettleClass::RejectedStale
        | PanelSettleClass::Denied
        | PanelSettleClass::Unavailable => {
            if trace.session.notice() != Some(SessionNotice::PanelNotApplied)
                || trace.session.pending_panel().is_some()
            {
                return Err("not-applied settlement");
            }
        }
        PanelSettleClass::Unchanged => {
            if trace.session.notice().is_some() || trace.session.pending_panel().is_some() {
                return Err("unchanged settlement");
            }
        }
        PanelSettleClass::CloseAccepted => {
            // A close acceptance is only a receipt: the leaf stays put until a later
            // snapshot folds the removal in, and the pending debt is published.
            let index = trace
                .session
                .cache()
                .tab_index(operation.target().tab_id())
                .ok_or("close tab")?;
            if trace.session.notice().is_some()
                || trace.session.pending_panel().is_some()
                || trace.session.publication_debt_kind() != PublicationDebt::Pending
                || trace
                    .session
                    .tab(index)
                    .and_then(|tab| tab.panel_index(operation.panel_id()))
                    .is_none()
                || trace.session.tab(index).map_or(0, |tab| tab.panel_count()) != baseline_count
            {
                return Err("close receipt treated as removal");
            }
        }
        PanelSettleClass::CommittedMerge => {
            let index = trace
                .session
                .cache()
                .tab_index(operation.target().tab_id())
                .ok_or("merged tab")?;
            let merged = trace.session.tab(index).ok_or("merged tab")?;
            if trace.session.notice().is_some()
                || trace.session.pending_panel().is_some()
                || trace.session.retained_panel_for_test().is_none()
            {
                return Err("committed merge settlement");
            }
            match operation.intent() {
                PanelIntent::Split { .. } => {
                    if merged.panel_count() != baseline_count + 1 {
                        return Err("split merge did not add a leaf");
                    }
                }
                PanelIntent::Move { .. } => {
                    if merged.panel_count() != baseline_count
                        || merged.panel_index(operation.panel_id()) == baseline_leaf_index
                    {
                        return Err("move merge did not relocate the leaf");
                    }
                }
                PanelIntent::Close => return Err("close committed"),
            }
            trace.delivered_tabs = trace.server_tabs.clone();
            trace.coverage.panel_merges += 1;
        }
        PanelSettleClass::CommittedRecovery => {
            if trace.session.notice().is_some()
                || trace.session.pending_panel().is_some()
                || trace.session.retained_panel_for_test().is_none()
                || trace.session.panel_recovery_for_test().is_none()
                || trace.session.phase_kind() != SessionPhaseKind::PanelProjectionUnavailable
            {
                return Err("recovery entry");
            }
            trace.coverage.panel_recovery_entries += 1;
        }
        PanelSettleClass::TabUnavailableGone | PanelSettleClass::CommittedLeafRepair => {
            return Err("membership routed through apply");
        }
    }
    Ok(())
}

/// The paired membership consume for a panel result, mirroring `ui/panel.rs` +
/// `run_membership_cleanup`: retain the known outcome, detach the old attachment when one
/// exists, then install the cleaned candidate — never the other way around.
fn install_panel_membership(
    trace: &mut TraceState,
    operation: &PanelOperation,
    prepared: PreparedPanel,
    committed: bool,
) -> Result<(), &'static str> {
    let (consumed, candidate) = trace
        .session
        .retain_prepared_panel_membership(prepared)
        .map_err(|_| "panel membership retain")?;
    if consumed != *operation {
        return Err("membership consumed a different operation");
    }
    // A Committed result retains the exact commit fact for diagnostics and raises no
    // notice; a refusal carries no commit, so the receipt is a not-applied notice.
    if committed {
        if trace.session.retained_panel_for_test().is_none() || trace.session.notice().is_some() {
            return Err("membership dropped the committed fact");
        }
    } else if trace.session.notice() != Some(SessionNotice::PanelNotApplied) {
        return Err("membership retain notice");
    }
    let deadlines = OperationDeadlines::from_sample(Instant::now()).ok_or("deadline")?;
    let fresh = candidate.selected_tab_id().map(|tab| {
        AttachmentKey::new(
            operation.target().generation().saturating_add(1),
            tab.clone(),
        )
    });
    let decision = if trace.session.attached_key().is_some() {
        let cleanup = trace
            .session
            .stage_panel_membership_cleanup(operation.request_id(), candidate, deadlines)
            .map_err(|_| "panel membership cleanup")?;
        trace.coverage.panel_membership_cleanups += 1;
        let cleaned = trace
            .session
            .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
            .map_err(|_| "cleaned candidate")?;
        trace
            .session
            .prepare_cleaned_membership_install(&cleanup, cleaned, fresh)
            .map_err(|_| "cleaned install")?
            .commit()
    } else {
        trace
            .session
            .prepare_panel_membership_install(
                operation.request_id(),
                candidate,
                fresh,
                Some(deadlines),
            )
            .map_err(|_| "panel install")?
            .commit()
    };
    trace.coverage.panel_membership_installs += 1;
    trace.delivered_tabs = trace.server_tabs.clone();
    match decision {
        MembershipInstallDecision::Attach(target) => {
            let key = target.selected().key().clone();
            trace.generation = trace.generation.max(key.generation());
            settle_attached_ready(&mut trace.session, &key, &mut trace.frame)?;
        }
        MembershipInstallDecision::SelectedWithoutAttachment => {
            if trace.session.phase_kind() != SessionPhaseKind::SelectedWithoutAttachment {
                return Err("membership did not settle unattached");
            }
        }
        MembershipInstallDecision::Empty => {
            if trace.session.phase_kind() != SessionPhaseKind::Empty {
                return Err("membership did not settle empty");
            }
        }
        MembershipInstallDecision::Preserved => {
            return Err("panel membership preserved selection");
        }
    }
    if committed {
        // The leaf the commit left selected was folded away, so the membership install
        // must retarget onto a surviving leaf of the same Tab.
        let index = trace
            .session
            .cache()
            .tab_index(operation.target().tab_id())
            .ok_or("leaf-repair tab")?;
        let tab = trace.session.tab(index).ok_or("leaf-repair tab")?;
        let active = trace.session.active().ok_or("leaf-repair active")?;
        if tab.panel_index(operation.panel_id()).is_some()
            || active.index() != index
            || tab.panel(active.panel_index()).is_none()
        {
            return Err("leaf repair did not retarget selection");
        }
    }
    if trace.session.pending_panel().is_some() {
        return Err("membership left a pending panel");
    }
    Ok(())
}

/// The minimal real recovery fold: the next observed revision carries the
/// committed graft (alongside any later peer history), so the projection
/// validator must match it at a revision ≥ `commit_revision`.
fn panel_recovery_refresh(trace: &mut TraceState, generation_step: u8) -> Result<(), &'static str> {
    if trace.session.panel_recovery_for_test().is_none() || trace.recovery_revision.is_none() {
        return Ok(());
    }
    if !trace.session.phase_allows_refresh() {
        return Err("panel recovery blocked refresh admission");
    }
    trace.refresh = trace
        .refresh
        .checked_add(u64::from(generation_step.max(1)))
        .ok_or("refresh")?;
    if !trace.session.start_pending_refresh(trace.refresh) {
        return Err("recovery refresh admission");
    }
    // The candidate is stamped at the CURRENT authority revision: peer commits
    // (for example a tab.create admitted before the stale-rejection path) may
    // have advanced it past `recovery_revision`, and stamping earlier would
    // misattribute their content to the commit revision.
    let revision = trace.server_revision;
    let prepared = trace.session.prepare_refresh(
        trace.refresh,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot_with_tabs(trace.server_tabs.clone(), revision),
        },
    );
    if !matches!(&prepared, PreparedRefresh::Append { .. }) {
        return Err("recovery refresh did not classify as append");
    }
    if trace.session.apply_prepared_refresh(prepared) != RefreshDecision::Continue {
        return Err("recovery refresh settlement");
    }
    if trace.session.panel_recovery_for_test().is_some()
        || trace.session.pending_refresh_generation().is_some()
        || trace.session.refresh_status()
            != Some(NavigatorRefreshStatus::Success(WorkbenchRevision::new(
                revision,
            )))
    {
        return Err("recovery not resolved");
    }
    trace.delivered_tabs = trace.server_tabs.clone();
    trace.recovery_revision = None;
    trace.coverage.panel_recovery_resolutions += 1;
    Ok(())
}

/// A peer close folds the selected leaf away while the Tab keeps its identity — the real
/// refresh-side selected-leaf-loss repair through the paired membership path.
fn refresh_leaf_repair(trace: &mut TraceState, generation_step: u8) -> Result<(), &'static str> {
    if !trace.session.phase_allows_refresh() {
        return Ok(());
    }
    let Some(active) = trace.session.active().cloned() else {
        return Ok(());
    };
    let Some(tab) = trace.session.tab(active.index()) else {
        return Ok(());
    };
    if tab.panel_count() < 2 {
        return Ok(());
    }
    let removed = tab
        .panel(active.panel_index())
        .ok_or("repair leaf")?
        .panel_id()
        .clone();
    let expected_panel_index = active
        .panel_index()
        .min(tab.panel_count().saturating_sub(2));
    let Some(wire) = trace
        .server_tabs
        .iter_mut()
        .find(|tab| tab.tab_id == *active.key().tab_id())
    else {
        return Err("repair tab missing");
    };
    if !collapse_leaf(&mut wire.root, &removed) {
        return Err("repair collapse");
    }
    trace.server_revision = trace.server_revision.checked_add(1).ok_or("revision")?;
    trace.refresh = trace
        .refresh
        .checked_add(u64::from(generation_step.max(1)))
        .ok_or("refresh")?;
    if !trace.session.start_pending_refresh(trace.refresh) {
        return Err("repair refresh admission");
    }
    let prepared = trace.session.prepare_refresh(
        trace.refresh,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot_with_tabs(trace.server_tabs.clone(), trace.server_revision),
        },
    );
    let PreparedRefresh::Membership {
        generation,
        prepared,
    } = prepared
    else {
        return Err("leaf repair did not route to membership");
    };
    let deadlines = OperationDeadlines::from_sample(Instant::now()).ok_or("deadline")?;
    let fresh_generation = trace
        .session
        .desired()
        .map(|desired| desired.generation())
        .unwrap_or(0)
        .checked_add(1)
        .ok_or("generation")?;
    let fresh = prepared
        .selected_tab_id()
        .map(|tab| AttachmentKey::new(fresh_generation, tab.clone()));
    let decision = if trace.session.attached_key().is_some() {
        let cleanup = trace
            .session
            .stage_refresh_membership_cleanup(generation, prepared, deadlines)
            .map_err(|_| "repair cleanup")?;
        trace.coverage.panel_membership_cleanups += 1;
        let cleaned = trace
            .session
            .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
            .map_err(|_| "cleaned candidate")?;
        trace
            .session
            .prepare_cleaned_membership_install(&cleanup, cleaned, fresh)
            .map_err(|_| "cleaned install")?
            .commit()
    } else {
        trace
            .session
            .prepare_refresh_membership_install(generation, prepared, fresh, Some(deadlines))
            .map_err(|_| "repair install")?
            .commit()
    };
    trace.coverage.panel_membership_installs += 1;
    trace.delivered_tabs = trace.server_tabs.clone();
    trace.recovery_revision = None;
    match decision {
        MembershipInstallDecision::Attach(target) => {
            let key = target.selected().key().clone();
            trace.generation = trace.generation.max(key.generation());
            settle_attached_ready(&mut trace.session, &key, &mut trace.frame)?;
        }
        MembershipInstallDecision::SelectedWithoutAttachment => {
            if trace.session.phase_kind() != SessionPhaseKind::SelectedWithoutAttachment {
                return Err("repair did not settle unattached");
            }
        }
        MembershipInstallDecision::Empty => {
            if trace.session.phase_kind() != SessionPhaseKind::Empty {
                return Err("repair did not settle empty");
            }
        }
        MembershipInstallDecision::Preserved => {
            return Err("leaf repair preserved selection");
        }
    }
    if trace.session.active().map(SelectedTab::panel_index) != Some(expected_panel_index) {
        return Err("leaf repair landed on wrong index");
    }
    trace.coverage.refresh_leaf_repairs += 1;
    Ok(())
}

fn panel_focus(
    trace: &mut TraceState,
    direction: u8,
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
    let direction = panel_direction(direction);
    let active = trace.session.active().cloned().ok_or("active")?;
    let expected = trace
        .session
        .tab(active.index())
        .and_then(|tab| tab.panel_focus_target(active.panel_index(), direction));
    let generation = advance_generation(trace, generation_step)?;
    let deadlines = OperationDeadlines::from_sample(Instant::now()).ok_or("deadline")?;
    let Some(target) = trace
        .session
        .plan_panel_focus(direction, generation, deadlines)
    else {
        trace.coverage.panel_plan_refusals += 1;
        return Ok(());
    };
    if expected.is_none() {
        return Err("focus planned without a neighbor");
    }
    let marker = target.selected().key().clone();
    if !trace.session.begin_switch(target) {
        return Err("focus begin");
    }
    let fresh = prepare_and_install_switch_candidate(&mut trace.session, &marker)
        .map_err(|_| "focus install")?;
    trace.generation = fresh.generation();
    settle_attached_ready(&mut trace.session, &fresh, &mut trace.frame)?;
    if trace.session.active().map(SelectedTab::panel_index) != expected {
        return Err("focus landed on wrong leaf");
    }
    trace.coverage.panel_focuses += 1;
    Ok(())
}

/// The shared Ready tail after a fresh attach key is installed (switch, focus, or
/// membership recovery all end here).
fn settle_attached_ready(
    session: &mut NativeSessionState,
    key: &AttachmentKey,
    frame: &mut u64,
) -> Result<(), &'static str> {
    if !session.install_projection(key)
        || !session.record_setup_configuration(key, controlling(), Some(session.latest_geometry()))
    {
        return Err("setup");
    }
    *frame = frame.checked_add(1).ok_or("frame")?;
    let frame = FrameIdentity::new(*frame);
    if !session.stage_drawn_frame(key, frame)
        || !session.mark_feedback_accepted(key, frame)
        || session
            .settle_ready_candidate(key, frame, controlling(), Some(session.latest_geometry()))
            .map_err(|_| "ready")?
            != ReadyDecision::Ready
    {
        return Err("ready");
    }
    Ok(())
}

fn panel_anchor(trace: &mut TraceState, class: PanelAnchorClass) -> Result<(), &'static str> {
    match class {
        PanelAnchorClass::DivergentCommit => anchor_divergent_commit()?,
        PanelAnchorClass::StaleOperation => anchor_stale_operation()?,
        PanelAnchorClass::ForeignRetain => anchor_foreign_retain()?,
        PanelAnchorClass::MembershipGates => anchor_membership_gates()?,
        PanelAnchorClass::ApplyMembershipRetires => anchor_apply_membership_retires()?,
        PanelAnchorClass::RefreshMembershipRetires => anchor_refresh_membership_retires()?,
        PanelAnchorClass::RetireWithPending => anchor_retire_with_pending()?,
    }
    trace.coverage.panel_anchor_witnesses[class.index()] += 1;
    Ok(())
}

/// A fresh attached Session whose selected Tab has two real leaves (committed through one
/// leaf-level split) and a pending Move — the shared fixture for membership anchors.
fn anchor_pending_move()
-> Result<(NativeSessionState, PanelOperation, Vec<Tab>, u128), &'static str> {
    let mut session = ready(2);
    let mut identity = 0xBEEF_0000u128;
    let mut frame = 1u64;
    let deadlines = OperationDeadlines::from_sample(Instant::now()).ok_or("deadline")?;
    let mut tabs = snapshot(2, 1, None).terminal_contexts[0].tabs.clone();
    let operation = session
        .plan_panel(
            workbench_request_id(next_identity(&mut identity)),
            PanelRequestKind::Split {
                direction: PanelDirection::Right,
            },
            Instant::now(),
        )
        .ok_or("anchor split plan")?;
    if !session.begin_panel(operation.clone()) {
        return Err("anchor split begin");
    }
    let (commit, grafted, _) = commit_edit(&mut identity, &tabs, &operation, 80)?;
    tabs = grafted;
    let revision = operation.baseline().revision().value() + 1;
    let prepared = session.prepare_panel(
        &operation,
        PanelResult::Committed {
            commit: Box::new(commit),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: snapshot_with_tabs(tabs.clone(), revision),
            },
        },
    );
    if session.apply_prepared_panel(prepared) != PanelDecision::Continue {
        return Err("anchor split merge");
    }
    let target = session
        .plan_panel_focus(PanelDirection::Right, 2, deadlines)
        .ok_or("anchor focus")?;
    let marker = target.selected().key().clone();
    if !session.begin_switch(target) {
        return Err("anchor focus begin");
    }
    let fresh = prepare_and_install_switch_candidate(&mut session, &marker)
        .map_err(|_| "anchor focus install")?;
    settle_attached_ready(&mut session, &fresh, &mut frame)?;
    let operation = session
        .plan_panel(
            workbench_request_id(next_identity(&mut identity)),
            PanelRequestKind::Move {
                direction: PanelDirection::Left,
            },
            Instant::now(),
        )
        .ok_or("anchor move plan")?;
    if !session.begin_panel(operation.clone()) {
        return Err("anchor move begin");
    }
    Ok((session, operation, tabs, identity))
}

/// A committed result whose candidate omits the graft retires, but the exact committed
/// fact must survive the failure for diagnostics.
fn anchor_divergent_commit() -> Result<(), &'static str> {
    let mut session = ready(1);
    let mut identity = 0xD1B5_0000u128;
    let operation = session
        .plan_panel(
            workbench_request_id(next_identity(&mut identity)),
            PanelRequestKind::Split {
                direction: PanelDirection::Right,
            },
            Instant::now(),
        )
        .ok_or("anchor plan")?;
    if !session.begin_panel(operation.clone()) {
        return Err("anchor begin");
    }
    let tabs = snapshot(1, 1, None).terminal_contexts[0].tabs.clone();
    let (commit, _grafted, _) = commit_edit(&mut identity, &tabs, &operation, 80)?;
    let prepared = session.prepare_panel(
        &operation,
        PanelResult::Committed {
            commit: Box::new(commit),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: snapshot_with_tabs(tabs, 2),
            },
        },
    );
    if session.apply_prepared_panel(prepared) != PanelDecision::Retire
        || session.phase_kind() != SessionPhaseKind::Retired
        || session.retired_failure() != Some(CarrierFailure::AuthorityDivergence)
        || session.retained_panel_for_test().is_none()
        || !session.invariant_holds()
    {
        return Err("divergent commit retention");
    }
    Ok(())
}

/// Applying a foreign operation's result is a stale-operation rejection, never a merge.
fn anchor_stale_operation() -> Result<(), &'static str> {
    let mut session = ready(1);
    let first = session
        .plan_panel(
            workbench_request_id(1),
            PanelRequestKind::Split {
                direction: PanelDirection::Right,
            },
            Instant::now(),
        )
        .ok_or("first plan")?;
    let second = session
        .plan_panel(
            workbench_request_id(2),
            PanelRequestKind::Split {
                direction: PanelDirection::Down,
            },
            Instant::now(),
        )
        .ok_or("second plan")?;
    if !session.begin_panel(first) {
        return Err("stale begin");
    }
    let prepared = session.prepare_panel(&second, PanelResult::NotSubmitted);
    if session.apply_prepared_panel(prepared) != PanelDecision::Retire
        || session.phase_kind() != SessionPhaseKind::Retired
        || session.retired_failure() != Some(CarrierFailure::Protocol)
        || !session.invariant_holds()
    {
        return Err("stale operation");
    }
    Ok(())
}

/// `retain_pending_panel` refuses a foreign operation, retains the real commit
/// idempotently, and the same commit then merges cleanly.
fn anchor_foreign_retain() -> Result<(), &'static str> {
    let mut session = ready(1);
    let mut identity = 0xF00D_0000u128;
    let tabs = snapshot(1, 1, None).terminal_contexts[0].tabs.clone();
    let operation = session
        .plan_panel(
            workbench_request_id(next_identity(&mut identity)),
            PanelRequestKind::Split {
                direction: PanelDirection::Right,
            },
            Instant::now(),
        )
        .ok_or("retain plan")?;
    let foreign = session
        .plan_panel(
            workbench_request_id(next_identity(&mut identity)),
            PanelRequestKind::Split {
                direction: PanelDirection::Down,
            },
            Instant::now(),
        )
        .ok_or("foreign plan")?;
    if !session.begin_panel(operation.clone()) {
        return Err("retain begin");
    }
    let (foreign_commit, _, _) = commit_edit(&mut identity, &tabs, &foreign, 80)?;
    if session.retain_pending_panel(PendingPanelCommit::new(foreign, foreign_commit)) {
        return Err("foreign commit retained");
    }
    let (commit, grafted, _) = commit_edit(&mut identity, &tabs, &operation, 80)?;
    let pending = PendingPanelCommit::new(operation.clone(), commit.clone());
    if !session.retain_pending_panel(pending.clone())
        || !session.retain_pending_panel(pending.clone())
        || session.retained_panel_for_test() != Some(&pending)
    {
        return Err("commit not retained idempotently");
    }
    let prepared = session.prepare_panel(
        &operation,
        PanelResult::Committed {
            commit: Box::new(commit),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: snapshot_with_tabs(grafted, 2),
            },
        },
    );
    if session.apply_prepared_panel(prepared) != PanelDecision::Continue
        || !session.invariant_holds()
    {
        return Err("retained commit did not merge");
    }
    Ok(())
}

/// The membership pair is strictly ordered and currency-bound: direct install while
/// attached refuses, a second cleanup cannot stage over the first, the cleaned candidate
/// only binds to the exact cleanup, and once the pair commits the consumed request is
/// stale for every entry point.
fn anchor_membership_gates() -> Result<(), &'static str> {
    let (mut session, operation, mut tabs, _identity) = anchor_pending_move()?;
    let request_id = operation.request_id().clone();
    tabs.retain(|tab| tab.tab_id != *operation.target().tab_id());
    let revision = operation.baseline().revision().value() + 1;
    let snapshot = snapshot_with_tabs(tabs, revision);
    let prepared = session.prepare_panel(
        &operation,
        PanelResult::TabUnavailable {
            revision: WorkbenchRevision::new(revision),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: snapshot.clone(),
            },
        },
    );
    if prepared.membership_candidate().is_none() {
        return Err("membership routing missed");
    }
    let (consumed, candidate) = session
        .retain_prepared_panel_membership(prepared)
        .map_err(|_| "membership retain")?;
    if consumed != operation || session.notice() != Some(SessionNotice::PanelNotApplied) {
        return Err("membership retain facts");
    }
    let deadlines = OperationDeadlines::from_sample(Instant::now()).ok_or("deadline")?;
    let rebuild = |session: &NativeSessionState| {
        session
            .prepare_membership_candidate(
                project_native_standalone_tabs(&snapshot).map_err(|_| "candidate projection")?,
                Some(operation.target().tab_id()),
                operation.baseline().tab_index(operation.target().tab_id()),
                None,
            )
            .map_err(|_| "candidate rebuild")
    };
    // Install while attached without the cleanup refuses before the candidate is consumed.
    let fresh = rebuild(&session)?
        .selected_tab_id()
        .map(|tab| AttachmentKey::new(operation.target().generation() + 1, tab.clone()));
    if session
        .prepare_panel_membership_install(&request_id, rebuild(&session)?, fresh, Some(deadlines))
        .is_ok()
    {
        return Err("install admitted before cleanup");
    }
    let cleanup = session
        .stage_panel_membership_cleanup(&request_id, candidate, deadlines)
        .map_err(|_| "cleanup stage")?;
    if session.phase_kind() != SessionPhaseKind::Switching
        || session
            .stage_panel_membership_cleanup(&request_id, rebuild(&session)?, deadlines)
            .is_ok()
    {
        return Err("second cleanup staged");
    }
    // The cleaned candidate only binds to the exact cleanup: a foreign detached key refuses.
    let foreign = AttachmentKey::new(u64::MAX - 1, operation.target().tab_id().clone());
    if session
        .prepare_cleaned_membership_candidate(&cleanup, &foreign)
        .is_ok()
    {
        return Err("cleaned candidate bound a foreign key");
    }
    let cleaned = session
        .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
        .map_err(|_| "cleaned candidate")?;
    let fresh = cleaned
        .selected_tab_id()
        .map(|tab| AttachmentKey::new(operation.target().generation() + 1, tab.clone()));
    let decision = session
        .prepare_cleaned_membership_install(&cleanup, cleaned, fresh)
        .map_err(|_| "cleaned install")?
        .commit();
    if session.pending_panel().is_some()
        || !matches!(decision, MembershipInstallDecision::Attach(_))
        || session.phase_kind() != SessionPhaseKind::Switching
    {
        return Err("membership pair commit");
    }
    // Post-commit the consumed request is stale for both entry points.
    if session
        .prepare_panel_membership_install(&request_id, rebuild(&session)?, None, Some(deadlines))
        .is_ok()
        || session
            .stage_panel_membership_cleanup(&request_id, rebuild(&session)?, deadlines)
            .is_ok()
    {
        return Err("stale membership entry admitted");
    }
    if !session.invariant_holds() {
        return Err("membership gates invariant");
    }
    Ok(())
}

/// `apply_prepared_panel` must never consume a membership result — the paired consumer is
/// the only legal route.
fn anchor_apply_membership_retires() -> Result<(), &'static str> {
    let (mut session, operation, mut tabs, _identity) = anchor_pending_move()?;
    tabs.retain(|tab| tab.tab_id != *operation.target().tab_id());
    let revision = operation.baseline().revision().value() + 1;
    let prepared = session.prepare_panel(
        &operation,
        PanelResult::TabUnavailable {
            revision: WorkbenchRevision::new(revision),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: snapshot_with_tabs(tabs, revision),
            },
        },
    );
    if prepared.membership_candidate().is_none() {
        return Err("membership routing missed");
    }
    if session.apply_prepared_panel(prepared) != PanelDecision::Retire
        || session.retired_failure() != Some(CarrierFailure::Protocol)
        || !session.invariant_holds()
    {
        return Err("membership apply did not retire");
    }
    Ok(())
}

/// The refresh-side membership routing gate: a candidate that folds away the selected leaf
/// must never pass through `apply_prepared_refresh`.
fn anchor_refresh_membership_retires() -> Result<(), &'static str> {
    let (mut session, operation, tabs, mut identity) = anchor_pending_move()?;
    // Settle the pending move through a real committed merge so refresh admission opens.
    let revision = operation.baseline().revision().value() + 1;
    let (commit, grafted, _) = commit_edit(&mut identity, &tabs, &operation, 80)?;
    let tabs = grafted;
    let prepared = session.prepare_panel(
        &operation,
        PanelResult::Committed {
            commit: Box::new(commit),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: snapshot_with_tabs(tabs.clone(), revision),
            },
        },
    );
    if session.apply_prepared_panel(prepared) != PanelDecision::Continue {
        return Err("anchor move merge");
    }
    let active = session.active().cloned().ok_or("active")?;
    let leaf = session
        .tab(active.index())
        .and_then(|tab| tab.panel(active.panel_index()))
        .ok_or("leaf")?
        .panel_id()
        .clone();
    let mut folded = tabs;
    let Some(wire) = folded
        .iter_mut()
        .find(|tab| tab.tab_id == *active.key().tab_id())
    else {
        return Err("wire tab");
    };
    if !collapse_leaf(&mut wire.root, &leaf) {
        return Err("wire collapse");
    }
    if !session.start_pending_refresh(9) {
        return Err("refresh admission");
    }
    let prepared = session.prepare_refresh(
        9,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot_with_tabs(folded, revision + 1),
        },
    );
    if !matches!(prepared, PreparedRefresh::Membership { .. }) {
        return Err("refresh leaf-loss did not route to membership");
    }
    if session.apply_prepared_refresh(prepared) != RefreshDecision::Retire
        || session.retired_failure() != Some(CarrierFailure::Protocol)
        || !session.invariant_holds()
    {
        return Err("refresh membership apply did not retire");
    }
    Ok(())
}

/// Retirement keeps the pending operation as diagnostic state without dropping it.
fn anchor_retire_with_pending() -> Result<(), &'static str> {
    let mut session = ready(1);
    let operation = session
        .plan_panel(
            workbench_request_id(7),
            PanelRequestKind::Close,
            Instant::now(),
        )
        .ok_or("plan")?;
    if !session.begin_panel(operation) {
        return Err("begin");
    }
    session.retire(CarrierFailure::Deadline);
    if session.phase_kind() != SessionPhaseKind::Retired
        || session.pending_panel().is_none()
        || !session.invariant_holds()
    {
        return Err("pending panel did not survive retirement as diagnostic");
    }
    Ok(())
}

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
        | TraceAction::ClearNotice
        | TraceAction::PanelSettle { .. }
        | TraceAction::PanelAnchor { .. } => Vec::new(),
        TraceAction::PanelFocus {
            direction,
            generation_step,
        } => shrink_pair(direction, generation_step)
            .into_iter()
            .map(|(direction, generation_step)| TraceAction::PanelFocus {
                direction,
                generation_step,
            })
            .collect(),
        TraceAction::PanelSubmit { kind, direction } => shrink_pair(kind, direction)
            .into_iter()
            .map(|(kind, direction)| TraceAction::PanelSubmit { kind, direction })
            .collect(),
        TraceAction::PanelRecoveryRefresh { generation_step }
        | TraceAction::RefreshLeafRepair { generation_step } => shrink_u8(generation_step)
            .into_iter()
            .map(|generation_step| match action {
                TraceAction::PanelRecoveryRefresh { .. } => {
                    TraceAction::PanelRecoveryRefresh { generation_step }
                }
                TraceAction::RefreshLeafRepair { .. } => {
                    TraceAction::RefreshLeafRepair { generation_step }
                }
                _ => unreachable!(),
            })
            .collect(),
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
            TraceAction::PanelFocus {
                direction,
                generation_step,
            } => format!("pf:{direction}:{generation_step}"),
            TraceAction::PanelSubmit { kind, direction } => format!("ps:{kind}:{direction}"),
            TraceAction::PanelSettle { class, tier } => {
                format!(
                    "pt:{}:{}",
                    panel_settle_class_token(*class),
                    settle_tier_token(*tier)
                )
            }
            TraceAction::PanelRecoveryRefresh { generation_step } => {
                format!("pv:{generation_step}")
            }
            TraceAction::RefreshLeafRepair { generation_step } => {
                format!("rl:{generation_step}")
            }
            TraceAction::PanelAnchor { class } => {
                format!("pa:{}", panel_anchor_class_token(*class))
            }
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
        ["pf", _, _] => Ok(TraceAction::PanelFocus {
            direction: number(1)?,
            generation_step: number(2)?,
        }),
        ["ps", _, _] => Ok(TraceAction::PanelSubmit {
            kind: number(1)?,
            direction: number(2)?,
        }),
        ["pt", class, tier] => Ok(TraceAction::PanelSettle {
            class: parse_panel_settle_class(class)?,
            tier: parse_settle_tier(tier)?,
        }),
        ["pv", _] => Ok(TraceAction::PanelRecoveryRefresh {
            generation_step: number(1)?,
        }),
        ["rl", _] => Ok(TraceAction::RefreshLeafRepair {
            generation_step: number(1)?,
        }),
        ["pa", class] => Ok(TraceAction::PanelAnchor {
            class: parse_panel_anchor_class(class)?,
        }),
        _ => Err("invalid action"),
    }
}

const fn settle_tier_token(tier: SettleTier) -> &'static str {
    match tier {
        SettleTier::Normal => "n",
        SettleTier::Defensive => "d",
    }
}

fn parse_settle_tier(token: &str) -> Result<SettleTier, &'static str> {
    match token {
        "n" => Ok(SettleTier::Normal),
        "d" => Ok(SettleTier::Defensive),
        _ => Err("invalid settle tier"),
    }
}

const fn panel_settle_class_token(class: PanelSettleClass) -> &'static str {
    match class {
        PanelSettleClass::NotSubmitted => "n",
        PanelSettleClass::RejectedFresh => "f",
        PanelSettleClass::RejectedStale => "s",
        PanelSettleClass::Denied => "d",
        PanelSettleClass::Unavailable => "u",
        PanelSettleClass::Unchanged => "c",
        PanelSettleClass::CloseAccepted => "a",
        PanelSettleClass::CommittedMerge => "m",
        PanelSettleClass::CommittedRecovery => "r",
        PanelSettleClass::TabUnavailableGone => "g",
        PanelSettleClass::CommittedLeafRepair => "l",
    }
}

fn parse_panel_settle_class(token: &str) -> Result<PanelSettleClass, &'static str> {
    match token {
        "n" => Ok(PanelSettleClass::NotSubmitted),
        "f" => Ok(PanelSettleClass::RejectedFresh),
        "s" => Ok(PanelSettleClass::RejectedStale),
        "d" => Ok(PanelSettleClass::Denied),
        "u" => Ok(PanelSettleClass::Unavailable),
        "c" => Ok(PanelSettleClass::Unchanged),
        "a" => Ok(PanelSettleClass::CloseAccepted),
        "m" => Ok(PanelSettleClass::CommittedMerge),
        "r" => Ok(PanelSettleClass::CommittedRecovery),
        "g" => Ok(PanelSettleClass::TabUnavailableGone),
        "l" => Ok(PanelSettleClass::CommittedLeafRepair),
        _ => Err("invalid panel settle class"),
    }
}

const fn panel_anchor_class_token(class: PanelAnchorClass) -> &'static str {
    match class {
        PanelAnchorClass::DivergentCommit => "d",
        PanelAnchorClass::StaleOperation => "s",
        PanelAnchorClass::ForeignRetain => "f",
        PanelAnchorClass::MembershipGates => "g",
        PanelAnchorClass::ApplyMembershipRetires => "m",
        PanelAnchorClass::RefreshMembershipRetires => "r",
        PanelAnchorClass::RetireWithPending => "p",
    }
}

fn parse_panel_anchor_class(token: &str) -> Result<PanelAnchorClass, &'static str> {
    match token {
        "d" => Ok(PanelAnchorClass::DivergentCommit),
        "s" => Ok(PanelAnchorClass::StaleOperation),
        "f" => Ok(PanelAnchorClass::ForeignRetain),
        "g" => Ok(PanelAnchorClass::MembershipGates),
        "m" => Ok(PanelAnchorClass::ApplyMembershipRetires),
        "r" => Ok(PanelAnchorClass::RefreshMembershipRetires),
        "p" => Ok(PanelAnchorClass::RetireWithPending),
        _ => Err("invalid panel anchor class"),
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
        TraceAction::PanelFocus {
            direction: 3,
            generation_step: 13,
        },
        TraceAction::PanelSubmit {
            kind: 2,
            direction: 1,
        },
        TraceAction::PanelSettle {
            class: PanelSettleClass::CommittedMerge,
            tier: SettleTier::Normal,
        },
        TraceAction::PanelSettle {
            class: PanelSettleClass::CommittedLeafRepair,
            tier: SettleTier::Normal,
        },
        TraceAction::PanelSettle {
            class: PanelSettleClass::RejectedStale,
            tier: SettleTier::Defensive,
        },
        TraceAction::PanelRecoveryRefresh {
            generation_step: 14,
        },
        TraceAction::RefreshLeafRepair {
            generation_step: 15,
        },
        TraceAction::PanelAnchor {
            class: PanelAnchorClass::MembershipGates,
        },
    ];
    assert_eq!(deserialize(&serialize(&actions)), Ok(actions.clone()));
    assert!(deserialize("").is_err());
    assert!(deserialize("cc:x:1").is_err());
    assert!(deserialize("pt:x").is_err());
    assert!(deserialize("pt:s").is_err());
    assert!(deserialize("pt:s:x").is_err());
    assert!(deserialize("pa:x").is_err());
    for action in actions {
        assert!(
            shrink_values(action)
                .into_iter()
                .all(|candidate| candidate.class() == action.class())
        );
    }
}

/// Submit fan + settle pair for every (intent, class, tier) the plan admits.
/// A multi-leaf tab is committed first so Move can be planned; combinations
/// the class/tier gate rejects self-skip inside `panel_settle`.
fn settle_probe_actions() -> Vec<TraceAction> {
    let mut actions = vec![
        TraceAction::PanelSubmit {
            kind: 0,
            direction: 1,
        },
        TraceAction::PanelSettle {
            class: PanelSettleClass::CommittedMerge,
            tier: SettleTier::Normal,
        },
    ];
    for kind in [0u8, 1, 2] {
        for class in [
            PanelSettleClass::NotSubmitted,
            PanelSettleClass::RejectedFresh,
            PanelSettleClass::RejectedStale,
            PanelSettleClass::Denied,
            PanelSettleClass::Unavailable,
            PanelSettleClass::Unchanged,
            PanelSettleClass::TabUnavailableGone,
        ] {
            for tier in [SettleTier::Normal, SettleTier::Defensive] {
                for direction in 0..4u8 {
                    actions.push(TraceAction::PanelSubmit { kind, direction });
                }
                actions.push(TraceAction::PanelSettle { class, tier });
                // A gate-skipped settle leaves the pending op alive; drain it
                // so the next iteration's fan mints the intended intent.
                actions.push(TraceAction::PanelSettle {
                    class: PanelSettleClass::NotSubmitted,
                    tier: SettleTier::Normal,
                });
            }
        }
    }
    actions
}

/// Audits the recorded settle log: every Normal input must satisfy the real
/// Server/adapter necessary conditions (producible revision, wire-compatible
/// reason), and every Defensive input must carry the forged shape its tier
/// marker claims. The in-driver `assert_normal_wire_honest` already fails a
/// violating Normal input on the spot; this test additionally proves the probe
/// actually reached every repaired edge.
#[test]
fn panel_settle_inputs_match_real_server_and_adapter_legality() {
    let mut trace = new_trace();
    for action in settle_probe_actions() {
        apply(&mut trace, action).expect("settle probe");
    }
    let log = &trace.settle_log;
    assert!(!log.is_empty(), "probe must deliver settle inputs");
    for witness in log {
        match (witness.tier, witness.wire) {
            (SettleTier::Normal, SettleWire::Rejected { revision, reason }) => {
                // The only honest rejection is the stale RevisionMismatch
                // citing the current authority revision after a peer advance.
                assert_eq!(reason, WorkbenchRejectionReason::RevisionMismatch);
                assert_eq!(revision, witness.server_revision);
                assert!(revision > witness.baseline_revision);
            }
            (SettleTier::Normal, SettleWire::Unavailable { reason }) => assert!(
                honest_unavailable_reasons(
                    witness.intent,
                    witness.baseline_revision == witness.server_revision
                )
                .contains(&reason),
                "normal unavailable reason must be stage- and wire-compatible"
            ),
            (SettleTier::Normal, SettleWire::Unchanged { .. }) => {
                panic!("Unchanged has no honest variant in this fixture")
            }
            (SettleTier::Normal, SettleWire::TabUnavailable { .. }) => {
                panic!("TabUnavailable has no honest variant")
            }
            (SettleTier::Defensive, SettleWire::Rejected { revision, reason }) => {
                match witness.class {
                    // RejectedFresh's synthetic baseline-consistent rejection.
                    PanelSettleClass::RejectedFresh => {
                        assert_eq!(reason, WorkbenchRejectionReason::PanelUnavailable);
                        assert_eq!(revision, witness.baseline_revision);
                    }
                    // The forged stale edge: below the baseline, unreachable
                    // under a monotone authority revision.
                    PanelSettleClass::RejectedStale => {
                        assert_eq!(reason, WorkbenchRejectionReason::RevisionMismatch);
                        assert!(revision < witness.baseline_revision);
                    }
                    _ => panic!("unexpected defensive rejection class"),
                }
            }
            (SettleTier::Defensive, SettleWire::Unavailable { reason }) => {
                // The forged unavailable edge is ServerBusy on a Move only.
                assert_eq!(reason, WorkbenchCommandUnavailableReason::ServerBusy);
                assert_eq!(witness.intent, IntentTag::Move);
            }
            (SettleTier::Defensive, SettleWire::TabUnavailable { revision }) => {
                assert_eq!(witness.class, PanelSettleClass::TabUnavailableGone);
                assert!(revision > witness.baseline_revision);
            }
            (SettleTier::Defensive, SettleWire::Unchanged { revision }) => {
                // The white-box edge stays a baseline-stamped no-op on Move.
                assert_eq!(witness.class, PanelSettleClass::Unchanged);
                assert_eq!(witness.intent, IntentTag::Move);
                assert_eq!(revision, witness.baseline_revision);
            }
            (SettleTier::Defensive, _) => {
                panic!("defensive input must carry a forged shape")
            }
            (SettleTier::Normal, _) => {}
        }
    }
    // Reachability: every repaired edge was actually exercised per intent.
    let has = |intent: IntentTag, class: PanelSettleClass, tier: SettleTier| {
        log.iter()
            .any(|w| w.intent == intent && w.class == class && w.tier == tier)
    };
    for intent in [IntentTag::Split, IntentTag::Move, IntentTag::Close] {
        assert!(has(
            intent,
            PanelSettleClass::RejectedStale,
            SettleTier::Normal
        ));
        assert!(has(
            intent,
            PanelSettleClass::RejectedStale,
            SettleTier::Defensive
        ));
        assert!(has(
            intent,
            PanelSettleClass::Unavailable,
            SettleTier::Normal
        ));
        assert!(has(
            intent,
            PanelSettleClass::RejectedFresh,
            SettleTier::Defensive
        ));
        assert!(!has(
            intent,
            PanelSettleClass::RejectedFresh,
            SettleTier::Normal
        ));
    }
    assert!(has(
        IntentTag::Move,
        PanelSettleClass::Unavailable,
        SettleTier::Defensive
    ));
    assert!(!has(
        IntentTag::Split,
        PanelSettleClass::Unavailable,
        SettleTier::Defensive
    ));
    assert!(!has(
        IntentTag::Close,
        PanelSettleClass::Unavailable,
        SettleTier::Defensive
    ));
    assert!(has(
        IntentTag::Move,
        PanelSettleClass::TabUnavailableGone,
        SettleTier::Defensive
    ));
    assert!(!has(
        IntentTag::Move,
        PanelSettleClass::TabUnavailableGone,
        SettleTier::Normal
    ));
    assert!(!has(
        IntentTag::Split,
        PanelSettleClass::TabUnavailableGone,
        SettleTier::Defensive
    ));
    // Unchanged migrated to the defensive bucket: the white-box edge must be
    // exercised on Move, stay Move-only, and never flow back into Normal.
    assert!(has(
        IntentTag::Move,
        PanelSettleClass::Unchanged,
        SettleTier::Defensive
    ));
    for intent in [IntentTag::Split, IntentTag::Move, IntentTag::Close] {
        assert!(!has(
            intent,
            PanelSettleClass::Unchanged,
            SettleTier::Normal
        ));
    }
    assert!(!has(
        IntentTag::Split,
        PanelSettleClass::Unchanged,
        SettleTier::Defensive
    ));
    assert!(!has(
        IntentTag::Close,
        PanelSettleClass::Unchanged,
        SettleTier::Defensive
    ));
}

/// Deterministic stage control for the stale-baseline Unavailable repair: once
/// a peer commit leaves the operation's baseline behind the authority revision,
/// only the pre-admission `ServiceUnavailable` is an honest unavailable reason —
/// every reason admission reaches only after the revision check (TerminalBusy,
/// NoCompatibleWorker, ServerBusy, TerminalRuntimeUnavailable) must be caught by
/// the same `assert_normal_wire_honest` criterion the driver applies, and must
/// pass again on a fresh baseline wherever the intent's wire set contains it.
/// (`RequestLedgerFull` is likewise pre-admission in the real Server; the stale
/// set stays the minimal uniform `ServiceUnavailable`.)
#[test]
fn stale_unavailable_reasons_are_stage_checked() {
    for (kind, intent) in [
        (0u8, IntentTag::Split),
        (1, IntentTag::Close),
        (2, IntentTag::Move),
    ] {
        let mut trace = new_trace();
        // A committed split gives the two-leaf structure every intent plans on.
        apply(
            &mut trace,
            TraceAction::PanelSubmit {
                kind: 0,
                direction: 1,
            },
        )
        .expect("seed split submit");
        apply(
            &mut trace,
            TraceAction::PanelSettle {
                class: PanelSettleClass::CommittedMerge,
                tier: SettleTier::Normal,
            },
        )
        .expect("seed split commit");
        apply(&mut trace, TraceAction::PanelSubmit { kind, direction: 1 }).expect("intent submit");
        let operation = trace
            .session
            .pending_panel()
            .cloned()
            .expect("pending panel op");
        assert_eq!(intent_tag(operation.intent()), intent);
        let baseline = operation.baseline().revision().value();
        // The peer advance that makes the baseline stale — the same shape the
        // driver uses for a Normal stale rejection.
        let peer_tab = fresh_tab(&mut trace);
        trace.server_tabs.push(peer_tab);
        trace.server_revision = trace.server_revision.checked_add(1).expect("revision");
        assert!(baseline < trace.server_revision);
        // Positive control: the pre-admission reason stays honest on a stale
        // baseline.
        assert_normal_wire_honest(
            &operation,
            &PanelResult::Precommit(PanelPrecommit::Unavailable {
                reason: WorkbenchCommandUnavailableReason::ServiceUnavailable,
            }),
            trace.server_revision,
        );
        // Negative control: each post-admission reason is caught on the stale
        // baseline even where the intent's fresh wire set would allow it, and
        // is honest again on a fresh baseline.
        for reason in [
            WorkbenchCommandUnavailableReason::NoCompatibleWorker,
            WorkbenchCommandUnavailableReason::TerminalBusy,
            WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable,
            WorkbenchCommandUnavailableReason::ServerBusy,
        ] {
            let forged = PanelResult::Precommit(PanelPrecommit::Unavailable { reason });
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    assert_normal_wire_honest(&operation, &forged, trace.server_revision)
                }))
                .is_err(),
                "stale baseline must catch post-admission reason {reason:?}"
            );
            if honest_unavailable_reasons(intent, true).contains(&reason) {
                assert_normal_wire_honest(&operation, &forged, baseline);
            }
        }
    }
}
