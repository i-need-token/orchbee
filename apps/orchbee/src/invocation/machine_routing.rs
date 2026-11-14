//! Saved-machine prefix and the shared remote command admission boundary.
use super::{
    FailureEnvelope, MutationCommand, OutputFormat, ParseFailure, QueryCommand, Request,
    RuntimeStatusScope, SelectedRequest, ServerStopCommand, TerminalSendInput, UsageContext,
    UsageReason, format_envelope, plugins, scan, selection_envelope, usage_with,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Route {
    pub(crate) selector: String,
    pub(crate) arguments: Vec<String>,
    pub(crate) format: OutputFormat,
    pub(crate) input: Option<TerminalSendInput>,
}

/// `--machine <selector> server stop`: never routed as a command line. The near side confirms it
/// as a local `server stop` is confirmed, then asks the machine hub to stop the remote Session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RemoteStop {
    pub(crate) selector: String,
    /// Whether `--confirm` was given; a release Windows build refuses it at admission.
    pub(crate) confirm: bool,
    pub(crate) format: OutputFormat,
}

/// How a refused option of the remote stop names its command.
const REMOTE_STOP: UsageContext = UsageContext::Literal("orchbee --machine <selector> server stop");

pub(super) fn parse(tokens: Vec<String>) -> Result<SelectedRequest, ParseFailure> {
    // This helper grammar is permanent; a change adds a new helper name instead.
    if tokens == ["--machine-link-v1"] {
        return Ok(SelectedRequest {
            availability: super::grammar::Availability::Windows,
            format: None,
            request: Request::MachineLink,
            session: None,
        });
    }
    // Internal pairings between this program and its own start path, not frozen helpers: a release
    // may rename them.
    for (token, request) in [
        ("--machine-hub-v1", Request::MachineHub),
        ("--machine-hub-state-v1", Request::MachineHubState),
        ("--machine-hub-hold-v1", Request::MachineHubHold),
    ] {
        if tokens == [token] {
            return Ok(SelectedRequest {
                availability: super::grammar::Availability::Windows,
                format: None,
                request,
                session: None,
            });
        }
    }
    let mut arguments = tokens;
    let Some(selector) = take_machine(&mut arguments)? else {
        return scan(arguments);
    };
    if !super::machines::valid_selector(&selector) {
        return Err(usage_with(
            UsageReason::MissingValue {
                option: "--machine",
            },
            FailureEnvelope::Text,
        ));
    }
    if arguments.len() > 1024 {
        return Err(usage_with(
            UsageReason::ArgumentEnvelope,
            FailureEnvelope::Silent,
        ));
    }
    let selected = scan(arguments.clone())?;
    let envelope = selection_envelope(format_envelope(&arguments)?);
    if selected.session.is_some() {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context: UsageContext::Root,
                first: "--machine",
                second: "--session",
            },
            envelope,
        ));
    }
    if matches!(
        selected.request,
        Request::Help(_) | Request::RootHelp | Request::Withheld(_)
    ) {
        return Ok(selected);
    }
    if let Request::ServerStop {
        command:
            ServerStopCommand::Stop {
                id,
                confirm,
                retry,
                detach,
                ..
            },
        format,
    } = &selected.request
    {
        // A local stop's request identity, retry, detachment and wait have no counterpart in the
        // hub's `stop-session`, so they are refused rather than ignored.
        let unexpected = if *retry {
            Some("--retry")
        } else if id.is_some() {
            Some("--request-id")
        } else if *detach {
            Some("--detach")
        } else if given(&arguments, "--timeout") {
            Some("--timeout")
        } else {
            None
        };
        if let Some(option) = unexpected {
            return Err(usage_with(
                UsageReason::UnexpectedOption {
                    context: REMOTE_STOP,
                    option,
                },
                envelope,
            ));
        }
        return Ok(SelectedRequest {
            availability: super::grammar::Availability::Windows,
            format: Some(*format),
            session: None,
            request: Request::MachineStop(RemoteStop {
                selector,
                confirm: *confirm,
                format: *format,
            }),
        });
    }
    let (format, input) = admit(&selected.request).ok_or_else(|| {
        usage_with(
            UsageReason::UnexpectedOption {
                context: UsageContext::Root,
                option: "--machine",
            },
            envelope,
        )
    })?;
    Ok(SelectedRequest {
        availability: super::grammar::Availability::Windows,
        format: Some(format),
        session: None,
        request: Request::MachineRoute(Route {
            selector,
            arguments,
            format,
            input,
        }),
    })
}

/// Takes `--machine <selector>` or `--machine=<selector>` from anywhere before `--`, as `--session`
/// is accepted anywhere, with the option scan's value ownership so an option's value is never read
/// as the prefix. Removes it and returns the selector, or leaves every token in place when no
/// `--machine` is given; a second one is a usage failure.
fn take_machine(tokens: &mut Vec<String>) -> Result<Option<String>, ParseFailure> {
    let mut found: Option<(usize, usize)> = None;
    let mut index = 0;
    while let Some(token) = tokens.get(index) {
        let token = token.as_str();
        if token == "--" {
            break;
        }
        let width = if token == "--machine" {
            2
        } else if token.starts_with("--machine=") {
            1
        } else {
            index += if scan::owns_value(token) { 2 } else { 1 };
            continue;
        };
        if found.is_some() {
            return Err(usage_with(
                UsageReason::DuplicateOption {
                    option: "--machine",
                },
                selection_envelope(format_envelope(tokens)?),
            ));
        }
        found = Some((index, width));
        index += width;
    }
    let Some((start, width)) = found else {
        return Ok(None);
    };
    let end = (start + width).min(tokens.len());
    let mut taken = tokens.drain(start..end);
    let flag = taken.next().unwrap_or_default();
    let selector = match flag.strip_prefix("--machine=") {
        Some(value) => value.to_owned(),
        None => taken.next().unwrap_or_default(),
    };
    Ok(Some(selector))
}

/// Whether the parsed arguments spelled `option`, alone or with `=`, before any `--` terminator.
/// The grammar already admitted them, so a value can spell an option name only where that value
/// was itself refused.
fn given(arguments: &[String], option: &str) -> bool {
    arguments
        .iter()
        .take_while(|argument| *argument != "--")
        .any(|argument| {
            argument == option
                || argument
                    .strip_prefix(option)
                    .is_some_and(|rest| rest.starts_with('='))
        })
}

/// This is used on both sides. Local administrative commands and streams never
/// acquire remote execution authority merely because a prefix was accepted.
pub(crate) fn admit(request: &Request) -> Option<(OutputFormat, Option<TerminalSendInput>)> {
    let format = match request {
        Request::Query {
            command:
                QueryCommand::AgentDoctor { .. }
                | QueryCommand::ApiSchema { .. }
                | QueryCommand::ConfigCheck,
            ..
        } => return None,
        Request::Query { format, .. }
        | Request::TerminalClear { format, .. }
        | Request::LayoutHistory { format }
        | Request::LayoutReopen { format, .. }
        | Request::ClientQuery { format, .. }
        | Request::Client { format, .. }
        | Request::PanelAgent { format, .. }
        | Request::PanelMetadata { format, .. }
        | Request::WorkspaceMetadata { format, .. }
        | Request::PanelRelocation { format, .. }
        | Request::WorkspaceCreation { format, .. }
        | Request::PanelTerminalCreation { format, .. }
        | Request::Creation { format, .. }
        | Request::RetainedClose { format, .. }
        | Request::ServerOperationStatus { format, .. }
        | Request::WorkspaceGroupClose { format, .. }
        | Request::WorktreeQuery { format, .. }
        | Request::Worktree { format, .. }
        // A stop goes through the machine hub instead; see [`RemoteStop`].
        | Request::ServerStop {
            command: ServerStopCommand::Status { .. },
            format,
        }
        | Request::WindowTitle { format, .. }
        | Request::Notification { format, .. }
        | Request::RuntimeConfiguration { format, .. }
        | Request::AgentManifests { format, .. }
        | Request::AgentStart { format, .. }
        | Request::AgentView { format, .. }
        | Request::AgentNames { format, .. }
        | Request::RuntimeStatus {
            scope: RuntimeStatusScope::Server,
            format,
        } => *format,
        Request::Mutation { command, format } => {
            let input = match command {
                MutationCommand::TerminalSend { input, .. }
                    if matches!(
                        input,
                        TerminalSendInput::File { .. } | TerminalSendInput::Stdin { .. }
                    ) =>
                {
                    Some(input.clone())
                }
                _ => None,
            };
            return Some((*format, input));
        }
        Request::Plugin { command, format } => {
            if matches!(
                command,
                plugins::PluginCommand::Install { .. }
                    | plugins::PluginCommand::Uninstall(_)
                    | plugins::PluginCommand::ConfigDir(_)
            ) {
                return None;
            }
            *format
        }
        _ => return None,
    };
    Some((format, None))
}

#[cfg(test)]
mod tests;
