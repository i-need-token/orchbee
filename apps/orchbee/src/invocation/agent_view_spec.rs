//! Whole-spec validation precedes Site discovery or any view mutation.
use super::{
    ExpectedValue, FailureEnvelope, NO_OPTIONS, Options, OutputFormat, ParseFailure, Request,
    Scope, UsageContext, UsageReason, check_scope, missing_required, usage_with,
};
use orchbee_workbench_protocol::{AgentViewAction, parse_agent_view_spec};
pub(super) fn parse(
    verb: &str,
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path(match verb {
        "set" => "agent view set",
        "clear" => "agent view clear",
        _ => "agent view get",
    });
    check_scope(
        options,
        Scope {
            agent_view_spec: verb == "set",
            output_source: verb == "clear",
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    let invalid = |option, value: String, expected| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option,
                value,
                expected,
            },
            envelope,
        )
    };
    let action = match verb {
        "set" => {
            let spec = options
                .agent_view_spec
                .as_deref()
                .ok_or_else(|| missing_required(context, "--spec", envelope))?;
            let parsed = parse_agent_view_spec(spec.as_bytes())
                .map_err(|_| invalid("--spec", spec.into(), ExpectedValue::AgentViewSpec))?;
            AgentViewAction::Set {
                spec: Box::new(parsed),
            }
        }
        "clear" => {
            let action = AgentViewAction::Clear {
                source: options.output_source.as_ref().map(|s| s.trim().to_owned()),
            };
            action.validate().map_err(|_| {
                invalid(
                    "--source",
                    options.output_source.clone().unwrap_or_default(),
                    ExpectedValue::AgentViewSource,
                )
            })?;
            action
        }
        _ => AgentViewAction::Read {},
    };
    Ok(Request::AgentView {
        action,
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}
#[cfg(test)]
mod tests;
