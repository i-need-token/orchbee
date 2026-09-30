//! Whole-spec validation precedes Site discovery or any view mutation.
use super::*;
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
mod tests {
    use super::*;
    fn parse(args: &[&str]) -> Result<Request, ParseFailure> {
        super::super::parse_request(args.iter().map(std::ffi::OsString::from))
    }
    #[test]
    fn agent_view_commands_parse_full_json_and_source_guard_before_runtime() {
        assert!(matches!(
            parse(&["agent", "view", "get", "--format=json"]).unwrap(),
            Request::AgentView {
                action: AgentViewAction::Read {},
                format: OutputFormat::Json
            }
        ));
        let Request::AgentView{action:AgentViewAction::Set{spec},..}=parse(&["agent","view","set","--spec",r#"{"source":"  tools.views ","filter":{"op":"eq","field":"seen","value":false},"sort":[{"field":{"token":"phase"},"order":"desc"}]}"#]).unwrap() else {panic!()};
        assert_eq!(spec.source, "tools.views");
        assert_eq!(spec.sort.len(), 1);
        assert!(
            matches!(parse(&["agent","view","clear","--source= tools.views "]).unwrap(),Request::AgentView{action:AgentViewAction::Clear{source:Some(s)},..} if s=="tools.views")
        );
        for args in [
            vec!["agent", "view", "set"],
            vec!["agent", "view", "set", "--spec", "{bad}"],
            vec!["agent", "view", "clear", "--source", ""],
            vec!["agent", "view", "get", "--spec", "{}"],
            vec![
                "agent",
                "view",
                "set",
                "--spec",
                "{\"source\":\"x\"}",
                "--source",
                "x",
            ],
            vec!["agent", "view", "get", "extra"],
            vec!["agent", "list", "--spec", "{}"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }
    #[test]
    fn agent_view_help_and_saved_machine_route_are_admitted() {
        for verb in ["get", "set", "clear"] {
            assert!(matches!(
                parse(&["help", "agent", "view", verb]).unwrap(),
                Request::Help(_)
            ));
        }
        assert!(matches!(
            parse(&[
                "--machine",
                "remote",
                "agent",
                "view",
                "set",
                "--spec",
                "{\"source\":\"x\"}"
            ])
            .unwrap(),
            Request::MachineRoute(_)
        ));
    }
}
