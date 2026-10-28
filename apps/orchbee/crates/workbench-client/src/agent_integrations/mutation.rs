//! The mutation frontier — the CLI port of the canonical enable/disable
//! vectors plus the controller's gate/re-inspection contract, strengthened
//! per the accepted design:
//!
//! * Publication and replacement are no-replace (`files.rs`), never
//!   delete-then-recreate.
//! * A moved-aside owned entry is restored on a no-replace conflict; a
//!   backup that cannot be restored is named in `residuals`.
//! * Steps record into `attempted[]`; the plan only contains the steps a
//!   fresh inspection still requires — completed work is never replayed.
//! * The single monotonic deadline gates admission: no budget for the
//!   provider's own re-inspection reserve → refused before any effect.

use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use super::capsule::{self, CapsuleValidity};
use super::copilot::Command as CopilotCommand;
use super::exec::{self, CommandResult, SLOT};
use super::files::{self, FsStep};
use super::inspect::{self, BootstrapKind, ClinePackage, Marketplace, TargetState};
use super::model::{Availability, IntegrationId, ManagementMode, Provenance, SnapshotItem, State};
use super::providers::{self, ProviderContext};

/// The wall the frontier consults — tests inject a clock skew so deadline
/// and reserve math is exercised without real sleeps.
#[cfg(not(all(test, unix)))]
#[inline]
fn now() -> Instant {
    Instant::now()
}

#[cfg(all(test, unix))]
#[inline]
fn now() -> Instant {
    super::tests::fault_injection::now()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Enable,
    Disable,
}

#[cfg(windows)]
mod codex_windows;
#[cfg(windows)]
mod file_publication_windows;
#[cfg(windows)]
mod hermes_windows;
#[cfg(windows)]
mod hook_configuration_windows;

impl Direction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Enable => "enable",
            Self::Disable => "disable",
        }
    }
}

/// One executed or refused step of the plan — the machine-readable
/// `attempted[]` member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StepKind {
    /// A provider CLI command — `argv` is the literal argument vector.
    Command {
        executable: String,
        argv: Vec<String>,
    },
    /// A managed-filesystem effect.
    Filesystem {
        operation: &'static str,
        path: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepOutcome {
    Completed,
    /// The no-replace primitive found the destination occupied.
    Conflict,
    Failed,
    /// The invocation deadline expired before the step could run — the step
    /// never produced an effect.
    Deadline,
    TimedOut,
    Leaked,
    OutputTooLarge,
    /// The termination signal fired — the step's kill path ran or the step
    /// never launched.
    Cancelled,
}

impl StepOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Conflict => "conflict",
            Self::Failed => "failed",
            Self::Deadline => "deadline",
            Self::TimedOut => "timed-out",
            Self::Leaked => "leaked",
            Self::OutputTooLarge => "output-too-large",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AttemptedStep {
    pub step: StepKind,
    pub outcome: StepOutcome,
}

/// One invocation's effect journal; vectors borrow its records and frontier
/// together instead of growing independent mutation-output argument lists.
struct EffectTrace<'a> {
    attempted: &'a mut Vec<AttemptedStep>,
    residuals: &'a mut Vec<PathBuf>,
    begun: &'a mut bool,
}

/// The canonical `AgentIntegrationMutationAttempt` vocabulary — every
/// provider vector's terminal classification. `Failed` carries the typed
/// code so a timed-out or leaked step never collapses into the generic
/// failure the verdict would then misreport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Attempt {
    Attempted,
    Conflict,
    /// The termination signal fired mid-frontier — exit 130, never a
    /// provider or fs verdict.
    Cancelled,
    /// The mid-window re-inspection gate refused on a changed target
    /// state — the refusal family (exit 3/4) even though the staging
    /// effect already landed.
    Refused(Refusal),
    Failed(FailureCode),
}

/// Pre-effect refusal codes — nothing was mutated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    AgentUnavailable,
    Conflict,
    /// The provider is `manual-only` on this platform — the CLI names the
    /// manual path instead of attempting effects.
    ManualActionRequired,
    InspectionFailed,
    /// No adapter source: no cohort sidecar and no override.
    CapsuleUnavailable,
    /// The deadline cannot cover the plan plus the re-inspection reserve.
    Deadline,
}

impl Refusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AgentUnavailable => "agent-unavailable",
            Self::Conflict => "conflict",
            Self::ManualActionRequired => "manual-action-required",
            Self::InspectionFailed => "inspection-failed",
            Self::CapsuleUnavailable => "capsule-unavailable",
            Self::Deadline => "timed-out",
        }
    }
}

/// Post-effect failure codes — the canonical failure vocabulary plus the
/// CLI-typed step outcomes the honest frontier surfaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureCode {
    Conflict,
    /// Canonical failure-code member — canonical maps a provider command
    /// timeout onto `manual-action-required`; this port keeps the honest
    /// `timed-out` code instead, so nothing produces it here.
    #[allow(dead_code)]
    ManualActionRequired,
    OperationFailed,
    VerificationFailed,
    TimedOut,
    OutputTooLarge,
    Leaked,
}

impl FailureCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Conflict => "conflict",
            Self::ManualActionRequired => "manual-action-required",
            Self::OperationFailed => "operation-failed",
            Self::VerificationFailed => "verification-failed",
            Self::TimedOut => "timed-out",
            Self::OutputTooLarge => "output-too-large",
            Self::Leaked => "leaked",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// The goal state is observed (or was already true pre-effect).
    Completed,
    /// The controller refused the goal. Whether the refusal precedes any
    /// integration effect is told by `attempted`: a nonempty attempted[]
    /// means the capsule/vector frontier was already crossed and the exit
    /// map must not emit a pre-effect class.
    Refused(Refusal),
    /// A known post-effect failure.
    Failed(FailureCode),
    /// The outcome could not be determined — re-inspection was unavailable
    /// or the deadline expired mid-frontier. Closed-vocabulary member: the
    /// frontier maps undetermined observation onto `Failed(TimedOut)`, but
    /// the schema's verdict word stays reserved.
    #[allow(dead_code)]
    Unknown,
    /// The termination signal fired — exit 130 at any stage.
    Cancelled,
}

pub struct MutationReport {
    pub integration_id: IntegrationId,
    pub direction: Direction,
    pub attempted: Vec<AttemptedStep>,
    /// Retained backups/staging a same-call cleanup could not remove —
    /// named, never silently swept.
    pub residuals: Vec<PathBuf>,
    pub observed: Option<SnapshotItem>,
    pub provenance: Provenance,
    /// Whether any Orchbee-owned effect could already exist — capsule
    /// staging/publication, a spawned provider command, or a guarded fs
    /// write. This is the private frontier the exit map classifies by; a
    /// deadline-refused step recorded in `attempted` is a log entry, not
    /// proof the frontier was crossed.
    pub effect_begun: bool,
    pub verdict: Verdict,
}

/// The frontier: entry gates → capsule publish (enable only, before any
/// provider read) → inspection → controller gates → provider vector →
/// post-effect observation verdict.
pub(crate) fn mutate(
    id: IntegrationId,
    direction: Direction,
    context: &ProviderContext,
) -> MutationReport {
    let mut attempted: Vec<AttemptedStep> = Vec::new();
    let mut residuals: Vec<PathBuf> = Vec::new();
    // The private effect frontier: set immediately before every real
    // possibly-effectful call (capsule publish, provider spawn, guarded fs
    // write) and carried monotonically — a deadline-refused step logged in
    // `attempted` never sets it.
    let mut effect_begun = false;
    // Entry gates precede the boundary witness — a run refused before the
    // first step records an empty attempted[].
    if cancelled(context) {
        return MutationReport {
            integration_id: id,
            direction,
            attempted,
            residuals,
            observed: None,
            provenance: Provenance::PreEffect,
            effect_begun,
            verdict: Verdict::Cancelled,
        };
    }
    if now() >= context.deadline {
        return MutationReport {
            integration_id: id,
            direction,
            attempted,
            residuals,
            observed: None,
            provenance: Provenance::PreEffect,
            effect_begun,
            verdict: Verdict::Refused(Refusal::Deadline),
        };
    }
    #[cfg(all(test, unix))]
    super::tests::fault_injection::boundary("preflight-inspect", Path::new(""));
    if cancelled(context) {
        return MutationReport {
            integration_id: id,
            direction,
            attempted,
            residuals,
            observed: None,
            provenance: Provenance::PreEffect,
            effect_begun,
            verdict: Verdict::Cancelled,
        };
    }
    // `manual-only` gates before any provider read and before the capsule
    // effect — the mode is a spec property, not an inspected value.
    if providers::management_mode(id, context.management_mode) == ManagementMode::ManualOnly {
        return MutationReport {
            integration_id: id,
            direction,
            attempted,
            residuals,
            observed: None,
            provenance: Provenance::PreEffect,
            effect_begun,
            verdict: Verdict::Refused(Refusal::ManualActionRequired),
        };
    }

    if direction == Direction::Enable {
        // Availability resolves without issuing provider commands — the
        // gate precedes the capsule publish so a refused run never leaves
        // an effect behind.
        if !providers::executable_available(id, context) {
            return MutationReport {
                integration_id: id,
                direction,
                attempted,
                residuals,
                observed: None,
                provenance: Provenance::PreEffect,
                effect_begun,
                verdict: Verdict::Refused(Refusal::AgentUnavailable),
            };
        }
        // The wrapper publishes the adapter capsule before the provider
        // delegate — inspection reads included — ever runs. A publish
        // failure is already post-effect: no provider state was observed.
        match prepare_capsule(context, &mut attempted, &mut residuals, &mut effect_begun) {
            CapsuleStep::Ready => {}
            CapsuleStep::Refused(refusal) => {
                return MutationReport {
                    integration_id: id,
                    direction,
                    attempted,
                    residuals,
                    observed: None,
                    provenance: Provenance::PreEffect,
                    effect_begun,
                    verdict: Verdict::Refused(refusal),
                };
            }
            CapsuleStep::Failed(code) => {
                return MutationReport {
                    integration_id: id,
                    direction,
                    attempted,
                    residuals,
                    observed: None,
                    provenance: Provenance::Unavailable,
                    effect_begun,
                    verdict: Verdict::Failed(code),
                };
            }
        }
    }

    let (initial, detail) = providers::inspect_full(id, context);
    // Whether the capsule publish step already ran decides what `initial`
    // proves: a snapshot taken behind an Orchbee-owned effect can never be
    // reported as admission-before-effect, even when the provider state it
    // observed predates the provider mutation itself. And a fresh read
    // attempt is not a complete observation — a snapshot that failed to
    // classify behind an effect is partial post-effect evidence, not the
    // determinate picture `post-effect` claims.
    let initial_provenance = if !effect_begun {
        Provenance::PreEffect
    } else if initial.state == State::Unknown
        && initial.issue == Some(super::model::Issue::InspectionFailed)
    {
        Provenance::Partial
    } else {
        Provenance::PostEffect
    };
    if cancelled(context) {
        return MutationReport {
            integration_id: id,
            direction,
            attempted,
            residuals,
            observed: Some(initial),
            provenance: initial_provenance,
            effect_begun,
            verdict: Verdict::Cancelled,
        };
    }
    // A timed-out read is the typed deadline verdict (exit 7) only when it
    // really consumed the invocation deadline — a command-level timeout
    // with budget left is just an unclassifiable read and falls through to
    // the inspection-failed refusal like any other unknown state.
    if initial.timed_out && now() >= context.deadline {
        return MutationReport {
            integration_id: id,
            direction,
            attempted,
            residuals,
            observed: Some(initial),
            provenance: Provenance::Unavailable,
            effect_begun,
            verdict: Verdict::Refused(Refusal::Deadline),
        };
    }

    // Controller pre-gates, in canonical order.
    let refused = match (direction, &initial) {
        _ if initial.state == State::Conflict => Some(Refusal::Conflict),
        _ if initial.state == State::Unknown => Some(Refusal::InspectionFailed),
        (Direction::Enable, item) if item.availability != Availability::Available => {
            Some(Refusal::AgentUnavailable)
        }
        _ => None,
    };
    if let Some(refusal) = refused {
        return MutationReport {
            integration_id: id,
            direction,
            attempted,
            residuals,
            observed: Some(initial),
            provenance: initial_provenance,
            effect_begun,
            verdict: Verdict::Refused(refusal),
        };
    }
    let already_at_goal = match direction {
        Direction::Enable => initial.state == State::Enabled,
        Direction::Disable => {
            matches!(initial.state, State::Disabled | State::NotInstalled)
        }
    };
    if already_at_goal {
        return MutationReport {
            integration_id: id,
            direction,
            attempted,
            residuals,
            observed: Some(initial),
            provenance: initial_provenance,
            effect_begun,
            verdict: Verdict::Completed,
        };
    }
    // The deadline must cover the provider's own re-inspection reserve
    // before the first mutation step — the honest pre-mutation timeout. A
    // provider whose re-inspection issues no commands reserves nothing.
    let reserve = providers::inspect_slots(id);
    if now() + SLOT * (reserve as u32) >= context.deadline {
        return MutationReport {
            integration_id: id,
            direction,
            attempted,
            residuals,
            observed: Some(initial),
            provenance: initial_provenance,
            effect_begun,
            verdict: Verdict::Refused(Refusal::Deadline),
        };
    }

    let attempt = match direction {
        Direction::Enable => enable_vector(
            id,
            context,
            &initial,
            &detail,
            EffectTrace {
                attempted: &mut attempted,
                residuals: &mut residuals,
                begun: &mut effect_begun,
            },
        ),
        Direction::Disable => disable_vector(
            id,
            context,
            &initial,
            &detail,
            EffectTrace {
                attempted: &mut attempted,
                residuals: &mut residuals,
                begun: &mut effect_begun,
            },
        ),
    };

    match attempt {
        // The mid-window re-inspection gate aborted on a changed state —
        // the refusal family, recorded against a post-effect frontier.
        Attempt::Refused(refusal) => {
            return MutationReport {
                integration_id: id,
                direction,
                attempted,
                residuals,
                observed: None,
                provenance: Provenance::Unavailable,
                effect_begun,
                verdict: Verdict::Refused(refusal),
            };
        }
        // A primitive no-replace conflict is already determinate — the
        // occupant is foreign, no further observation is needed.
        Attempt::Conflict => {
            return MutationReport {
                integration_id: id,
                direction,
                attempted,
                residuals,
                observed: None,
                provenance: Provenance::Unavailable,
                effect_begun,
                verdict: Verdict::Failed(FailureCode::Conflict),
            };
        }
        _ => {}
    }

    // Cancel mid-frontier: no further reads — attempted[] and the named
    // residue stand as the honest record.
    if attempt == Attempt::Cancelled || cancelled(context) {
        return MutationReport {
            integration_id: id,
            direction,
            attempted,
            residuals,
            observed: Some(initial),
            provenance: initial_provenance,
            effect_begun,
            verdict: Verdict::Cancelled,
        };
    }

    if attempt == Attempt::Attempted {
        if providers::guarded_spec(id).is_some()
            || cfg!(windows) && id == IntegrationId::Codex
            || matches!(
                id,
                IntegrationId::Cursor
                    | IntegrationId::QoderCli
                    | IntegrationId::AntigravityCli
                    | IntegrationId::Mastracode
                    | IntegrationId::Droid
                    | IntegrationId::Kimi
                    | IntegrationId::Letta
                    | IntegrationId::Devin
            )
        {
            // A filesystem vector's post-effect observation is a fresh
            // fs scan — no provider commands — while the budget lasts.
            if now() < context.deadline {
                #[cfg(all(test, unix))]
                super::tests::fault_injection::boundary("reinspect", Path::new(""));
                let item = providers::inspect_provider(id, context);
                let verdict = if goal_observed(direction, item.state) {
                    Verdict::Completed
                } else {
                    Verdict::Failed(FailureCode::VerificationFailed)
                };
                return MutationReport {
                    integration_id: id,
                    direction,
                    attempted,
                    residuals,
                    observed: Some(item),
                    provenance: Provenance::PostEffect,
                    effect_begun,
                    verdict,
                };
            }
            // The step's own post-verify already observed the goal — the
            // unbudgeted scan adds nothing to an honest completion.
            return MutationReport {
                integration_id: id,
                direction,
                attempted,
                residuals,
                observed: None,
                provenance: Provenance::Unavailable,
                effect_begun,
                verdict: Verdict::Completed,
            };
        }
        // A provider-command vector's own completion is the post-effect
        // observation — but completion is only claimed while the
        // re-inspection reserve was still affordable: `provider exit 0
        // alone` never claims success against an exhausted budget.
        let verdict = if now() + SLOT * (reserve as u32) >= context.deadline {
            Verdict::Failed(FailureCode::TimedOut)
        } else {
            Verdict::Completed
        };
        let provenance = if matches!(verdict, Verdict::Completed) {
            Provenance::PostEffect
        } else {
            Provenance::Unavailable
        };
        return MutationReport {
            integration_id: id,
            direction,
            attempted,
            residuals,
            observed: None,
            provenance,
            effect_begun,
            verdict,
        };
    }

    // An uncertain attempt re-inspects while the deadline covers the
    // provider's full read reserve — the re-inspection may issue provider
    // reads, which is exactly what the pre-vector reserve was held for. A
    // timed-out step already consumed the observation budget, so nothing
    // further is owed: the provenance stays unavailable.
    let reserve_deadline_ok = now() + SLOT * (reserve as u32) < context.deadline
        && attempt != Attempt::Failed(FailureCode::TimedOut);
    let (observed, provenance) = if reserve_deadline_ok {
        #[cfg(all(test, unix))]
        super::tests::fault_injection::boundary("reinspect", Path::new(""));
        let item = providers::inspect_provider(id, context);
        let provenance = if item.state == State::Unknown
            && item.issue == Some(super::model::Issue::InspectionFailed)
        {
            Provenance::Partial
        } else {
            Provenance::PostEffect
        };
        (Some(item), provenance)
    } else {
        (None, Provenance::Unavailable)
    };

    let verdict = match observed {
        Some(item) if goal_observed(direction, item.state) => Verdict::Completed,
        // A degraded observation can neither confirm nor refute the goal —
        // verification-failed, undetermined.
        Some(_) if provenance == Provenance::Partial => {
            Verdict::Failed(FailureCode::VerificationFailed)
        }
        Some(_) => Verdict::Failed(match attempt {
            Attempt::Attempted => FailureCode::VerificationFailed,
            Attempt::Conflict => FailureCode::Conflict,
            Attempt::Refused(refusal) => {
                return MutationReport {
                    integration_id: id,
                    direction,
                    attempted,
                    residuals,
                    observed,
                    provenance,
                    effect_begun,
                    verdict: Verdict::Refused(refusal),
                };
            }
            Attempt::Cancelled => {
                return MutationReport {
                    integration_id: id,
                    direction,
                    attempted,
                    residuals,
                    observed,
                    provenance,
                    effect_begun,
                    verdict: Verdict::Cancelled,
                };
            }
            Attempt::Failed(code) => code,
        }),
        // Re-inspection unfunded: the budget ran out — the timed-out
        // family, never a fabricated snapshot.
        None => Verdict::Failed(FailureCode::TimedOut),
    };
    MutationReport {
        integration_id: id,
        direction,
        attempted,
        residuals,
        observed,
        provenance,
        effect_begun,
        verdict,
    }
}

fn goal_observed(direction: Direction, state: State) -> bool {
    match direction {
        Direction::Enable => state == State::Enabled,
        Direction::Disable => matches!(state, State::Disabled | State::NotInstalled),
    }
}

enum CapsuleStep {
    Ready,
    /// Refused before any capsule effect — a pre-effect refusal.
    Refused(Refusal),
    /// The publish step was attempted and failed — post-effect by
    /// definition (publication is an Orchbee-owned effect).
    Failed(FailureCode),
}

/// Canonical `prepare()`: isReady → stage → copy → validate staged →
/// no-replace publish. Source invalidity is
/// discovered on the staged copy, so the attempted publish records
/// `failed` and the verdict is a determinate post-effect failure.
fn prepare_capsule(
    context: &ProviderContext,
    attempted: &mut Vec<AttemptedStep>,
    residuals: &mut Vec<PathBuf>,
    effect: &mut bool,
) -> CapsuleStep {
    if capsule::is_ready(context.capsule_root, context.adapter_source) {
        return CapsuleStep::Ready;
    }
    // Occupied-and-invalid is a pre-effect conflict: refuse before any
    // staging exists. Only a genuinely absent root proceeds to publish —
    // an occupant that lands mid-window is caught by the no-replace
    // primitive instead (post-effect).
    match std::fs::symlink_metadata(context.capsule_root) {
        Ok(_) if capsule::validate(context.capsule_root) != CapsuleValidity::Ready => {
            return CapsuleStep::Refused(Refusal::Conflict);
        }
        Ok(_) | Err(_) => {}
    }
    let Some(source) = context.adapter_source else {
        return CapsuleStep::Refused(Refusal::CapsuleUnavailable);
    };
    // Staging is the first real effect — the frontier crosses here, even
    // if a later same-call cleanup removes every byte again.
    *effect = true;
    let publish = publish_capsule(source, context.capsule_root);
    attempted.push(AttemptedStep {
        step: StepKind::Filesystem {
            operation: "publish-capsule",
            path: context.capsule_root.to_path_buf(),
        },
        outcome: publish.outcome,
    });
    if let Some(residual) = &publish.residual {
        residuals.push(residual.clone());
    }
    match publish.outcome {
        StepOutcome::Completed => CapsuleStep::Ready,
        StepOutcome::Conflict => {
            // A publish conflict is only tolerable when a concurrent
            // installation landed a valid capsule — recheck readiness. The
            // retained stage cleans up only on that pass; a foreign or
            // invalid occupant keeps it named as residue and the verdict
            // is the post-effect conflict family.
            #[cfg(all(test, unix))]
            super::tests::fault_injection::boundary(
                "capsule-isvalid-existing",
                context.capsule_root,
            );
            if capsule::is_ready(context.capsule_root, Some(source)) {
                // The retained stage drops from `residuals` only on an
                // affirmative removal — a failed cleanup leaves the owned
                // bytes on disk and the report must keep naming them.
                if let Some(retained) = &publish.residual
                    && files::remove_owned(retained, true) == FsStep::Done
                {
                    residuals.retain(|path| path != retained);
                }
                CapsuleStep::Ready
            } else {
                CapsuleStep::Failed(FailureCode::Conflict)
            }
        }
        StepOutcome::Deadline => CapsuleStep::Refused(Refusal::Deadline),
        StepOutcome::Failed => CapsuleStep::Failed(FailureCode::OperationFailed),
        StepOutcome::TimedOut => CapsuleStep::Failed(FailureCode::TimedOut),
        StepOutcome::Leaked => CapsuleStep::Failed(FailureCode::Leaked),
        StepOutcome::OutputTooLarge => CapsuleStep::Failed(FailureCode::OutputTooLarge),
        // The capsule publish path has no cancellable step — unreachable;
        // the frontier's own cancel checks carry the real signal.
        StepOutcome::Cancelled => CapsuleStep::Failed(FailureCode::OperationFailed),
    }
}

pub(crate) struct PublishResult {
    pub(crate) outcome: StepOutcome,
    pub(crate) residual: Option<PathBuf>,
}

/// Stage → copy → validate staged → no-replace publish → cleanup staging.
pub(crate) fn publish_capsule(source: &Path, target: &Path) -> PublishResult {
    let parent = match target.parent() {
        Some(parent) => parent,
        None => {
            return PublishResult {
                outcome: StepOutcome::Failed,
                residual: None,
            };
        }
    };
    let staging = match files::stage_directory(parent, "capsule") {
        Ok(staging) => staging,
        Err(_) => {
            return PublishResult {
                outcome: StepOutcome::Failed,
                residual: None,
            };
        }
    };
    let staged_root = staging.join("payload");
    let mut verify_failed = false;
    let result = (|| {
        if files::copy_tree(source, &staged_root).is_err() {
            return StepOutcome::Failed;
        }
        #[cfg(all(test, unix))]
        super::tests::fault_injection::boundary("capsule-verify", &staged_root);
        if capsule::validate(&staged_root) != CapsuleValidity::Ready {
            verify_failed = true;
            return StepOutcome::Failed;
        }
        #[cfg(all(test, unix))]
        super::tests::fault_injection::boundary("capsule-rename", target);
        match files::publish_no_replace(&staged_root, target) {
            FsStep::Done => StepOutcome::Completed,
            FsStep::Conflict => StepOutcome::Conflict,
            FsStep::Failed => StepOutcome::Failed,
        }
    })();
    // Same-call staging cleanup on the paths that still vouch for the
    // staged bytes — never a background sweep. A failed capsule verify
    // retains the stage as named residue; a publish conflict retains it
    // too — the occupant's validity is the caller's re-check to make, and
    // until that passes the intact stage stays named.
    if result == StepOutcome::Conflict {
        return PublishResult {
            outcome: result,
            residual: Some(staging),
        };
    }
    if !verify_failed && files::remove_owned(&staging, true) != FsStep::Done {
        return PublishResult {
            outcome: result,
            residual: Some(staging),
        };
    }
    PublishResult {
        outcome: result,
        residual: if verify_failed { Some(staging) } else { None },
    }
}

/// Run one provider command as an attempted step; the slot is clipped by
/// both the provider's re-inspection reserve and the invocation deadline.
/// The termination source check shared by every frontier boundary — absent
/// in contexts that cannot be cancelled (tests, `--adapter-source` drives
/// without an armed signal owner).
fn cancelled(context: &ProviderContext) -> bool {
    context
        .cancel
        .map(|source| source.requested())
        .unwrap_or(false)
}

fn run_step(
    executable: &Path,
    argv: &[&str],
    context: &ProviderContext,
    reserve_slots: usize,
    attempted: &mut Vec<AttemptedStep>,
    effect: &mut bool,
) -> Attempt {
    run_step_with(
        executable,
        argv,
        context,
        reserve_slots,
        attempted,
        effect,
        |argv_owned| {
            exec::run_command(
                executable,
                argv_owned,
                context.home,
                context.deadline,
                context.cancel,
            )
        },
    )
}
fn run_step_with(
    executable: &Path,
    argv: &[&str],
    context: &ProviderContext,
    reserve_slots: usize,
    attempted: &mut Vec<AttemptedStep>,
    effect: &mut bool,
    execute: impl FnOnce(&[String]) -> CommandResult,
) -> Attempt {
    let deadline_ok = now() + SLOT * (reserve_slots.saturating_add(1) as u32) <= context.deadline;
    if !deadline_ok {
        // Slot admission refused: the command was never spawned, so the
        // log entry is a planned-step record, not an effect — the refusal
        // keeps the deadline class and crosses no frontier.
        attempted.push(AttemptedStep {
            step: StepKind::Command {
                executable: executable.to_string_lossy().into_owned(),
                argv: argv.iter().map(|arg| (*arg).to_owned()).collect(),
            },
            outcome: StepOutcome::Deadline,
        });
        return Attempt::Refused(Refusal::Deadline);
    }
    if cancelled(context) {
        return Attempt::Cancelled;
    }
    #[cfg(all(test, unix))]
    super::tests::fault_injection::boundary("provider-command", Path::new(""));
    let argv_owned: Vec<String> = argv.iter().map(|arg| (*arg).to_owned()).collect();
    // A spawned provider command may have mutated state before any
    // outcome is known — the frontier crosses at the spawn.
    *effect = true;
    let result = execute(&argv_owned);
    let outcome = match result {
        CommandResult::Completed { .. } => StepOutcome::Completed,
        CommandResult::Failed => StepOutcome::Failed,
        CommandResult::OutputTooLarge => StepOutcome::OutputTooLarge,
        CommandResult::TimedOut => StepOutcome::TimedOut,
        CommandResult::Leaked => StepOutcome::Leaked,
        CommandResult::Cancelled => StepOutcome::Cancelled,
    };
    attempted.push(AttemptedStep {
        step: StepKind::Command {
            executable: executable.to_string_lossy().into_owned(),
            argv: argv_owned,
        },
        outcome,
    });
    #[cfg(all(test, unix))]
    if outcome == StepOutcome::Completed {
        // Harness-only elapsed time after a legal command completion lives
        // outside exec's run phase. Fire only after both the effect frontier
        // and attempted-step witness are durable, before the next command's
        // slot admission consult.
        super::tests::fault_injection::boundary("provider-command-completed", Path::new(""));
    }
    match outcome {
        StepOutcome::Completed => Attempt::Attempted,
        StepOutcome::Cancelled => Attempt::Cancelled,
        StepOutcome::TimedOut | StepOutcome::Deadline => Attempt::Failed(FailureCode::TimedOut),
        StepOutcome::Leaked => Attempt::Failed(FailureCode::Leaked),
        StepOutcome::OutputTooLarge => Attempt::Failed(FailureCode::OutputTooLarge),
        StepOutcome::Failed | StepOutcome::Conflict => {
            Attempt::Failed(FailureCode::OperationFailed)
        }
    }
}

/// Canonical `runCommandSequence`: stop at the first non-attempted step.
fn run_sequence(
    executable: &Path,
    commands: &[Vec<String>],
    context: &ProviderContext,
    reserve_slots: usize,
    attempted: &mut Vec<AttemptedStep>,
    effect: &mut bool,
) -> Attempt {
    for command in commands {
        let argv: Vec<&str> = command.iter().map(String::as_str).collect();
        let result = run_step(executable, &argv, context, reserve_slots, attempted, effect);
        if result != Attempt::Attempted {
            return result;
        }
    }
    Attempt::Attempted
}

// -- Provider vectors -------------------------------------------------------

/// Codex mutations use the inspection profile explicitly on Windows. Other
/// platforms retain their existing provider environment and conformance vectors.
fn run_codex_sequence(
    executable: &Path,
    commands: &[Vec<String>],
    context: &ProviderContext,
    reserve_slots: usize,
    attempted: &mut Vec<AttemptedStep>,
    effect: &mut bool,
) -> Attempt {
    #[cfg(not(windows))]
    return run_sequence(
        executable,
        commands,
        context,
        reserve_slots,
        attempted,
        effect,
    );
    #[cfg(windows)]
    {
        let _profile_pins =
            match codex_windows::prepare_home(context, reserve_slots, attempted, effect) {
                Ok(pins) => pins,
                Err(attempt) => return attempt,
            };
        for command in commands {
            let argv: Vec<&str> = command.iter().map(String::as_str).collect();
            let result = run_step_with(
                executable,
                &argv,
                context,
                reserve_slots,
                attempted,
                effect,
                |arguments| {
                    exec::run_codex_command(
                        executable,
                        arguments,
                        context.home,
                        context.deadline,
                        context.cancel,
                    )
                },
            );
            if result != Attempt::Attempted {
                return result;
            }
        }
        Attempt::Attempted
    }
}

/// The read pass's guard dimensions the vectors plan against — destructured
/// per provider kind so a vector never re-issues a provider read command.
struct VectorInspection {
    state: State,
    marketplace: Marketplace,
    plugin_present: bool,
}

/// Destructure the single-pass detail into the vector's planning view; a
/// missing dimension means the read itself failed, which the pre-gates
/// already refused — anything else is a bug, not a provider state.
fn vector_inspection(
    state: State,
    detail: &providers::InspectionDetail,
) -> Option<VectorInspection> {
    match detail {
        providers::InspectionDetail::Cli {
            marketplace: Some(marketplace),
            plugin_present: Some(plugin_present),
        } => Some(VectorInspection {
            state,
            marketplace: *marketplace,
            plugin_present: *plugin_present,
        }),
        providers::InspectionDetail::Codex {
            marketplace,
            plugin_present,
        } => Some(VectorInspection {
            state,
            marketplace: *marketplace,
            plugin_present: *plugin_present,
        }),
        _ => None,
    }
}

fn enable_vector(
    id: IntegrationId,
    context: &ProviderContext,
    initial: &SnapshotItem,
    detail: &providers::InspectionDetail,
    trace: EffectTrace,
) -> Attempt {
    let EffectTrace {
        attempted,
        residuals,
        begun: effect,
    } = trace;
    let options = providers::resource_options(context);
    let resource_root = context.capsule_root.to_string_lossy().into_owned();
    let reserve = providers::inspect_slots(id);
    match id {
        IntegrationId::Hermes => {
            #[cfg(windows)]
            {
                hermes_windows::update(
                    context,
                    &options,
                    true,
                    EffectTrace {
                        attempted,
                        residuals,
                        begun: effect,
                    },
                )
            }
            #[cfg(not(windows))]
            {
                Attempt::Refused(Refusal::ManualActionRequired)
            }
        }
        IntegrationId::Cursor
        | IntegrationId::QoderCli
        | IntegrationId::AntigravityCli
        | IntegrationId::Mastracode
        | IntegrationId::Droid
        | IntegrationId::Kimi
        | IntegrationId::Letta
        | IntegrationId::Devin => {
            #[cfg(windows)]
            {
                hook_configuration_windows::update(
                    id.as_str(),
                    context,
                    &options,
                    true,
                    attempted,
                    residuals,
                    effect,
                )
            }
            #[cfg(not(windows))]
            {
                let _ = residuals;
                Attempt::Refused(Refusal::ManualActionRequired)
            }
        }
        IntegrationId::ClaudeCode => {
            let Some(inspection) = vector_inspection(initial.state, detail) else {
                return Attempt::Failed(FailureCode::OperationFailed);
            };
            match inspection.state {
                State::Enabled => Attempt::Attempted,
                State::Conflict => Attempt::Conflict,
                State::Disabled => {
                    let Some(exe) = exec::resolve_executable("claude", context.path_env) else {
                        return Attempt::Failed(FailureCode::OperationFailed);
                    };
                    run_step(
                        &exe,
                        &[
                            "plugin",
                            "enable",
                            inspect::CLAUDE_PLUGIN_ID,
                            "--scope",
                            "user",
                        ],
                        context,
                        reserve,
                        attempted,
                        effect,
                    )
                }
                State::NotInstalled | State::UpdateRequired => {
                    let Some(exe) = exec::resolve_executable("claude", context.path_env) else {
                        return Attempt::Failed(FailureCode::OperationFailed);
                    };
                    let mut commands: Vec<Vec<String>> = Vec::new();
                    if inspection.state == State::UpdateRequired && inspection.plugin_present {
                        commands.push(vec![
                            "plugin".into(),
                            "uninstall".into(),
                            inspect::CLAUDE_PLUGIN_ID.into(),
                            "--scope".into(),
                            "user".into(),
                        ]);
                    }
                    if inspection.marketplace == Marketplace::Outdated {
                        commands.push(vec![
                            "plugin".into(),
                            "marketplace".into(),
                            "remove".into(),
                            inspect::MARKETPLACE_NAME.into(),
                            "--scope".into(),
                            "user".into(),
                        ]);
                    }
                    if inspection.marketplace != Marketplace::Current {
                        commands.push(vec![
                            "plugin".into(),
                            "marketplace".into(),
                            "add".into(),
                            resource_root.clone(),
                            "--scope".into(),
                            "user".into(),
                        ]);
                    }
                    commands.push(vec![
                        "plugin".into(),
                        "install".into(),
                        inspect::CLAUDE_PLUGIN_ID.into(),
                        "--scope".into(),
                        "user".into(),
                    ]);
                    commands.push(vec![
                        "plugin".into(),
                        "enable".into(),
                        inspect::CLAUDE_PLUGIN_ID.into(),
                        "--scope".into(),
                        "user".into(),
                    ]);
                    run_sequence(&exe, &commands, context, reserve, attempted, effect)
                }
                _ => Attempt::Failed(FailureCode::OperationFailed),
            }
        }
        IntegrationId::Codex => {
            let Some(exe) = exec::resolve_executable("codex", context.path_env) else {
                return Attempt::Failed(FailureCode::OperationFailed);
            };
            let Some(inspection) = vector_inspection(initial.state, detail) else {
                return Attempt::Failed(FailureCode::OperationFailed);
            };
            match inspection.state {
                State::Enabled => Attempt::Attempted,
                State::Conflict => Attempt::Conflict,
                State::Unknown => Attempt::Failed(FailureCode::OperationFailed),
                _ => {
                    let mut commands: Vec<Vec<String>> = Vec::new();
                    if inspection.plugin_present {
                        commands.push(vec![
                            "plugin".into(),
                            "remove".into(),
                            inspect::CODEX_PLUGIN_ID.into(),
                            "--json".into(),
                        ]);
                    }
                    if inspection.marketplace == Marketplace::Outdated {
                        commands.push(vec![
                            "plugin".into(),
                            "marketplace".into(),
                            "remove".into(),
                            inspect::MARKETPLACE_NAME.into(),
                            "--json".into(),
                        ]);
                    }
                    if inspection.marketplace != Marketplace::Current {
                        commands.push(vec![
                            "plugin".into(),
                            "marketplace".into(),
                            "add".into(),
                            resource_root.clone(),
                            "--json".into(),
                        ]);
                    }
                    commands.push(vec![
                        "plugin".into(),
                        "add".into(),
                        inspect::CODEX_PLUGIN_ID.into(),
                        "--json".into(),
                    ]);
                    run_codex_sequence(&exe, &commands, context, reserve, attempted, effect)
                }
            }
        }
        IntegrationId::Grok => {
            let Some(inspection) = vector_inspection(initial.state, detail) else {
                return Attempt::Failed(FailureCode::OperationFailed);
            };
            let bundle_root = context.capsule_root.join("bundle");
            match inspection.state {
                State::Enabled => Attempt::Attempted,
                State::Conflict => Attempt::Conflict,
                State::Disabled => {
                    let Some(exe) = exec::resolve_executable("grok", context.path_env) else {
                        return Attempt::Failed(FailureCode::OperationFailed);
                    };
                    run_step(
                        &exe,
                        &["plugin", "enable", inspect::GROK_PLUGIN_NAME],
                        context,
                        reserve,
                        attempted,
                        effect,
                    )
                }
                State::NotInstalled | State::UpdateRequired => {
                    let Some(exe) = exec::resolve_executable("grok", context.path_env) else {
                        return Attempt::Failed(FailureCode::OperationFailed);
                    };
                    let mut commands: Vec<Vec<String>> = Vec::new();
                    if inspection.plugin_present {
                        commands.push(vec![
                            "plugin".into(),
                            "uninstall".into(),
                            inspect::GROK_PLUGIN_NAME.into(),
                            "--confirm".into(),
                        ]);
                    }
                    commands.push(vec![
                        "plugin".into(),
                        "install".into(),
                        bundle_root.to_string_lossy().into_owned(),
                        "--trust".into(),
                    ]);
                    run_sequence(&exe, &commands, context, reserve, attempted, effect)
                }
                _ => Attempt::Failed(FailureCode::OperationFailed),
            }
        }
        IntegrationId::GitHubCopilot => {
            let Some(inspection) = vector_inspection(initial.state, detail) else {
                return Attempt::Failed(FailureCode::OperationFailed);
            };
            match inspection.state {
                State::Enabled => Attempt::Attempted,
                State::Conflict => Attempt::Conflict,
                State::Disabled => {
                    let Some(exe) = exec::resolve_executable("copilot", context.path_env) else {
                        return Attempt::Failed(FailureCode::OperationFailed);
                    };
                    run_step(
                        &exe,
                        &CopilotCommand::Enable.arguments(),
                        context,
                        reserve,
                        attempted,
                        effect,
                    )
                }
                State::NotInstalled | State::UpdateRequired => {
                    let Some(exe) = exec::resolve_executable("copilot", context.path_env) else {
                        return Attempt::Failed(FailureCode::OperationFailed);
                    };
                    let mut commands: Vec<Vec<String>> = Vec::new();
                    if inspection.plugin_present {
                        commands.push(CopilotCommand::Remove.owned_arguments());
                    }
                    if inspection.marketplace == Marketplace::Outdated {
                        commands.push(CopilotCommand::MarketplaceRemove.owned_arguments());
                    }
                    if inspection.marketplace != Marketplace::Current {
                        commands
                            .push(CopilotCommand::MarketplaceAdd(&resource_root).owned_arguments());
                    }
                    commands.push(CopilotCommand::Install.owned_arguments());
                    run_sequence(&exe, &commands, context, reserve, attempted, effect)
                }
                _ => Attempt::Failed(FailureCode::OperationFailed),
            }
        }
        IntegrationId::Cline => {
            let bundle_root = context.capsule_root.join("bundle");
            // Canonical order: source readiness gates before the state read.
            if !matches!(
                inspect::cline_package_state(&bundle_root),
                ClinePackage::Current
            ) {
                return Attempt::Failed(FailureCode::OperationFailed);
            }
            let state = initial.state;
            match state {
                State::Enabled => return Attempt::Attempted,
                State::NotInstalled | State::UpdateRequired => {}
                State::Conflict => return Attempt::Conflict,
                _ => return Attempt::Failed(FailureCode::OperationFailed),
            }
            let Some(exe) = exec::resolve_executable("cline", context.path_env) else {
                return Attempt::Failed(FailureCode::OperationFailed);
            };
            let mut commands: Vec<Vec<String>> = Vec::new();
            if state == State::UpdateRequired {
                commands.push(vec![
                    "plugin".into(),
                    "uninstall".into(),
                    "orchbee-agent-observation".into(),
                    "--json".into(),
                ]);
            }
            commands.push(vec![
                "plugin".into(),
                "install".into(),
                bundle_root.to_string_lossy().into_owned(),
                "--json".into(),
            ]);
            run_sequence(&exe, &commands, context, reserve, attempted, effect)
        }
        IntegrationId::Kilo
        | IntegrationId::Opencode
        | IntegrationId::Pi
        | IntegrationId::Omp
        | IntegrationId::Gemini
        | IntegrationId::QwenCode => {
            let Some(spec) = providers::guarded_spec(id) else {
                return Attempt::Failed(FailureCode::OperationFailed);
            };
            let outcome = guarded_enable(
                id, spec, context, &options, initial, attempted, residuals, effect,
            );
            if !matches!(outcome, Attempt::Attempted) {
                return outcome;
            }
            remove_obsolete_guarded(id, context, &options, attempted, effect)
        }
    }
}

fn disable_vector(
    id: IntegrationId,
    context: &ProviderContext,
    initial: &SnapshotItem,
    detail: &providers::InspectionDetail,
    trace: EffectTrace,
) -> Attempt {
    let EffectTrace {
        attempted,
        residuals,
        begun: effect,
    } = trace;
    let options = providers::resource_options(context);
    let reserve = providers::inspect_slots(id);
    match id {
        IntegrationId::Hermes => {
            #[cfg(windows)]
            {
                hermes_windows::update(
                    context,
                    &options,
                    false,
                    EffectTrace {
                        attempted,
                        residuals,
                        begun: effect,
                    },
                )
            }
            #[cfg(not(windows))]
            {
                Attempt::Refused(Refusal::ManualActionRequired)
            }
        }
        IntegrationId::Cursor
        | IntegrationId::QoderCli
        | IntegrationId::AntigravityCli
        | IntegrationId::Mastracode
        | IntegrationId::Droid
        | IntegrationId::Kimi
        | IntegrationId::Letta
        | IntegrationId::Devin => {
            #[cfg(windows)]
            {
                hook_configuration_windows::update(
                    id.as_str(),
                    context,
                    &options,
                    false,
                    attempted,
                    residuals,
                    effect,
                )
            }
            #[cfg(not(windows))]
            {
                let _ = residuals;
                Attempt::Refused(Refusal::ManualActionRequired)
            }
        }
        IntegrationId::ClaudeCode => {
            let Some(inspection) = vector_inspection(initial.state, detail) else {
                return Attempt::Failed(FailureCode::OperationFailed);
            };
            match inspection.state {
                State::NotInstalled | State::Disabled => Attempt::Attempted,
                State::Conflict => Attempt::Conflict,
                State::Enabled => {
                    let Some(exe) = exec::resolve_executable("claude", context.path_env) else {
                        return Attempt::Failed(FailureCode::OperationFailed);
                    };
                    run_step(
                        &exe,
                        &[
                            "plugin",
                            "disable",
                            inspect::CLAUDE_PLUGIN_ID,
                            "--scope",
                            "user",
                        ],
                        context,
                        reserve,
                        attempted,
                        effect,
                    )
                }
                State::UpdateRequired => {
                    let Some(exe) = exec::resolve_executable("claude", context.path_env) else {
                        return Attempt::Failed(FailureCode::OperationFailed);
                    };
                    let mut commands: Vec<Vec<String>> = Vec::new();
                    if inspection.plugin_present {
                        commands.push(vec![
                            "plugin".into(),
                            "uninstall".into(),
                            inspect::CLAUDE_PLUGIN_ID.into(),
                            "--scope".into(),
                            "user".into(),
                        ]);
                    }
                    if inspection.marketplace == Marketplace::Outdated {
                        commands.push(vec![
                            "plugin".into(),
                            "marketplace".into(),
                            "remove".into(),
                            inspect::MARKETPLACE_NAME.into(),
                            "--scope".into(),
                            "user".into(),
                        ]);
                    }
                    if commands.is_empty() {
                        Attempt::Failed(FailureCode::OperationFailed)
                    } else {
                        run_sequence(&exe, &commands, context, reserve, attempted, effect)
                    }
                }
                _ => Attempt::Failed(FailureCode::OperationFailed),
            }
        }
        IntegrationId::Codex => {
            let Some(exe) = exec::resolve_executable("codex", context.path_env) else {
                return Attempt::Failed(FailureCode::OperationFailed);
            };
            let Some(inspection) = vector_inspection(initial.state, detail) else {
                return Attempt::Failed(FailureCode::OperationFailed);
            };
            match inspection.state {
                State::Conflict => Attempt::Conflict,
                State::Unknown => Attempt::Failed(FailureCode::OperationFailed),
                _ => {
                    let mut commands: Vec<Vec<String>> = Vec::new();
                    if inspection.plugin_present {
                        commands.push(vec![
                            "plugin".into(),
                            "remove".into(),
                            inspect::CODEX_PLUGIN_ID.into(),
                            "--json".into(),
                        ]);
                    }
                    if inspection.marketplace == Marketplace::Outdated {
                        commands.push(vec![
                            "plugin".into(),
                            "marketplace".into(),
                            "remove".into(),
                            inspect::MARKETPLACE_NAME.into(),
                            "--json".into(),
                        ]);
                    }
                    if commands.is_empty() {
                        Attempt::Attempted
                    } else {
                        run_codex_sequence(&exe, &commands, context, reserve, attempted, effect)
                    }
                }
            }
        }
        IntegrationId::Grok => {
            let Some(inspection) = vector_inspection(initial.state, detail) else {
                return Attempt::Failed(FailureCode::OperationFailed);
            };
            match inspection.state {
                State::NotInstalled | State::Disabled => Attempt::Attempted,
                State::Conflict => Attempt::Conflict,
                _ if !inspection.plugin_present => Attempt::Failed(FailureCode::OperationFailed),
                _ => {
                    let Some(exe) = exec::resolve_executable("grok", context.path_env) else {
                        return Attempt::Failed(FailureCode::OperationFailed);
                    };
                    run_step(
                        &exe,
                        &[
                            "plugin",
                            "uninstall",
                            inspect::GROK_PLUGIN_NAME,
                            "--confirm",
                        ],
                        context,
                        reserve,
                        attempted,
                        effect,
                    )
                }
            }
        }
        IntegrationId::GitHubCopilot => {
            let Some(inspection) = vector_inspection(initial.state, detail) else {
                return Attempt::Failed(FailureCode::OperationFailed);
            };
            match inspection.state {
                State::NotInstalled | State::Disabled => Attempt::Attempted,
                State::Conflict => Attempt::Conflict,
                State::Enabled => {
                    let Some(exe) = exec::resolve_executable("copilot", context.path_env) else {
                        return Attempt::Failed(FailureCode::OperationFailed);
                    };
                    run_step(
                        &exe,
                        &CopilotCommand::Disable.arguments(),
                        context,
                        reserve,
                        attempted,
                        effect,
                    )
                }
                State::UpdateRequired => {
                    let Some(exe) = exec::resolve_executable("copilot", context.path_env) else {
                        return Attempt::Failed(FailureCode::OperationFailed);
                    };
                    let mut commands: Vec<Vec<String>> = Vec::new();
                    if inspection.plugin_present {
                        commands.push(CopilotCommand::Remove.owned_arguments());
                    }
                    if inspection.marketplace == Marketplace::Outdated {
                        commands.push(CopilotCommand::MarketplaceRemove.owned_arguments());
                    }
                    if commands.is_empty() {
                        Attempt::Failed(FailureCode::OperationFailed)
                    } else {
                        run_sequence(&exe, &commands, context, reserve, attempted, effect)
                    }
                }
                _ => Attempt::Failed(FailureCode::OperationFailed),
            }
        }
        IntegrationId::Cline => {
            match initial.state {
                State::NotInstalled => return Attempt::Attempted,
                State::Enabled | State::UpdateRequired => {}
                State::Conflict => return Attempt::Conflict,
                _ => return Attempt::Failed(FailureCode::OperationFailed),
            }
            let Some(exe) = exec::resolve_executable("cline", context.path_env) else {
                return Attempt::Failed(FailureCode::OperationFailed);
            };
            run_step(
                &exe,
                &["plugin", "uninstall", "orchbee-agent-observation", "--json"],
                context,
                reserve,
                attempted,
                effect,
            )
        }
        IntegrationId::Kilo
        | IntegrationId::Opencode
        | IntegrationId::Pi
        | IntegrationId::Omp
        | IntegrationId::Gemini
        | IntegrationId::QwenCode => {
            let Some(spec) = providers::guarded_spec(id) else {
                return Attempt::Failed(FailureCode::OperationFailed);
            };
            let outcome = guarded_disable(id, spec, context, &options, attempted, effect);
            if !matches!(outcome, Attempt::Attempted) {
                return outcome;
            }
            remove_obsolete_guarded(id, context, &options, attempted, effect)
        }
    }
}

// -- Guarded (filesystem) providers -----------------------------------------

/// Canonical guarded `enable`: source-ready → inspect → stage → re-inspect
/// (state must not have moved) → remove owned → publish — strengthened to
/// move-aside + no-replace publish + publish verification + backup restore.
#[expect(clippy::too_many_arguments)]
fn guarded_enable(
    id: IntegrationId,
    spec: providers::GuardedSpec,
    context: &ProviderContext,
    options: &inspect::ResourceOptions,
    _initial: &SnapshotItem,
    attempted: &mut Vec<AttemptedStep>,
    residuals: &mut Vec<PathBuf>,
    effect: &mut bool,
) -> Attempt {
    let source_path = context.capsule_root.join(spec.source_relative);
    let runtime_source = spec
        .runtime_relative
        .map(|relative| context.capsule_root.join(relative));
    let Some(target) = providers::guarded_target(id, spec, context.home) else {
        return Attempt::Failed(FailureCode::OperationFailed);
    };
    if !inspect::guarded_source_ready(&source_path, spec.kind, runtime_source.as_deref()) {
        return Attempt::Failed(FailureCode::OperationFailed);
    }
    let initial_state = inspect::inspect_guarded_target(
        &target,
        spec.kind,
        id,
        Path::new(spec.source_relative),
        options,
    );
    if initial_state == TargetState::Current {
        return Attempt::Attempted;
    }
    if !matches!(
        initial_state,
        TargetState::Missing | TargetState::UpdateRequired
    ) {
        return if initial_state == TargetState::Conflict {
            Attempt::Conflict
        } else {
            Attempt::Failed(FailureCode::OperationFailed)
        };
    }
    // Stage the entry in a sibling container — the first possibly
    // effectful call of the guarded vector, so the frontier crosses here.
    let Some(parent) = target.parent().map(|parent| parent.to_path_buf()) else {
        return Attempt::Failed(FailureCode::OperationFailed);
    };
    *effect = true;
    let container = match files::stage_directory(&parent, "entry") {
        Ok(container) => container,
        Err(_) => return Attempt::Failed(FailureCode::OperationFailed),
    };
    let entry_name = match target.file_name() {
        Some(name) => name.to_os_string(),
        None => return Attempt::Failed(FailureCode::OperationFailed),
    };
    let entry_path = container.join(&entry_name);
    let staged = stage_entry(
        &spec,
        id,
        &source_path,
        runtime_source.as_deref(),
        &entry_path,
    );
    if !staged {
        #[cfg(all(test, unix))]
        super::tests::fault_injection::boundary("cleanup-stage", &container);
        let _ = files::remove_owned(&container, true);
        return Attempt::Failed(FailureCode::OperationFailed);
    }
    // Staged digest verify before any install: a staged module file must
    // still carry the deterministic bootstrap bytes (extension directories
    // re-verify inside `stage_entry`). A mismatch is verification-failed —
    // undetermined — and the container stays named residue.
    if spec.kind != BootstrapKind::ExtensionDirectory {
        #[cfg(all(test, unix))]
        super::tests::fault_injection::boundary("digest-verify", &entry_path);
        let expected = inspect::module_bootstrap(spec.kind, id, &source_path);
        let intact = std::fs::read(&entry_path)
            .map(|bytes| bytes == expected.as_bytes())
            .unwrap_or(false);
        if !intact {
            attempted.push(AttemptedStep {
                step: StepKind::Filesystem {
                    operation: "digest-verify",
                    path: entry_path.clone(),
                },
                outcome: StepOutcome::Failed,
            });
            residuals.push(container);
            return Attempt::Failed(FailureCode::VerificationFailed);
        }
        attempted.push(AttemptedStep {
            step: StepKind::Filesystem {
                operation: "digest-verify",
                path: entry_path.clone(),
            },
            outcome: StepOutcome::Completed,
        });
    }
    // Canonical re-inspection gate: the target must not have moved.
    #[cfg(all(test, unix))]
    super::tests::fault_injection::boundary("reinspect-target", &target);
    let current_state = inspect::inspect_guarded_target(
        &target,
        spec.kind,
        id,
        Path::new(spec.source_relative),
        options,
    );
    if current_state != initial_state {
        #[cfg(all(test, unix))]
        super::tests::fault_injection::boundary("cleanup-stage", &container);
        if files::remove_owned(&container, true) != FsStep::Done {
            residuals.push(container);
        }
        return if current_state == TargetState::Conflict {
            Attempt::Refused(Refusal::Conflict)
        } else {
            Attempt::Failed(FailureCode::OperationFailed)
        };
    }
    // Strengthened replacement: move the owned target aside, publish
    // no-replace, verify, then drop the backup. A foreign arrival during the
    // gap restores the backup and reports conflict — never clobbered.
    let mut backup: Option<PathBuf> = None;
    let mut backup_digest: Option<String> = None;
    if matches!(current_state, TargetState::UpdateRequired) {
        match files::move_aside(&target) {
            Ok(moved) => {
                backup = moved;
                // Capture the backup's digest at move-aside time — the
                // digest-guarded removal below drops it only while these
                // exact bytes still answer.
                backup_digest = backup.as_ref().and_then(|path| files::entry_digest(path));
            }
            Err(step) => {
                #[cfg(all(test, unix))]
                super::tests::fault_injection::boundary("cleanup-stage", &container);
                let _ = files::remove_owned(&container, true);
                attempted.push(AttemptedStep {
                    step: StepKind::Filesystem {
                        operation: "move-aside",
                        path: target.clone(),
                    },
                    outcome: step_outcome(step),
                });
                return if step == FsStep::Conflict {
                    Attempt::Conflict
                } else {
                    Attempt::Failed(FailureCode::OperationFailed)
                };
            }
        }
        attempted.push(AttemptedStep {
            step: StepKind::Filesystem {
                operation: "move-aside",
                path: target.clone(),
            },
            outcome: StepOutcome::Completed,
        });
    }
    #[cfg(all(test, unix))]
    super::tests::fault_injection::boundary("install", &target);
    let publish = files::publish_no_replace(&entry_path, &target);
    attempted.push(AttemptedStep {
        step: StepKind::Filesystem {
            operation: "publish-entry",
            path: target.clone(),
        },
        outcome: step_outcome(publish),
    });
    match publish {
        FsStep::Done => {}
        FsStep::Conflict | FsStep::Failed => {
            // Restore what we moved aside — no-replace again, so a foreign
            // arrival survives and the backup is named as residual. A
            // deadline reached with a restore still owed funds no rollback:
            // the backup and the stage stay named residue instead.
            if backup.is_some() && now() >= context.deadline {
                if let Some(backup_path) = backup {
                    residuals.push(backup_path);
                }
                residuals.push(container);
                return Attempt::Failed(FailureCode::TimedOut);
            }
            if let Some(backup_path) = backup {
                #[cfg(all(test, unix))]
                super::tests::fault_injection::boundary("restore", &target);
                let restore = files::restore_backup(&backup_path, &target);
                attempted.push(AttemptedStep {
                    step: StepKind::Filesystem {
                        operation: "restore-backup",
                        path: target.clone(),
                    },
                    outcome: step_outcome(restore),
                });
                if restore != FsStep::Done {
                    residuals.push(backup_path);
                }
            }
            #[cfg(all(test, unix))]
            super::tests::fault_injection::boundary("cleanup-stage", &container);
            if files::remove_owned(&container, true) != FsStep::Done {
                residuals.push(container);
            }
            return if publish == FsStep::Conflict {
                Attempt::Conflict
            } else {
                Attempt::Failed(FailureCode::OperationFailed)
            };
        }
    }
    // Post-publish verification — the published entry must verify as ours.
    #[cfg(all(test, unix))]
    super::tests::fault_injection::boundary("post-verify", &target);
    let verified = inspect::inspect_guarded_target(
        &target,
        spec.kind,
        id,
        Path::new(spec.source_relative),
        options,
    ) == TargetState::Current;
    if !verified {
        // The published entry does not verify as ours — undetermined
        // post-effect; the backup stays named, never silently restored.
        if let Some(backup_path) = backup {
            residuals.push(backup_path);
        }
        return Attempt::Failed(FailureCode::VerificationFailed);
    }
    if let Some(backup_path) = backup {
        // Digest-guarded removal: the backup drops only while it still
        // carries the bytes captured at move-aside time. A foreign swap
        // keeps the occupant and reports conflict — never clobbered.
        #[cfg(all(test, unix))]
        super::tests::fault_injection::boundary("backup-remove", &backup_path);
        let still_ours = files::entry_digest(&backup_path)
            .zip(backup_digest.as_ref())
            .map(|(now, captured)| now == *captured)
            .unwrap_or(false);
        if !still_ours {
            attempted.push(AttemptedStep {
                step: StepKind::Filesystem {
                    operation: "remove-backup",
                    path: backup_path.clone(),
                },
                outcome: StepOutcome::Conflict,
            });
            residuals.push(backup_path);
            return Attempt::Conflict;
        }
        let is_dir = spec.kind == BootstrapKind::ExtensionDirectory;
        match files::remove_owned(&backup_path, is_dir) {
            FsStep::Done => {
                attempted.push(AttemptedStep {
                    step: StepKind::Filesystem {
                        operation: "remove-backup",
                        path: backup_path,
                    },
                    outcome: StepOutcome::Completed,
                });
            }
            step => {
                attempted.push(AttemptedStep {
                    step: StepKind::Filesystem {
                        operation: "remove-backup",
                        path: backup_path.clone(),
                    },
                    outcome: step_outcome(step),
                });
                residuals.push(backup_path);
            }
        }
    }
    // Stage cleanup is the last step: earlier failures leave it as named
    // residue rather than sweeping under an unverified or unrestored state.
    #[cfg(all(test, unix))]
    super::tests::fault_injection::boundary("cleanup-stage", &container);
    if files::remove_owned(&container, true) != FsStep::Done {
        residuals.push(container);
    }
    Attempt::Attempted
}

/// Build the staged entry: the module bootstrap file or the extension
/// directory (source copy + runtime scripts + marker + canonical
/// re-verification of the staged tree).
fn stage_entry(
    spec: &providers::GuardedSpec,
    id: IntegrationId,
    source_path: &Path,
    runtime_source: Option<&Path>,
    entry_path: &Path,
) -> bool {
    if spec.kind == BootstrapKind::ExtensionDirectory {
        let Some(runtime) = runtime_source else {
            return false;
        };
        if files::copy_tree(source_path, entry_path).is_err() {
            return false;
        }
        if files::copy_tree(runtime, &entry_path.join("scripts")).is_err() {
            return false;
        }
        #[cfg(windows)]
        if id == IntegrationId::QwenCode {
            let manifest = entry_path.join("qwen-extension.json");
            let bytes = files::read_bounded(&manifest, 512 * 1024)
                .ok()
                .and_then(|bytes| inspect::windows_qwen_manifest(&bytes));
            let Some(bytes) = bytes else {
                return false;
            };
            if files::remove_owned(&manifest, false) != FsStep::Done
                || files::write_file_exclusive(&manifest, &bytes).is_err()
            {
                return false;
            }
        }
        let Some(files) = inspect::fingerprint_directory(entry_path) else {
            return false;
        };
        let marker = serde_json::json!({
            "contractVersion": inspect::ENTRY_CONTRACT_VERSION,
            "files": files
                .into_iter()
                .map(|(path, sha256)| serde_json::json!({ "path": path, "sha256": sha256 }))
                .collect::<Vec<_>>(),
            "integrationId": id.as_str(),
            "kind": "guarded-extension",
        });
        if files::write_file_exclusive(
            &entry_path.join(inspect::DIRECTORY_MARKER_NAME),
            marker.to_string().as_bytes(),
        )
        .is_err()
        {
            return false;
        }
        // Canonical re-verifies the staged directory before publishing.
        matches!(
            inspect::inspect_owned_directory(entry_path, id),
            TargetState::Current
        )
    } else {
        let body = inspect::module_bootstrap(spec.kind, id, source_path);
        files::write_file_exclusive(entry_path, body.as_bytes()).is_ok()
    }
}

fn remove_obsolete_guarded(
    id: IntegrationId,
    context: &ProviderContext,
    options: &inspect::ResourceOptions,
    attempted: &mut Vec<AttemptedStep>,
    effect: &mut bool,
) -> Attempt {
    let Some(old) = providers::obsolete_guarded_spec(id) else {
        return Attempt::Attempted;
    };
    if cancelled(context) {
        return Attempt::Cancelled;
    }
    if now() >= context.deadline {
        return Attempt::Failed(FailureCode::TimedOut);
    }
    guarded_disable(id, old, context, options, attempted, effect)
}

fn guarded_disable(
    id: IntegrationId,
    spec: providers::GuardedSpec,
    context: &ProviderContext,
    options: &inspect::ResourceOptions,
    attempted: &mut Vec<AttemptedStep>,
    effect: &mut bool,
) -> Attempt {
    let Some(target) = providers::guarded_target(id, spec, context.home) else {
        return Attempt::Failed(FailureCode::OperationFailed);
    };
    #[cfg(all(test, unix))]
    super::tests::fault_injection::boundary("remove-verify-owned", &target);
    let state = inspect::inspect_guarded_target(
        &target,
        spec.kind,
        id,
        Path::new(spec.source_relative),
        options,
    );
    match state {
        TargetState::Missing => return Attempt::Attempted,
        TargetState::Current | TargetState::UpdateRequired => {}
        TargetState::Conflict => return Attempt::Conflict,
        _ => return Attempt::Failed(FailureCode::OperationFailed),
    }
    // Owned removal only: module entries unlink; an
    // extension directory removes through the no-follow walk.
    let is_directory = spec.kind == BootstrapKind::ExtensionDirectory;
    #[cfg(all(test, unix))]
    super::tests::fault_injection::boundary(
        if is_directory {
            "remove-dir-walk"
        } else {
            "remove-unlink"
        },
        &target,
    );
    // The owned removal is the guarded-disable vector's first effect.
    *effect = true;
    let step = files::remove_owned(&target, is_directory);
    attempted.push(AttemptedStep {
        step: StepKind::Filesystem {
            operation: "remove-entry",
            path: target,
        },
        outcome: step_outcome(step),
    });
    match step {
        FsStep::Done => Attempt::Attempted,
        FsStep::Conflict => Attempt::Conflict,
        _ => Attempt::Failed(FailureCode::OperationFailed),
    }
}

fn step_outcome(step: FsStep) -> StepOutcome {
    match step {
        FsStep::Done => StepOutcome::Completed,
        FsStep::Conflict => StepOutcome::Conflict,
        FsStep::Failed => StepOutcome::Failed,
    }
}

// -- `explain` plan projection ------------------------------------------------

/// One step of the guard-evaluated enable plan — the same vector the
/// frontier would run for the observed dimensions, projected for `explain`
/// without any effect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlannedStep {
    /// The managed capsule publication precedes provider steps on `enable`.
    CapsulePublish { target: PathBuf },
    /// A provider CLI command; `executable` is the resolved absolute path
    /// when PATH resolves it, else the bare registered name.
    Command {
        executable: String,
        argv: Vec<String>,
    },
    /// A guarded filesystem effect on the provider-owned target.
    GuardedWrite { target: PathBuf },
    /// Removal of a source-proven obsolete target after current publication.
    GuardedRemove { target: PathBuf },
}

/// `agent integration explain`'s plan — the enable-direction vector the
/// frontier would run for the observed dims. An empty plan means the
/// pre-effect gates admit nothing; `state`/`issue` carry why.
pub(crate) fn plan_enable(
    id: IntegrationId,
    context: &ProviderContext,
    initial: &SnapshotItem,
    detail: &providers::InspectionDetail,
) -> Vec<PlannedStep> {
    // The same pre-gates as `mutate` — a refused vector emits no plan.
    if matches!(
        initial.state,
        State::Conflict | State::Unknown | State::Enabled
    ) || initial.management_mode == ManagementMode::ManualOnly
        || initial.availability != Availability::Available
    {
        return Vec::new();
    }
    let resource_root = context.capsule_root.to_string_lossy().into_owned();
    let mut steps = Vec::new();
    if context.adapter_source.is_some()
        && !capsule::is_ready(context.capsule_root, context.adapter_source)
    {
        steps.push(PlannedStep::CapsulePublish {
            target: context.capsule_root.to_path_buf(),
        });
    }
    let command = |name: &'static str, argv: &[&str]| PlannedStep::Command {
        executable: exec::resolve_executable(name, context.path_env)
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| name.to_owned()),
        argv: argv.iter().map(|arg| (*arg).to_owned()).collect(),
    };
    match id {
        IntegrationId::Hermes => {
            #[cfg(windows)]
            if let Some(profile) = crate::agent_configuration_windows::hermes_home(context.home) {
                if initial.state != State::Disabled {
                    for name in ["__init__.py", "plugin.yaml"] {
                        steps.push(PlannedStep::GuardedWrite {
                            target: profile.join("plugins/orchbee-agent-observation").join(name),
                        });
                    }
                }
                steps.push(command(
                    "hermes",
                    &["plugins", "enable", super::hermes_windows::NAME],
                ));
                steps.push(command("hermes", &["plugins", "list", "--json"]));
            }
        }
        IntegrationId::Cursor
        | IntegrationId::QoderCli
        | IntegrationId::AntigravityCli
        | IntegrationId::Mastracode
        | IntegrationId::Droid
        | IntegrationId::Kimi
        | IntegrationId::Letta
        | IntegrationId::Devin => {
            #[cfg(windows)]
            if let Ok((target, _)) =
                super::hook_configuration_windows::load(id.as_str(), context.home)
            {
                steps.push(PlannedStep::GuardedWrite {
                    target: target.path,
                });
            }
        }
        IntegrationId::ClaudeCode => {
            let Some(inspection) = vector_inspection(initial.state, detail) else {
                return steps;
            };
            match inspection.state {
                State::Disabled => steps.push(command(
                    "claude",
                    &[
                        "plugin",
                        "enable",
                        inspect::CLAUDE_PLUGIN_ID,
                        "--scope",
                        "user",
                    ],
                )),
                State::NotInstalled | State::UpdateRequired => {
                    if inspection.state == State::UpdateRequired && inspection.plugin_present {
                        steps.push(command(
                            "claude",
                            &[
                                "plugin",
                                "uninstall",
                                inspect::CLAUDE_PLUGIN_ID,
                                "--scope",
                                "user",
                            ],
                        ));
                    }
                    if inspection.marketplace == Marketplace::Outdated {
                        steps.push(command(
                            "claude",
                            &[
                                "plugin",
                                "marketplace",
                                "remove",
                                inspect::MARKETPLACE_NAME,
                                "--scope",
                                "user",
                            ],
                        ));
                    }
                    if inspection.marketplace != Marketplace::Current {
                        steps.push(command(
                            "claude",
                            &[
                                "plugin",
                                "marketplace",
                                "add",
                                &resource_root,
                                "--scope",
                                "user",
                            ],
                        ));
                    }
                    steps.push(command(
                        "claude",
                        &[
                            "plugin",
                            "install",
                            inspect::CLAUDE_PLUGIN_ID,
                            "--scope",
                            "user",
                        ],
                    ));
                    steps.push(command(
                        "claude",
                        &[
                            "plugin",
                            "enable",
                            inspect::CLAUDE_PLUGIN_ID,
                            "--scope",
                            "user",
                        ],
                    ));
                }
                _ => {}
            }
        }
        IntegrationId::Codex => {
            if exec::resolve_executable("codex", context.path_env).is_none() {
                return steps;
            }
            let Some(inspection) = vector_inspection(initial.state, detail) else {
                return steps;
            };
            match inspection.state {
                State::Enabled | State::Conflict | State::Unknown => {}
                _ => {
                    #[cfg(windows)]
                    if let Some(target) = codex_windows::missing_default_home(context.home) {
                        steps.push(PlannedStep::GuardedWrite { target });
                    }
                    if inspection.plugin_present {
                        steps.push(command(
                            "codex",
                            &["plugin", "remove", inspect::CODEX_PLUGIN_ID, "--json"],
                        ));
                    }
                    if inspection.marketplace == Marketplace::Outdated {
                        steps.push(command(
                            "codex",
                            &[
                                "plugin",
                                "marketplace",
                                "remove",
                                inspect::MARKETPLACE_NAME,
                                "--json",
                            ],
                        ));
                    }
                    if inspection.marketplace != Marketplace::Current {
                        steps.push(command(
                            "codex",
                            &["plugin", "marketplace", "add", &resource_root, "--json"],
                        ));
                    }
                    steps.push(command(
                        "codex",
                        &["plugin", "add", inspect::CODEX_PLUGIN_ID, "--json"],
                    ));
                }
            }
        }
        IntegrationId::Grok => {
            let Some(inspection) = vector_inspection(initial.state, detail) else {
                return steps;
            };
            let bundle_root = context.capsule_root.join("bundle");
            match inspection.state {
                State::Disabled => steps.push(command(
                    "grok",
                    &["plugin", "enable", inspect::GROK_PLUGIN_NAME],
                )),
                State::NotInstalled | State::UpdateRequired => {
                    if inspection.plugin_present {
                        steps.push(command(
                            "grok",
                            &[
                                "plugin",
                                "uninstall",
                                inspect::GROK_PLUGIN_NAME,
                                "--confirm",
                            ],
                        ));
                    }
                    steps.push(command(
                        "grok",
                        &[
                            "plugin",
                            "install",
                            &bundle_root.to_string_lossy(),
                            "--trust",
                        ],
                    ));
                }
                _ => {}
            }
        }
        IntegrationId::GitHubCopilot => {
            let Some(inspection) = vector_inspection(initial.state, detail) else {
                return steps;
            };
            match inspection.state {
                State::Disabled => {
                    steps.push(command("copilot", &CopilotCommand::Enable.arguments()))
                }
                State::NotInstalled | State::UpdateRequired => {
                    if inspection.plugin_present {
                        steps.push(command("copilot", &CopilotCommand::Remove.arguments()));
                    }
                    if inspection.marketplace == Marketplace::Outdated {
                        steps.push(command(
                            "copilot",
                            &CopilotCommand::MarketplaceRemove.arguments(),
                        ));
                    }
                    if inspection.marketplace != Marketplace::Current {
                        steps.push(command(
                            "copilot",
                            &CopilotCommand::MarketplaceAdd(&resource_root).arguments(),
                        ));
                    }
                    steps.push(command("copilot", &CopilotCommand::Install.arguments()));
                }
                _ => {}
            }
        }
        IntegrationId::Cline => {
            let bundle_root = context.capsule_root.join("bundle");
            if !matches!(
                inspect::cline_package_state(&bundle_root),
                ClinePackage::Current
            ) {
                return steps;
            }
            match initial.state {
                State::Enabled | State::Conflict | State::Unknown => {}
                state => {
                    if state == State::UpdateRequired {
                        steps.push(command(
                            "cline",
                            &["plugin", "uninstall", "orchbee-agent-observation", "--json"],
                        ));
                    }
                    steps.push(command(
                        "cline",
                        &[
                            "plugin",
                            "install",
                            &bundle_root.to_string_lossy(),
                            "--json",
                        ],
                    ));
                }
            }
        }
        IntegrationId::Kilo
        | IntegrationId::Opencode
        | IntegrationId::Pi
        | IntegrationId::Omp
        | IntegrationId::Gemini
        | IntegrationId::QwenCode => {
            let Some(spec) = providers::guarded_spec(id) else {
                return steps;
            };
            let Some(target) = providers::guarded_target(id, spec, context.home) else {
                return steps;
            };
            if providers::obsolete_guarded_spec(id).is_none()
                || !matches!(
                    detail,
                    providers::InspectionDetail::Guarded {
                        target_state: TargetState::Current,
                        ..
                    }
                )
            {
                steps.push(PlannedStep::GuardedWrite { target });
            }
            if let providers::InspectionDetail::Guarded {
                obsolete_target_state: Some(state),
                ..
            } = detail
                && *state != TargetState::Missing
                && let Some(old) = providers::obsolete_guarded_spec(id)
                && let Some(target) = providers::guarded_target(id, old, context.home)
            {
                steps.push(PlannedStep::GuardedRemove { target });
            }
        }
    }
    steps
}
