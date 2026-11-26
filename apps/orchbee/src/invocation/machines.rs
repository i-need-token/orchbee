//! Constructs typed machine commands from admitted options and validates machine selectors, names,
//! remote targets, Session names, and scope. It performs no catalog persistence, SSH connection,
//! hub request, or machine discovery.

use super::{
    ExpectedValue, FailureEnvelope, Options, OutputFormat, ParseFailure, Request, UsageContext,
    UsageReason, check_scope, missing_command, missing_required, unexpected_argument, usage_with,
};
use crate::machines::{Draft, MachineColor, default_label, valid_label, valid_target};
use orchbee_local_runtime::SessionName;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MachineCommand {
    List,
    /// Without a color, the machine takes the one the fewest saved machines use.
    Add(Draft, Option<MachineColor>),
    /// `machine` names one saved machine by ID or exact label; the catalog resolves it.
    Rename {
        machine: String,
        label: String,
    },
    SetColor {
        machine: String,
        color: MachineColor,
    },
    /// With `stop_session`, the remote Session stops first and the record goes only if it did.
    Remove {
        machine: String,
        stop_session: bool,
        /// Whether `--confirm` was given; a release build refuses it at admission.
        confirm: bool,
    },
    Enable {
        machine: String,
        enabled: bool,
    },
    /// Connect the machine the selector names through the machine hub; with `remember`, an answer
    /// typed at its login is remembered once the login succeeds.
    Connect {
        machine: String,
        remember: bool,
    },
    /// End the hub's connection to the machine the selector names.
    Disconnect(String),
    /// A verb or option whose slice has not landed. The whole invocation was parsed and is
    /// refused before any effect.
    NotYetAvailable(&'static str),
}
impl MachineCommand {
    pub(crate) fn operation(&self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Add(..) => "add",
            Self::Rename { .. } => "rename",
            Self::SetColor { .. } => "set-color",
            Self::Remove { .. } => "remove",
            Self::Enable { enabled: true, .. } => "enable",
            Self::Enable { enabled: false, .. } => "disable",
            Self::Connect { .. } => "connect",
            Self::Disconnect(_) => "disconnect",
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
        "set-color" => "machine set-color",
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
    let color = match (verb, options.color.as_deref()) {
        (_, Some(value)) => Some(
            MachineColor::parse(value)
                .ok_or_else(|| invalid("--color", value, ExpectedValue::MachineColor))?,
        ),
        ("set-color", None) => return Err(missing_required(context, "'--color'", envelope)),
        _ => None,
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
            MachineCommand::Add(draft, color)
        }
    } else {
        if !valid_selector(arg) {
            return Err(invalid("<id|label>", arg, ExpectedValue::MachineSelector));
        }
        let machine = (*arg).to_owned();
        match verb {
            "rename" => MachineCommand::Rename { machine, label },
            "set-color" => MachineCommand::SetColor {
                machine,
                // Present: `set-color` without it was refused above.
                color: color.unwrap_or(MachineColor::DEFAULT),
            },
            "remove" => {
                let stop_session = seen("--stop-session");
                // Stopping the remote Session ends its terminals: it is confirmed as `server stop`
                // is, and `--confirm` confirms nothing else.
                if options.confirm && !stop_session {
                    return Err(missing_required(context, "'--stop-session'", envelope));
                }
                if stop_session && !options.confirm && super::server_stop::confirm_option_required()
                {
                    return Err(missing_required(context, "'--confirm'", envelope));
                }
                MachineCommand::Remove {
                    stop_session,
                    confirm: options.confirm,
                    machine,
                }
            }
            "enable" | "disable" => MachineCommand::Enable {
                machine,
                enabled: verb == "enable",
            },
            "connect" => MachineCommand::Connect {
                machine,
                remember: seen("--remember"),
            },
            _ => MachineCommand::Disconnect(machine),
        }
    };
    Ok(Request::Machine { command, format })
}
#[cfg(test)]
mod tests;
