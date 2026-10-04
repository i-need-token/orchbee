//! Ordered Windows link-handler routing with no presentation-surface types.

use std::time::Instant;

use orchbee_local_site_runtime::{PluginRegistry, PluginRegistryEntry};
use orchbee_workbench_protocol::{
    PluginAction, PluginFailure, PluginInvocationContext, PluginLinkCatalogGuard, PluginOutcome,
    PluginRequest, PluginWorker, Snapshot, WorkbenchRequestId,
};
use serde::Serialize;

use crate::local_client::LocalClient;

/// Cancellation/deadline policy supplied by the owning presentation lane.
pub(crate) trait Guard {
    /// The owning adapter's cancellation or deadline refusal.
    type Error;

    /// Refuse work whose cancellation or deadline owner has settled.
    fn check(&mut self, deadline: Instant) -> Result<(), Self::Error>;
    /// Return the optional descriptor that interrupts an in-flight local transport wait.
    fn wake(&self) -> Option<orchbee_local_ipc::LocalIo<'_>>;
}

/// A typed refusal before submission, preserving whether the owning gate caused it.
#[derive(Debug)]
pub(crate) enum Error<E> {
    Gate(E),
    RegistryUnavailable,
    IdentityUnavailable,
    InvalidContext,
    MissingClickedUrl,
    RegistryChanged,
    InvalidRequest,
    DefinitelyNotSubmitted,
    TransportUnavailable,
    ProtocolInvalid,
    LimitExceeded,
    SourceChanged,
}

/// The exact registry entry, handler, and action chosen by deterministic priority.
#[derive(Clone)]
pub(crate) struct Selection {
    pub(crate) entry: PluginRegistryEntry,
    pub(crate) handler: String,
    pub(crate) action: String,
    catalog: Option<PluginLinkCatalogGuard>,
}

/// One exact persisted-catalog read, including a definite no-handler result.
/// The catalog identity survives selection so browser fallback can reject any
/// intervening registry publication even when both catalogs are nonmatching.
pub(crate) struct SelectionSnapshot {
    pub(crate) selection: Option<Selection>,
    pub(crate) catalog: PluginLinkCatalogGuard,
}

/// Immutable handler choice and operation identity carried across context
/// observation. `finish` refuses an observer that changed the clicked URL,
/// handler, correlation identity, or invocation context.
pub(crate) struct Preflight {
    selection: Selection,
    operation_id: String,
    url: String,
}

impl Preflight {
    /// Fix one selection, clicked URL, and fresh operation identity before context observation.
    ///
    /// # Errors
    ///
    /// Returns an identity or context error before any plugin request exists.
    pub(crate) fn new<E>(url: &str, selection: Selection) -> Result<Self, Error<E>> {
        let preflight = Self {
            selection,
            operation_id: request_id::<E>()?.as_str().to_owned(),
            url: url.to_owned(),
        };
        validate_url::<E>(&preflight.url)?;
        if preflight.provided_context().validate().is_err() {
            return Err(Error::InvalidContext);
        }
        Ok(preflight)
    }

    /// Build the explicit link-click fields that override observed display context.
    pub(crate) fn provided_context(&self) -> PluginInvocationContext {
        PluginInvocationContext {
            invocation_source: Some("link_click".into()),
            correlation_id: Some(self.operation_id.clone()),
            clicked_url: Some(self.url.clone()),
            link_handler_id: Some(self.selection.handler.clone()),
            ..PluginInvocationContext::default()
        }
    }

    /// Bind an observed context to the fixed selection and operation identity.
    ///
    /// # Errors
    ///
    /// Returns `InvalidContext` when any fixed link-click field changed or the merged context is
    /// outside the protocol bounds.
    pub(crate) fn finish<E>(
        self,
        context: PluginInvocationContext,
    ) -> Result<Invocation, Error<E>> {
        if context.clicked_url.as_deref() != Some(self.url.as_str())
            || context.link_handler_id.as_deref() != Some(self.selection.handler.as_str())
            || context.correlation_id.as_deref() != Some(self.operation_id.as_str())
            || context.invocation_source.as_deref() != Some("link_click")
            || context.validate().is_err()
        {
            return Err(Error::InvalidContext);
        }
        Ok(Invocation {
            selection: self.selection,
            operation_id: self.operation_id,
            context,
        })
    }
}

/// Validate a clicked URL with the plugin invocation context's canonical bounds.
///
/// # Errors
///
/// Returns `InvalidContext` for an over-bound or otherwise invalid URL.
pub(crate) fn validate_url<E>(url: &str) -> Result<(), Error<E>> {
    let context = PluginInvocationContext {
        clicked_url: Some(url.to_owned()),
        ..PluginInvocationContext::default()
    };
    context.validate().map_err(|_| Error::InvalidContext)
}

/// A fixed handler choice plus fully observed, validated invocation context.
pub(crate) struct Invocation {
    selection: Selection,
    operation_id: String,
    context: PluginInvocationContext,
}

/// The terminal receipt for one plugin link activation attempt.
#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "state",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub(crate) enum Activation {
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
    /// Return the public CLI exit status for this receipt.
    pub(crate) const fn status(&self) -> u8 {
        match self {
            Self::Failed { .. } => 3,
            Self::Unknown { .. } => 8,
            _ => 0,
        }
    }

    /// Report whether the handler acknowledged a started job.
    pub(crate) const fn handled(&self) -> bool {
        matches!(self, Self::Started { .. })
    }

    /// Report whether a request may have reached a plugin worker.
    pub(crate) const fn submitted(&self) -> bool {
        !matches!(self, Self::NoLink | Self::NoHandler)
    }

    /// Return the stable machine-readable state name.
    pub(crate) const fn state(&self) -> &'static str {
        match self {
            Self::NoLink => "no-link",
            Self::NoHandler => "no-handler",
            Self::Started { .. } => "started",
            Self::Failed { .. } => "failed",
            Self::Unknown { .. } => "unknown",
        }
    }

    /// Return the plugin's typed terminal failure, when one was reported.
    pub(crate) const fn failure(&self) -> Option<PluginFailure> {
        match self {
            Self::Failed { reason, .. } => Some(*reason),
            _ => None,
        }
    }

    /// Return the correlation identity carried by submitted outcomes.
    pub(crate) fn operation(&self) -> Option<&str> {
        match self {
            Self::Started { operation_id, .. }
            | Self::Failed { operation_id, .. }
            | Self::Unknown { operation_id, .. } => Some(operation_id),
            _ => None,
        }
    }
}

fn registry<E>() -> Result<PluginRegistry, Error<E>> {
    PluginRegistry::from_environment(|name| std::env::var_os(name))
        .map_err(|_| Error::RegistryUnavailable)
}

/// Select the first matching enabled Windows handler from a supplied registry snapshot.
///
/// # Errors
///
/// Returns the caller's gate error; malformed or ineligible individual handlers are skipped.
pub(crate) fn select_entries<E>(
    entries: &[PluginRegistryEntry],
    url: &str,
    mut check: impl FnMut() -> Result<(), E>,
) -> Result<Option<Selection>, Error<E>> {
    let mut ordered: Vec<_> = entries
        .iter()
        .filter(|entry| entry.enabled && entry.warning.is_none())
        .collect();
    ordered.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    for entry in ordered {
        for handler in &entry.manifest.link_handlers {
            check().map_err(Error::Gate)?;
            let Some(action) = entry
                .manifest
                .actions
                .iter()
                .find(|action| action.id == handler.action)
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
                    catalog: None,
                }));
            }
        }
    }
    Ok(None)
}

/// Read the current registry and select its first matching enabled Windows handler.
///
/// # Errors
///
/// Returns a gate or registry-read failure. A snapshot whose selection is `None` proves this exact
/// persisted catalog had no eligible handler while retaining its identity for effect handoff.
pub(crate) fn select<G: Guard>(
    url: &str,
    deadline: Instant,
    guard: &mut G,
) -> Result<SelectionSnapshot, Error<G::Error>> {
    guard.check(deadline).map_err(Error::Gate)?;
    let catalog = registry()?
        .list_registered()
        .map_err(|_| Error::RegistryUnavailable)?;
    let mut selection = select_entries(&catalog.plugins, url, || guard.check(deadline))?;
    let catalog = PluginLinkCatalogGuard {
        revision: catalog.revision,
        fingerprint: catalog.fingerprint,
    };
    if let Some(selection) = &mut selection {
        selection.catalog = Some(catalog.clone());
    }
    guard.check(deadline).map_err(Error::Gate)?;
    Ok(SelectionSnapshot { selection, catalog })
}

/// Re-select under the current registry, then submit exactly one plugin
/// request. Once transport reports submission, every ambiguous answer is a
/// terminal `Unknown`; it never becomes browser eligibility.
///
/// # Errors
///
/// Returns a pre-submission gate, registry, context, request, or definite transport failure. A
/// request that may have been submitted is returned as `Activation::Unknown` instead.
pub(crate) fn submit<G: Guard>(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    invocation: Invocation,
    deadline: Instant,
    guard: &mut G,
) -> Result<Activation, Error<G::Error>> {
    guard.check(deadline).map_err(Error::Gate)?;
    let Invocation {
        selection,
        operation_id,
        context,
    } = invocation;
    let url = context
        .clicked_url
        .as_deref()
        .ok_or(Error::MissingClickedUrl)?;
    let current = registry()?
        .list_registered()
        .map_err(|_| Error::RegistryUnavailable)?;
    let current_guard = PluginLinkCatalogGuard {
        revision: current.revision,
        fingerprint: current.fingerprint,
    };
    let Some(mut selected) = select_entries(&current.plugins, url, || guard.check(deadline))?
    else {
        return Err(Error::RegistryChanged);
    };
    selected.catalog = Some(current_guard.clone());
    if selected.entry != selection.entry
        || selected.handler != selection.handler
        || selected.action != selection.action
        || selected.catalog != selection.catalog
    {
        return Err(Error::RegistryChanged);
    }
    guard.check(deadline).map_err(Error::Gate)?;
    let request = PluginRequest {
        protocol_version: snapshot.protocol_version,
        request_id: request_id()?,
        site_id: snapshot.site_id.clone(),
        action: PluginAction::Invoke {
            operation_id: operation_id.clone(),
            plugin_id: Some(selection.entry.manifest.id.clone()),
            action_id: selection.action.clone(),
            context: Box::new(context),
            link_catalog: Some(current_guard),
        },
    };
    request.validate().map_err(|_| Error::InvalidRequest)?;
    let outcome = match client.plugin(request, deadline, guard.wake()) {
        Ok(reply) => Some(reply.outcome),
        Err(error) if error.submitted => None,
        Err(_) => {
            guard.check(deadline).map_err(Error::Gate)?;
            return Err(Error::DefinitelyNotSubmitted);
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

fn request_id<E>() -> Result<WorkbenchRequestId, Error<E>> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Error::IdentityUnavailable)?;
    let mut text = String::with_capacity(32);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(text, "{byte:02x}");
    }
    WorkbenchRequestId::parse(&text).map_err(|_| Error::IdentityUnavailable)
}

#[cfg(test)]
mod tests;
