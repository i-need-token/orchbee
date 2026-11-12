//! Constructs typed machine commands from admitted options and validates machine selectors, names,
//! remote targets, Session names, and scope. It performs no catalog persistence, SSH connection, or
//! machine discovery.

use super::{
    ExpectedValue, FailureEnvelope, Options, OutputFormat, ParseFailure, Request, UsageContext,
    UsageReason, check_scope, missing_command, missing_required, unexpected_argument, usage_with,
};
use crate::machines::{Draft, default_label, valid_label, valid_target};
use orchbee_local_runtime::SessionName;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MachineCommand {
    List,
    Add(Draft),
    /// `machine` names one saved machine by ID or exact label; the catalog resolves it.
    Rename {
        machine: String,
        label: String,
    },
    Remove(String),
    Enable {
        machine: String,
        enabled: bool,
    },
    /// A verb or option whose slice has not landed. The whole invocation was parsed and is
    /// refused before any effect.
    NotYetAvailable(&'static str),
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
            Self::NotYetAvailable(operation) => operation,
        }
    }
}
/// Whether a word can name a saved machine: the label grammar, as the `--machine` prefix admits.
pub(crate) fn valid_selector(value: &str) -> bool {
    valid_label(value) && !value.starts_with('-')
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
        "connect" => "machine connect",
        "disconnect" => "machine disconnect",
        _ => {
            return Err(unexpected_argument(
                UsageContext::Path("machine"),
                verb,
                envelope,
            ));
        }
    };
    let context = UsageContext::Path(path);
    check_scope(options, context, envelope)?;
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
    let format = options.format.unwrap_or(OutputFormat::Text);
    if verb == "list" {
        if let Some(arg) = args.first() {
            return Err(unexpected_argument(context, arg, envelope));
        }
        return Ok(Request::Machine {
            command: MachineCommand::List,
            format,
        });
    }
    let operand = if verb == "add" {
        "<ssh-target>"
    } else {
        "<id|label>"
    };
    let [arg] = args else {
        return Err(if args.is_empty() {
            missing_required(context, operand, envelope)
        } else {
            unexpected_argument(context, args[1], envelope)
        });
    };
    let label = match (verb, options.label.as_deref()) {
        (_, Some(value)) if !valid_label(value) => {
            return Err(invalid("--label", value, ExpectedValue::MachineLabel));
        }
        (_, Some(value)) => value.to_owned(),
        ("rename", None) => return Err(missing_required(context, "'--label'", envelope)),
        _ => String::new(),
    };
    let seen = |flag| options.seen.contains(&flag);
    let command = if verb == "add" {
        if !valid_target(arg) {
            return Err(invalid("<ssh-target>", arg, ExpectedValue::SshTarget));
        }
        let session = options.remote_session.as_deref().unwrap_or("default");
        SessionName::parse(session)
            .map_err(|_| invalid("--remote-session", session, ExpectedValue::SessionName))?;
        let label = if label.is_empty() {
            default_label(arg)
        } else {
            label
        };
        let draft = Draft::parse(&label, arg, session)
            .ok_or_else(|| invalid("<ssh-target>", arg, ExpectedValue::SshTarget))?;
        if seen("--no-install") {
            MachineCommand::NotYetAvailable("add")
        } else {
            MachineCommand::Add(draft)
        }
    } else {
        if !valid_selector(arg) {
            return Err(invalid("<id|label>", arg, ExpectedValue::MachineSelector));
        }
        let machine = (*arg).to_owned();
        match verb {
            "rename" => MachineCommand::Rename { machine, label },
            "remove" if seen("--stop-session") => MachineCommand::NotYetAvailable("remove"),
            "remove" => MachineCommand::Remove(machine),
            "enable" | "disable" => MachineCommand::Enable {
                machine,
                enabled: verb == "enable",
            },
            _ => MachineCommand::NotYetAvailable(if verb == "connect" {
                "connect"
            } else {
                "disconnect"
            }),
        }
    };
    Ok(Request::Machine { command, format })
}
#[cfg(test)]
mod tests;
