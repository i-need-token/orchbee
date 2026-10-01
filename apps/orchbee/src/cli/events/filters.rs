//! Resolve shorthand identities once; retained replies must still match the admitted immutable filters.
use super::{
    Command, Failure, FailureKind, Filter, Kind, Resource, Scope, SemanticEventData,
    SemanticEventMatch, SemanticEventSource, Snapshot, results,
};
use orchbee_workbench_protocol::{SemanticEvent, SemanticOutputMatch};
use regex::{Regex, RegexBuilder};
pub(super) type Filters = Vec<(Filter, Option<Regex>)>;
pub(super) fn resolve(command: &Command, snapshot: &Snapshot) -> Result<Filters, Failure> {
    let mut filters = command.filters.clone();
    let scope = if let Some(target) = &command.workspace {
        let workspace = results::resolve_or_fail(
            snapshot.navigation.projects.iter().flat_map(|project| {
                project.workspaces.iter().map(|workspace| {
                    (
                        workspace.workspace_id.as_str(),
                        workspace.label.as_str(),
                        workspace,
                    )
                })
            }),
            target,
            Resource::Workspace,
        )?;
        Scope::Workspace {
            workspace_id: workspace.workspace_id.clone(),
        }
    } else if let Some(target) = &command.tab {
        let tab =
            results::resolve_or_fail(
                snapshot.terminal_contexts.iter().flat_map(|context| {
                    context.tabs.iter().enumerate().map(|(position, tab)| {
                        (tab.tab_id.as_str(), tab.display_label(position), tab)
                    })
                }),
                target,
                Resource::Tab,
            )?;
        Scope::Tab {
            tab_id: tab.tab_id.clone(),
        }
    } else if let Some(target) = &command.panel {
        let id = results::resolve_or_fail(
            results::panel_entries(snapshot)
                .into_iter()
                .map(|(panel, _, _)| (panel.as_str(), "", panel)),
            target,
            Resource::Panel,
        )?;
        Scope::Pane {
            panel_id: id.clone(),
        }
    } else if let Some(client) = &command.client {
        Scope::Client {
            client_id: client.clone(),
        }
    } else {
        Scope::All {}
    };
    for kind in &command.events {
        let source = match &scope {
            Scope::Pane { panel_id } => Some(SemanticEventSource {
                panel_id: panel_id.clone(),
                presentation_id: None,
            }),
            _ => None,
        };
        filters.push(match kind {
            Kind::PaneOutputMatched => Filter::OutputMatched {
                target: source.ok_or_else(|| Failure::new(FailureKind::InvalidUsage))?,
                source: command.source,
                lines: command.lines,
                matcher: command
                    .matcher
                    .clone()
                    .ok_or_else(|| Failure::new(FailureKind::InvalidUsage))?,
                strip_ansi: command.strip_ansi,
                client_id: command.client.clone(),
            },
            Kind::PaneScrollChanged => Filter::ScrollChanged {
                target: source.ok_or_else(|| Failure::new(FailureKind::InvalidUsage))?,
                client_id: command
                    .client
                    .clone()
                    .ok_or_else(|| Failure::new(FailureKind::InvalidUsage))?,
            },
            Kind::PaneAgentStatusChanged if source.is_some() => Filter::AgentStatusChanged {
                target: source.ok_or_else(|| Failure::new(FailureKind::InvalidUsage))?,
                state: command.state,
            },
            _ => Filter::Event {
                event: *kind,
                scope: scope.clone(),
                condition: command.state.map_or(SemanticEventMatch::Any {}, |state| {
                    SemanticEventMatch::AgentState { state }
                }),
            },
        });
    }
    filters
        .into_iter()
        .map(|filter| {
            filter
                .validate()
                .map_err(|_| Failure::new(FailureKind::InvalidUsage))?;
            let regex = if let Filter::OutputMatched {
                matcher: SemanticOutputMatch::Regex { value },
                ..
            } = &filter
            {
                Some(
                    RegexBuilder::new(value)
                        .size_limit(1024 * 1024)
                        .dfa_size_limit(1024 * 1024)
                        .build()
                        .map_err(|_| Failure::new(FailureKind::InvalidUsage))?,
                )
            } else {
                None
            };
            Ok((filter, regex))
        })
        .collect()
}
pub(super) fn matches(event: &SemanticEvent, filter: &Filter, regex: Option<&Regex>) -> bool {
    match filter {
        Filter::OutputMatched {
            target, matcher, ..
        } => {
            event.event == Kind::PaneOutputMatched
                && matches!(&event.data, SemanticEventData::Output {pane, matched_line: Some(line), ..} if pane.panel_id == target.panel_id && target.presentation_id.as_ref().is_none_or(|id| id == &pane.presentation_id) && match matcher {SemanticOutputMatch::Substring {value} => line.contains(value), SemanticOutputMatch::Regex {..} => regex.is_some_and(|regex| regex.is_match(line))})
        }
        Filter::ScrollChanged { target, client_id } => {
            event.event == Kind::PaneScrollChanged
                && matches!(&event.data, SemanticEventData::Scroll {pane, client_id: actual, ..} if pane.panel_id == target.panel_id && target.presentation_id.as_ref().is_none_or(|id| id == &pane.presentation_id) && client_id == actual)
        }
        _ => orchbee_workbench_protocol::matches_semantic_filter(event, filter),
    }
}
