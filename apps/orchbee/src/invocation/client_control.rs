//! Client discovery and exact, acknowledged navigation grammar.
use super::{
    ExpectedValue, FailureEnvelope, NO_OPTIONS, Options, OutputFormat, ParseFailure, Request,
    Resource, Scope, Target, UsageContext, UsageReason, check_scope, missing_required, target,
    usage_with, wait_timeout_option,
};
use orchbee_workbench_protocol::{
    ClientPanelDirection, ClientResizeAmount, ClientSplitPath, ClientSplitRatio, WorkbenchClientId,
    WorkbenchRequestId,
};

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
pub(crate) enum ClientLayoutTarget {
    Tab(Target),
    Panel(Option<Target>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ClientNavigationTarget {
    Agent(super::AgentTarget),
    AgentAttention(super::AgentTarget),
    Resize {
        target: Option<Target>,
        direction: ClientPanelDirection,
        amount: ClientResizeAmount,
    },
    SetSplitRatio {
        target: ClientLayoutTarget,
        path: ClientSplitPath,
        ratio: ClientSplitRatio,
    },
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
pub(crate) enum PanelSwapDestination {
    Pair(Target),
    Direction(ClientPanelDirection),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PanelSwapCommand {
    pub(crate) source: Option<Target>,
    pub(crate) destination: PanelSwapDestination,
    pub(crate) client: Option<WorkbenchClientId>,
    pub(crate) timeout_ms: u32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ClientCommand {
    Swap(PanelSwapCommand),
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
        ["agent", verb @ ("focus" | "attention"), rest @ ..] => {
            let context = UsageContext::Path(if *verb == "focus" {
                "agent focus"
            } else {
                "agent attention"
            });
            check_scope(
                options,
                Scope {
                    session: true,
                    name: true,
                    panel: true,
                    client: true,
                    request_id: true,
                    detach: true,
                    timeout: true,
                    ..NO_OPTIONS
                },
                context,
                envelope,
            )?;
            navigation(
                if *verb == "focus" {
                    ClientNavigationTarget::Agent(super::agent_selector(
                        rest, options, context, envelope,
                    )?)
                } else {
                    ClientNavigationTarget::AgentAttention(super::agent_selector(
                        rest, options, context, envelope,
                    )?)
                },
                options,
                context,
                envelope,
            )?
        }
        ["panel", "swap", rest @ ..] => parse_swap(rest, options, envelope)?,
        ["layout", "set-split-ratio"] => parse_ratio(options, envelope)?,
        ["layout"] => {
            return Err(usage_with(
                UsageReason::MissingCommand {
                    context: UsageContext::Path("layout"),
                },
                envelope,
            ));
        }
        ["layout", "set-split-ratio", extra, ..] => {
            return Err(usage_with(
                UsageReason::UnexpectedArgument {
                    context: UsageContext::Path("layout set-split-ratio"),
                    value: (*extra).into(),
                },
                envelope,
            ));
        }
        ["layout", ..] => {
            return Err(usage_with(
                UsageReason::UnknownCommand {
                    context: UsageContext::Path("layout"),
                    word: words.join(" "),
                },
                envelope,
            ));
        }
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
            verb @ ("focus" | "zoom" | "layout" | "neighbor" | "edges" | "resize"),
            rest @ ..,
        ] => {
            let panel = *family == "panel";
            let zoom = *verb == "zoom";
            let path = match (*family, *verb) {
                ("workspace", "focus") => "workspace focus",
                ("tab", "focus") => "tab focus",
                ("panel", "focus") => "panel focus",
                ("panel", "resize") => "panel resize",
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
                    direction: matches!(*verb, "neighbor" | "resize")
                        || (panel && *verb == "focus"),
                    amount: *verb == "resize",
                    client: true,
                    request_id: !matches!(*verb, "layout" | "neighbor" | "edges"),
                    detach: !matches!(*verb, "layout" | "neighbor" | "edges"),
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
                    if *verb == "resize" {
                        let direction = panel_direction(
                            options.direction.as_deref().ok_or_else(|| {
                                missing_required(context, "--direction", envelope)
                            })?,
                            context,
                            envelope,
                        )?;
                        let amount = finite_number(
                            options.amount.as_deref().unwrap_or("0.05"),
                            "--amount",
                            context,
                            envelope,
                        )?
                        .abs()
                        .min(0.5);
                        ClientNavigationTarget::Resize {
                            target,
                            direction,
                            amount: ClientResizeAmount::new(amount)
                                .expect("normalized finite amount"),
                        }
                    } else if matches!(*verb, "layout" | "neighbor" | "edges") {
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
            navigation(target, options, context, envelope)?
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

fn navigation(
    target: ClientNavigationTarget,
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<ClientCommand, ParseFailure> {
    let client = client_id(options, context, envelope)?;
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
    Ok(ClientCommand::Navigate {
        target,
        client,
        operation_id,
        detach: options.detach,
        timeout_ms: wait_timeout_option(options, context, envelope)?,
    })
}

pub(super) fn finite_number(
    value: &str,
    option: &'static str,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<f64, ParseFailure> {
    value
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
        .ok_or_else(|| {
            invalid(
                value,
                option,
                ExpectedValue::FiniteNumber,
                context,
                envelope,
            )
        })
}

fn parse_ratio(
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<ClientCommand, ParseFailure> {
    let context = UsageContext::Path("layout set-split-ratio");
    check_scope(
        options,
        Scope {
            tab: true,
            panel: true,
            current: true,
            path: true,
            ratio: true,
            client: true,
            request_id: true,
            detach: false,
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
    .filter(|(_, present)| *present)
    .map(|(flag, _)| flag)
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
    let value = options
        .path
        .as_deref()
        .ok_or_else(|| missing_required(context, "--path", envelope))?;
    let path = if value == "root" {
        Some(vec![])
    } else {
        value
            .split('.')
            .map(|step| match step {
                "0" => Some(0),
                "1" => Some(1),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
    }
    .and_then(|steps| ClientSplitPath::new(steps).ok())
    .ok_or_else(|| invalid(value, "--path", ExpectedValue::SplitPath, context, envelope))?;
    let value = options
        .ratio
        .as_deref()
        .ok_or_else(|| missing_required(context, "--ratio", envelope))?;
    let ratio =
        ClientSplitRatio::new(finite_number(value, "--ratio", context, envelope)?.clamp(0.1, 0.9))
            .expect("normalized finite ratio");
    navigation(
        ClientNavigationTarget::SetSplitRatio {
            target,
            path,
            ratio,
        },
        options,
        context,
        envelope,
    )
}

pub(super) fn client_id(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<WorkbenchClientId>, ParseFailure> {
    options
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
        .transpose()
}

fn parse_swap(
    rest: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<ClientCommand, ParseFailure> {
    let context = UsageContext::Path("panel swap");
    check_scope(
        options,
        Scope {
            panel: true,
            current: true,
            swap_target: true,
            direction: true,
            client: true,
            timeout: true,
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
    let selectors: Vec<_> = [
        ("<source>", !rest.is_empty()),
        ("--panel", options.panel.is_some()),
        ("--current", options.current),
    ]
    .into_iter()
    .filter(|(_, present)| *present)
    .collect();
    if selectors.len() > 1 {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: selectors[0].0,
                second: selectors[1].0,
            },
            envelope,
        ));
    }
    let source = rest
        .first()
        .copied()
        .or(options.panel.as_deref())
        .map(|value| target(value.into(), Resource::Panel, envelope))
        .transpose()?;
    let destination = match (&options.swap_target, &options.direction) {
        (Some(_), Some(_)) => {
            return Err(usage_with(
                UsageReason::ConflictingOptions {
                    context,
                    first: "--with",
                    second: "--direction",
                },
                envelope,
            ));
        }
        (Some(value), None) => {
            PanelSwapDestination::Pair(target(value.clone(), Resource::Panel, envelope)?)
        }
        (None, Some(value)) => {
            PanelSwapDestination::Direction(panel_direction(value, context, envelope)?)
        }
        (None, None) => {
            return Err(missing_required(
                context,
                "'--with' or '--direction'",
                envelope,
            ));
        }
    };
    Ok(ClientCommand::Swap(PanelSwapCommand {
        source,
        destination,
        client: client_id(options, context, envelope)?,
        timeout_ms: wait_timeout_option(options, context, envelope)?,
    }))
}
