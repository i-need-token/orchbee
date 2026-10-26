use super::{
    ExpectedValue, FailureEnvelope, NO_OPTIONS, Options, OutputFormat, ParseFailure, Request,
    Scope, UsageContext, UsageReason, check_scope, missing_command, missing_required,
    unexpected_argument, usage_with,
};
use crate::machines::{Draft, ProfileId, valid_label, valid_target};
use orchbee_local_runtime::SessionName;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MachineCommand {
    List,
    Add(Draft),
    Rename { id: ProfileId, label: String },
    Remove(ProfileId),
    Enable { id: ProfileId, enabled: bool },
}
impl MachineCommand {
    pub(crate) fn operation(&self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Add(_) => "add",
            Self::Rename { .. } => "rename",
            Self::Remove(_) => "remove",
            Self::Enable { enabled: true, .. } => "enable",
            Self::Enable { enabled: false, .. } => "disable",
        }
    }
}
pub(super) fn parse(
    words: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let (verb, args) = match words {
        ["machine"] => return Err(missing_command(UsageContext::Path("machine"), envelope)),
        ["machine", verb, args @ ..] => (*verb, args),
        _ => unreachable!("machine dispatch"),
    };
    let path = match verb {
        "list" => "machine list",
        "add" => "machine add",
        "rename" => "machine rename",
        "remove" => "machine remove",
        "enable" => "machine enable",
        "disable" => "machine disable",
        _ => {
            return Err(unexpected_argument(
                UsageContext::Path("machine"),
                verb,
                envelope,
            ));
        }
    };
    let context = UsageContext::Path(path);
    check_scope(
        options,
        Scope {
            label: matches!(verb, "add" | "rename"),
            remote_session: verb == "add",
            machine_json: verb == "list",
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    let invalid = |option, value: &str, expected| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option,
                value: value.to_owned(),
                expected,
            },
            envelope,
        )
    };
    let command = if verb == "list" {
        if let Some(arg) = args.first() {
            return Err(unexpected_argument(context, arg, envelope));
        }
        MachineCommand::List
    } else {
        let operand = if verb == "add" {
            "<ssh-target>"
        } else {
            "<profile-id>"
        };
        let [arg] = args else {
            return Err(if args.is_empty() {
                missing_required(context, operand, envelope)
            } else {
                unexpected_argument(context, args[1], envelope)
            });
        };
        let label = if matches!(verb, "add" | "rename") {
            let value = options
                .label
                .as_deref()
                .ok_or_else(|| missing_required(context, "'--label'", envelope))?;
            if !valid_label(value) {
                return Err(invalid("--label", value, ExpectedValue::MachineLabel));
            }
            value
        } else {
            ""
        };
        if verb == "add" {
            if !valid_target(arg) {
                return Err(invalid("<ssh-target>", arg, ExpectedValue::SshTarget));
            }
            let session = options.remote_session.as_deref().unwrap_or("default");
            SessionName::parse(session)
                .map_err(|_| invalid("--remote-session", session, ExpectedValue::SessionName))?;
            let draft = Draft::parse(label, arg, session)
                .ok_or_else(|| invalid("<ssh-target>", arg, ExpectedValue::SshTarget))?;
            MachineCommand::Add(draft)
        } else {
            let id = ProfileId::parse(arg)
                .ok_or_else(|| invalid("<profile-id>", arg, ExpectedValue::MachineId))?;
            match verb {
                "rename" => MachineCommand::Rename {
                    id,
                    label: label.to_owned(),
                },
                "remove" => MachineCommand::Remove(id),
                _ => MachineCommand::Enable {
                    id,
                    enabled: verb == "enable",
                },
            }
        }
    };
    Ok(Request::Machine {
        command,
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}
#[cfg(test)]
mod tests;
