//! Ordered link matching is display preflight; the existing Server/Worker owns execution.
use crate::cli::{
    Failure, FailureKind, query, results,
    source::{ArmedSignals, SignalWatch},
};
use crate::local_client::LocalClient;
use orchbee_local_site_runtime::{PluginRegistry, PluginRegistryEntry};
use orchbee_workbench_protocol::{
    PluginAction, PluginFailure, PluginInvocationContext, PluginOutcome, PluginRequest,
    PluginWorker, Snapshot,
};
use serde::Serialize;
use std::time::Instant;

#[derive(Clone)]
pub(in crate::cli) struct Selection {
    entry: PluginRegistryEntry,
    handler: String,
    action: String,
}
#[derive(Serialize)]
#[serde(
    tag = "state",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub(in crate::cli) enum Activation {
    NoLink,
    NoHandler,
    Started {
        operation_id: String,
        plugin_id: String,
        handler_id: String,
        action_id: String,
        worker: PluginWorker,
        job_id: String,
        existing: bool,
    },
    Failed {
        operation_id: String,
        plugin_id: String,
        handler_id: String,
        action_id: String,
        reason: PluginFailure,
    },
    Unknown {
        operation_id: String,
        plugin_id: String,
        handler_id: String,
        action_id: String,
        worker: Option<PluginWorker>,
    },
}
impl Activation {
    pub(in crate::cli) fn status(&self) -> u8 {
        match self {
            Self::Failed { .. } => 3,
            Self::Unknown { .. } => 8,
            _ => 0,
        }
    }
    pub(in crate::cli) fn handled(&self) -> bool {
        matches!(self, Self::Started { .. })
    }
    pub(in crate::cli) fn submitted(&self) -> bool {
        !matches!(self, Self::NoLink | Self::NoHandler)
    }
    pub(in crate::cli) fn state(&self) -> &'static str {
        match self {
            Self::NoLink => "no-link",
            Self::NoHandler => "no-handler",
            Self::Started { .. } => "started",
            Self::Failed { .. } => "failed",
            Self::Unknown { .. } => "unknown",
        }
    }
    pub(in crate::cli) fn failure(&self) -> Option<PluginFailure> {
        match self {
            Self::Failed { reason, .. } => Some(*reason),
            _ => None,
        }
    }
    pub(in crate::cli) fn operation(&self) -> Option<&str> {
        match self {
            Self::Started { operation_id, .. }
            | Self::Failed { operation_id, .. }
            | Self::Unknown { operation_id, .. } => Some(operation_id),
            _ => None,
        }
    }
}
fn registry() -> Result<PluginRegistry, Failure> {
    PluginRegistry::from_environment(|name| std::env::var_os(name))
        .map_err(|_| Failure::new(FailureKind::ServiceUnavailable))
}
fn select_entries(
    entries: &[PluginRegistryEntry],
    url: &str,
    mut check: impl FnMut() -> Result<(), Failure>,
) -> Result<Option<Selection>, Failure> {
    let mut ordered: Vec<_> = entries
        .iter()
        .filter(|p| p.enabled && p.warning.is_none())
        .collect();
    ordered.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    for entry in ordered {
        for handler in &entry.manifest.link_handlers {
            check()?;
            let Some(action) = entry
                .manifest
                .actions
                .iter()
                .find(|a| a.id == handler.action)
            else {
                continue;
            };
            if !entry.manifest.supports_windows(&handler.platforms)
                || !entry.manifest.supports_windows(&action.platforms)
            {
                continue;
            }
            let Ok(pattern) = regex::RegexBuilder::new(&handler.pattern)
                .size_limit(1024 * 1024)
                .build()
            else {
                continue;
            };
            if pattern.is_match(url) {
                return Ok(Some(Selection {
                    entry: entry.clone(),
                    handler: handler.id.clone(),
                    action: action.id.clone(),
                }));
            }
        }
    }
    Ok(None)
}
pub(in crate::cli) fn select(
    url: &str,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Option<Selection>, Failure> {
    signals.check_before(deadline)?;
    let catalog = registry()?
        .list(None)
        .map_err(|_| Failure::new(FailureKind::ServiceUnavailable))?;
    select_entries(&catalog.plugins, url, || signals.check_before(deadline))
}
pub(in crate::cli) struct Invocation {
    selection: Selection,
    operation_id: String,
    context: PluginInvocationContext,
}
pub(in crate::cli) fn preflight(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    terminal: &results::TerminalRow,
    url: &str,
    selection: Selection,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Invocation, Failure> {
    let operation_id = query::request_id()?.as_str().to_owned();
    let provided = PluginInvocationContext {
        invocation_source: Some("link_click".into()),
        clicked_url: Some(url.into()),
        link_handler_id: Some(selection.handler.clone()),
        ..Default::default()
    };
    provided
        .validate()
        .map_err(|_| Failure::new(FailureKind::Rejected))?;
    let (context, _) = super::context::observe(
        client,
        snapshot,
        super::context::Source::terminal(terminal),
        provided,
        &operation_id,
        deadline,
        signals,
    )?;
    Ok(Invocation {
        selection,
        operation_id,
        context,
    })
}
pub(in crate::cli) fn submit(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    invocation: Invocation,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Activation, Failure> {
    signals.check_before(deadline)?;
    let Invocation {
        selection,
        operation_id,
        context,
    } = invocation;
    let current = registry()?
        .list(None)
        .map_err(|_| Failure::new(FailureKind::ServiceUnavailable))?;
    let Some(selected) = select_entries(
        &current.plugins,
        context
            .clicked_url
            .as_deref()
            .ok_or_else(|| Failure::new(FailureKind::ProtocolInvalid))?,
        || signals.check_before(deadline),
    )?
    else {
        return Err(Failure::new(FailureKind::Rejected));
    };
    if selected.entry != selection.entry
        || selected.handler != selection.handler
        || selected.action != selection.action
    {
        return Err(Failure::new(FailureKind::Rejected));
    }
    signals.check_before(deadline)?;
    let request = PluginRequest {
        protocol_version: snapshot.protocol_version,
        request_id: query::request_id()?,
        site_id: snapshot.site_id.clone(),
        action: PluginAction::Invoke {
            operation_id: operation_id.clone(),
            plugin_id: Some(selection.entry.manifest.id.clone()),
            action_id: selection.action.clone(),
            context: Box::new(context),
        },
    };
    request
        .validate()
        .map_err(|_| Failure::new(FailureKind::Rejected))?;
    let outcome = match client.plugin(request, deadline, signals.poll_fd()) {
        Ok(reply) => Some(reply.outcome),
        Err(error) if error.submitted => None,
        Err(_) => {
            signals.check_before(deadline)?;
            return Err(Failure::new(FailureKind::TransportLoss));
        }
    };
    let plugin_id = selection.entry.manifest.id;
    let handler_id = selection.handler;
    let action_id = selection.action;
    Ok(match outcome {
        Some(PluginOutcome::Started {
            worker,
            log,
            existing,
        }) if log.job_id == operation_id
            && log.plugin_id == plugin_id
            && log.command_id == action_id =>
        {
            Activation::Started {
                operation_id,
                plugin_id,
                handler_id,
                action_id,
                worker,
                job_id: log.job_id,
                existing,
            }
        }
        Some(PluginOutcome::Failed { reason }) => Activation::Failed {
            operation_id,
            plugin_id,
            handler_id,
            action_id,
            reason,
        },
        other => Activation::Unknown {
            operation_id,
            plugin_id,
            handler_id,
            action_id,
            worker: match other {
                Some(PluginOutcome::Unknown { worker }) => Some(worker),
                _ => None,
            },
        },
    })
}

#[cfg(test)]
mod tests;
