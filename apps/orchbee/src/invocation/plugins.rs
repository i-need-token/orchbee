use super::{
    ExpectedValue, FailureEnvelope, NO_OPTIONS, Options, OutputFormat, ParseFailure, Request,
    Scope, UsageContext, UsageReason, check_scope, missing_command, missing_required,
    unexpected_argument, unknown_command, usage_with,
};
use orchbee_local_site_runtime::{
    PluginRegistryMutation, normalize_windows_user_path, plugin_identifier,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PluginCommand {
    Install {
        source: orchbee_local_site_runtime::PluginGithubSource,
        reference: Option<String>,
        timeout_ms: u32,
    },
    Uninstall(String),
    Mutate(PluginRegistryMutation),
    List(Option<String>),
    Actions(Option<String>),
    ConfigDir(String),
    Invoke {
        client: Option<orchbee_workbench_protocol::WorkbenchClientId>,
        plugin_id: Option<String>,
        action_id: String,
        context: Box<orchbee_workbench_protocol::PluginInvocationContext>,
    },
    Logs {
        plugin_id: Option<String>,
        limit: u16,
    },
}
pub(super) fn parse(
    words: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let (verb, args) = match words {
        ["plugin"] => return Err(missing_command(UsageContext::Path("plugin"), envelope)),
        ["plugin", "action"] => {
            return Err(missing_command(
                UsageContext::Path("plugin action"),
                envelope,
            ));
        }
        ["plugin", "action", "list", args @ ..] => ("action list", args),
        ["plugin", "action", "invoke", args @ ..] => ("action invoke", args),
        ["plugin", "log"] => {
            return Err(missing_command(UsageContext::Path("plugin log"), envelope));
        }
        ["plugin", "log", "list", args @ ..] => ("log list", args),
        ["plugin", "log", unknown, ..] => {
            return Err(unknown_command(
                UsageContext::Path("plugin log"),
                unknown,
                envelope,
            ));
        }
        ["plugin", "action", unknown, ..] => {
            return Err(unknown_command(
                UsageContext::Path("plugin action"),
                unknown,
                envelope,
            ));
        }
        ["plugin", verb, args @ ..] => (*verb, args),
        _ => unreachable!("plugin family routed here"),
    };
    let context = UsageContext::Path(match verb {
        "install" => "plugin install",
        "uninstall" => "plugin uninstall",
        "link" => "plugin link",
        "unlink" => "plugin unlink",
        "enable" => "plugin enable",
        "disable" => "plugin disable",
        "list" => "plugin list",
        "config-dir" => "plugin config-dir",
        "action list" => "plugin action list",
        "action invoke" => "plugin action invoke",
        "log list" => "plugin log list",
        _ => {
            return Err(unknown_command(
                UsageContext::Path("plugin"),
                verb,
                envelope,
            ));
        }
    });
    check_scope(
        options,
        Scope {
            plugin: verb == "link",
            plugin_install: verb == "install",
            timeout: verb == "install",
            plugin_select: matches!(verb, "list" | "action list" | "action invoke" | "log list"),
            plugin_context: verb == "action invoke",
            client: verb == "action invoke",
            limit: verb == "log list",
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    if options.plugin_enabled && options.plugin_disabled {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--enabled",
                second: "--disabled",
            },
            envelope,
        ));
    }
    if args.len() > 1 {
        return Err(unexpected_argument(context, args[1], envelope));
    }
    let id = |value: &str| {
        plugin_identifier(value).map_err(|_| {
            usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "<plugin-id>",
                    value: value.into(),
                    expected: ExpectedValue::PluginId,
                },
                envelope,
            )
        })
    };
    let selected = options.plugin_id.as_deref().map(id).transpose()?;
    let invalid = |option: &'static str, value: &str, expected| {
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
    let command = if verb == "install" {
        let value = args
            .first()
            .ok_or_else(|| missing_required(context, "<owner/repo[/subdir]>", envelope))?;
        let source = orchbee_local_site_runtime::PluginGithubSource::parse(value)
            .map_err(|_| invalid("<source>", value, ExpectedValue::PluginGithubSource))?;
        if !options.plugin_yes {
            return Err(missing_required(context, "--yes", envelope));
        }
        if let Some(reference) = &options.plugin_ref
            && !orchbee_local_site_runtime::plugin_git_reference(reference)
        {
            return Err(invalid(
                "--ref",
                reference,
                ExpectedValue::PluginGitReference,
            ));
        }
        let timeout_ms = match options.timeout.as_deref() {
            None => 600_000,
            Some(value) => value
                .parse::<u32>()
                .ok()
                .filter(|n| (1..=3_600_000).contains(n))
                .ok_or_else(|| invalid("--timeout", value, ExpectedValue::WaitTimeout))?,
        };
        PluginCommand::Install {
            source,
            reference: options.plugin_ref.clone(),
            timeout_ms,
        }
    } else if verb == "uninstall" {
        let value = args
            .first()
            .ok_or_else(|| missing_required(context, "<plugin-id-or-source>", envelope))?;
        let target = if orchbee_local_site_runtime::PluginGithubSource::parse(value).is_ok() {
            (*value).into()
        } else {
            id(value)?
        };
        PluginCommand::Uninstall(target)
    } else if verb == "action invoke" {
        let action_id = args
            .first()
            .ok_or_else(|| missing_required(context, "<action-id>", envelope))?;
        if !orchbee_workbench_protocol::plugin_identifier(action_id, 241) {
            return Err(invalid(
                "<action-id>",
                action_id,
                ExpectedValue::PluginActionId,
            ));
        }
        let mut invocation: orchbee_workbench_protocol::PluginInvocationContext = match options
            .plugin_context
            .as_deref()
        {
            Some(value) if value.len() <= 16 * 1024 => serde_json::from_str(value)
                .map_err(|_| invalid("--context", value, ExpectedValue::PluginContext))?,
            Some(value) => return Err(invalid("--context", value, ExpectedValue::PluginContext)),
            None => Default::default(),
        };
        invocation
            .invocation_source
            .get_or_insert_with(|| "cli".into());
        invocation.validate().map_err(|_| {
            invalid(
                "--context",
                options.plugin_context.as_deref().unwrap_or(""),
                ExpectedValue::PluginContext,
            )
        })?;
        PluginCommand::Invoke {
            client: super::client_control::client_id(options, context, envelope)?,
            plugin_id: selected,
            action_id: (*action_id).into(),
            context: Box::new(invocation),
        }
    } else if matches!(verb, "list" | "action list" | "log list") {
        if selected.is_some() && !args.is_empty() {
            return Err(usage_with(
                UsageReason::ConflictingOptions {
                    context,
                    first: "<plugin-id>",
                    second: "--plugin",
                },
                envelope,
            ));
        }
        let filter = selected.or(args.first().map(|value| id(value)).transpose()?);
        if verb == "list" {
            PluginCommand::List(filter)
        } else if verb == "log list" {
            let limit = match options.limit.as_deref() {
                None => 50,
                Some(value) => value
                    .parse::<u16>()
                    .ok()
                    .filter(|n| (1..=200).contains(n))
                    .ok_or_else(|| invalid("--limit", value, ExpectedValue::PluginLogLimit))?,
            };
            PluginCommand::Logs {
                plugin_id: filter,
                limit,
            }
        } else {
            PluginCommand::Actions(filter)
        }
    } else {
        let value = args.first().ok_or_else(|| {
            missing_required(
                context,
                if verb == "link" {
                    "<path>"
                } else {
                    "<plugin-id>"
                },
                envelope,
            )
        })?;
        if verb == "link" {
            let path = normalize_windows_user_path(
                value,
                |name| std::env::var_os(name),
                std::env::current_dir,
            )
            .map_err(|_| {
                usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "<path>",
                        value: (*value).into(),
                        expected: ExpectedValue::DirectoryPath,
                    },
                    envelope,
                )
            })?;
            PluginCommand::Mutate(PluginRegistryMutation::Link {
                path: path.into(),
                enabled: !options.plugin_disabled,
            })
        } else {
            let id = id(value)?;
            match verb {
                "config-dir" => PluginCommand::ConfigDir(id),
                "unlink" => PluginCommand::Mutate(PluginRegistryMutation::Unlink { id }),
                _ => PluginCommand::Mutate(PluginRegistryMutation::SetEnabled {
                    id,
                    enabled: verb == "enable",
                }),
            }
        }
    };
    Ok(Request::Plugin {
        command,
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}
