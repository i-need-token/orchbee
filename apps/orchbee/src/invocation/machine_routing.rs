//! Saved-machine prefix and the shared remote command admission boundary.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Route {
    pub(crate) selector: String,
    pub(crate) arguments: Vec<String>,
    pub(crate) format: OutputFormat,
    pub(crate) input: Option<TerminalSendInput>,
}

pub(super) fn parse(tokens: Vec<String>) -> Result<SelectedRequest, ParseFailure> {
    if let [mode, site, cohort] = tokens.as_slice()
        && mode == "--machine-tunnel-v1"
        && cohort.len() == 64
        && cohort
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && let Ok(site) = orchbee_local_site_runtime::WindowsSiteName::parse(site)
    {
        return Ok(SelectedRequest {
            request: Request::MachineTunnel {
                cohort: cohort.clone(),
            },
            site: Some(site),
        });
    }
    if tokens == ["--machine-relay-v1"] {
        return Ok(SelectedRequest {
            request: Request::MachineRelay,
            site: None,
        });
    }
    let Some(first) = tokens.first() else {
        return scan(tokens);
    };
    let (selector, skip) = if first == "--machine" {
        (tokens.get(1).cloned().unwrap_or_default(), 2)
    } else if let Some(value) = first.strip_prefix("--machine=") {
        (value.to_owned(), 1)
    } else {
        return scan(tokens);
    };
    if !crate::machines::valid_label(&selector) || selector.starts_with('-') {
        return Err(usage_with(
            UsageReason::MissingValue {
                option: "--machine",
            },
            FailureEnvelope::Text,
        ));
    }
    let arguments = tokens.into_iter().skip(skip).collect::<Vec<_>>();
    if arguments.len() > 1024 {
        return Err(usage_with(
            UsageReason::ArgumentEnvelope,
            FailureEnvelope::Silent,
        ));
    }
    let selected = scan(arguments.clone())?;
    let envelope = selection_envelope(format_envelope(&arguments)?);
    if selected.site.is_some() {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context: UsageContext::Root,
                first: "--machine",
                second: "--site",
            },
            envelope,
        ));
    }
    if matches!(selected.request, Request::Help(_) | Request::RootHelp) {
        return Ok(selected);
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
        site: None,
        request: Request::MachineRoute(Route {
            selector,
            arguments,
            format,
            input,
        }),
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
        | Request::TerminalSelection { format, .. }
        | Request::TerminalCopy { format, .. }
        | Request::TerminalLink { format, .. }
        | Request::TerminalClear { format, .. }
        | Request::TerminalEditor { format, .. }
        | Request::ClientQuery { format, .. }
        | Request::Client { format, .. }
        | Request::PanelAgent { format, .. }
        | Request::PanelMetadata { format, .. }
        | Request::WorkspaceMetadata { format, .. }
        | Request::PanelRelocation { format, .. }
        | Request::WorkspaceCreation { format, .. }
        | Request::PanelTerminalCreation { format, .. }
        | Request::Creation { format, .. }
        | Request::WorkspaceGroupClose { format, .. }
        | Request::WorktreeQuery { format, .. }
        | Request::Worktree { format, .. }
        | Request::SiteStop { format, .. }
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
mod tests {
    use super::*;
    #[test]
    fn prefix_preserves_sources_and_rejects_local_and_conflicting_modes() {
        for args in [
            vec![
                "--machine",
                "work",
                "terminal",
                "send",
                "@terminal:22",
                "--file=x",
                "--format=json",
            ],
            vec!["--machine=中文", "workspace", "list"],
            vec!["--machine=work", "client", "list"],
            vec!["--machine=work", "panel", "focus", "@panel:22"],
        ] {
            assert!(matches!(
                parse_request(args.into_iter().map(Into::into)).unwrap(),
                Request::MachineRoute(_)
            ));
        }
        for args in [
            vec!["--machine", "work"],
            vec!["--machine=work", "--site=x", "workspace", "list"],
            vec!["--machine=work", "machine", "list"],
            vec!["--machine=work", "status", "client"],
            vec!["--machine=work", "tui"],
            vec!["--machine=work", "update"],
            vec!["--machine=work", "--machine=x", "status"],
            vec!["status", "--machine=work"],
            vec!["--machine=work", "agent", "doctor"],
            vec!["--machine=work", "terminal", "attach", "@terminal:22"],
        ] {
            assert!(
                parse_request(args.iter().map(Into::into)).is_err(),
                "{args:?}"
            );
        }
        assert!(matches!(
            parse_request(
                ["--machine=no-such-host", "workspace", "--help"]
                    .into_iter()
                    .map(Into::into)
            )
            .unwrap(),
            Request::Help(_)
        ));
    }
}
