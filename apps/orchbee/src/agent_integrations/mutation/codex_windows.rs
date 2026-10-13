//! Pin one Codex configuration root and prepare only its absent profile default.
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

use super::providers::ProviderContext;
use super::{
    Attempt, AttemptedStep, FailureCode, Refusal, SLOT, StepKind, StepOutcome, cancelled, files,
    inspect, now,
};

pub(super) fn missing_default_home(home: &Path) -> Option<PathBuf> {
    let profile = inspect::codex_home(home)?;
    (profile == home.join(".codex")
        && files::pin_tree(&profile, false)
            .is_err_and(|error| error.kind() == io::ErrorKind::NotFound))
    .then_some(profile)
}

pub(super) fn prepare_home(
    context: &ProviderContext,
    reserve_slots: usize,
    attempted: &mut Vec<AttemptedStep>,
    effect: &mut bool,
) -> Result<Vec<File>, Attempt> {
    let failed = || Attempt::Failed(FailureCode::OperationFailed);
    let profile = inspect::codex_home(context.home).ok_or_else(failed)?;
    // The caller's home must already exist. Holding its ancestors prevents this
    // preparation from recreating a removed home or traversing a replacement.
    let _home_pins = files::pin_tree(context.home, false).map_err(|_| failed())?;
    match files::pin_tree(&profile, false) {
        Ok(pins) => return Ok(pins),
        Err(error)
            if error.kind() == io::ErrorKind::NotFound
                && profile == context.home.join(".codex") => {}
        Err(_) => return Err(failed()),
    }
    if cancelled(context) {
        return Err(Attempt::Cancelled);
    }
    if now() + SLOT * (reserve_slots.saturating_add(1) as u32) > context.deadline {
        return Err(Attempt::Refused(Refusal::Deadline));
    }
    // Explicit nonempty overrides were required to exist during selection.
    // This creates only the guarded default, never a fallback or foreign target.
    *effect = true;
    let prepared = files::pin_tree(&profile, true);
    attempted.push(AttemptedStep {
        step: StepKind::Filesystem {
            operation: "prepare-codex-home",
            path: profile,
        },
        outcome: if prepared.is_ok() {
            StepOutcome::Completed
        } else {
            StepOutcome::Failed
        },
    });
    prepared.map_err(|_| failed())
}
