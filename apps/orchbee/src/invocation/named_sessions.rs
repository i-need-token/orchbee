//! Constructs typed named-Session commands with validated names, command-specific scope, and
//! explicit confirmation operands. It does not inspect a runtime namespace or activate, stop,
//! or delete a Session.

use super::{
    ExpectedValue, FailureEnvelope, Options, OutputFormat, ParseFailure, Request,
    ServerStopCommand, UsageContext, UsageReason, check_scope, missing_command, missing_required,
    reject_extras_scope, reject_filters, stop_command, unexpected_argument, usage_with,
};
use orchbee_local_runtime::SessionName;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NamedSessionCommand {
    List,
    Attach {
        name: SessionName,
        mouse: bool,
    },
    Stop {
        name: SessionName,
        command: ServerStopCommand,
    },
    Delete {
        name: SessionName,
    },
}
impl NamedSessionCommand {
    pub(super) fn name(&self) -> Option<&SessionName> {
        match self {
            Self::List => None,
            Self::Attach { name, .. } | Self::Stop { name, .. } | Self::Delete { name } => {
                Some(name)
            }
        }
    }
}

pub(super) fn parse(
    words: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let command = match words {
        ["session"] => return Err(missing_command(UsageContext::Path("session"), envelope)),
        ["session", "list"] => {
            reject_filters(options, UsageContext::Path("session list"), envelope)?;
            NamedSessionCommand::List
        }
        ["session", verb @ ("attach" | "stop" | "delete"), rest @ ..] => {
            let context = UsageContext::Path(match *verb {
                "attach" => "session attach",
                "stop" => "session stop",
                _ => "session delete",
            });
            let [value] = rest else {
                return Err(if rest.is_empty() {
                    missing_required(context, "<name>", envelope)
                } else {
                    unexpected_argument(context, rest[1], envelope)
                });
            };
            let name = SessionName::parse(value).map_err(|_| {
                usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "<name>",
                        value: (*value).to_owned(),
                        expected: ExpectedValue::SessionName,
                    },
                    envelope,
                )
            })?;
            match *verb {
                "attach" => {
                    reject_extras_scope(options, context, envelope)?;
                    NamedSessionCommand::Attach {
                        name,
                        mouse: options.mouse.unwrap_or(true),
                    }
                }
                "stop" => NamedSessionCommand::Stop {
                    name,
                    command: stop_command(options, context, envelope)?,
                },
                _ => {
                    check_scope(options, context, envelope)?;
                    if !options.confirm {
                        return Err(missing_required(context, "'--confirm'", envelope));
                    }
                    NamedSessionCommand::Delete { name }
                }
            }
        }
        ["session", word, ..] => {
            return Err(unexpected_argument(
                UsageContext::Path("session"),
                word,
                envelope,
            ));
        }
        _ => return Err(missing_command(UsageContext::Path("session"), envelope)),
    };
    Ok(Request::NamedSession {
        command,
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

#[cfg(test)]
mod tests;
