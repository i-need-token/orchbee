use super::{
    ExpectedValue, FailureEnvelope, MutationCommand, NO_OPTIONS, Options, ParseFailure, Request,
    Resource, Scope, UsageContext, UsageReason, check_scope, missing_required, mutation, target,
    unexpected_argument, usage_with,
};
use orchbee_workbench_protocol::PanelRightClickTarget;

pub(super) fn policy(
    value: &str,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<PanelRightClickTarget, ParseFailure> {
    match value {
        "orchbee" => Ok(PanelRightClickTarget::Orchbee),
        "panel" => Ok(PanelRightClickTarget::Panel),
        _ => Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--right-click",
                value: value.to_owned(),
                expected: ExpectedValue::RightClick,
            },
            envelope,
        )),
    }
}
pub(super) fn parse(
    operands: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("panel input");
    check_scope(
        options,
        Scope {
            right_click: true,
            current: true,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    if let [_, extra, ..] = operands {
        return Err(unexpected_argument(context, extra, envelope));
    }
    let selectors: Vec<_> = [
        ("panel-id", !operands.is_empty()),
        ("--current", options.current),
    ]
    .into_iter()
    .filter(|(_, present)| *present)
    .map(|(name, _)| name)
    .collect();
    if selectors.len() > 1 {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: selectors[0],
                second: selectors[1],
            },
            envelope,
        ));
    }
    if selectors.is_empty() {
        return Err(missing_required(
            context,
            "'<panel>' or '--current'",
            envelope,
        ));
    }
    let value = options
        .right_click
        .as_deref()
        .ok_or_else(|| missing_required(context, "'--right-click'", envelope))?;
    let target = operands
        .first()
        .map(|value| (*value).to_owned())
        .map(|value| target(value, Resource::Panel, envelope))
        .transpose()?;
    Ok(mutation(
        MutationCommand::PanelInput {
            target,
            right_click: policy(value, context, envelope)?,
        },
        options,
    ))
}
