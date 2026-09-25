//! Optimistic, bounded replacement of a provider-owned JSON configuration.
//! The original remains recoverable until the published bytes are verified.
use super::super::{
    files, inspect::ResourceOptions, json_hooks_windows as hooks, model::State,
    providers::ProviderContext,
};
use super::{
    Attempt, AttemptedStep, EffectTrace, FailureCode, FsStep, StepKind, cancelled, now,
    step_outcome,
};
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

pub(super) fn update(
    id: &str,
    context: &ProviderContext,
    options: &ResourceOptions,
    enable: bool,
    attempted: &mut Vec<AttemptedStep>,
    residuals: &mut Vec<PathBuf>,
    effect: &mut bool,
) -> Attempt {
    let Some(target) = crate::agent_configuration_windows::json_hooks(id, context.home) else {
        return Attempt::Failed(FailureCode::OperationFailed);
    };
    update_observed(
        id,
        &target,
        context,
        options,
        enable,
        EffectTrace {
            attempted,
            residuals,
            begun: effect,
        },
        |_, _| {},
    )
}

fn update_observed(
    id: &str,
    target: &Path,
    context: &ProviderContext,
    options: &ResourceOptions,
    enable: bool,
    trace: EffectTrace,
    mut observe: impl FnMut(&str, &Path),
) -> Attempt {
    let EffectTrace {
        attempted,
        residuals,
        begun: effect,
    } = trace;
    let expected = match hooks::read(target) {
        Ok(bytes) => bytes,
        Err(_) => return Attempt::Failed(FailureCode::OperationFailed),
    };
    if !matches!(
        hooks::state(id, expected.as_deref(), options),
        State::NotInstalled | State::Enabled | State::UpdateRequired
    ) {
        return Attempt::Conflict;
    }
    let Some(replacement) = hooks::patch(id, expected.as_deref(), context.capsule_root, enable)
    else {
        return Attempt::Failed(FailureCode::OperationFailed);
    };
    observe("snapshot", target);
    if let Some(stop) = stop(context) {
        return stop;
    }
    let Some(parent) = target.parent() else {
        return Attempt::Failed(FailureCode::OperationFailed);
    };
    *effect = true;
    let container = match files::stage_directory(parent, "json-hook") {
        Ok(path) => path,
        Err(_) => return Attempt::Failed(FailureCode::OperationFailed),
    };
    let staged = container.join("hooks.json");
    if files::write_file_exclusive(&staged, &replacement).is_err() {
        residuals.push(container);
        return Attempt::Failed(FailureCode::OperationFailed);
    }
    record(attempted, "stage-configuration", &staged, FsStep::Done);
    let stage_digest = files::entry_digest(&container);
    let mut backup = None;
    let mut private_result = None;
    let result = (|| {
        if let Some(stop) = stop(context) {
            return stop;
        }
        let original: Option<File> = match files::read_for_move(target, hooks::LIMIT) {
            Ok((file, bytes)) if expected.as_ref() == Some(&bytes) => Some(file),
            Err(error) if error.kind() == io::ErrorKind::NotFound && expected.is_none() => None,
            _ => return Attempt::Conflict,
        };
        observe("pre-move", target);
        if let Some(stop) = stop(context) {
            return stop;
        }
        if original.is_some() {
            match files::move_aside(target) {
                Ok(Some(path)) => backup = Some(path),
                Ok(None) => return Attempt::Conflict,
                Err(step) => {
                    record(attempted, "move-configuration-aside", target, step);
                    return failed(step);
                }
            }
            record(attempted, "move-configuration-aside", target, FsStep::Done);
        }
        observe("moved", target);
        let mut publish_source = staged.clone();
        if let Some(moved) = backup.clone() {
            if hooks::read(&moved).ok().flatten() != expected {
                return Attempt::Conflict;
            }
            if let Some(stop) = stop(context) {
                return stop;
            }
            let recovery = container.join("original.json");
            let merged = files::preserve_configuration_metadata(&moved, &staged, &recovery);
            record(attempted, "preserve-configuration-metadata", &moved, merged);
            if hooks::read(&recovery).ok().flatten() == expected {
                backup = Some(recovery);
            } else if hooks::read(&moved).ok().flatten() != expected {
                // No proven original may be restored. Every uncertain name survives.
                backup = None;
                residuals.push(moved.clone());
                residuals.push(container.clone());
                return Attempt::Failed(FailureCode::VerificationFailed);
            }
            if merged != FsStep::Done
                || hooks::read(&moved).ok().flatten().as_ref() != Some(&replacement)
            {
                if backup.as_ref() != Some(&moved) && std::fs::symlink_metadata(&moved).is_ok() {
                    residuals.push(moved);
                }
                return Attempt::Failed(FailureCode::OperationFailed);
            }
            private_result = Some(moved.clone());
            publish_source = moved;
        }
        observe("metadata", target);
        // Keep the verified backup immovable until the publication finishes.
        let backup_guard = if let Some(path) = &backup {
            let Ok(file) = files::regular_file(path) else {
                return Attempt::Conflict;
            };
            if hooks::read(path).ok().flatten() != expected {
                return Attempt::Conflict;
            }
            Some(file)
        } else {
            None
        };
        if let Some(stop) = stop(context) {
            return stop;
        }
        let published = files::publish_no_replace(&publish_source, target);
        record(attempted, "publish-configuration", target, published);
        if published != FsStep::Done {
            return failed(published);
        }
        private_result = None;
        observe("published", target);
        if hooks::read(target).ok().flatten().as_ref() != Some(&replacement) {
            return Attempt::Failed(FailureCode::VerificationFailed);
        }
        record(attempted, "verify-configuration", target, FsStep::Done);
        drop(backup_guard);
        drop(original);
        Attempt::Attempted
    })();
    if let Some(path) = private_result {
        residuals.push(path);
    }
    if let Some(path) = backup {
        if stop(context).is_some() {
            residuals.push(path);
        } else if result == Attempt::Attempted {
            // Only the exact original bytes may be discarded after verification.
            let step = if hooks::read(&path).ok().flatten() == expected {
                files::remove_owned(&path, false)
            } else {
                FsStep::Conflict
            };
            record(attempted, "remove-configuration-backup", &path, step);
            if step != FsStep::Done {
                residuals.push(path);
            }
        } else {
            // Restoring never replaces a writer's arrival, even after a failed verify.
            let step = files::restore_backup(&path, target);
            record(attempted, "restore-configuration-backup", target, step);
            if step != FsStep::Done {
                residuals.push(path);
            }
        }
    }
    let intact_stage = stage_digest.is_some() && files::entry_digest(&container) == stage_digest;
    // A successful publish consumes the sole staged file; the empty directory is ours.
    let consumed_stage = result == Attempt::Attempted
        && std::fs::read_dir(&container).is_ok_and(|mut entries| entries.next().is_none());
    if stop(context).is_some()
        || !(intact_stage || consumed_stage)
        || files::remove_owned(&container, true) != FsStep::Done
    {
        residuals.push(container);
    }
    let mut unique = Vec::new();
    for path in residuals.drain(..) {
        if !unique.contains(&path) {
            unique.push(path);
        }
    }
    *residuals = unique;
    stop(context).unwrap_or(result)
}

fn stop(context: &ProviderContext) -> Option<Attempt> {
    if cancelled(context) {
        Some(Attempt::Cancelled)
    } else if now() >= context.deadline {
        Some(Attempt::Failed(FailureCode::TimedOut))
    } else {
        None
    }
}
fn failed(step: FsStep) -> Attempt {
    if step == FsStep::Conflict {
        Attempt::Conflict
    } else {
        Attempt::Failed(FailureCode::OperationFailed)
    }
}
fn record(
    attempted: &mut Vec<AttemptedStep>,
    operation: &'static str,
    path: &Path,
    outcome: FsStep,
) {
    attempted.push(AttemptedStep {
        step: StepKind::Filesystem {
            operation,
            path: path.into(),
        },
        outcome: step_outcome(outcome),
    });
}

#[cfg(test)]
mod tests {
    use super::super::super::{model::ManagementMode, windows_tests::Fixture};
    use super::*;
    use std::{
        cell::Cell,
        fs,
        time::{Duration, Instant},
    };
    struct Cancel(Cell<bool>);
    impl crate::termination_signal::TerminationSource for Cancel {
        fn requested(&self) -> bool {
            self.0.get()
        }
    }
    #[test]
    fn changed_preimages_arrivals_and_cancellation_preserve_user_data() {
        for scenario in [
            "snapshot",
            "pre-move",
            "moved",
            "published",
            "cancel",
            "metadata-cancel",
        ] {
            let fixture = Fixture::new();
            let target = fixture.0.join("hooks.json");
            let original = br#"{"user":"original","hooks":{}}"#;
            let foreign = br#"{"user":"newer","hooks":{}}"#;
            fs::write(&target, original).unwrap();
            let capsule = fixture.0.join("capsules/0.4.9");
            let cancel = Cancel(Cell::new(false));
            let context = ProviderContext {
                home: &fixture.0,
                capsule_root: &capsule,
                adapter_source: None,
                path_env: None,
                deadline: Instant::now() + Duration::from_secs(10),
                cancel: Some(&cancel),
                management_mode: ManagementMode::ManagedLocal,
            };
            let options = ResourceOptions {
                expected_version: "0.4.9",
                legacy_roots: &[],
                resource_root: &capsule,
            };
            let mut attempted = Vec::new();
            let mut residuals = Vec::new();
            let mut effect = false;
            let result = update_observed(
                "cursor",
                &target,
                &context,
                &options,
                true,
                EffectTrace {
                    attempted: &mut attempted,
                    residuals: &mut residuals,
                    begun: &mut effect,
                },
                |boundary, path| {
                    if boundary == "pre-move" {
                        // The retained original handle really denies in-place changes.
                        assert!(fs::write(path, foreign).is_err());
                    }
                    if scenario == "pre-move" && boundary == "pre-move" {
                        fs::rename(path, path.with_extension("user-saved")).unwrap();
                        fs::write(path, foreign).unwrap();
                    } else if boundary == scenario {
                        fs::write(path, foreign).unwrap();
                    } else if (scenario == "cancel" && boundary == "moved")
                        || (scenario == "metadata-cancel" && boundary == "metadata")
                    {
                        cancel.0.set(true);
                    }
                },
            );
            assert!(effect);
            if matches!(scenario, "cancel" | "metadata-cancel") {
                assert_eq!(result, Attempt::Cancelled);
                assert!(!target.exists());
                assert!(
                    residuals
                        .iter()
                        .any(|p| fs::read(p).is_ok_and(|bytes| bytes == original))
                );
            } else {
                assert_ne!(result, Attempt::Attempted);
                assert_eq!(fs::read(&target).unwrap(), foreign);
                if matches!(scenario, "moved" | "published") {
                    assert!(
                        residuals
                            .iter()
                            .any(|p| fs::read(p).is_ok_and(|bytes| bytes == original))
                    );
                } else {
                    assert!(residuals.is_empty());
                }
            }
        }
    }
}
