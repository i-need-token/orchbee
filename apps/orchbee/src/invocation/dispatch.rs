//! Command-word dispatch: maps recognized command words to the feature parser that owns their
//! typed request and builds the remaining simple requests in place.

use super::{
    AgentIntegrationCommand, AgentNamesCommand, AgentPromptWait, AgentStartCommand, CommandPath,
    CompletionShell, ExpectedValue, FailureEnvelope, MAXIMUM_SEND_RECORDS, ManifestCommand,
    MutationCommand, Options, OutputFormat, OutputTarget, PanelQueryTarget, ParseFailure,
    QueryCommand, Request, Resource, RuntimeStatusScope, ServerStopCommand, Target,
    TerminalListFilter, UsageContext, UsageReason, adapter_source_option,
    agent_after_revision_option, agent_limit_option, agent_name, agent_selector,
    agent_until_option, agent_view_spec, agent_wait_timeout_option, business_options, check_scope,
    client_control, close_generation_option, close_request_option, close_wait, configured_commands,
    creation, doctor_asset_roots_option, doctor_providers_option, events, grammar, label_option,
    layouts, machines, manifest_operation_id, missing_command, missing_required, mutation,
    named_sessions, operation, pane_graphics, panel_agent, panel_input, panel_metadata,
    panel_move_destination, panel_relocation, panel_terminal_creation, parse_send_key, path_option,
    plugins, profile_name_option, profile_selector, provider_option, query, reject_extras,
    reject_extras_scope, reject_filters, required, required_launch_option, required_provider,
    resolve_help_path, schema_export, send_input, stop_command, stop_id, tab_move_command, target,
    terminal_copy, terminal_links, terminal_output, terminal_run, terminal_selection,
    terminal_stream, terminal_target, unexpected_argument, unknown_command, usage_with,
    wait_timeout_option, workspace_creation, workspace_group_close, workspace_id_option,
    workspace_metadata, worktree,
};

pub(super) fn resolve_words(
    words: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    match words {
        [
            family @ ("workspace" | "tab" | "panel" | "project"),
            verb @ ("status" | "resume"),
            operands @ ..,
        ]
        | [
            family @ ("client" | "server"),
            verb @ "status",
            operands @ ..,
        ] => operation::parse(family, verb, operands, options, envelope),
        ["command", ..] => configured_commands::parse(words, options, envelope),
        ["events", ..] => events::parse(words, options, envelope),
        ["panel", "graphics", ..] => pane_graphics::parse(words, options, envelope),
        ["plugin", ..] => plugins::parse(words, options, envelope),
        ["popup", ..] => plugins::panes::parse_close(words, options, envelope),
        ["layout", "export"] => layouts::export(options, envelope),
        ["layout", "apply"] => layouts::apply(options, envelope),
        ["session", ..] => named_sessions::parse(words, options, envelope),
        ["machine", ..] => machines::parse(words, options, envelope),
        ["client", ..]
        | ["layout", ..]
        | ["agent", "focus" | "attention", ..]
        | ["workspace" | "tab", "focus", ..]
        | [
            "panel",
            "focus" | "zoom" | "layout" | "neighbor" | "edges" | "resize" | "swap",
            ..,
        ] => client_control::parse(words, options, envelope),

        [] => {
            // Bare invocation keeps TUI; a selected format or any option
            // (including `--mouse`) is usage — the flag exists only on the
            // explicit `tui` path.
            if options.format.is_some()
                || options.project.is_some()
                || options.workspace.is_some()
                || options.tab.is_some()
                || options.mouse.is_some()
                || business_options(options)
            {
                return Err(missing_command(UsageContext::Root, envelope));
            }
            Ok(Request::Tui { mouse: true })
        }
        ["tui"] => reject_extras_scope(options, UsageContext::Path("tui"), envelope).map(|()| {
            Request::Tui {
                mouse: options.mouse.unwrap_or(true),
            }
        }),
        ["help", rest @ ..] => resolve_help_path(rest, options, envelope),
        ["status"] => {
            reject_filters(options, UsageContext::Path("status"), envelope)?;
            Ok(query(QueryCommand::Status, options))
        }
        ["status", scope @ ("server" | "client")] => {
            let (scope, path) = if *scope == "server" {
                (RuntimeStatusScope::Server, "status server")
            } else {
                (RuntimeStatusScope::Client, "status client")
            };
            reject_filters(options, UsageContext::Path(path), envelope)?;
            Ok(Request::RuntimeStatus {
                scope,
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["agent", "view", verb @ ("get" | "set" | "clear")] => {
            agent_view_spec::parse(verb, options, envelope)
        }
        ["agent", "changes"] => {
            let context = UsageContext::Path("agent changes");
            check_scope(options, context, envelope)?;
            let cursor = options
                .cursor
                .as_ref()
                .map(|value| {
                    orchbee_agent_observation_protocol::AgentChangesCursor::parse(value).map_err(
                        |_| {
                            usage_with(
                                UsageReason::InvalidValue {
                                    context,
                                    option: "--cursor",
                                    value: value.clone(),
                                    expected: ExpectedValue::AgentCursor,
                                },
                                envelope,
                            )
                        },
                    )
                })
                .transpose()?;
            Ok(query(
                QueryCommand::AgentChanges {
                    limit: agent_limit_option(options, context, envelope)?,
                    cursor,
                },
                options,
            ))
        }
        ["agent", "list"] => {
            check_scope(options, UsageContext::Path("agent list"), envelope)?;
            let context = UsageContext::Path("agent list");
            Ok(query(
                QueryCommand::AgentList {
                    client: client_control::client_id(options, context, envelope)?,
                    include_ended: options.include_ended,
                    limit: agent_limit_option(options, context, envelope)?,
                    cursor: options.cursor.clone(),
                },
                options,
            ))
        }
        ["agent", verb @ ("get" | "explain"), rest @ ..] => {
            let context = UsageContext::Path(if *verb == "get" {
                "agent get"
            } else {
                "agent explain"
            });
            check_scope(options, context, envelope)?;
            let target = agent_selector(rest, options, context, envelope)?;
            Ok(query(
                if *verb == "get" {
                    if let Some(client) = client_control::client_id(options, context, envelope)? {
                        QueryCommand::AgentClientGet { target, client }
                    } else {
                        QueryCommand::AgentGet(target)
                    }
                } else {
                    QueryCommand::AgentExplain(target)
                },
                options,
            ))
        }
        ["agent", "read", rest @ ..] => {
            let context = UsageContext::Path("agent read");
            check_scope(options, context, envelope)?;
            terminal_output::request(
                OutputTarget::Agent(agent_selector(rest, options, context, envelope)?),
                options,
                context,
                envelope,
            )
        }
        ["agent", "start", rest @ ..] => {
            let context = UsageContext::Path("agent start");
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
            let Some(name) = rest.first() else {
                return Err(missing_required(context, "<name>", envelope));
            };
            let name = agent_name(name, context, envelope)?;
            if options.providers.len() != 1 {
                return Err(missing_required(context, "one --provider", envelope));
            }
            let provider = options
                .provider
                .as_deref()
                .ok_or_else(|| missing_required(context, "'--provider'", envelope))?;
            if crate::agent_provider::executable(provider).is_none() {
                return Err(invalid(
                    "--provider",
                    provider,
                    ExpectedValue::AgentStartProvider,
                ));
            }
            let timeout_ms = match options.timeout.as_deref() {
                None => 30000,
                Some(value) => value
                    .parse::<u32>()
                    .ok()
                    .filter(|n| (3001..=300000).contains(n))
                    .ok_or_else(|| invalid("--timeout", value, ExpectedValue::AgentStartTimeout))?,
            };
            if options.current && options.panel.is_some() {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context,
                        first: "--current",
                        second: "--panel",
                    },
                    envelope,
                ));
            }
            if rest.len() > 1 && !options.args.is_empty() {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context,
                        first: "--arg",
                        second: "<args>",
                    },
                    envelope,
                ));
            }
            let args = if options.args.is_empty() {
                rest[1..].iter().map(|v| (*v).to_owned()).collect()
            } else {
                options.args.clone()
            };
            for arg in &args {
                if arg.chars().any(char::is_control) {
                    return Err(invalid("<args>", arg, ExpectedValue::AgentStartArgument));
                }
            }
            let panel = options
                .panel
                .as_deref()
                .map(|value| target(value.to_owned(), Resource::Panel, envelope))
                .transpose()?;
            Ok(Request::AgentStart {
                command: AgentStartCommand {
                    name,
                    provider: provider.to_owned(),
                    panel,
                    args,
                    timeout_ms,
                },
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["agent", "prompt", rest @ ..] => {
            let context = UsageContext::Path("agent prompt");
            check_scope(options, context, envelope)?;
            let selector_len = usize::from(
                options.name.is_none()
                    && options.agent_session.is_none()
                    && options.panel.is_none()
                    && !rest.is_empty(),
            );
            let target = agent_selector(&rest[..selector_len], options, context, envelope)?;
            let values = &rest[selector_len..];
            let Some(text) = values.first() else {
                return Err(missing_required(context, "<text>", envelope));
            };
            if values.len() > 1 {
                return Err(usage_with(
                    UsageReason::UnexpectedArgument {
                        context,
                        value: values[1].to_owned(),
                    },
                    envelope,
                ));
            }
            if text.is_empty() || text.len() > 256 * 1024 {
                return Err(usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "<text>",
                        value: String::new(),
                        expected: ExpectedValue::PromptText,
                    },
                    envelope,
                ));
            }
            if !options.prompt_wait
                && (!options.until.is_empty()
                    || options.timeout.is_some()
                    || options.forever
                    || options.client.is_some())
            {
                return Err(missing_required(context, "'--wait'", envelope));
            }
            let wait = if options.prompt_wait {
                Some(AgentPromptWait {
                    client: client_control::client_id(options, context, envelope)?,
                    until: agent_until_option(options, context, envelope)?,
                    timeout_ms: agent_wait_timeout_option(options, context, envelope)?,
                })
            } else {
                None
            };
            Ok(mutation(
                MutationCommand::AgentPrompt {
                    target,
                    text: (*text).to_owned(),
                    wait,
                },
                options,
            ))
        }
        ["agent", "send-keys", rest @ ..] => {
            let context = UsageContext::Path("agent send-keys");
            check_scope(options, context, envelope)?;
            let selector_len = usize::from(
                options.name.is_none()
                    && options.agent_session.is_none()
                    && options.panel.is_none()
                    && !rest.is_empty(),
            );
            let target = agent_selector(&rest[..selector_len], options, context, envelope)?;
            let values = &rest[selector_len..];
            if values.is_empty() {
                return Err(missing_required(context, "<key>...", envelope));
            }
            if values.len() > MAXIMUM_SEND_RECORDS {
                return Err(usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "<key>",
                        value: values[MAXIMUM_SEND_RECORDS].to_owned(),
                        expected: ExpectedValue::SendKeySequence,
                    },
                    envelope,
                ));
            }
            let keys = values
                .iter()
                .map(|value| {
                    parse_send_key(value).ok_or_else(|| {
                        usage_with(
                            UsageReason::InvalidValue {
                                context,
                                option: "<key>",
                                value: (*value).to_owned(),
                                expected: ExpectedValue::SendKey,
                            },
                            envelope,
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(mutation(
                MutationCommand::AgentSendKeys { target, keys },
                options,
            ))
        }
        ["agent", "wait", rest @ ..] => {
            let context = UsageContext::Path("agent wait");
            check_scope(options, context, envelope)?;
            Ok(query(
                QueryCommand::AgentWait {
                    session: agent_selector(rest, options, context, envelope)?,
                    client: client_control::client_id(options, context, envelope)?,
                    until: agent_until_option(options, context, envelope)?,
                    after_revision: agent_after_revision_option(options, context, envelope)?,
                    timeout_ms: agent_wait_timeout_option(options, context, envelope)?,
                },
                options,
            ))
        }
        ["agent", "names"] => {
            reject_filters(options, UsageContext::Path("agent names"), envelope)?;
            Ok(Request::AgentNames {
                command: AgentNamesCommand::List,
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["agent", "resolve", rest @ ..] => {
            let context = UsageContext::Path("agent resolve");
            check_scope(options, context, envelope)?;
            Ok(Request::AgentNames {
                command: AgentNamesCommand::Resolve(agent_selector(
                    rest, options, context, envelope,
                )?),
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["agent", "rename", rest @ ..] => {
            let context = UsageContext::Path("agent rename");
            check_scope(options, context, envelope)?;
            let command = if options.clear {
                AgentNamesCommand::Release(agent_selector(rest, options, context, envelope)?)
            } else {
                let Some((value, selectors)) = rest.split_last() else {
                    return Err(usage_with(
                        UsageReason::MissingRequired {
                            context,
                            requirement: "<target> <new-name> or --clear",
                        },
                        envelope,
                    ));
                };
                AgentNamesCommand::Assign {
                    target: agent_selector(selectors, options, context, envelope)?,
                    name: agent_name(value, context, envelope)?,
                }
            };
            Ok(Request::AgentNames {
                command,
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["agent", "doctor"] => {
            let context = UsageContext::Path("agent doctor");
            check_scope(options, context, envelope)?;
            Ok(query(
                QueryCommand::AgentDoctor {
                    providers: doctor_providers_option(options, context, envelope)?,
                    asset_roots: doctor_asset_roots_option(options, context, envelope)?,
                },
                options,
            ))
        }
        ["agent", "integration", "list"] => {
            let context = UsageContext::Path("agent integration list");
            check_scope(options, context, envelope)?;
            Ok(Request::AgentIntegration {
                command: AgentIntegrationCommand::List {
                    outdated_only: options.outdated_only,
                    provider: provider_option(options, context, envelope)?,
                    adapter_source: adapter_source_option(options, context, envelope)?,
                },
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["agent", "integration", "explain"] => {
            let context = UsageContext::Path("agent integration explain");
            check_scope(options, context, envelope)?;
            Ok(Request::AgentIntegration {
                command: AgentIntegrationCommand::Explain {
                    provider: required_provider(options, context, envelope)?,
                    adapter_source: adapter_source_option(options, context, envelope)?,
                },
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["agent", "integration", "enable"] => {
            let context = UsageContext::Path("agent integration enable");
            check_scope(options, context, envelope)?;
            Ok(Request::AgentIntegration {
                command: AgentIntegrationCommand::Enable {
                    provider: required_provider(options, context, envelope)?,
                    adapter_source: adapter_source_option(options, context, envelope)?,
                },
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["agent", "integration", "disable"] => {
            let context = UsageContext::Path("agent integration disable");
            check_scope(options, context, envelope)?;
            Ok(Request::AgentIntegration {
                command: AgentIntegrationCommand::Disable {
                    provider: required_provider(options, context, envelope)?,
                    adapter_source: adapter_source_option(options, context, envelope)?,
                },
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["project", "list"] => {
            reject_filters(options, UsageContext::Path("project list"), envelope)?;
            Ok(query(QueryCommand::ProjectList, options))
        }
        ["project", "get", value] => {
            reject_filters(options, UsageContext::Path("project get"), envelope)?;
            Ok(query(
                QueryCommand::ProjectGet(target((*value).to_owned(), Resource::Project, envelope)?),
                options,
            ))
        }
        ["project", "dissolve", value] => {
            check_scope(options, UsageContext::Path("project dissolve"), envelope)?;
            Ok(mutation(
                MutationCommand::ProjectDissolve {
                    target: target((*value).to_owned(), Resource::Project, envelope)?,
                },
                options,
            ))
        }
        ["workspace", "set-project", value] => {
            let context = UsageContext::Path("workspace set-project");
            check_scope(options, context, envelope)?;
            let project = required(&options.project, context, "'--project'", envelope)?;
            Ok(mutation(
                MutationCommand::WorkspaceSetProject {
                    target: target((*value).to_owned(), Resource::Workspace, envelope)?,
                    project: Some(target(project, Resource::Project, envelope)?),
                },
                options,
            ))
        }
        ["workspace", "create-project", value] => {
            let context = UsageContext::Path("workspace create-project");
            check_scope(options, context, envelope)?;
            Ok(mutation(
                MutationCommand::WorkspaceCreateProject {
                    target: target((*value).to_owned(), Resource::Workspace, envelope)?,
                    label: label_option(options, context, envelope)?,
                },
                options,
            ))
        }
        ["workspace", "ungroup", value] => {
            check_scope(options, UsageContext::Path("workspace ungroup"), envelope)?;
            Ok(mutation(
                MutationCommand::WorkspaceSetProject {
                    target: target((*value).to_owned(), Resource::Workspace, envelope)?,
                    project: None,
                },
                options,
            ))
        }
        ["project", "create"] => {
            check_scope(options, UsageContext::Path("project create"), envelope)?;
            Ok(mutation(
                MutationCommand::ProjectCreate {
                    label: label_option(options, UsageContext::Path("project create"), envelope)?,
                },
                options,
            ))
        }
        ["project", "rename", value] => {
            check_scope(options, UsageContext::Path("project rename"), envelope)?;
            Ok(mutation(
                MutationCommand::ProjectRename {
                    target: target((*value).to_owned(), Resource::Project, envelope)?,
                    label: label_option(options, UsageContext::Path("project rename"), envelope)?,
                },
                options,
            ))
        }
        ["workspace", "list"] => {
            check_scope(options, UsageContext::Path("workspace list"), envelope)?;
            let project = options
                .project
                .clone()
                .map(|value| target(value, Resource::Project, envelope))
                .transpose()?;
            client_query(
                QueryCommand::WorkspaceList { project },
                options,
                UsageContext::Path("workspace list"),
                envelope,
            )
        }
        ["workspace", "get", value] => {
            check_scope(options, UsageContext::Path("workspace get"), envelope)?;
            client_query(
                QueryCommand::WorkspaceGet(target(
                    (*value).to_owned(),
                    Resource::Workspace,
                    envelope,
                )?),
                options,
                UsageContext::Path("workspace get"),
                envelope,
            )
        }
        ["workspace", "create"] => workspace_creation::parse(options, envelope),
        ["workspace", "move", value] => {
            let context = UsageContext::Path("workspace move");
            check_scope(options, context, envelope)?;
            let raw = required(&options.insert_index, context, "'--insert-index'", envelope)?;
            let insert_index = (!raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()))
                .then(|| raw.parse::<usize>().ok())
                .flatten()
                .and_then(|n| {
                    orchbee_workbench_protocol::WorkbenchWorkspaceInsertIndex::new(n).ok()
                })
                .ok_or_else(|| {
                    usage_with(
                        UsageReason::InvalidValue {
                            context,
                            option: "--insert-index",
                            value: raw,
                            expected: ExpectedValue::WorkspaceInsertIndex,
                        },
                        envelope,
                    )
                })?;
            Ok(mutation(
                MutationCommand::WorkspaceMove {
                    target: target((*value).to_owned(), Resource::Workspace, envelope)?,
                    insert_index,
                },
                options,
            ))
        }
        ["workspace", "move-block", values @ ..] => {
            let context = UsageContext::Path("workspace move-block");
            check_scope(options, context, envelope)?;
            if values.is_empty()
                || values.len()
                    > grammar::specification(CommandPath::WorkspaceMoveBlock).maximum_operands
            {
                return Err(usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "targets",
                        value: values.len().to_string(),
                        expected: ExpectedValue::WorkspaceBlockSize,
                    },
                    envelope,
                ));
            }
            let targets = values
                .iter()
                .map(|value| target((*value).to_owned(), Resource::Workspace, envelope))
                .collect::<Result<Vec<_>, _>>()?;
            let before = options
                .before
                .as_ref()
                .map(|value| target(value.clone(), Resource::Workspace, envelope))
                .transpose()?;
            Ok(mutation(
                MutationCommand::WorkspaceMoveBlock { targets, before },
                options,
            ))
        }
        ["panel", "report-metadata", value] => panel_metadata::parse(value, options, envelope),
        [
            "panel",
            verb @ ("report-agent" | "clear-agent-authority" | "release-agent"),
            value,
        ] => panel_agent::parse(verb, value, options, envelope),
        ["workspace", "report-metadata", value] => {
            workspace_metadata::parse(value, options, envelope)
        }
        ["workspace", "rename", value] => {
            check_scope(options, UsageContext::Path("workspace rename"), envelope)?;
            Ok(mutation(
                MutationCommand::WorkspaceRename {
                    target: target((*value).to_owned(), Resource::Workspace, envelope)?,
                    label: label_option(options, UsageContext::Path("workspace rename"), envelope)?,
                },
                options,
            ))
        }
        ["workspace", "accept-directory", value] => {
            check_scope(
                options,
                UsageContext::Path("workspace accept-directory"),
                envelope,
            )?;
            Ok(mutation(
                MutationCommand::WorkspaceAcceptDirectory {
                    target: target((*value).to_owned(), Resource::Workspace, envelope)?,
                    path: path_option(
                        options,
                        UsageContext::Path("workspace accept-directory"),
                        envelope,
                    )?,
                },
                options,
            ))
        }
        ["project", "close-all", value] => workspace_group_close::start(value, options, envelope),
        ["project", verb @ ("close-status" | "close-resume")] => {
            workspace_group_close::retained(verb, options, envelope)
        }
        ["workspace", "close", value] => {
            let wait = close_wait(options, UsageContext::Path("workspace close"), envelope)?;
            Ok(mutation(
                MutationCommand::WorkspaceClose {
                    target: target((*value).to_owned(), Resource::Workspace, envelope)?,
                    wait,
                },
                options,
            ))
        }
        ["workspace", "close-status"] => {
            check_scope(
                options,
                UsageContext::Path("workspace close-status"),
                envelope,
            )?;
            Ok(query(
                QueryCommand::WorkspaceCloseStatus {
                    workspace: workspace_id_option(
                        options,
                        UsageContext::Path("workspace close-status"),
                        envelope,
                    )?,
                },
                options,
            ))
        }
        ["workspace", "close-resume"] => {
            check_scope(
                options,
                UsageContext::Path("workspace close-resume"),
                envelope,
            )?;
            let context = UsageContext::Path("workspace close-resume");
            let workspace = workspace_id_option(options, context, envelope)?;
            let request = close_request_option(options, context, envelope)?;
            let generation = close_generation_option(options, context, envelope)?;
            if !options.confirm {
                return Err(missing_required(context, "'--confirm'", envelope));
            }
            Ok(mutation(
                MutationCommand::WorkspaceCloseResume {
                    workspace,
                    request,
                    generation,
                },
                options,
            ))
        }
        ["tab", "list"] => {
            check_scope(options, UsageContext::Path("tab list"), envelope)?;
            let workspace = options
                .workspace
                .clone()
                .map(|value| target(value, Resource::Workspace, envelope))
                .transpose()?;
            client_query(
                QueryCommand::TabList { workspace },
                options,
                UsageContext::Path("tab list"),
                envelope,
            )
        }
        ["tab", "get", value] => {
            check_scope(options, UsageContext::Path("tab get"), envelope)?;
            client_query(
                QueryCommand::TabGet(target((*value).to_owned(), Resource::Tab, envelope)?),
                options,
                UsageContext::Path("tab get"),
                envelope,
            )
        }
        ["tab", "create"] => creation::parse(None, options, envelope),
        ["panel", "input", operands @ ..] => panel_input::parse(operands, options, envelope),
        ["panel", "rename", value] => {
            let context = UsageContext::Path("panel rename");
            check_scope(options, context, envelope)?;
            if options.clear && options.label.is_some() {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context,
                        first: "--clear",
                        second: "--label",
                    },
                    envelope,
                ));
            }
            if !options.clear && options.label.is_none() {
                return Err(missing_required(
                    context,
                    "'--label' or '--clear'",
                    envelope,
                ));
            }
            let label =
                orchbee_workbench_protocol::PanelLabelUpdate::parse(options.label.as_deref())
                    .map_err(|_| {
                        usage_with(
                            UsageReason::InvalidValue {
                                context,
                                option: "--label",
                                value: options.label.clone().unwrap_or_default(),
                                expected: ExpectedValue::PanelLabel,
                            },
                            envelope,
                        )
                    })?;
            Ok(mutation(
                MutationCommand::PanelRename {
                    target: target((*value).to_owned(), Resource::Panel, envelope)?,
                    label,
                },
                options,
            ))
        }
        ["tab", "rename", value] => {
            check_scope(options, UsageContext::Path("tab rename"), envelope)?;
            Ok(mutation(
                MutationCommand::TabRename {
                    target: target((*value).to_owned(), Resource::Tab, envelope)?,
                    label: label_option(options, UsageContext::Path("tab rename"), envelope)?,
                },
                options,
            ))
        }
        ["tab", "move", value] => {
            check_scope(options, UsageContext::Path("tab move"), envelope)?;
            Ok(mutation(
                tab_move_command(value, options, envelope)?,
                options,
            ))
        }
        ["tab", "close", value] => {
            let wait = close_wait(options, UsageContext::Path("tab close"), envelope)?;
            Ok(mutation(
                MutationCommand::TabClose {
                    target: target((*value).to_owned(), Resource::Tab, envelope)?,
                    wait,
                },
                options,
            ))
        }
        ["panel", "list"] => {
            check_scope(options, UsageContext::Path("panel list"), envelope)?;
            if options.tab.is_some() && options.workspace.is_some() {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context: UsageContext::Path("panel list"),
                        first: "--tab",
                        second: "--workspace",
                    },
                    envelope,
                ));
            }
            let tab = options
                .tab
                .clone()
                .map(|value| target(value, Resource::Tab, envelope))
                .transpose()?;
            let workspace = options
                .workspace
                .clone()
                .map(|value| target(value, Resource::Workspace, envelope))
                .transpose()?;
            client_query(
                QueryCommand::PanelList { tab, workspace },
                options,
                UsageContext::Path("panel list"),
                envelope,
            )
        }
        ["panel", "process-info", operands @ ..] => {
            let context = UsageContext::Path("panel process-info");
            check_scope(options, context, envelope)?;
            if let [_, extra, ..] = operands {
                return Err(unexpected_argument(context, extra, envelope));
            }
            if !operands.is_empty() && options.current {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context,
                        first: "panel-id",
                        second: "--current",
                    },
                    envelope,
                ));
            }
            let panel = operands
                .first()
                .map(|value| (*value).to_owned())
                .map(|value| target(value, Resource::Panel, envelope))
                .transpose()?;
            let target = panel_query_target(panel, options, context, envelope)?;
            Ok(query(QueryCommand::PanelProcessInfo(target), options))
        }
        ["panel", "current", operands @ ..] => {
            let context = UsageContext::Path("panel current");
            check_scope(options, context, envelope)?;
            if let [_, extra, ..] = operands {
                return Err(unexpected_argument(context, extra, envelope));
            }
            if !operands.is_empty() && options.current {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context,
                        first: "<panel>",
                        second: "--current",
                    },
                    envelope,
                ));
            }
            let panel = operands
                .first()
                .map(|value| target((*value).to_owned(), Resource::Panel, envelope))
                .transpose()?;
            let target = panel_query_target(panel, options, context, envelope)?;
            Ok(query(QueryCommand::PanelCurrent(target), options))
        }
        ["panel", "get", value] => {
            check_scope(options, UsageContext::Path("panel get"), envelope)?;
            client_query(
                QueryCommand::PanelGet(target((*value).to_owned(), Resource::Panel, envelope)?),
                options,
                UsageContext::Path("panel get"),
                envelope,
            )
        }
        ["panel", "create-terminal", rest @ ..] => {
            panel_terminal_creation::parse(rest, options, envelope)
        }
        ["panel", "split", rest @ ..] => creation::parse(Some(rest), options, envelope),
        ["panel", "close", value] => {
            check_scope(options, UsageContext::Path("panel close"), envelope)?;
            if !options.confirm {
                return Err(missing_required(
                    UsageContext::Path("panel close"),
                    "'--confirm'",
                    envelope,
                ));
            }
            Ok(mutation(
                MutationCommand::PanelClose {
                    target: target((*value).to_owned(), Resource::Panel, envelope)?,
                },
                options,
            ))
        }
        ["panel", "move", value] => {
            if options.into_tab.is_some() || options.new_tab || options.new_workspace {
                return panel_relocation::parse(value, options, envelope);
            }
            check_scope(options, UsageContext::Path("panel move"), envelope)?;
            Ok(mutation(
                MutationCommand::PanelMove {
                    target: target((*value).to_owned(), Resource::Panel, envelope)?,
                    destination: panel_move_destination(options, envelope)?,
                },
                options,
            ))
        }
        ["terminal", "list"] => {
            let context = UsageContext::Path("terminal list");
            check_scope(options, context, envelope)?;
            let filters = [
                (
                    "--workspace",
                    options.workspace.clone(),
                    Resource::Workspace,
                ),
                ("--tab", options.tab.clone(), Resource::Tab),
                ("--panel", options.panel.clone(), Resource::Panel),
            ];
            let mut given = filters.into_iter().filter_map(|(option, value, resource)| {
                value.map(|value| (option, value, resource))
            });
            let filter = given.next();
            if let (Some((first, ..)), Some((second, ..))) = (&filter, given.next()) {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context,
                        first,
                        second,
                    },
                    envelope,
                ));
            }
            let filter = filter
                .map(|(_option, value, resource)| {
                    target(value, resource, envelope).map(|target| match resource {
                        Resource::Workspace => TerminalListFilter::Workspace(target),
                        Resource::Tab => TerminalListFilter::Tab(target),
                        _ => TerminalListFilter::Panel(target),
                    })
                })
                .transpose()?;
            Ok(query(QueryCommand::TerminalList { filter }, options))
        }
        ["terminal", "get", value] => {
            reject_filters(options, UsageContext::Path("terminal get"), envelope)?;
            Ok(query(
                QueryCommand::TerminalGet(target(
                    (*value).to_owned(),
                    Resource::Terminal,
                    envelope,
                )?),
                options,
            ))
        }
        ["terminal", "attach", operands @ ..] => {
            terminal_stream::attach(operands, options, envelope, false)
        }
        ["agent", "attach", operands @ ..] => {
            terminal_stream::attach(operands, options, envelope, true)
        }
        ["terminal", "session", "control", operands @ ..] => {
            terminal_stream::control(operands, options, envelope)
        }
        ["terminal", "session", "observe", operands @ ..] => {
            terminal_stream::observe(operands, options, envelope)
        }
        ["terminal", "clear", value] => {
            check_scope(options, UsageContext::Path("terminal clear"), envelope)?;
            Ok(Request::TerminalClear {
                target: target((*value).to_owned(), Resource::Terminal, envelope)?,
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["terminal", "edit-scrollback", value] => {
            let context = UsageContext::Path("terminal edit-scrollback");
            check_scope(options, context, envelope)?;
            Ok(Request::TerminalEditor {
                target: target((*value).to_owned(), Resource::Terminal, envelope)?,
                client: client_control::client_id(options, context, envelope)?,
                timeout_ms: wait_timeout_option(options, context, envelope)?,
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["terminal", "link-resolve", value] => {
            terminal_links::parse(value, options, envelope, false)
        }
        ["terminal", "link-activate", value] => {
            terminal_links::parse(value, options, envelope, true)
        }
        ["terminal", "copy-motion", value] => terminal_copy::parse(value, options, false, envelope),
        ["terminal", "copy-search", value] => terminal_copy::parse(value, options, true, envelope),
        ["terminal", "selection-read", value] => {
            terminal_selection::parse(value, options, envelope)
        }
        ["terminal", "read", operands @ ..] => {
            let context = UsageContext::Path("terminal read");
            check_scope(options, context, envelope)?;
            terminal_output::request(
                OutputTarget::Terminal(terminal_target::parse(
                    operands,
                    options,
                    UsageContext::Path("terminal read"),
                    envelope,
                )?),
                options,
                context,
                envelope,
            )
        }
        ["terminal", "wait-output", operands @ ..] => {
            let context = UsageContext::Path("terminal wait-output");
            check_scope(options, context, envelope)?;
            terminal_output::wait_request(
                terminal_target::parse(
                    operands,
                    options,
                    UsageContext::Path("terminal wait-output"),
                    envelope,
                )?,
                options,
                context,
                envelope,
            )
        }
        ["terminal", "run", operands @ ..] => terminal_run::request(operands, options, envelope),
        ["terminal", "send", operands @ ..] => {
            check_scope(options, UsageContext::Path("terminal send"), envelope)?;
            let context = UsageContext::Path("terminal send");
            Ok(mutation(
                MutationCommand::TerminalSend {
                    target: terminal_target::parse(
                        operands,
                        options,
                        UsageContext::Path("terminal send"),
                        envelope,
                    )?,
                    input: send_input(options, context, envelope)?,
                },
                options,
            ))
        }
        ["worktree", verb @ ("create" | "open" | "remove")] => {
            worktree::start(verb, options, envelope)
        }
        ["worktree", verb @ ("status" | "resume"), id] => {
            worktree::observe(verb, id, options, envelope)
        }
        ["worktree", "status"] => Err(missing_required(
            UsageContext::Path("worktree status"),
            "<operation-id>",
            envelope,
        )),
        ["worktree", "resume"] => Err(missing_required(
            UsageContext::Path("worktree resume"),
            "<operation-id>",
            envelope,
        )),
        ["worktree", "list"] => worktree::list(options, envelope),
        ["launch-profile", "list"] => {
            reject_filters(options, UsageContext::Path("launch-profile list"), envelope)?;
            Ok(query(QueryCommand::LaunchProfileList, options))
        }
        ["launch-profile", "get", value] => {
            reject_filters(options, UsageContext::Path("launch-profile get"), envelope)?;
            Ok(query(
                QueryCommand::LaunchProfileGet(profile_selector(
                    value,
                    UsageContext::Path("launch-profile get"),
                    envelope,
                )?),
                options,
            ))
        }
        ["launch-profile", "create"] => {
            let context = UsageContext::Path("launch-profile create");
            check_scope(options, context, envelope)?;
            Ok(mutation(
                MutationCommand::LaunchProfileCreate {
                    name: profile_name_option(options, context, envelope)?,
                    launch: required_launch_option(options, context, envelope)?,
                },
                options,
            ))
        }
        ["launch-profile", "delete", value] => {
            reject_filters(
                options,
                UsageContext::Path("launch-profile delete"),
                envelope,
            )?;
            Ok(mutation(
                MutationCommand::LaunchProfileDelete {
                    selector: profile_selector(
                        value,
                        UsageContext::Path("launch-profile delete"),
                        envelope,
                    )?,
                },
                options,
            ))
        }
        ["api", "snapshot"] => {
            reject_filters(options, UsageContext::Path("api snapshot"), envelope)?;
            Ok(query(QueryCommand::ApiSnapshot, options))
        }
        ["api", "schema"] => schema_export::request(None, options, envelope),
        ["api", "schema", schema_id] => schema_export::request(Some(schema_id), options, envelope),
        ["config", "reset-keys"] => {
            reject_filters(options, UsageContext::Path("config reset-keys"), envelope)?;
            Ok(Request::ConfigResetKeys {
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["channel", "show"] => {
            reject_filters(options, UsageContext::Path("channel show"), envelope)?;
            Ok(Request::ChannelShow {
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["update"] => {
            reject_filters(options, UsageContext::Path("update"), envelope)?;
            Ok(Request::Update {
                channel: None,
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["channel", "set", channel @ ("stable" | "preview")] => {
            reject_filters(options, UsageContext::Path("channel set"), envelope)?;
            Ok(Request::Update {
                channel: Some(if *channel == "stable" {
                    orchbee_local_runtime::UpdateChannel::Stable
                } else {
                    orchbee_local_runtime::UpdateChannel::Preview
                }),
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["config", "check"] => {
            reject_filters(options, UsageContext::Path("config check"), envelope)?;
            Ok(query(QueryCommand::ConfigCheck, options))
        }
        ["server", "start"] => {
            reject_filters(options, UsageContext::Path("server start"), envelope)?;
            Ok(Request::ServerStart {
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["terminal", "title", "set", title] => {
            let context = UsageContext::Path("terminal title set");
            reject_filters(options, context, envelope)?;
            let title =
                orchbee_workbench_protocol::WindowTitle::normalize(title).map_err(|_| {
                    usage_with(
                        UsageReason::InvalidValue {
                            context,
                            option: "<title>",
                            value: (*title).into(),
                            expected: ExpectedValue::WindowTitle,
                        },
                        envelope,
                    )
                })?;
            Ok(Request::WindowTitle {
                title: Some(title),
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["terminal", "title", "clear"] => {
            reject_filters(
                options,
                UsageContext::Path("terminal title clear"),
                envelope,
            )?;
            Ok(Request::WindowTitle {
                title: None,
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["terminal", "title"] => Err(missing_command(
            UsageContext::Path("terminal title"),
            envelope,
        )),
        ["terminal", "title", "set"] => Err(missing_required(
            UsageContext::Path("terminal title set"),
            "<title>",
            envelope,
        )),
        ["terminal", "title", "set", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("terminal title set"),
            extra,
            envelope,
        )),
        ["terminal", "title", "clear", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("terminal title clear"),
            extra,
            envelope,
        )),
        ["terminal", "title", verb, ..] => Err(unknown_command(
            UsageContext::Path("terminal title"),
            verb,
            envelope,
        )),
        ["notification", "show", title] => {
            use orchbee_workbench_protocol::Notification;
            let context = UsageContext::Path("notification show");
            check_scope(options, context, envelope)?;
            let invalid = |option, value: &str, expected| {
                usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option,
                        value: value.into(),
                        expected,
                    },
                    envelope,
                )
            };
            let position = options
                .notification_position
                .as_deref()
                .map(|value| {
                    grammar::values::notification_position(value).ok_or_else(|| {
                        invalid("--position", value, ExpectedValue::NotificationPosition)
                    })
                })
                .transpose()?;
            let raw_sound = options.notification_sound.as_deref().unwrap_or("none");
            let sound = grammar::values::notification_sound(raw_sound)
                .ok_or_else(|| invalid("--sound", raw_sound, ExpectedValue::NotificationSound))?;
            let message = Notification::normalize(
                title,
                options.notification_body.as_deref(),
                position,
                sound,
            )
            .map_err(|_| invalid("<title>", title, ExpectedValue::NotificationTitle))?;
            Ok(Request::Notification {
                message,
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["server", verb @ ("config" | "reload-config")] => {
            reject_filters(
                options,
                UsageContext::Path(if *verb == "config" {
                    "server config"
                } else {
                    "server reload-config"
                }),
                envelope,
            )?;
            Ok(Request::RuntimeConfiguration {
                action: if *verb == "config" {
                    orchbee_workbench_protocol::ConfigurationAction::Current
                } else {
                    orchbee_workbench_protocol::ConfigurationAction::Reload
                },
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["server", "agent-manifests"] => {
            reject_filters(
                options,
                UsageContext::Path("server agent-manifests"),
                envelope,
            )?;
            Ok(Request::AgentManifests {
                command: ManifestCommand::Current,
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        [
            "server",
            verb @ ("reload-agent-manifests" | "update-agent-manifests"),
        ] => {
            let update = *verb == "update-agent-manifests";
            let context = UsageContext::Path(if update {
                "server update-agent-manifests"
            } else {
                "server reload-agent-manifests"
            });
            check_scope(options, context, envelope)?;
            let operation_id = options
                .request_id
                .as_deref()
                .map(|s| manifest_operation_id(s, context, "--request-id", envelope))
                .transpose()?;
            Ok(Request::AgentManifests {
                command: if update {
                    ManifestCommand::Update { operation_id }
                } else {
                    ManifestCommand::Reload { operation_id }
                },
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["server", "agent-manifests-status", id] => {
            let context = UsageContext::Path("server agent-manifests-status");
            reject_filters(options, context, envelope)?;
            Ok(Request::AgentManifests {
                command: ManifestCommand::Status {
                    operation_id: manifest_operation_id(id, context, "<operation-id>", envelope)?,
                },
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["server", "agent-manifests-status"] => Err(missing_required(
            UsageContext::Path("server agent-manifests-status"),
            "<operation-id>",
            envelope,
        )),
        ["server", "diagnose"] => {
            reject_filters(options, UsageContext::Path("server diagnose"), envelope)?;
            Ok(Request::ServerDiagnose {
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["server", "stop"] => Ok(Request::ServerStop {
            command: stop_command(options, UsageContext::Path("server stop"), envelope)?,
            format: options.format.unwrap_or(OutputFormat::Text),
        }),
        ["server", "stop-status", id] => {
            let context = UsageContext::Path("server stop-status");
            reject_filters(options, context, envelope)?;
            Ok(Request::ServerStop {
                command: ServerStopCommand::Status {
                    id: stop_id(id, context, "<request-id>", envelope)?,
                },
                format: options.format.unwrap_or(OutputFormat::Text),
            })
        }
        ["server", "stop-status"] => Err(missing_required(
            UsageContext::Path("server stop-status"),
            "<request-id>",
            envelope,
        )),
        ["completion", shell] => {
            reject_extras(options, UsageContext::Path("completion"), envelope)?;
            let shell = match *shell {
                "bash" => CompletionShell::Bash,
                "zsh" => CompletionShell::Zsh,
                "fish" => CompletionShell::Fish,
                "powershell" => CompletionShell::PowerShell,
                "elvish" => CompletionShell::Elvish,
                _ => {
                    return Err(usage_with(
                        UsageReason::InvalidValue {
                            context: UsageContext::Path("completion"),
                            option: "<shell>",
                            value: (*shell).to_owned(),
                            expected: ExpectedValue::Shell,
                        },
                        envelope,
                    ));
                }
            };
            Ok(Request::Completion(shell))
        }
        // Command-scoped catch-alls classify the real failure — an extra
        // positional, a missing operand, a missing subcommand, or an
        // unrecognized word — after every legal arm has kept precedence.
        ["tui", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tui"),
            extra,
            envelope,
        )),
        ["status", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("status"),
            extra,
            envelope,
        )),
        ["agent"] => Err(missing_command(UsageContext::Path("agent"), envelope)),
        ["agent", "changes", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("agent changes"),
            extra,
            envelope,
        )),
        ["agent", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("agent list"),
            extra,
            envelope,
        )),
        ["agent", "doctor", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("agent doctor"),
            extra,
            envelope,
        )),
        ["agent", "integration"] => Err(missing_command(
            UsageContext::Path("agent integration"),
            envelope,
        )),
        ["agent", "integration", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("agent integration list"),
            extra,
            envelope,
        )),
        [
            "agent",
            "integration",
            verb @ ("explain" | "enable" | "disable"),
            extra,
            ..,
        ] => Err(unexpected_argument(
            UsageContext::Path(match *verb {
                "explain" => "agent integration explain",
                "enable" => "agent integration enable",
                _ => "agent integration disable",
            }),
            extra,
            envelope,
        )),
        ["agent", "integration", verb, ..] => Err(unknown_command(
            UsageContext::Path("agent integration"),
            verb,
            envelope,
        )),
        ["agent", verb, ..] => Err(unknown_command(UsageContext::Path("agent"), verb, envelope)),
        ["project"] => Err(missing_command(UsageContext::Path("project"), envelope)),
        ["project", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("project list"),
            extra,
            envelope,
        )),
        ["project", "get"] => Err(missing_required(
            UsageContext::Path("project get"),
            "<project>",
            envelope,
        )),
        ["project", "get", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("project get"),
            extra,
            envelope,
        )),
        ["project", "create", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("project create"),
            extra,
            envelope,
        )),
        ["project", "rename"] => Err(missing_required(
            UsageContext::Path("project rename"),
            "<project>",
            envelope,
        )),
        ["project", "rename", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("project rename"),
            extra,
            envelope,
        )),
        ["project", verb @ ("close-all" | "dissolve")] => Err(missing_required(
            UsageContext::Path(if *verb == "close-all" {
                "project close-all"
            } else {
                "project dissolve"
            }),
            "<project>",
            envelope,
        )),
        ["project", verb @ ("close-all" | "dissolve"), _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path(if *verb == "close-all" {
                "project close-all"
            } else {
                "project dissolve"
            }),
            extra,
            envelope,
        )),
        [
            "project",
            verb @ ("close-status" | "close-resume"),
            extra,
            ..,
        ] => Err(unexpected_argument(
            UsageContext::Path(if *verb == "close-status" {
                "project close-status"
            } else {
                "project close-resume"
            }),
            extra,
            envelope,
        )),
        ["project", verb, ..] => Err(unknown_command(
            UsageContext::Path("project"),
            verb,
            envelope,
        )),
        ["workspace"] => Err(missing_command(UsageContext::Path("workspace"), envelope)),
        ["workspace", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace list"),
            extra,
            envelope,
        )),
        ["workspace", "get"] => Err(missing_required(
            UsageContext::Path("workspace get"),
            "<workspace>",
            envelope,
        )),
        ["workspace", "get", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace get"),
            extra,
            envelope,
        )),
        ["workspace", "create", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace create"),
            extra,
            envelope,
        )),
        ["workspace", "rename"] => Err(missing_required(
            UsageContext::Path("workspace rename"),
            "<workspace>",
            envelope,
        )),
        ["workspace", "rename", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace rename"),
            extra,
            envelope,
        )),
        ["workspace", "accept-directory"] => Err(missing_required(
            UsageContext::Path("workspace accept-directory"),
            "<workspace>",
            envelope,
        )),
        ["workspace", "accept-directory", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace accept-directory"),
            extra,
            envelope,
        )),
        ["workspace", "close"] => Err(missing_required(
            UsageContext::Path("workspace close"),
            "<workspace>",
            envelope,
        )),
        ["workspace", "close", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace close"),
            extra,
            envelope,
        )),
        ["workspace", "close-status", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace close-status"),
            extra,
            envelope,
        )),
        ["workspace", "close-resume", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace close-resume"),
            extra,
            envelope,
        )),
        [
            "workspace",
            verb @ ("set-project" | "create-project" | "ungroup"),
        ] => Err(missing_required(
            UsageContext::Path(match *verb {
                "set-project" => "workspace set-project",
                "create-project" => "workspace create-project",
                _ => "workspace ungroup",
            }),
            "<workspace>",
            envelope,
        )),
        [
            "workspace",
            verb @ ("set-project" | "create-project" | "ungroup"),
            _,
            extra,
            ..,
        ] => Err(unexpected_argument(
            UsageContext::Path(match *verb {
                "set-project" => "workspace set-project",
                "create-project" => "workspace create-project",
                _ => "workspace ungroup",
            }),
            extra,
            envelope,
        )),
        ["workspace", verb, ..] => Err(unknown_command(
            UsageContext::Path("workspace"),
            verb,
            envelope,
        )),
        ["tab"] => Err(missing_command(UsageContext::Path("tab"), envelope)),
        ["tab", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab list"),
            extra,
            envelope,
        )),
        ["tab", "get"] => Err(missing_required(
            UsageContext::Path("tab get"),
            "<tab>",
            envelope,
        )),
        ["tab", "get", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab get"),
            extra,
            envelope,
        )),
        ["tab", "create", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab create"),
            extra,
            envelope,
        )),
        ["tab", "rename"] => Err(missing_required(
            UsageContext::Path("tab rename"),
            "<tab>",
            envelope,
        )),
        ["tab", "rename", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab rename"),
            extra,
            envelope,
        )),
        ["tab", "move"] => Err(missing_required(
            UsageContext::Path("tab move"),
            "<tab>",
            envelope,
        )),
        ["tab", "move", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab move"),
            extra,
            envelope,
        )),
        ["tab", "close"] => Err(missing_required(
            UsageContext::Path("tab close"),
            "<tab>",
            envelope,
        )),
        ["tab", "close", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab close"),
            extra,
            envelope,
        )),
        ["tab", verb, ..] => Err(unknown_command(UsageContext::Path("tab"), verb, envelope)),
        ["panel"] => Err(missing_command(UsageContext::Path("panel"), envelope)),
        ["panel", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("panel list"),
            extra,
            envelope,
        )),
        ["panel", "get"] => Err(missing_required(
            UsageContext::Path("panel get"),
            "<panel>",
            envelope,
        )),
        ["panel", "get", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("panel get"),
            extra,
            envelope,
        )),
        ["panel", "close"] => Err(missing_required(
            UsageContext::Path("panel close"),
            "<panel>",
            envelope,
        )),
        ["panel", "close", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("panel close"),
            extra,
            envelope,
        )),
        ["panel", "move"] => Err(missing_required(
            UsageContext::Path("panel move"),
            "<panel>",
            envelope,
        )),
        ["panel", "move", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("panel move"),
            extra,
            envelope,
        )),
        ["panel", verb, ..] => Err(unknown_command(UsageContext::Path("panel"), verb, envelope)),
        ["terminal"] => Err(missing_command(UsageContext::Path("terminal"), envelope)),
        ["terminal", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("terminal list"),
            extra,
            envelope,
        )),
        ["terminal", "get"] => Err(missing_required(
            UsageContext::Path("terminal get"),
            "<terminal>",
            envelope,
        )),
        ["terminal", "get", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("terminal get"),
            extra,
            envelope,
        )),

        ["terminal", verb, ..] => Err(unknown_command(
            UsageContext::Path("terminal"),
            verb,
            envelope,
        )),
        ["worktree"] => Err(missing_command(UsageContext::Path("worktree"), envelope)),
        ["worktree", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("worktree list"),
            extra,
            envelope,
        )),
        ["worktree", verb @ ("create" | "open" | "remove"), extra, ..] => Err(unexpected_argument(
            UsageContext::Path(match *verb {
                "create" => "worktree create",
                "open" => "worktree open",
                _ => "worktree remove",
            }),
            extra,
            envelope,
        )),
        ["worktree", verb @ ("status" | "resume"), _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path(if *verb == "status" {
                "worktree status"
            } else {
                "worktree resume"
            }),
            extra,
            envelope,
        )),
        ["worktree", verb, ..] => Err(unknown_command(
            UsageContext::Path("worktree"),
            verb,
            envelope,
        )),
        ["launch-profile"] => Err(missing_command(
            UsageContext::Path("launch-profile"),
            envelope,
        )),
        ["launch-profile", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("launch-profile list"),
            extra,
            envelope,
        )),
        ["launch-profile", "get"] => Err(missing_required(
            UsageContext::Path("launch-profile get"),
            "<profile>",
            envelope,
        )),
        ["launch-profile", "get", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("launch-profile get"),
            extra,
            envelope,
        )),
        ["launch-profile", "create", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("launch-profile create"),
            extra,
            envelope,
        )),
        ["launch-profile", "delete"] => Err(missing_required(
            UsageContext::Path("launch-profile delete"),
            "<profile>",
            envelope,
        )),
        ["launch-profile", "delete", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("launch-profile delete"),
            extra,
            envelope,
        )),
        ["launch-profile", verb, ..] => Err(unknown_command(
            UsageContext::Path("launch-profile"),
            verb,
            envelope,
        )),
        ["api"] => Err(missing_command(UsageContext::Path("api"), envelope)),
        ["api", "snapshot", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("api snapshot"),
            extra,
            envelope,
        )),
        ["api", "schema", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("api schema"),
            extra,
            envelope,
        )),
        ["api", verb, ..] => Err(unknown_command(UsageContext::Path("api"), verb, envelope)),
        ["update", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("update"),
            extra,
            envelope,
        )),
        ["channel", "set"] => Err(missing_command(UsageContext::Path("channel set"), envelope)),
        ["channel", "set", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("channel set"),
            extra,
            envelope,
        )),
        ["channel"] => Err(missing_command(UsageContext::Path("channel"), envelope)),
        ["channel", "show", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("channel show"),
            extra,
            envelope,
        )),
        ["channel", verb, ..] => Err(unknown_command(
            UsageContext::Path("channel"),
            verb,
            envelope,
        )),
        ["config"] => Err(missing_command(UsageContext::Path("config"), envelope)),
        ["config", "reset-keys", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("config reset-keys"),
            extra,
            envelope,
        )),
        ["config", "check", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("config check"),
            extra,
            envelope,
        )),
        ["config", verb, ..] => Err(unknown_command(
            UsageContext::Path("config"),
            verb,
            envelope,
        )),
        ["notification"] => Err(missing_command(
            UsageContext::Path("notification"),
            envelope,
        )),
        ["notification", "show"] => Err(missing_required(
            UsageContext::Path("notification show"),
            "<title>",
            envelope,
        )),
        ["notification", "show", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("notification show"),
            extra,
            envelope,
        )),
        ["notification", verb, ..] => Err(unknown_command(
            UsageContext::Path("notification"),
            verb,
            envelope,
        )),
        ["server"] => Err(missing_command(UsageContext::Path("server"), envelope)),
        ["server", "start", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("server start"),
            extra,
            envelope,
        )),
        ["server", "diagnose", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("server diagnose"),
            extra,
            envelope,
        )),
        ["server", "stop", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("server stop"),
            extra,
            envelope,
        )),
        ["server", "stop-status", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("server stop-status"),
            extra,
            envelope,
        )),
        ["server", verb, ..] => Err(unknown_command(
            UsageContext::Path("server"),
            verb,
            envelope,
        )),
        ["completion"] => Err(missing_required(
            UsageContext::Path("completion"),
            "<shell>",
            envelope,
        )),
        ["completion", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("completion"),
            extra,
            envelope,
        )),
        [word, ..] => Err(unknown_command(UsageContext::Root, word, envelope)),
    }
}

fn client_query(
    command: QueryCommand,
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    if let Some(client) = client_control::client_id(options, context, envelope)? {
        return Ok(Request::ClientQuery {
            command,
            client,
            format: options.format.unwrap_or(OutputFormat::Text),
        });
    }
    Ok(query(command, options))
}

fn panel_query_target(
    panel: Option<Target>,
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<PanelQueryTarget, ParseFailure> {
    if panel.is_some() && options.client.is_some() {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "<panel>",
                second: "--client",
            },
            envelope,
        ));
    }
    if let Some(panel) = panel {
        return Ok(PanelQueryTarget::Panel(panel));
    }
    Ok(client_control::client_id(options, context, envelope)?
        .map_or(PanelQueryTarget::Current, PanelQueryTarget::Client))
}
