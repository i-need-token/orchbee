//! Closed event filters and ergonomic selectors are admitted before joining an existing Session.
use super::{
    ExpectedValue, FailureEnvelope, OutputFormat, ParseFailure, Request, Resource, Target,
    UsageContext, UsageReason, check_scope, missing_command, target, unknown_command, usage_with,
};
use orchbee_workbench_protocol::{
    PanelAgentState, SemanticEventCursor, SemanticEventFilter, SemanticEventKind,
    SemanticOutputMatch, SemanticOutputSource, WorkbenchClientId,
};

/// The CLI vocabulary is independent of the private event protocol's historical noun.
pub(crate) fn public_name(kind: SemanticEventKind) -> String {
    kind.as_str()
        .strip_prefix("pane.")
        .map_or_else(|| kind.as_str().to_owned(), |tail| format!("panel.{tail}"))
}
fn public_kind(name: &str) -> Option<SemanticEventKind> {
    SemanticEventKind::ALL
        .iter()
        .copied()
        .find(|kind| public_name(*kind) == name)
}

#[derive(Default)]
pub(super) struct Options {
    pub(super) events: Vec<String>,
    pub(super) filters: Vec<String>,
    pub(super) after: Option<String>,
}
impl Options {
    pub(super) fn first_option(&self) -> Option<&'static str> {
        if !self.events.is_empty() {
            Some("--event")
        } else if !self.filters.is_empty() {
            Some("--filter")
        } else if self.after.is_some() {
            Some("--after-cursor")
        } else {
            None
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Command {
    pub wait: bool,
    pub filters: Vec<SemanticEventFilter>,
    pub events: Vec<SemanticEventKind>,
    pub after: Option<SemanticEventCursor>,
    pub timeout_ms: Option<u32>,
    pub workspace: Option<Target>,
    pub tab: Option<Target>,
    pub panel: Option<Target>,
    pub client: Option<WorkbenchClientId>,
    pub matcher: Option<SemanticOutputMatch>,
    pub source: SemanticOutputSource,
    pub lines: u16,
    pub strip_ansi: bool,
    pub state: Option<PanelAgentState>,
}

pub(super) fn parse(
    words: &[&str],
    options: &super::Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let wait = match words {
        ["events"] => return Err(missing_command(UsageContext::Path("events"), envelope)),
        ["events", "subscribe"] => false,
        ["events", "wait"] => true,
        ["events", verb, ..] if !matches!(*verb, "wait" | "subscribe") => {
            return Err(unknown_command(
                UsageContext::Path("events"),
                verb,
                envelope,
            ));
        }
        ["events", verb, args @ ..] => {
            return Err(usage_with(
                UsageReason::UnexpectedArgument {
                    context: UsageContext::Path(if *verb == "wait" {
                        "events wait"
                    } else {
                        "events subscribe"
                    }),
                    value: args[0].into(),
                },
                envelope,
            ));
        }
        _ => unreachable!("events family"),
    };
    let usage = UsageContext::Path(if wait {
        "events wait"
    } else {
        "events subscribe"
    });
    check_scope(options, usage, envelope)?;
    let invalid = |option, value: &str| {
        usage_with(
            UsageReason::InvalidValue {
                context: usage,
                option,
                value: value.into(),
                expected: ExpectedValue::EventsSpecification,
            },
            envelope,
        )
    };
    let mut filters = Vec::new();
    for raw in &options.events.filters {
        if raw.len() > 16 * 1024 {
            return Err(invalid("--filter", raw));
        }
        let mut value: serde_json::Value =
            serde_json::from_str(raw).map_err(|_| invalid("--filter", raw))?;
        if value["kind"] == "event" {
            let kind = value["event"]
                .as_str()
                .and_then(public_kind)
                .ok_or_else(|| invalid("--filter", raw))?;
            value["event"] = serde_json::json!(kind.as_str());
        }
        let filter: SemanticEventFilter =
            serde_json::from_value(value).map_err(|_| invalid("--filter", raw))?;
        filter.validate().map_err(|_| invalid("--filter", raw))?;
        filters.push(filter);
    }
    let mut events = Vec::new();
    for raw in &options.events.events {
        let event = public_kind(raw).ok_or_else(|| invalid("--event", raw))?;
        if !events.contains(&event) {
            events.push(event);
        }
    }
    let after = options
        .events
        .after
        .as_deref()
        .map(|raw| serde_json::from_str(raw).map_err(|_| invalid("--after-cursor", raw)))
        .transpose()?;
    let timeout_ms = options
        .timeout
        .as_deref()
        .map(|raw| {
            raw.parse::<u32>()
                .ok()
                .filter(|n| *n <= 3_600_000)
                .ok_or_else(|| invalid("--timeout", raw))
        })
        .transpose()?
        .or(wait.then_some(30_000));
    let lines = options
        .lines
        .as_deref()
        .map(|raw| {
            raw.parse::<u16>()
                .ok()
                .filter(|n| (1..=4096).contains(n))
                .ok_or_else(|| invalid("--lines", raw))
        })
        .transpose()?
        .unwrap_or(80);
    let source = match options.output_source.as_deref().unwrap_or("recent") {
        "recent" => SemanticOutputSource::Recent,
        "recent-unwrapped" => SemanticOutputSource::RecentUnwrapped,
        "visible" => SemanticOutputSource::Viewport,
        "detection" => SemanticOutputSource::Detection,
        raw => return Err(invalid("--source", raw)),
    };
    if options.plain && options.ansi
        || options.match_text.is_some() && options.regex.is_some()
        || options.until.len() > 1
    {
        return Err(invalid("--filter", "conflicting event conditions"));
    }
    let matcher = options
        .match_text
        .as_ref()
        .map(|value| SemanticOutputMatch::Substring {
            value: value.clone(),
        })
        .or_else(|| {
            options
                .regex
                .as_ref()
                .map(|value| SemanticOutputMatch::Regex {
                    value: value.clone(),
                })
        });
    let state = options
        .until
        .first()
        .map(|raw| match raw.as_str() {
            "idle" => Ok(PanelAgentState::Idle),
            "working" => Ok(PanelAgentState::Working),
            "blocked" => Ok(PanelAgentState::Blocked),
            "done" => Ok(PanelAgentState::Done),
            "error" => Ok(PanelAgentState::Error),
            "unknown" => Ok(PanelAgentState::Unknown),
            _ => Err(invalid("--until", raw)),
        })
        .transpose()?;
    let selectors = usize::from(options.workspace.is_some())
        + usize::from(options.tab.is_some())
        + usize::from(options.panel.is_some());
    if selectors > 1 {
        return Err(invalid("--filter", "choose one resource scope"));
    }
    if !filters.is_empty()
        && (!events.is_empty()
            || selectors > 0
            || options.client.is_some()
            || matcher.is_some()
            || state.is_some()
            || options.output_source.is_some()
            || options.lines.is_some()
            || options.plain
            || options.ansi)
    {
        return Err(invalid(
            "--filter",
            "typed filters own their scopes and conditions",
        ));
    }
    if filters.len() + events.len() > 32 || wait && filters.len() + events.len() != 1 {
        return Err(invalid(
            "--event",
            "wait requires one filter; subscribe admits at most 32",
        ));
    }
    if filters.is_empty() && events.is_empty() {
        events.extend(SemanticEventKind::ALL.iter().copied().filter(|kind| {
            !matches!(
                kind,
                SemanticEventKind::PaneOutputMatched | SemanticEventKind::PaneScrollChanged
            )
        }));
    }
    if matcher.is_some() && events.as_slice() != [SemanticEventKind::PaneOutputMatched]
        || state.is_some() && events.as_slice() != [SemanticEventKind::PaneAgentStatusChanged]
    {
        return Err(invalid("--event", "condition requires its specific event"));
    }
    if events.contains(&SemanticEventKind::PaneOutputMatched)
        && (matcher.is_none() || options.panel.is_none())
        || events.contains(&SemanticEventKind::PaneScrollChanged)
            && (options.panel.is_none() || options.client.is_none())
        || source == SemanticOutputSource::Viewport && options.client.is_none()
    {
        return Err(invalid(
            "--panel",
            "output/scroll requires its exact Panel and visible/scroll requires Client",
        ));
    }
    if (options.output_source.is_some() || options.lines.is_some() || options.plain || options.ansi)
        && matcher.is_none()
    {
        return Err(invalid(
            "--source",
            "output options require an output-matched filter",
        ));
    }
    Ok(Request::Events {
        command: Command {
            wait,
            filters,
            events,
            after,
            timeout_ms,
            workspace: options
                .workspace
                .as_ref()
                .map(|raw| target(raw.clone(), Resource::Workspace, envelope))
                .transpose()?,
            tab: options
                .tab
                .as_ref()
                .map(|raw| target(raw.clone(), Resource::Tab, envelope))
                .transpose()?,
            panel: options
                .panel
                .as_ref()
                .map(|raw| target(raw.clone(), Resource::Panel, envelope))
                .transpose()?,
            client: super::client_control::client_id(options, usage, envelope)?,
            matcher,
            source,
            lines,
            strip_ansi: !options.ansi,
            state,
        },
        format: options.format.unwrap_or(if wait {
            OutputFormat::Text
        } else {
            OutputFormat::Json
        }),
    })
}
