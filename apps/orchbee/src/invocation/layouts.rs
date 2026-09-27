//! Windows portable-layout grammar; no runtime discovery while parsing.
use super::{
    ClientLayoutTarget, FailureEnvelope, NO_OPTIONS, Options, OutputFormat, ParseFailure, Request,
    Resource, Scope, TabContext, Target, UsageContext, UsageReason, WorkbenchLabel, check_scope,
    client_control, label_option, missing_required, target, usage_with, wait_timeout_option,
};
use orchbee_workbench_protocol::WorkbenchClientId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LayoutExportCommand {
    pub(crate) target: ClientLayoutTarget,
    pub(crate) client: Option<WorkbenchClientId>,
    pub(crate) timeout_ms: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// The explicit creation context or confirmed replacement target; never an exported identifier.
pub(crate) enum LayoutDestination {
    New(Option<TabContext>),
    Replace(Target),
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Validated command options; portable JSON admission belongs to the layout input owner.
pub(crate) struct LayoutApplyCommand {
    /// None is the explicitly declared inherited stdin, never an implicit prompt.
    pub(crate) file: Option<String>,
    pub(crate) destination: LayoutDestination,
    pub(crate) label: Option<WorkbenchLabel>,
    pub(crate) focus: bool,
    pub(crate) client: Option<WorkbenchClientId>,
    pub(crate) timeout_ms: u32,
}

pub(super) fn apply(options: &Options, envelope: FailureEnvelope) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("layout apply");
    check_scope(
        options,
        Scope {
            file: true,
            stdin: true,
            tab: true,
            workspace: true,
            standalone: true,
            confirm: true,
            label: true,
            focus: true,
            no_focus: true,
            client: true,
            timeout: true,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    for choices in [
        vec![
            ("--file", options.file.is_some()),
            ("--stdin", options.stdin),
        ],
        vec![
            ("--tab", options.tab.is_some()),
            ("--workspace", options.workspace.is_some()),
            ("--standalone", options.standalone),
        ],
        vec![("--focus", options.focus), ("--no-focus", options.no_focus)],
    ] {
        let present = choices
            .iter()
            .filter(|(_, present)| *present)
            .map(|(name, _)| *name)
            .collect::<Vec<_>>();
        if present.len() > 1 {
            return Err(usage_with(
                UsageReason::ConflictingOptions {
                    context,
                    first: present[0],
                    second: present[1],
                },
                envelope,
            ));
        }
    }
    if options.file.is_none() && !options.stdin {
        return Err(missing_required(context, "'--file' or '--stdin'", envelope));
    }
    if options.tab.is_some() && !options.confirm {
        return Err(missing_required(context, "'--confirm'", envelope));
    }
    if options.confirm && options.tab.is_none() {
        return Err(missing_required(
            context,
            "'--tab' with '--confirm'",
            envelope,
        ));
    }
    let destination = if let Some(tab) = &options.tab {
        LayoutDestination::Replace(target(tab.clone(), Resource::Tab, envelope)?)
    } else if options.standalone {
        LayoutDestination::New(Some(TabContext::Standalone))
    } else {
        LayoutDestination::New(
            options
                .workspace
                .as_ref()
                .map(|value| {
                    target(value.clone(), Resource::Workspace, envelope).map(TabContext::Workspace)
                })
                .transpose()?,
        )
    };
    Ok(Request::LayoutApply {
        command: LayoutApplyCommand {
            file: options.file.clone(),
            destination,
            label: options
                .label
                .as_ref()
                .map(|_| label_option(options, context, envelope))
                .transpose()?,
            focus: options.focus,
            client: client_control::client_id(options, context, envelope)?,
            timeout_ms: wait_timeout_option(options, context, envelope)?,
        },
        format: options.format.unwrap_or(OutputFormat::Text),
    })
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
