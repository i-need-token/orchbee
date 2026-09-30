use super::super::{
    CreationCommand, CreationDirectory, CreationKind, ExpectedValue, FailureEnvelope, NO_OPTIONS,
    Options, OutputFormat, ParseFailure, Request, Resource, Scope, TabContext, UsageContext,
    UsageReason, check_scope, client_control, geometry_option, launch_options_with_directory,
    missing_command, missing_required, target, unexpected_argument, unknown_command, usage_with,
    wait_timeout_option,
};
use orchbee_local_site_runtime::PluginPanePlacement;
use orchbee_workbench_protocol::{PluginPaneSelector, WorkbenchPanelSplitDirection};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Open {
    pub(crate) placement: Option<PluginPanePlacement>,
    pub(crate) active_target_overrides: bool,
    pub(crate) workspace: Option<TabContext>,
    pub(crate) creation: CreationCommand,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Manage {
    pub(crate) target: super::super::Target,
    pub(crate) client: Option<orchbee_workbench_protocol::WorkbenchClientId>,
    pub(crate) focus: bool,
    pub(crate) timeout_ms: u32,
}

pub(super) fn parse(
    words: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    if let ["plugin", "pane", verb @ ("focus" | "close"), rest @ ..] = words {
        let focus = *verb == "focus";
        let context = UsageContext::Path(if focus {
            "plugin pane focus"
        } else {
            "plugin pane close"
        });
        check_scope(
            options,
            Scope {
                client: focus,
                timeout: true,
                ..NO_OPTIONS
            },
            context,
            envelope,
        )?;
        let value = rest
            .first()
            .ok_or_else(|| missing_required(context, "<pane-id>", envelope))?;
        if let Some(extra) = rest.get(1) {
            return Err(unexpected_argument(context, extra, envelope));
        }
        return Ok(Request::Plugin {
            command: super::PluginCommand::PaneManage(Box::new(Manage {
                target: target((*value).into(), Resource::Panel, envelope)?,
                focus,
                client: client_control::client_id(options, context, envelope)?,
                timeout_ms: if options.timeout.is_some() {
                    wait_timeout_option(options, context, envelope)?
                } else {
                    30_000
                },
            })),
            format: options.format.unwrap_or(OutputFormat::Text),
        });
    }
    let context = UsageContext::Path("plugin pane open");
    match words {
        ["plugin", "pane"] => {
            return Err(missing_command(UsageContext::Path("plugin pane"), envelope));
        }
        ["plugin", "pane", "open"] => {}
        ["plugin", "pane", "open", extra, ..] => {
            return Err(unexpected_argument(context, extra, envelope));
        }
        ["plugin", "pane", verb, ..] => {
            return Err(unknown_command(
                UsageContext::Path("plugin pane"),
                verb,
                envelope,
            ));
        }
        _ => unreachable!(),
    }
    check_scope(
        options,
        Scope {
            plugin_select: true,
            plugin_pane: true,
            workspace: true,
            direction: true,
            client: true,
            focus: true,
            no_focus: true,
            geometry: true,
            cwd: true,
            environment: true,
            timeout: true,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
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
    let plugin_id = options
        .plugin_id
        .clone()
        .ok_or_else(|| missing_required(context, "--plugin", envelope))?;
    let entrypoint_id = options
        .plugin_entrypoint
        .clone()
        .ok_or_else(|| missing_required(context, "--entrypoint", envelope))?;
    let selector = PluginPaneSelector::new(plugin_id, entrypoint_id).map_err(|_| {
        invalid(
            "--entrypoint",
            options.plugin_entrypoint.as_deref().unwrap_or_default(),
            ExpectedValue::PluginActionId,
        )
    })?;
    let placement = options
        .plugin_placement
        .as_deref()
        .map(|value| match value {
            "tab" => Ok(PluginPanePlacement::Tab),
            "split" => Ok(PluginPanePlacement::Split),
            "zoomed" => Ok(PluginPanePlacement::Zoomed),
            "overlay" => Ok(PluginPanePlacement::Overlay),
            "popup" => Ok(PluginPanePlacement::Popup),
            _ => Err(invalid(
                "--placement",
                value,
                ExpectedValue::PluginPanePlacement,
            )),
        })
        .transpose()?;
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
    let direction = match options.direction.as_deref().unwrap_or("right") {
        "right" => WorkbenchPanelSplitDirection::Right,
        "down" => WorkbenchPanelSplitDirection::Below,
        value => {
            return Err(invalid(
                "--direction",
                value,
                ExpectedValue::PluginPaneDirection,
            ));
        }
    };
    let mut launch_options =
        launch_options_with_directory(options, options.cwd.as_deref(), context, envelope)?;
    launch_options.plugin_pane = Some(selector);
    Ok(Request::Plugin {
        command: super::PluginCommand::PaneOpen(Box::new(Open {
            placement,
            active_target_overrides: options.workspace.is_some()
                || options.plugin_target.is_some()
                || options.direction.is_some(),
            workspace: options
                .workspace
                .clone()
                .map(|id| target(id, Resource::Workspace, envelope).map(TabContext::Workspace))
                .transpose()?,
            creation: CreationCommand {
                directory: if options.cwd.is_some() {
                    CreationDirectory::Explicit
                } else {
                    CreationDirectory::Default
                },
                kind: CreationKind::Split {
                    right_click: Default::default(),
                    target: options
                        .plugin_target
                        .clone()
                        .map(|id| target(id, Resource::Panel, envelope))
                        .transpose()?,
                    direction,
                    ratio: None,
                },
                label: None,
                geometry: geometry_option(options, context, envelope)?,
                launch: None,
                profile: None,
                options: launch_options,
                client: client_control::client_id(options, context, envelope)?,
                focus: !options.no_focus,
                zoom: false,
                overlay: false,
                timeout_ms: if options.timeout.is_some() {
                    wait_timeout_option(options, context, envelope)?
                } else {
                    30_000
                },
            },
        })),
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(args: &[&str]) -> Result<Request, ParseFailure> {
        crate::invocation::parse_request(args.iter().map(std::ffi::OsString::from))
    }
    #[test]
    fn pane_open_defaults_and_scoped_launch_overrides() {
        let Request::Plugin {
            command: super::super::PluginCommand::PaneOpen(open),
            ..
        } = request(&[
            "plugin",
            "pane",
            "open",
            "--plugin",
            "tools",
            "--entrypoint",
            "dashboard",
            "--placement",
            "split",
            "--direction",
            "down",
            "--no-focus",
            "--env",
            "USER_INPUT=hello world",
        ])
        .unwrap()
        else {
            panic!()
        };
        assert!(!open.creation.focus);
        assert_eq!(
            open.creation.options.environment.entries(),
            &[("USER_INPUT".into(), "hello world".into())]
        );
        assert!(matches!(
            open.creation.kind,
            CreationKind::Split {
                direction: WorkbenchPanelSplitDirection::Below,
                ..
            }
        ));
        let base = [
            "plugin",
            "pane",
            "open",
            "--plugin",
            "tools",
            "--entrypoint",
            "dashboard",
        ];
        for invalid in [
            vec!["--exec", "C:/fake.exe"],
            vec!["--env", "ORCHBEE_PLUGIN_ID=wrong"],
            vec!["--focus", "--no-focus"],
            vec!["--direction", "left"],
            vec!["--placement", "floating"],
            vec!["unexpected"],
        ] {
            let mut args = base.to_vec();
            args.extend(invalid);
            assert!(request(&args).is_err(), "{args:?}");
        }
        assert!(request(&["plugin", "pane", "open", "--plugin", "tools"]).is_err());
        assert!(
            request(&[
                "panel",
                "split",
                "--target-pane",
                "00000000000000000000000000000001"
            ])
            .is_err()
        );
    }
}
