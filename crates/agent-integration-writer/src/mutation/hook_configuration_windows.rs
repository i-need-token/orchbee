//! JSON/TOML preparation; filesystem publication belongs to the shared Windows owner.
use super::super::{
    hook_configuration_windows as hooks, inspect::ResourceOptions, model::State,
    providers::ProviderContext,
};
use super::{Attempt, AttemptedStep, EffectTrace, FailureCode};
use std::path::PathBuf;
pub(super) fn update(
    id: &str,
    context: &ProviderContext,
    options: &ResourceOptions,
    enable: bool,
    attempted: &mut Vec<AttemptedStep>,
    residuals: &mut Vec<PathBuf>,
    effect: &mut bool,
) -> Attempt {
    let Ok((target, expected)) = hooks::load(id, context.home) else {
        return Attempt::Failed(FailureCode::OperationFailed);
    };
    if !matches!(
        hooks::state(target.kind, expected.as_deref(), options),
        State::NotInstalled | State::Enabled | State::Disabled | State::UpdateRequired
    ) {
        return Attempt::Conflict;
    }
    let Some(replacement) = hooks::patch(target.kind, expected.as_deref(), options, enable) else {
        return Attempt::Failed(FailureCode::OperationFailed);
    };
    super::file_publication_windows::replace(
        &target.path,
        expected,
        replacement,
        context,
        EffectTrace {
            attempted,
            residuals,
            begun: effect,
        },
    )
}
