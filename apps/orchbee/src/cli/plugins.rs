//! Offline catalog commands and admitted runtime invocation/log commands.
use crate::cli_text::CliText;
pub(in crate::cli) mod context;
mod failure;
pub(in crate::cli) mod links;
mod panes;
mod popup;
mod provision;
mod runtime;
use super::{Failure, FailureKind, Output, failure_output, output};
use crate::{
    invocation::{OutputFormat, plugins::PluginCommand},
    localization::Locale,
};
use orchbee_local_runtime::{PluginRegistry, PluginRegistryError, PluginRegistryMutation};
use serde_json::{Value, json};

enum RegistryCommand {
    Mutate(PluginRegistryMutation),
    List(Option<String>),
    Actions(Option<String>),
    ConfigDir(String),
}

/// Executes complete plugin grammar; link context is resolved before acquiring the catalog.
pub(crate) fn run(command: PluginCommand, format: OutputFormat, locale: Locale) -> Output {
    if let Err(failure) = super::execution::admit(super::execution::Capability::Plugins) {
        return failure_output(failure, Some(format), locale);
    }
    let command = match command {
        PluginCommand::PopupClose { client, timeout_ms } => {
            return popup::close(client.as_ref(), timeout_ms, format, locale);
        }
        PluginCommand::PaneManage(command) => return panes::manage(&command, format, locale),
        PluginCommand::PaneOpen(command) => return panes::open(*command, format, locale),
        command @ (PluginCommand::Install { .. } | PluginCommand::Uninstall(_)) => {
            return provision::run(command, format, locale);
        }
        command @ (PluginCommand::Invoke { .. } | PluginCommand::Logs { .. }) => {
            return runtime::run(command, format, locale);
        }
        PluginCommand::List(id) => RegistryCommand::List(id),
        PluginCommand::Actions(id) => RegistryCommand::Actions(id),
        PluginCommand::ConfigDir(id) => RegistryCommand::ConfigDir(id),
        PluginCommand::Mutate(mutation) => match mutation.into_registry_mutation(
            format,
            |name| std::env::var_os(name),
            std::env::current_dir,
        ) {
            Ok(mutation) => RegistryCommand::Mutate(mutation),
            Err(failure) => return super::parse_failure(failure, locale),
        },
    };
    let operation = match &command {
        RegistryCommand::List(_) => "list",
        RegistryCommand::Actions(_) => "action-list",
        RegistryCommand::ConfigDir(_) => "config-dir",
        RegistryCommand::Mutate(PluginRegistryMutation::Link { .. }) => "link",
        RegistryCommand::Mutate(PluginRegistryMutation::Unlink { .. }) => "unlink",
        RegistryCommand::Mutate(PluginRegistryMutation::SetEnabled { enabled: true, .. }) => {
            "enable"
        }
        RegistryCommand::Mutate(PluginRegistryMutation::SetEnabled { enabled: false, .. }) => {
            "disable"
        }
    };
    let result = PluginRegistry::from_environment(|name| std::env::var_os(name))
        .and_then(|registry| execute(&registry, command));
    let (status, result, text) = match result {
        Ok(result) => (0, result.clone(), render(&result, locale)),
        Err(reason) => (
            1,
            json!({"kind":"failed","reason":reason}),
            format!("{}\n", CliText::new(locale).plugin_error(reason)),
        ),
    };
    let document = json!({"schema":"orchbee.plugin.registry","version":1,"source":{"kind":"local-configuration"},"operation":operation,"result":result});
    let encoded = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text if status != 0 => Ok(vec![]),
        OutputFormat::Text => output::encode_text(&text),
    };
    match encoded {
        Ok(stdout) => Output {
            status,
            stdout,
            stderr: if status != 0 { text } else { String::new() },
            finalization: None,
        },
        Err(_) => failure_output(
            Failure::new(FailureKind::OutputTooLarge),
            Some(format),
            locale,
        ),
    }
}
fn execute(
    registry: &PluginRegistry,
    command: RegistryCommand,
) -> Result<Value, PluginRegistryError> {
    Ok(match command {
        RegistryCommand::ConfigDir(id) => {
            json!({"kind":"directory","pluginId":id,"path":registry.config_dir(&id)?.to_str().ok_or(PluginRegistryError::InvalidPath)?})
        }
        RegistryCommand::List(id) => {
            json!({"kind":"catalog","catalog":registry.list(id.as_deref())?})
        }
        RegistryCommand::Mutate(mutation) => {
            json!({"kind":"catalog","catalog":registry.mutate(mutation)?})
        }
        RegistryCommand::Actions(id) => {
            let catalog = registry.list(id.as_deref())?;
            let mut actions = Vec::new();
            for entry in catalog.plugins {
                if !entry.enabled || entry.warning.is_some() {
                    continue;
                }
                for action in &entry.manifest.actions {
                    if entry.manifest.supports_windows(&action.platforms) {
                        actions.push(json!({"pluginId":entry.manifest.id,"action":action}));
                    }
                }
            }
            json!({"kind":"actions","revision":catalog.revision,"actions":actions})
        }
    })
}
fn render(result: &Value, locale: Locale) -> String {
    let string = |value: &Value| super::escape::escape(value.as_str().unwrap_or(""));
    if result["kind"] == "directory" {
        return format!("{}\n", string(&result["path"]));
    }
    let mut lines = vec![CliText::new(locale).plugin_result_heading().to_owned()];
    if let Some(entries) = result["catalog"]["plugins"].as_array() {
        for entry in entries {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                string(&entry["manifest"]["id"]),
                string(&entry["manifest"]["name"]),
                string(&entry["manifest"]["version"]),
                CliText::new(locale).plugin_enabled(entry["enabled"] == true)
            ));
            if let Ok(reason) =
                serde_json::from_value::<PluginRegistryError>(entry["warning"].clone())
            {
                lines.push(format!("  {}", CliText::new(locale).plugin_error(reason)));
            }
        }
    }
    if let Some(actions) = result["actions"].as_array() {
        for entry in actions {
            lines.push(format!(
                "{}.{}\t{}",
                string(&entry["pluginId"]),
                string(&entry["action"]["id"]),
                string(&entry["action"]["title"])
            ));
        }
    }
    lines.join("\n") + "\n"
}
