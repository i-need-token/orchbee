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
    Mutate(PluginRegistryMutation),
    List(Option<String>),
    Actions(Option<String>),
    ConfigDir(String),
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
        "link" => "plugin link",
        "unlink" => "plugin unlink",
        "enable" => "plugin enable",
        "disable" => "plugin disable",
        "list" => "plugin list",
        "config-dir" => "plugin config-dir",
        "action list" => "plugin action list",
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
    let command = if verb == "list" || verb == "action list" {
        let filter = args.first().map(|value| id(value)).transpose()?;
        if verb == "list" {
            PluginCommand::List(filter)
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
