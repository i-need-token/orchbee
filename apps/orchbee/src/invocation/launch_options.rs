//! Launch options: program and profile selection plus the per-launch overrides shared by every
//! terminal-creating command.

use super::{
    ExpectedValue, FailureEnvelope, Options, ParseFailure, UsageContext, UsageReason,
    missing_required, required, usage_with,
};
use orchbee_workbench_protocol::{
    LaunchEnvironment, LaunchOptions, LaunchProfileName, LaunchProfileSelector, LaunchSelection,
    WorkbenchDirectoryPath,
};

/// Per-launch overrides are independent of the program/profile selection.
pub(super) fn launch_options_with_directory(
    options: &Options,
    directory: Option<&str>,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<LaunchOptions, ParseFailure> {
    let cwd = directory
        .map(WorkbenchDirectoryPath::parse)
        .transpose()
        .map_err(|_| {
            usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--cwd",
                    value: options.cwd.clone().unwrap_or_default(),
                    expected: ExpectedValue::DirectoryPath,
                },
                envelope,
            )
        })?;
    // Environment values can contain secrets. Neither the parse failure nor its
    // localized diagnostic retains the supplied value, even for malformed input.
    let invalid_environment = || {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--env",
                value: String::new(),
                expected: ExpectedValue::LaunchEnvironment,
            },
            envelope,
        )
    };
    let entries = options
        .environment
        .iter()
        .map(|entry| {
            entry
                .split_once('=')
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                .ok_or_else(invalid_environment)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let environment = LaunchEnvironment::new(entries).map_err(|_| invalid_environment())?;
    Ok(LaunchOptions {
        plugin_panel: None,
        plugin_context: None,
        agent_start: None,
        cwd,
        environment,
    })
}

/// `--exec`/`--arg` build the optional explicit launch selection: `--exec`
/// once with a non-empty absolute NUL-free executable, `--arg` repeat-ordered
/// verbatim argv values, at most the protocol argument count and aggregate
/// byte bound. `--arg` without `--exec` is a missing requirement — an
/// argument list has no launch to modify.
pub(super) fn launch_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<LaunchSelection>, ParseFailure> {
    let Some(executable) = options.exec.clone() else {
        if options.args.is_empty() {
            return Ok(None);
        }
        return Err(missing_required(context, "'--exec'", envelope));
    };
    if WorkbenchDirectoryPath::parse(&executable).is_err() {
        return Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--exec",
                value: executable,
                expected: ExpectedValue::Executable,
            },
            envelope,
        ));
    }
    LaunchSelection::new(executable, options.args.clone())
        .map(Some)
        .map_err(|_| {
            usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--arg",
                    value: options.args.join(" "),
                    expected: ExpectedValue::LaunchArguments,
                },
                envelope,
            )
        })
}

/// A `launch-profile` positional selector: id-first exact then name
/// cardinality, validated only against the shared selector bound — the
/// Server owns resolution, the CLI never pre-reads the catalog.
pub(super) fn profile_selector(
    value: &str,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<LaunchProfileSelector, ParseFailure> {
    LaunchProfileSelector::parse(value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "<profile>",
                value: value.to_owned(),
                expected: ExpectedValue::ProfileSelector,
            },
            envelope,
        )
    })
}

/// The `--profile <selector>` launch option on `tab create`/`panel split`:
/// the same selector grammar as the positional form, named by its flag.
pub(super) fn profile_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<LaunchProfileSelector>, ParseFailure> {
    let Some(value) = options.profile.clone() else {
        return Ok(None);
    };
    LaunchProfileSelector::parse(&value).map(Some).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--profile",
                value,
                expected: ExpectedValue::ProfileSelector,
            },
            envelope,
        )
    })
}

/// `--profile` and the explicit `--exec`/`--arg` launch are mutually
/// exclusive — the wire decodes the pair as one rejection, so the grammar
/// refuses it first with the concrete flag pair named.
pub(super) fn reject_launch_profile_conflict(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<(), ParseFailure> {
    if options.profile.is_none() {
        return Ok(());
    }
    let explicit = if options.exec.is_some() {
        Some("--exec")
    } else {
        (!options.args.is_empty()).then_some("--arg")
    };
    match explicit {
        Some(first) => Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first,
                second: "--profile",
            },
            envelope,
        )),
        None => Ok(()),
    }
}

/// `launch-profile create --name <n>`: the required, non-unique catalog
/// name under the protocol's 512-byte bound.
pub(super) fn profile_name_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<LaunchProfileName, ParseFailure> {
    let value = required(&options.name, context, "'--name'", envelope)?;
    LaunchProfileName::parse(&value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--name",
                value,
                expected: ExpectedValue::ProfileName,
            },
            envelope,
        )
    })
}

/// `launch-profile create` stores exactly one explicit launch selection:
/// `--exec` is required here even though it is optional on tab/panel
/// launches — a profile has no default-shell meaning.
pub(super) fn required_launch_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<LaunchSelection, ParseFailure> {
    launch_option(options, context, envelope)?
        .ok_or_else(|| missing_required(context, "'--exec'", envelope))
}
