//! Installed command IDs and hierarchical source selectors, parsed before effects.
use super::{
    ExpectedValue, FailureEnvelope, Options, OutputFormat, ParseFailure, Request, Resource, Target,
    UsageContext, UsageReason, check_scope, missing_command, missing_required, target,
    unknown_command, usage_with, wait_timeout_option,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Command {
    List,
    Invoke(Box<Invocation>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Invocation {
    pub id: String,
    pub workspace: Option<Target>,
    pub tab: Option<Target>,
    pub panel: Option<Target>,
    pub client: Option<orchbee_workbench_protocol::WorkbenchClientId>,
    pub context: Box<orchbee_workbench_protocol::PluginInvocationContext>,
    pub timeout_ms: u32,
    pub selection: Option<orchbee_workbench_protocol::ConfiguredCommandSelection>,
}

pub(super) fn parse(
    words: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let (verb, args) = match words {
        ["command"] => return Err(missing_command(UsageContext::Path("command"), envelope)),
        ["command", verb, args @ ..] if matches!(*verb, "list" | "invoke") => (*verb, args),
        ["command", other, ..] => {
            return Err(unknown_command(
                UsageContext::Path("command"),
                other,
                envelope,
            ));
        }
        _ => unreachable!("command family routed here"),
    };
    let usage = UsageContext::Path(if verb == "list" {
        "command list"
    } else {
        "command invoke"
    });
    let invoke = verb == "invoke";
    check_scope(options, usage, envelope)?;
    if args.len() > usize::from(invoke) {
        return Err(usage_with(
            UsageReason::UnexpectedArgument {
                context: usage,
                value: args[usize::from(invoke)].into(),
            },
            envelope,
        ));
    }
    let command = if invoke {
        let id = args
            .first()
            .ok_or_else(|| missing_required(usage, "<command-id>", envelope))?;
        let invalid = |option, value: &str, expected| {
            usage_with(
                UsageReason::InvalidValue {
                    context: usage,
                    option,
                    value: value.into(),
                    expected,
                },
                envelope,
            )
        };
        if id.len() > 120
            || !id.starts_with("cmd_")
            || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            return Err(invalid(
                "<command-id>",
                id,
                ExpectedValue::ConfiguredCommandId,
            ));
        }
        let context: orchbee_workbench_protocol::PluginInvocationContext = options
            .plugin_context
            .as_deref()
            .map(|raw| {
                if raw.len() > 16 * 1024 {
                    return Err(invalid("--context", raw, ExpectedValue::PluginContext));
                }
                serde_json::from_str(raw)
                    .map_err(|_| invalid("--context", raw, ExpectedValue::PluginContext))
            })
            .transpose()?
            .unwrap_or_default();
        context.validate().map_err(|_| {
            invalid(
                "--context",
                options.plugin_context.as_deref().unwrap_or(""),
                ExpectedValue::PluginContext,
            )
        })?;
        let selection = if options.selection_anchor.is_some()
            || options.cursor.is_some()
            || options.content_revision.is_some()
        {
            let anchor = super::terminal_buffer::point(
                options.selection_anchor.as_deref(),
                "--anchor",
                usage,
                envelope,
            )?;
            let cursor = super::terminal_buffer::point(
                options.cursor.as_deref(),
                "--cursor",
                usage,
                envelope,
            )?;
            let raw = options
                .content_revision
                .as_deref()
                .ok_or_else(|| missing_required(usage, "--content-revision", envelope))?;
            let revision =
                super::terminal_buffer::number(raw, "--content-revision", false, usage, envelope)?;
            let selection = orchbee_workbench_protocol::ConfiguredCommandSelection {
                anchor: orchbee_workbench_protocol::ConfiguredCommandTextPoint {
                    row: orchbee_workbench_protocol::WorkbenchRevision::new(u64::from(anchor.row)),
                    col: anchor.col,
                },
                cursor: orchbee_workbench_protocol::ConfiguredCommandTextPoint {
                    row: orchbee_workbench_protocol::WorkbenchRevision::new(u64::from(cursor.row)),
                    col: cursor.col,
                },
                revision: orchbee_workbench_protocol::WorkbenchRevision::new(revision),
            };
            selection.validate().map_err(|_| {
                invalid(
                    "--cursor",
                    options.cursor.as_deref().unwrap_or(""),
                    ExpectedValue::TerminalSelectionSpan,
                )
            })?;
            Some(selection)
        } else {
            None
        };
        Command::Invoke(Box::new(Invocation {
            id: (*id).into(),
            workspace: options
                .workspace
                .as_ref()
                .map(|value| target(value.clone(), Resource::Workspace, envelope))
                .transpose()?,
            tab: options
                .tab
                .as_ref()
                .map(|value| target(value.clone(), Resource::Tab, envelope))
                .transpose()?,
            panel: options
                .panel
                .as_ref()
                .map(|value| target(value.clone(), Resource::Panel, envelope))
                .transpose()?,
            client: super::client_control::client_id(options, usage, envelope)?,
            context: Box::new(context),
            selection,
            timeout_ms: wait_timeout_option(options, usage, envelope)?,
        }))
    } else {
        Command::List
    };
    Ok(Request::ConfiguredCommand {
        command,
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

#[cfg(test)]
mod tests;
