//! Executable intent stays inert until the final CLI process has admitted the whole command.
use super::{
    ExpectedValue, FailureEnvelope, Options, ParseFailure, UsageContext, UsageReason,
    launch_option, usage_with,
};
use orchbee_workbench_protocol::{LaunchSelection, WorkbenchDirectoryPath};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ParsedLaunch {
    Absolute(LaunchSelection),
    Path {
        name: String,
        arguments: Vec<String>,
    },
}

pub(super) fn parse(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<ParsedLaunch>, ParseFailure> {
    let Some(exec) = options.exec.as_deref() else {
        return launch_option(options, context, envelope)
            .map(|value| value.map(ParsedLaunch::Absolute));
    };
    if WorkbenchDirectoryPath::parse(exec).is_ok() {
        return launch_option(options, context, envelope)
            .map(|value| value.map(ParsedLaunch::Absolute));
    }
    if !valid_name(exec) {
        return Err(invalid(
            exec,
            context,
            envelope,
            ExpectedValue::ExecutableName,
        ));
    }
    // Validate the argument contract independently of a filesystem lookup. Recheck the final
    // executable+argv size after resolution, before configuration, discovery or mutation.
    LaunchSelection::new("/".into(), options.args.clone())
        .map_err(|_| invalid("--arg", context, envelope, ExpectedValue::LaunchArguments))?;
    Ok(Some(ParsedLaunch::Path {
        name: exec.to_owned(),
        arguments: options.args.clone(),
    }))
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !matches!(name, "." | "..")
        && !name
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '/' | '\\' | ':'))
}

fn invalid(
    value: &str,
    context: UsageContext,
    envelope: FailureEnvelope,
    expected: ExpectedValue,
) -> ParseFailure {
    usage_with(
        UsageReason::InvalidValue {
            context,
            option: "--exec",
            value: value.to_owned(),
            expected,
        },
        envelope,
    )
}

mod execution;
#[cfg(windows)]
#[path = "program/windows.rs"]
mod native;
#[cfg(unix)]
#[path = "program/unix.rs"]
mod native;
#[cfg(not(any(unix, windows)))]
#[path = "program/unsupported.rs"]
mod native;
pub(crate) use native::resolve;
