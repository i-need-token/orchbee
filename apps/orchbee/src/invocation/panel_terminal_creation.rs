//! Direct terminal-member launch inputs, fully admitted before runtime discovery.
use super::{
    CreationDirectory, FailureEnvelope, NO_OPTIONS, Options, OutputFormat, ParseFailure, Request,
    Resource, Scope, Target, UsageContext, UsageReason, check_scope, client_control,
    geometry_option, label_option, launch_option, launch_options_with_directory, profile_option,
    reject_launch_profile_conflict, report_target, target, unexpected_argument, usage_with,
    wait_timeout_option,
};
use orchbee_workbench_protocol::{
    LaunchOptions, LaunchProfileSelector, LaunchSelection, TerminalPresentationId,
    WorkbenchClientId, WorkbenchLabel, WorkbenchTerminalGeometry,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Member {
    Primary,
    Existing(TerminalPresentationId),
    Child,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PanelTerminalCreationCommand {
    pub(crate) panel: Option<Target>,
    pub(crate) client: Option<WorkbenchClientId>,
    pub(crate) member: Member,
    pub(crate) label: Option<WorkbenchLabel>,
    pub(crate) geometry: WorkbenchTerminalGeometry,
    pub(crate) launch: Option<LaunchSelection>,
    pub(crate) profile: Option<LaunchProfileSelector>,
    pub(crate) options: LaunchOptions,
    pub(crate) directory: CreationDirectory,
    pub(crate) timeout_ms: u32,
}
pub(super) fn parse(
    operands: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("panel create-terminal");
    check_scope(
        options,
        Scope {
            panel_terminal: true,
            panel: true,
            current: true,
            client: true,
            label: true,
            geometry: true,
            exec: true,
            args: true,
            profile: true,
            cwd: true,
            environment: true,
            timeout: true,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    if let [_, extra, ..] = operands {
        return Err(unexpected_argument(context, extra, envelope));
    }
    let conflict = |first, second| {
        usage_with(
            UsageReason::ConflictingOptions {
                context,
                first,
                second,
            },
            envelope,
        )
    };
    let selectors: Vec<_> = [
        ("panel-id", !operands.is_empty()),
        ("--panel", options.panel.is_some()),
        ("--current", options.current),
    ]
    .into_iter()
    .filter(|(_, yes)| *yes)
    .map(|(name, _)| name)
    .collect();
    if selectors.len() > 1 {
        return Err(conflict(selectors[0], selectors[1]));
    }
    if options.client.is_some() && (!operands.is_empty() || options.panel.is_some()) {
        return Err(conflict("panel-id/--panel", "--client"));
    }
    if options.child && options.report_presentation.is_some() {
        return Err(conflict("--child", "--presentation"));
    }
    if !options.child && options.label.is_some() {
        return Err(super::missing_required(
            context,
            "'--child' with '--label'",
            envelope,
        ));
    }
    reject_launch_profile_conflict(options, context, envelope)?;
    let member = if options.child {
        Member::Child
    } else {
        report_target::parse(options, context, envelope)?.map_or(Member::Primary, Member::Existing)
    };
    let directory = match options.cwd.as_deref() {
        None => CreationDirectory::Default,
        Some("follow") => CreationDirectory::Follow,
        Some("home") => CreationDirectory::Home,
        Some("current") => CreationDirectory::Current,
        Some("workspace") => CreationDirectory::Workspace,
        Some(_) => CreationDirectory::Explicit,
    };
    Ok(Request::PanelTerminalCreation {
        command: PanelTerminalCreationCommand {
            panel: operands
                .first()
                .map(|s| (*s).to_owned())
                .or_else(|| options.panel.clone())
                .map(|value| target(value, Resource::Panel, envelope))
                .transpose()?,
            client: client_control::client_id(options, context, envelope)?,
            member,
            label: options
                .label
                .as_ref()
                .map(|_| label_option(options, context, envelope))
                .transpose()?,
            geometry: geometry_option(options, context, envelope)?,
            launch: launch_option(options, context, envelope)?,
            profile: profile_option(options, context, envelope)?,
            options: launch_options_with_directory(
                options,
                if directory == CreationDirectory::Explicit {
                    options.cwd.as_deref()
                } else {
                    None
                },
                context,
                envelope,
            )?,
            directory,
            timeout_ms: if options.timeout.is_some() {
                wait_timeout_option(options, context, envelope)?
            } else {
                30_000
            },
        },
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

#[cfg(test)]
mod tests;
