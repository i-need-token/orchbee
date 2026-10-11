use super::*;
use orchbee_local_site_runtime::WindowsSiteName;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NamedSessionCommand {
    List,
    Attach {
        name: WindowsSiteName,
        mouse: bool,
    },
    Stop {
        name: WindowsSiteName,
        command: SiteStopCommand,
    },
    Delete {
        name: WindowsSiteName,
    },
}
impl NamedSessionCommand {
    pub(super) fn name(&self) -> Option<&WindowsSiteName> {
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
            let name = WindowsSiteName::parse(value).map_err(|_| {
                usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "<name>",
                        value: (*value).to_owned(),
                        expected: ExpectedValue::SiteName,
                    },
                    envelope,
                )
            })?;
            match *verb {
                "attach" => {
                    reject_extras_scope(options, TUI_SCOPE, context, envelope)?;
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
                    check_scope(
                        options,
                        Scope {
                            confirm: true,
                            ..NO_OPTIONS
                        },
                        context,
                        envelope,
                    )?;
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
