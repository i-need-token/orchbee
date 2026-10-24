//! Publish only proven Hermes entry members, then let Hermes own activation.
use super::super::{
    hermes_windows as hermes, inspect::ResourceOptions, model::State, providers::ProviderContext,
};
use super::{Attempt, EffectTrace, FailureCode, exec, file_publication_windows, run_step_with};
pub(super) fn update(
    context: &ProviderContext,
    options: &ResourceOptions,
    enable: bool,
    trace: EffectTrace,
) -> Attempt {
    let EffectTrace {
        attempted,
        residuals,
        begun: effect,
    } = trace;
    let Some(local) = hermes::local(context.home, options) else {
        return Attempt::Failed(FailureCode::OperationFailed);
    };
    if !matches!(
        local.state,
        State::NotInstalled | State::Enabled | State::UpdateRequired
    ) {
        return Attempt::Conflict;
    }
    if enable {
        for (member, expected) in [
            (hermes::Member::Entry, local.entry),
            (hermes::Member::Manifest, local.manifest),
        ] {
            let Some(replacement) = hermes::member_bytes(member, context.capsule_root) else {
                return Attempt::Failed(FailureCode::OperationFailed);
            };
            if expected.as_ref() == Some(&replacement) {
                continue;
            }
            let result = file_publication_windows::replace(
                &local.directory.join(member.name()),
                expected,
                replacement,
                context,
                EffectTrace {
                    attempted,
                    residuals,
                    begun: effect,
                },
            );
            if result != Attempt::Attempted {
                return result;
            }
        }
    } else if local.manifest.is_none() {
        // An undiscovered partial installation cannot be enabled by Hermes.
        return if local.entry.is_none() {
            Attempt::Attempted
        } else {
            Attempt::Failed(FailureCode::OperationFailed)
        };
    }
    let Some(checked) = hermes::local(context.home, options) else {
        return Attempt::Failed(FailureCode::VerificationFailed);
    };
    if (enable && checked.state != State::Enabled)
        || matches!(
            checked.state,
            State::Conflict | State::StaleSide | State::Unknown
        )
    {
        return Attempt::Conflict;
    }
    let Some(executable) = exec::resolve_executable("hermes", context.path_env) else {
        return Attempt::Failed(FailureCode::OperationFailed);
    };
    let result = run_step_with(
        &executable,
        &[
            "plugins",
            if enable { "enable" } else { "disable" },
            hermes::NAME,
        ],
        context,
        1,
        attempted,
        effect,
        |argv| {
            exec::run_hermes_command(
                &executable,
                argv,
                context.home,
                context.deadline,
                context.cancel,
            )
        },
    );
    if result != Attempt::Attempted {
        return result;
    }
    let Some(local) = hermes::local(context.home, options) else {
        return Attempt::Failed(FailureCode::VerificationFailed);
    };
    match hermes::read(&executable, context) {
        super::super::providers::ReadOutcome::Stdout(bytes)
            if if enable {
                hermes::list_state(
                    &bytes,
                    local.state,
                    context
                        .capsule_root
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default(),
                ) == State::Enabled
            } else {
                hermes::activation_disabled(&bytes)
            } =>
        {
            Attempt::Attempted
        }
        super::super::providers::ReadOutcome::TimedOut => Attempt::Failed(FailureCode::TimedOut),
        _ => Attempt::Failed(FailureCode::VerificationFailed),
    }
}
