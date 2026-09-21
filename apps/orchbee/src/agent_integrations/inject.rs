//! Test-only fault injection, step witnesses, and the scripted command
//! runner for the frozen fixture packs (Root57 `provider-effects52`,
//! Report64 `provider-conformance64`). Everything here is `#[cfg(test)]` —
//! production carries zero cost and zero surface.
//!
//! Two injection styles, per the packs' contract:
//! - primitive faults: a named fs/exec primitive answers a scripted errno;
//! - boundary interposition: a named step boundary runs a harness action
//!   (pre-create an occupant, swap a symlink mid-walk, raise the cancel
//!   flag, advance the test clock).
//! Every boundary also records a witness — `attempted[]` assertions check
//! that the steps the pack names really ran.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The fs primitives a fixture may fault — the pack's `inject.errno` lands
/// here, mapped once by the harness.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Prim {
    /// `fs::create_dir` inside `stage_directory`.
    StageCreateDir,
    /// `fs::hard_link` inside `publish_no_replace` (file install).
    HardLink,
    /// `renameat_with(NOREPLACE)`/`renameatx_np(EXCL)` inside
    /// `move_no_replace` (dir install, move-aside, restore).
    RenameNoReplace,
    /// `fs::remove_file` inside `remove_owned`.
    UnlinkFile,
    /// `Dir::openat` for a walk component inside `remove_contents`.
    WalkOpenDir,
    /// `unlinkat` inside `remove_contents`.
    WalkUnlink,
}

/// One scripted command outcome for the `exec::run_command` seam — the
/// pack's `commands[].result` vocabulary.
#[derive(Debug)]
pub(crate) enum ScriptedResult {
    Completed(Vec<u8>),
    /// Completed, then the test clock advances — a real command's wall
    /// time the harness needs so a later slot-admission consult sees the
    /// spend.
    CompletedAdvance(Vec<u8>, Duration),
    Failed,
    /// Scripted timeout — wired for the Report64 harness's command
    /// outcomes; no Root57 case injects one.
    #[allow(dead_code)]
    TimedOut,
    /// Scripted oversized output — same Report64 consumer.
    #[allow(dead_code)]
    OutputTooLarge,
    /// In-flight until the cancel flag fires — models the SIGINT kill path
    /// without a real child.
    WaitForCancel,
}

/// One queued boundary action. `Sigint`/`AdvanceClock` pop on first
/// arrival; `Act` runs on every arrival until it answers `true` — the
/// mid-walk swap waits for its named component.
pub(crate) enum BoundaryAction {
    /// Raise the shared cancel flag (the pack's `SIGINT` injection).
    Sigint,
    /// Advance the test clock — the pack's `setup.clock` states become a
    /// real skew every `now()` consults.
    AdvanceClock(Duration),
    /// Run a harness interposition (occupant pre-creation, mid-walk swap).
    /// `true` consumes the action; `false` keeps it queued for the next
    /// arrival at the same step.
    Act(Box<dyn FnMut(&Path) -> bool>),
}

struct PrimRule {
    /// Clean invocations before faults start.
    skip: usize,
    /// Each subsequent invocation pops one errno; an empty queue keeps the
    /// primitive clean again. An armed fault stays queued until its named
    /// step boundary has fired — step-scoped injection so an early user of
    /// the same primitive (move-aside vs restore, say) never consumes the
    /// wrong case's fault.
    faults: VecDeque<(i32, Option<&'static str>)>,
}

#[derive(Default)]
pub(crate) struct Plan {
    boundaries: HashMap<&'static str, VecDeque<BoundaryAction>>,
    /// Step boundaries that have fired — arms step-scoped prim faults.
    seen: std::collections::HashSet<&'static str>,
    prims: HashMap<Prim, PrimRule>,
    commands: VecDeque<ScriptedResult>,
    /// Scripted command argv expectations — `None` accepts any argv.
    argv_log: Vec<Vec<String>>,
}

thread_local! {
    static PLAN: RefCell<Option<Plan>> = const { RefCell::new(None) };
    static WITNESSED: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    static SKEW: RefCell<Duration> = const { RefCell::new(Duration::ZERO) };
    static CANCEL: RefCell<Option<Arc<AtomicBool>>> = const { RefCell::new(None) };
}

impl Plan {
    pub(crate) fn prim_fault(&mut self, prim: Prim, errno: i32, times: usize) {
        self.prims
            .entry(prim)
            .or_insert_with(|| PrimRule {
                skip: 0,
                faults: VecDeque::new(),
            })
            .faults
            .extend(std::iter::repeat_n((errno, None), times));
    }

    /// Fault `prim` `times` times, but only once `step` has fired — the
    /// pack's `inject.atStep` lands on the step's own primitive call, not
    /// an earlier caller sharing the primitive.
    pub(crate) fn prim_fault_at(
        &mut self,
        step: &'static str,
        prim: Prim,
        errno: i32,
        times: usize,
    ) {
        self.prims
            .entry(prim)
            .or_insert_with(|| PrimRule {
                skip: 0,
                faults: VecDeque::new(),
            })
            .faults
            .extend(std::iter::repeat_n((errno, Some(step)), times));
    }

    pub(crate) fn on(&mut self, step: &'static str, action: BoundaryAction) {
        self.boundaries.entry(step).or_default().push_back(action);
    }

    pub(crate) fn script_command(&mut self, result: ScriptedResult) {
        self.commands.push_back(result);
    }
}

/// Install the case plan; the previous plan is dropped wholesale.
pub(crate) fn install(plan: Plan) {
    PLAN.with(|slot| *slot.borrow_mut() = Some(plan));
    WITNESSED.with(|log| log.borrow_mut().clear());
    SKEW.with(|skew| *skew.borrow_mut() = Duration::ZERO);
    CANCEL.with(|slot| *slot.borrow_mut() = Some(Arc::new(AtomicBool::new(false))));
}

/// Drop the plan, witnesses, skew, and the cancel flag.
pub(crate) fn reset() {
    PLAN.with(|slot| *slot.borrow_mut() = None);
    WITNESSED.with(|log| log.borrow_mut().clear());
    SKEW.with(|skew| *skew.borrow_mut() = Duration::ZERO);
    CANCEL.with(|slot| *slot.borrow_mut() = None);
}

/// The harness-shared cancel flag — exposed for harnesses that hand the
/// flag itself to a context rather than the `InjectCancel` source.
#[allow(dead_code)]
pub(crate) fn cancel_flag() -> Option<Arc<AtomicBool>> {
    CANCEL.with(|slot| slot.borrow().clone())
}

/// A `TerminationSource` backed by the plan's shared cancel flag — the
/// harness hands it to `ProviderContext.cancel` so `SIGINT` boundary
/// actions read as a real signal to the frontier.
pub(crate) struct InjectCancel;

impl crate::termination_signal::TerminationSource for InjectCancel {
    fn requested(&self) -> bool {
        cancel_requested()
    }
}

pub(crate) fn cancel_requested() -> bool {
    CANCEL.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|flag| flag.load(Ordering::Relaxed))
            .unwrap_or(false)
    })
}

/// The test clock: `Instant::now()` plus the skew harness advances.
pub(crate) fn now() -> Instant {
    Instant::now() + SKEW.with(|skew| *skew.borrow())
}

/// Push the test clock forward — every `now()` consult then sees the
/// deadline closer. Boundary `Act` closures call this for "budget
/// exhausted mid-window" injections.
pub(crate) fn advance_clock(delta: Duration) {
    SKEW.with(|skew| *skew.borrow_mut() += delta);
}

/// A named step boundary: record the witness, then run the queued action.
/// `path` is the interposition context (e.g. the walk component's name).
pub(crate) fn boundary(step: &'static str, path: &Path) {
    WITNESSED.with(|log| log.borrow_mut().push(step));
    PLAN.with(|slot| {
        if let Some(plan) = slot.borrow_mut().as_mut() {
            plan.seen.insert(step);
        }
    });
    enum Fired {
        Sigint,
        Advance(Duration),
    }
    let fired = PLAN.with(|slot| {
        let mut slot = slot.borrow_mut();
        let plan = slot.as_mut()?;
        let queue = plan.boundaries.get_mut(step)?;
        match queue.front_mut()? {
            // The interposition runs under the plan borrow — harness
            // actions only ever touch the filesystem, never the plan.
            BoundaryAction::Act(act) => {
                if act(path) {
                    queue.pop_front();
                }
                None
            }
            BoundaryAction::Sigint => {
                queue.pop_front();
                Some(Fired::Sigint)
            }
            BoundaryAction::AdvanceClock(delta) => {
                let delta = *delta;
                queue.pop_front();
                Some(Fired::Advance(delta))
            }
        }
    });
    match fired {
        Some(Fired::Sigint) => {
            CANCEL.with(|slot| {
                if let Some(flag) = slot.borrow().as_ref() {
                    flag.store(true, Ordering::Relaxed);
                }
            });
        }
        Some(Fired::Advance(delta)) => advance_clock(delta),
        None => {}
    }
}

/// A fault for `prim` right now, or `None` while skips remain or the queue
/// ran dry. One call consumes one invocation's worth of state.
pub(crate) fn prim(prim: Prim) -> Option<i32> {
    PLAN.with(|slot| {
        let mut slot = slot.borrow_mut();
        let plan = slot.as_mut()?;
        let rule = plan.prims.get_mut(&prim)?;
        if let Some((_, arm)) = rule.faults.front()
            && arm.is_some_and(|step| !plan.seen.contains(step))
        {
            return None;
        }
        if rule.skip > 0 {
            rule.skip -= 1;
            return None;
        }
        rule.faults.pop_front().map(|(errno, _)| errno)
    })
}

/// Pop the next scripted command result. The argv is logged for the
/// pack's `expect.commands` assertion.
pub(crate) fn next_command(argv: &[String]) -> Option<ScriptedResult> {
    PLAN.with(|slot| {
        let mut slot = slot.borrow_mut();
        let plan = slot.as_mut()?;
        plan.argv_log.push(argv.to_vec());
        plan.commands.pop_front()
    })
}

/// Whether the scripted queue is installed at all — distinguishes "queue
/// empty" (a real spawn would be a bug) from "no script" (spawn normally).
pub(crate) fn commands_scripted() -> bool {
    PLAN.with(|slot| slot.borrow().is_some())
}

/// Materialize the next scripted command outcome — the full argv lands in
/// the log verbatim; `WaitForCancel` polls the shared flag inside the slot
/// exactly like the real loop polls a child.
pub(crate) fn run_scripted(
    executable: &Path,
    arguments: &[String],
    deadline: Instant,
    cancel: Option<&dyn crate::termination_signal::TerminationSource>,
) -> super::exec::CommandResult {
    let argv: Vec<String> = std::iter::once(executable.display().to_string())
        .chain(arguments.iter().cloned())
        .collect();
    match next_command(&argv).unwrap_or(ScriptedResult::Failed) {
        ScriptedResult::Completed(stdout) => super::exec::CommandResult::Completed { stdout },
        ScriptedResult::CompletedAdvance(stdout, advance) => {
            advance_clock(advance);
            super::exec::CommandResult::Completed { stdout }
        }
        ScriptedResult::Failed => super::exec::CommandResult::Failed,
        ScriptedResult::TimedOut => {
            // A timed-out command occupied its full slot — run plus the
            // bounded kill path — or whatever of it the invocation
            // deadline still funded; the skewed clock shows exactly that
            // spend so downstream reserve checks see the real exhaustion.
            let elapsed = super::exec::SLOT.min(deadline.saturating_duration_since(now()));
            advance_clock(elapsed);
            super::exec::CommandResult::TimedOut
        }
        ScriptedResult::OutputTooLarge => super::exec::CommandResult::OutputTooLarge,
        ScriptedResult::WaitForCancel => {
            while now() < deadline {
                if cancel.is_some_and(|source| source.requested()) || cancel_requested() {
                    return super::exec::CommandResult::Cancelled;
                }
                std::thread::yield_now();
            }
            super::exec::CommandResult::TimedOut
        }
    }
}

/// The argv sequence the scripted seam observed, in order — the Report64
/// harness asserts `expect.commands` against it.
#[allow(dead_code)]
pub(crate) fn argv_log() -> Vec<Vec<String>> {
    PLAN.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|plan| plan.argv_log.clone())
            .unwrap_or_default()
    })
}

/// The ordered step witnesses this invocation crossed.
pub(crate) fn witnessed() -> Vec<&'static str> {
    WITNESSED.with(|log| log.borrow().clone())
}

/// `io::Error` for an injected errno.
pub(crate) fn errno_error(errno: i32) -> io::Error {
    io::Error::from_raw_os_error(errno)
}
