//! Named-option admission: which declared options a command path admits, the shared rejection of
//! options a command does not accept, and required option slots.

use super::{
    FailureEnvelope, Options, ParseFailure, UsageContext, UsageReason, grammar, missing_required,
    usage_with,
};

/// The named-option set one command admits; anything seen outside it is usage.
/// `geometry` covers all four dimension flags together: a terminal geometry
/// is only meaningful whole.
pub(super) fn business_options(options: &Options) -> bool {
    options.seen.iter().any(|name| {
        grammar::option(name).is_some_and(|spec| spec.role == grammar::OptionRole::Business)
    })
}

fn first_out_of_scope(options: &Options, context: UsageContext) -> Option<&'static str> {
    let admitted = match context {
        UsageContext::Path(path) => grammar::command(path).map(|command| command.flags()),
        _ => None,
    }
    .unwrap_or(&[]);
    grammar::OPTIONS
        .iter()
        .find(|spec| {
            spec.role == grammar::OptionRole::Business
                && options.seen.contains(&spec.spelling)
                && !admitted.contains(&spec.spelling)
        })
        .map(|spec| spec.spelling)
}

pub(super) fn check_scope(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<(), ParseFailure> {
    if let Some(option) = first_out_of_scope(options, context) {
        return Err(usage_with(
            UsageReason::UnknownOption {
                context,
                option: option.to_owned(),
            },
            envelope,
        ));
    }
    Ok(())
}

pub(super) fn required(
    slot: &Option<String>,
    context: UsageContext,
    requirement: &'static str,
    envelope: FailureEnvelope,
) -> Result<String, ParseFailure> {
    slot.clone()
        .ok_or_else(|| missing_required(context, requirement, envelope))
}

/// Reject any option/filter/format on commands that accept none, naming the
/// first rejected flag in the fixed declaration order (`--format` last).
pub(super) fn reject_extras(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<(), ParseFailure> {
    reject_extras_scope(options, context, envelope)
}

/// `reject_extras` against an explicit admitted set: `--format` stays
/// rejected while the scope's own flags (today only `tui`'s `--mouse`)
/// pass through.
pub(super) fn reject_extras_scope(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<(), ParseFailure> {
    match first_out_of_scope(options, context)
        .or_else(|| options.format.is_some().then_some("--format"))
    {
        Some(option) => Err(usage_with(
            UsageReason::UnexpectedOption { context, option },
            envelope,
        )),
        None => Ok(()),
    }
}

/// Reject the named filters; `--format` remains legal on data commands.
pub(super) fn reject_filters(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<(), ParseFailure> {
    check_scope(options, context, envelope)
}
