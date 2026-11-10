//! Pin one Codex configuration root, prepare only its absent profile default, and
//! remove on disable what enable added.
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

use super::providers::ProviderContext;
use super::{
    Attempt, AttemptedStep, FailureCode, Marketplace, Refusal, SLOT, StepKind, StepOutcome,
    cancelled, files, inspect, now,
};

/// Enable adds Orchbee's marketplace together with its plugin, so disable removes
/// any marketplace this environment owns, current or outdated.
pub(super) fn disable_removes_marketplace(marketplace: Marketplace) -> bool {
    matches!(marketplace, Marketplace::Current | Marketplace::Outdated)
}

/// Codex keeps an empty `plugins/cache/<marketplace>` folder after removing Orchbee's
/// plugin and marketplace. Once Codex no longer names either, disable removes that
/// folder while it is empty; anything left in it stays, as a named residual.
pub(super) fn finish_disable(
    context: &ProviderContext,
    attempted: &mut Vec<AttemptedStep>,
    residuals: &mut Vec<PathBuf>,
    effect: &mut bool,
) -> Attempt {
    let options = super::providers::resource_options(context);
    let observed = inspect::inspect_codex(context.home, &options);
    if observed.plugin_present || observed.marketplace != Marketplace::Absent {
        return Attempt::Failed(FailureCode::VerificationFailed);
    }
    let Some(profile) = inspect::codex_home(context.home) else {
        return Attempt::Failed(FailureCode::OperationFailed);
    };
    let cache = profile
        .join("plugins")
        .join("cache")
        .join(inspect::MARKETPLACE_NAME);
    if std::fs::symlink_metadata(&cache).is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
    {
        return Attempt::Attempted;
    }
    if cancelled(context) {
        return Attempt::Cancelled;
    }
    *effect = true;
    let outcome = match files::remove_empty_directory(&cache) {
        Ok(_) => StepOutcome::Completed,
        Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => {
            residuals.push(cache);
            return Attempt::Attempted;
        }
        Err(_) => {
            residuals.push(cache.clone());
            StepOutcome::Failed
        }
    };
    attempted.push(AttemptedStep {
        step: StepKind::Filesystem {
            operation: "remove-entry",
            path: cache,
        },
        outcome,
    });
    Attempt::Attempted
}

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
