//! Windows retained-Panel relocation grammar, separate from legacy same-Tab placement.
use super::{
    FailureEnvelope, Options, OutputFormat, PANEL_MOVE_SCOPE, ParseFailure, Request, Resource,
    Scope, TabContext, Target, UsageContext, UsageReason, WorkbenchLabel,
    WorkbenchPanelSplitDirection, check_scope, client_control, missing_required, panel_direction,
    target, usage_with, wait_timeout_option,
};
use orchbee_workbench_protocol::{ClientSplitRatio, WorkbenchClientId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PanelRelocationCommand {
    pub(crate) source: Target,
    pub(crate) destination: PanelRelocationDestination,
    pub(crate) client: Option<WorkbenchClientId>,
    pub(crate) focus: bool,
    pub(crate) timeout_ms: u32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PanelRelocationDestination {
    Tab {
        tab: Target,
        panel: Option<Target>,
        direction: WorkbenchPanelSplitDirection,
        ratio: Option<ClientSplitRatio>,
    },
    NewTab {
        context: Option<TabContext>,
        label: Option<WorkbenchLabel>,
    },
    NewWorkspace {
        project: Option<Target>,
        label: Option<WorkbenchLabel>,
        tab_label: Option<WorkbenchLabel>,
    },
}
impl PanelRelocationCommand {
    pub(crate) fn ratio(&self) -> Option<ClientSplitRatio> {
        match self.destination {
            PanelRelocationDestination::Tab { ratio, .. } => ratio,
            _ => None,
        }
    }
}

pub(super) fn parse(
    value: &str,
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("panel move");
    check_scope(
        options,
        Scope {
            new_workspace: true,
            tab_label: true,
            workspace: true,
            standalone: true,
            project: true,
            ratio: true,
            focus: true,
            no_focus: true,
            client: true,
            timeout: true,
            ..PANEL_MOVE_SCOPE
        },
        context,
        envelope,
    )?;
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
    let modes: Vec<_> = [
        options.into_tab.is_some().then_some("--into-tab"),
        options.new_tab.then_some("--new-tab"),
        options.new_workspace.then_some("--new-workspace"),
    ]
    .into_iter()
    .flatten()
    .collect();
    let mode = match modes.as_slice() {
        [mode] => *mode,
        [first, second, ..] => return Err(conflict(first, second)),
        _ => {
            return Err(missing_required(
                context,
                "'--into-tab', '--new-tab', or '--new-workspace'",
                envelope,
            ));
        }
    };
    if options.focus && options.no_focus {
        return Err(conflict("--focus", "--no-focus"));
    }
    if options.standalone && options.workspace.is_some() {
        return Err(conflict("--standalone", "--workspace"));
    }
    let invalid = [
        (
            "--direction",
            options.direction.is_some() && mode != "--into-tab",
        ),
        (
            "--target-panel",
            options.target_panel.is_some() && mode != "--into-tab",
        ),
        ("--ratio", options.ratio.is_some() && mode != "--into-tab"),
        ("--label", options.label.is_some() && mode == "--into-tab"),
        (
            "--workspace",
            options.workspace.is_some() && mode != "--new-tab",
        ),
        ("--standalone", options.standalone && mode != "--new-tab"),
        (
            "--project",
            options.project.is_some() && mode != "--new-workspace",
        ),
        (
            "--tab-label",
            options.tab_label.is_some() && mode != "--new-workspace",
        ),
    ];
    if let Some((flag, _)) = invalid.iter().find(|(_, present)| *present) {
        return Err(conflict(mode, flag));
    }
    let parse_target = |value: &Option<String>, resource| {
        value
            .as_ref()
            .map(|value| target(value.clone(), resource, envelope))
            .transpose()
    };
    let label = |value: &Option<String>, option| {
        value
            .as_ref()
            .map(|value| {
                WorkbenchLabel::parse(value).map_err(|_| {
                    usage_with(
                        UsageReason::InvalidValue {
                            context,
                            option,
                            value: value.clone(),
                            expected: super::ExpectedValue::Label,
                        },
                        envelope,
                    )
                })
            })
            .transpose()
    };
    let destination = match mode {
        "--into-tab" => PanelRelocationDestination::Tab {
            tab: parse_target(&options.into_tab, Resource::Tab)?.expect("selected destination"),
            panel: parse_target(&options.target_panel, Resource::Panel)?,
            direction: panel_direction(options, context, envelope)?,
            ratio: options
                .ratio
                .as_deref()
                .map(|value| {
                    let value = client_control::finite_number(value, "--ratio", context, envelope)?;
                    Ok(ClientSplitRatio::new(value.clamp(0.1, 0.9)).expect("bounded finite ratio"))
                })
                .transpose()?,
        },
        "--new-tab" => PanelRelocationDestination::NewTab {
            context: if options.standalone {
                Some(TabContext::Standalone)
            } else {
                parse_target(&options.workspace, Resource::Workspace)?.map(TabContext::Workspace)
            },
            label: label(&options.label, "--label")?,
        },
        _ => PanelRelocationDestination::NewWorkspace {
            project: parse_target(&options.project, Resource::Project)?,
            label: label(&options.label, "--label")?,
            tab_label: label(&options.tab_label, "--tab-label")?,
        },
    };
    Ok(Request::PanelRelocation {
        command: PanelRelocationCommand {
            source: target(value.to_owned(), Resource::Panel, envelope)?,
            destination,
            client: client_control::client_id(options, context, envelope)?,
            focus: !options.no_focus,
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
