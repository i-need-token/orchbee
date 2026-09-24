//! Client discovery and exact, acknowledged navigation grammar.
use super::{
    ExpectedValue, FailureEnvelope, NO_OPTIONS, Options, OutputFormat, ParseFailure, Request,
    Resource, Scope, Target, UsageContext, UsageReason, check_scope, missing_required, target,
    usage_with, wait_timeout_option,
};
use orchbee_workbench_protocol::{ClientPanelDirection, WorkbenchClientId, WorkbenchRequestId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ZoomMode {
    Toggle,
    On,
    Off,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LayoutQuery {
    Layout,
    Neighbor(ClientPanelDirection),
    Edges,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ClientNavigationTarget {
    Layout {
        target: Option<Target>,
        query: LayoutQuery,
    },
    Workspace(Target),
    Tab(Target),
    Panel(Option<Target>),
    FocusDirection {
        target: Option<Target>,
        direction: ClientPanelDirection,
    },
    Zoom {
        target: Option<Target>,
        mode: ZoomMode,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ClientCommand {
    List,
    Get(WorkbenchClientId),
    ControlStatus(WorkbenchRequestId),
    Navigate {
        target: ClientNavigationTarget,
        client: Option<WorkbenchClientId>,
        operation_id: Option<WorkbenchRequestId>,
        detach: bool,
        timeout_ms: u32,
    },
}
fn invalid(
    value: &str,
    option: &'static str,
    expected: ExpectedValue,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> ParseFailure {
    usage_with(
        UsageReason::InvalidValue {
            context,
            option,
            value: value.into(),
            expected,
        },
        envelope,
    )
}
pub(super) fn parse(
    words: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let command = match words {
        ["client"] => {
            return Err(usage_with(
                UsageReason::MissingCommand {
                    context: UsageContext::Path("client"),
                },
                envelope,
            ));
        }
        ["client", "list", extra, ..] => {
            return Err(usage_with(
                UsageReason::UnexpectedArgument {
                    context: UsageContext::Path("client list"),
                    value: (*extra).into(),
                },
                envelope,
            ));
        }
        ["client", verb @ ("get" | "control-status"), _, extra, ..] => {
            return Err(usage_with(
                UsageReason::UnexpectedArgument {
                    context: UsageContext::Path(if *verb == "get" {
                        "client get"
                    } else {
                        "client control-status"
                    }),
                    value: (*extra).into(),
                },
                envelope,
            ));
        }

        ["client", "list"] => {
            check_scope(
                options,
                NO_OPTIONS,
                UsageContext::Path("client list"),
                envelope,
            )?;
            ClientCommand::List
        }
        ["client", "get", value] => {
            let context = UsageContext::Path("client get");
            check_scope(options, NO_OPTIONS, context, envelope)?;
            ClientCommand::Get(WorkbenchClientId::parse(value).map_err(|_| {
                invalid(
                    value,
                    "<client>",
                    ExpectedValue::ClientId,
                    context,
                    envelope,
                )
            })?)
        }
        ["client", "control-status", value] => {
            let context = UsageContext::Path("client control-status");
            check_scope(options, NO_OPTIONS, context, envelope)?;
            ClientCommand::ControlStatus(WorkbenchRequestId::parse(value).map_err(|_| {
                invalid(
                    value,
                    "<operation>",
                    ExpectedValue::ClientOperationId,
                    context,
                    envelope,
                )
            })?)
        }
        [
            family @ ("workspace" | "tab" | "panel"),
            verb @ ("focus" | "zoom" | "layout" | "neighbor" | "edges"),
            rest @ ..,
        ] => {
            let panel = *family == "panel";
            let zoom = *verb == "zoom";
            let path = match (*family, *verb) {
                ("workspace", "focus") => "workspace focus",
                ("tab", "focus") => "tab focus",
                ("panel", "focus") => "panel focus",
                ("panel", "zoom") => "panel zoom",
                ("panel", "layout") => "panel layout",
                ("panel", "neighbor") => "panel neighbor",
                ("panel", "edges") => "panel edges",
                _ => {
                    return Err(usage_with(
                        UsageReason::UnknownCommand {
                            context: UsageContext::Root,
                            word: words.join(" "),
                        },
                        envelope,
                    ));
                }
            };
            let context = UsageContext::Path(path);
            check_scope(
                options,
                Scope {
                    direction: *verb == "neighbor" || (panel && *verb == "focus"),
                    client: true,
                    request_id: true,
                    detach: true,
                    timeout: true,
                    panel,
                    current: panel,
                    zoom_on: zoom,
                    zoom_off: zoom,
                    zoom_toggle: zoom,
                    ..NO_OPTIONS
                },
                context,
                envelope,
            )?;
            if rest.len() > 1 {
                return Err(usage_with(
                    UsageReason::UnexpectedArgument {
                        context,
                        value: rest[1].into(),
                    },
                    envelope,
                ));
            }
            let sources = [
                ("<target>", !rest.is_empty()),
                ("--panel", options.panel.is_some()),
                ("--current", options.current),
            ];
            let selected: Vec<_> = sources
                .iter()
                .filter(|(_, present)| *present)
                .map(|(name, _)| *name)
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
            let explicit = rest.first().copied().or(options.panel.as_deref());
            let target = match *family {
                "workspace" => ClientNavigationTarget::Workspace(target(
                    explicit
                        .ok_or_else(|| missing_required(context, "<workspace>", envelope))?
                        .into(),
                    Resource::Workspace,
                    envelope,
                )?),
                "tab" => ClientNavigationTarget::Tab(target(
                    explicit
                        .ok_or_else(|| missing_required(context, "<tab>", envelope))?
                        .into(),
                    Resource::Tab,
                    envelope,
                )?),
                _ => {
                    let target = explicit
                        .map(|value| target(value.into(), Resource::Panel, envelope))
                        .transpose()?;
                    if matches!(*verb, "layout" | "neighbor" | "edges") {
                        let query = match *verb {
                            "layout" => LayoutQuery::Layout,
                            "edges" => LayoutQuery::Edges,
                            _ => {
                                let value = options.direction.as_deref().ok_or_else(|| {
                                    missing_required(context, "--direction", envelope)
                                })?;
                                let direction = panel_direction(value, context, envelope)?;
                                LayoutQuery::Neighbor(direction)
                            }
                        };
                        ClientNavigationTarget::Layout { target, query }
                    } else if zoom {
                        let selected: Vec<_> = [
                            ("--on", options.zoom_on),
                            ("--off", options.zoom_off),
                            ("--toggle", options.zoom_toggle),
                        ]
                        .into_iter()
                        .filter(|(_, present)| *present)
                        .map(|(name, _)| name)
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
                        ClientNavigationTarget::Zoom {
                            target,
                            mode: if options.zoom_on {
                                ZoomMode::On
                            } else if options.zoom_off {
                                ZoomMode::Off
                            } else {
                                ZoomMode::Toggle
                            },
                        }
                    } else if let Some(value) = options.direction.as_deref() {
                        ClientNavigationTarget::FocusDirection {
                            target,
                            direction: panel_direction(value, context, envelope)?,
                        }
                    } else {
                        ClientNavigationTarget::Panel(target)
                    }
                }
            };
            let client = options
                .client
                .as_deref()
                .map(|value| {
                    WorkbenchClientId::parse(value).map_err(|_| {
                        invalid(
                            value,
                            "--client",
                            ExpectedValue::ClientId,
                            context,
                            envelope,
                        )
                    })
                })
                .transpose()?;
            let operation_id = options
                .request_id
                .as_deref()
                .map(|value| {
                    WorkbenchRequestId::parse(value).map_err(|_| {
                        invalid(
                            value,
                            "--request-id",
                            ExpectedValue::ClientOperationId,
                            context,
                            envelope,
                        )
                    })
                })
                .transpose()?;
            ClientCommand::Navigate {
                target,
                client,
                operation_id,
                detach: options.detach,
                timeout_ms: wait_timeout_option(options, context, envelope)?,
            }
        }
        ["client", verb] if *verb == "get" || *verb == "control-status" => {
            return Err(missing_required(
                UsageContext::Path(if *verb == "get" {
                    "client get"
                } else {
                    "client control-status"
                }),
                if *verb == "get" {
                    "<client>"
                } else {
                    "<operation>"
                },
                envelope,
            ));
        }
        _ => {
            return Err(usage_with(
                UsageReason::UnknownCommand {
                    context: UsageContext::Path("client"),
                    word: words.join(" "),
                },
                envelope,
            ));
        }
    };
    Ok(Request::Client {
        command,
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

fn panel_direction(
    value: &str,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<ClientPanelDirection, ParseFailure> {
    match value {
        "left" => Ok(ClientPanelDirection::Left),
        "right" => Ok(ClientPanelDirection::Right),
        "up" => Ok(ClientPanelDirection::Up),
        "down" => Ok(ClientPanelDirection::Down),
        _ => Err(invalid(
            value,
            "--direction",
            ExpectedValue::LayoutDirection,
            context,
            envelope,
        )),
    }
}
