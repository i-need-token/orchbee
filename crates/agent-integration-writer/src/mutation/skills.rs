//! Filesystem publish and removal mutations for `<skills_root>/orchbee/SKILL.md`.

use std::io::ErrorKind;

use super::{
    Attempt, EffectTrace, FailureCode, StepKind, cancelled, file_publication, files, now,
    step_outcome,
};
use crate::{
    inspect::ResourceOptions,
    model::IntegrationId,
    providers::ProviderContext,
    skills::{self, SkillState},
};

/// Publishes or updates `<skills_root>/orchbee/SKILL.md` when missing or outdated.
pub(super) fn enable(
    id: IntegrationId,
    context: &ProviderContext,
    options: &ResourceOptions,
    trace: EffectTrace,
) -> Attempt {
    if !skills::supported(id) {
        return Attempt::Attempted;
    }
    if cancelled(context) {
        return Attempt::Cancelled;
    }
    let Some(target) = skills::skill_target(id, context.home, context.variables) else {
        return Attempt::Failed(FailureCode::OperationFailed);
    };
    let Some(replacement) = skills::skill_bytes(context.capsule_root) else {
        return Attempt::Failed(FailureCode::OperationFailed);
    };
    let existing = match skills::inspect(id, context.home, context.variables, options) {
        SkillState::NotSupported | SkillState::Current => return Attempt::Attempted,
        // A file this environment does not own is left as it is, and no step records it.
        SkillState::Foreign | SkillState::Elsewhere => return Attempt::Attempted,
        SkillState::Unknown => return Attempt::Failed(FailureCode::OperationFailed),
        SkillState::Missing => None,
        SkillState::Outdated => match files::read_bounded(&target, 64 * 1024) {
            Ok(bytes) => Some(bytes),
            Err(_) => return Attempt::Failed(FailureCode::OperationFailed),
        },
    };
    if now() >= context.deadline {
        return Attempt::Refused(super::Refusal::Deadline);
    }
    let outcome = file_publication::replace(&target, existing, replacement, context, trace);
    if outcome != Attempt::Attempted {
        return outcome;
    }
    if skills::inspect(id, context.home, context.variables, options) == SkillState::Current {
        Attempt::Attempted
    } else {
        Attempt::Failed(FailureCode::VerificationFailed)
    }
}

/// Removes this environment's `<skills_root>/orchbee/SKILL.md` and deletes `<skills_root>/orchbee`
/// if it becomes empty, preserving any user-authored or sibling files.
pub(super) fn disable(
    id: IntegrationId,
    context: &ProviderContext,
    options: &ResourceOptions,
    trace: EffectTrace,
) -> Attempt {
    if !skills::supported(id) {
        return Attempt::Attempted;
    }
    let Some(target) = skills::skill_target(id, context.home, context.variables) else {
        return Attempt::Failed(FailureCode::OperationFailed);
    };
    let current = skills::inspect(id, context.home, context.variables, options);
    match current {
        SkillState::NotSupported | SkillState::Missing => return Attempt::Attempted,
        // A file this environment does not own is left as it is, and no step records it.
        SkillState::Foreign | SkillState::Elsewhere => return Attempt::Attempted,
        SkillState::Unknown => return Attempt::Failed(FailureCode::OperationFailed),
        SkillState::Current | SkillState::Outdated => {}
    }
    if cancelled(context) {
        return Attempt::Cancelled;
    }
    if now() >= context.deadline {
        return Attempt::Refused(super::Refusal::Deadline);
    }
    let EffectTrace {
        attempted,
        begun: effect,
        ..
    } = trace;
    *effect = true;
    let step = files::remove_owned(&target, false);
    attempted.push(super::AttemptedStep {
        step: StepKind::Filesystem {
            operation: "remove-entry",
            path: target.clone(),
        },
        outcome: step_outcome(step),
    });
    match step {
        files::FsStep::Done => {}
        files::FsStep::Conflict => return Attempt::Conflict,
        files::FsStep::Failed => return Attempt::Failed(FailureCode::OperationFailed),
    }

    if let Some(dir) = target.parent() {
        match files::remove_empty_directory(dir) {
            Ok(true) => {
                attempted.push(super::AttemptedStep {
                    step: StepKind::Filesystem {
                        operation: "remove-entry",
                        path: dir.to_path_buf(),
                    },
                    outcome: super::StepOutcome::Completed,
                });
            }
            Ok(false) => {}
            Err(error) if is_directory_not_empty(&error) => {}
            Err(_) => {
                attempted.push(super::AttemptedStep {
                    step: StepKind::Filesystem {
                        operation: "remove-entry",
                        path: dir.to_path_buf(),
                    },
                    outcome: super::StepOutcome::Failed,
                });
                return Attempt::Failed(FailureCode::OperationFailed);
            }
        }
    }

    match skills::inspect(id, context.home, context.variables, options) {
        SkillState::Missing => Attempt::Attempted,
        _ => Attempt::Failed(FailureCode::VerificationFailed),
    }
}

fn is_directory_not_empty(error: &std::io::Error) -> bool {
    if error.kind() == ErrorKind::DirectoryNotEmpty {
        return true;
    }
    #[cfg(unix)]
    if matches!(
        error.raw_os_error(),
        Some(nix::libc::ENOTEMPTY | nix::libc::EEXIST)
    ) {
        return true;
    }
    false
}
