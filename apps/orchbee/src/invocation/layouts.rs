//! Windows portable-layout grammar; no runtime discovery while parsing.
use super::{
    ClientLayoutTarget, FailureEnvelope, NO_OPTIONS, Options, OutputFormat, ParseFailure, Request,
    Resource, Scope, UsageContext, UsageReason, check_scope, client_control, target, usage_with,
    wait_timeout_option,
};
use orchbee_workbench_protocol::WorkbenchClientId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LayoutExportCommand {
    pub(crate) target: ClientLayoutTarget,
    pub(crate) client: Option<WorkbenchClientId>,
    pub(crate) timeout_ms: u32,
}

pub(super) fn export(
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("layout export");
    check_scope(
        options,
        Scope {
            tab: true,
            panel: true,
            current: true,
            client: true,
            timeout: true,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    let selected: Vec<_> = [
        ("--tab", options.tab.is_some()),
        ("--panel", options.panel.is_some()),
        ("--current", options.current),
    ]
    .into_iter()
    .filter_map(|(flag, present)| present.then_some(flag))
    .collect();
    if selected.len() > 1 {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: selected[0],
                second: selected[1],
            },
            envelope,
        ));
    }
    let target = if let Some(tab) = &options.tab {
        ClientLayoutTarget::Tab(target(tab.clone(), Resource::Tab, envelope)?)
    } else {
        ClientLayoutTarget::Panel(
            options
                .panel
                .as_ref()
                .map(|panel| target(panel.clone(), Resource::Panel, envelope))
                .transpose()?,
        )
    };
    Ok(Request::LayoutExport {
        command: LayoutExportCommand {
            target,
            client: client_control::client_id(options, context, envelope)?,
            timeout_ms: wait_timeout_option(options, context, envelope)?,
        },
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}
