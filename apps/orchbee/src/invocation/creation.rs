//! Optional creation inputs, before any canonical target or Client is resolved.
use super::{
    FailureEnvelope, LaunchOptions, LaunchProfileSelector, LaunchSelection, Options, OutputFormat,
    PANEL_SPLIT_SCOPE, ParseFailure, Request, Resource, TAB_CREATE_SCOPE, TabContext, Target,
    UsageContext, UsageReason, WorkbenchLabel, WorkbenchPanelSplitDirection,
    WorkbenchTerminalGeometry, check_scope, client_control, geometry_option, label_option,
    launch_options_with_directory, panel_direction, profile_option, reject_launch_profile_conflict,
    target, unexpected_argument, usage_with, wait_timeout_option,
};
use orchbee_workbench_protocol::{ClientSplitRatio, WorkbenchClientId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CreationKind {
    Tab {
        context: Option<TabContext>,
    },
    Split {
        right_click: orchbee_workbench_protocol::PanelRightClickTarget,
        target: Option<Target>,
        direction: WorkbenchPanelSplitDirection,
        ratio: Option<ClientSplitRatio>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CreationCommand<L = LaunchSelection> {
    pub(crate) directory: CreationDirectory,
    pub(crate) kind: CreationKind,
    pub(crate) label: Option<WorkbenchLabel>,
    pub(crate) geometry: WorkbenchTerminalGeometry,
    pub(crate) launch: Option<L>,
    pub(crate) profile: Option<LaunchProfileSelector>,
    pub(crate) options: LaunchOptions,
    pub(crate) client: Option<WorkbenchClientId>,
    pub(crate) focus: bool,
    pub(crate) zoom: bool,
    pub(crate) overlay: bool,
    pub(crate) timeout_ms: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CreationDirectory {
    Default,
    Follow,
    Home,
    Current,
    Workspace,
    Explicit,
}

pub(super) fn parse(
    split_operands: Option<&[&str]>,
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path(if split_operands.is_some() {
        "panel split"
    } else {
        "tab create"
    });
    check_scope(
        options,
        if split_operands.is_some() {
            PANEL_SPLIT_SCOPE
        } else {
            TAB_CREATE_SCOPE
        },
        context,
        envelope,
    )?;
    if options.focus && options.no_focus {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--focus",
                second: "--no-focus",
            },
            envelope,
        ));
    }
    reject_launch_profile_conflict(options, context, envelope)?;
    let kind = if let Some(operands) = split_operands {
        if let [_, extra, ..] = operands {
            return Err(unexpected_argument(context, extra, envelope));
        }
        let selectors: Vec<_> = [
            ("panel-id", !operands.is_empty()),
            ("--panel", options.panel.is_some()),
            ("--current", options.current),
        ]
        .into_iter()
        .filter(|(_, present)| *present)
        .map(|(name, _)| name)
        .collect();
        if selectors.len() > 1 {
            return Err(usage_with(
                UsageReason::ConflictingOptions {
                    context,
                    first: selectors[0],
                    second: selectors[1],
                },
                envelope,
            ));
        }
        let target = operands
            .first()
            .map(|value| (*value).to_owned())
            .or_else(|| options.panel.clone())
            .map(|value| target(value, Resource::Panel, envelope))
            .transpose()?;
        let ratio = options
            .ratio
            .as_deref()
            .map(|value| {
                let number = client_control::finite_number(value, "--ratio", context, envelope)?;
                Ok(ClientSplitRatio::new(number.clamp(0.1, 0.9)).expect("normalized finite ratio"))
            })
            .transpose()?;
        CreationKind::Split {
            right_click: options
                .right_click
                .as_deref()
                .map(|value| super::panel_input::policy(value, context, envelope))
                .transpose()?
                .unwrap_or_default(),
            target,
            direction: panel_direction(options, context, envelope)?,
            ratio,
        }
    } else {
        let context = options
            .workspace
            .clone()
            .map(|value| target(value, Resource::Workspace, envelope).map(TabContext::Workspace))
            .transpose()?;
        CreationKind::Tab { context }
    };
    let directory = match options.cwd.as_deref() {
        None => CreationDirectory::Default,
        Some("follow") => CreationDirectory::Follow,
        Some("home") => CreationDirectory::Home,
        Some("current") => CreationDirectory::Current,
        Some("workspace") => CreationDirectory::Workspace,
        Some(_) => CreationDirectory::Explicit,
    };
    Ok(Request::Creation {
        command: CreationCommand {
            directory,
            kind,
            label: options
                .label
                .as_ref()
                .map(|_| label_option(options, context, envelope))
                .transpose()?,
            geometry: geometry_option(options, context, envelope)?,
            launch: super::program::parse(options, context, envelope)?,
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
            client: client_control::client_id(options, context, envelope)?,
            focus: options.focus,
            zoom: false,
            overlay: false,
            timeout_ms: if options.timeout.is_some() {
                wait_timeout_option(options, context, envelope)?
            } else {
                30_000
            },
        },
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

impl CreationCommand<super::program::ParsedLaunch> {
    pub(crate) fn into_resolved(
        self,
        format: OutputFormat,
    ) -> Result<CreationCommand, ParseFailure> {
        let context = UsageContext::Path(match self.kind {
            CreationKind::Tab { .. } => "tab create",
            CreationKind::Split { .. } => "panel split",
        });
        Ok(CreationCommand {
            directory: self.directory,
            kind: self.kind,
            label: self.label,
            geometry: self.geometry,
            launch: super::program::resolve(
                self.launch,
                context,
                super::selection_envelope(Some(format)),
            )?,
            profile: self.profile,
            options: self.options,
            client: self.client,
            focus: self.focus,
            zoom: self.zoom,
            overlay: self.overlay,
            timeout_ms: self.timeout_ms,
        })
    }
}
