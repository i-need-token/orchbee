//! The restart decision for a running Session against the store's active release: the newer side
//! decides from the floors and generations its manifest lists and the versions and generations the
//! Session's `query` answers, so every evaluator reaches the same tier.
use super::manifest::Component;
use orchbee_connection_handshake::{
    COMPATIBILITY_WINDOW, ConnectionKind, GenerationRange, ProcessEntry, ProcessRole, QueryResult,
};
use orchbee_local_runtime::compare_product_versions;
use serde_json::{Value, json};
use std::{cmp::Ordering, num::NonZeroU32};

/// How much a running process needs replacing by the active version, lowest first.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::cli) enum Tier {
    None,
    Recommended,
    Required,
}

impl Tier {
    pub(in crate::cli) const fn token(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Recommended => "recommended",
            Self::Required => "required",
        }
    }
}

/// The tier Server and Worker share, and each Terminal Host's own.
#[derive(Debug, Eq, PartialEq)]
pub(in crate::cli) struct Restart {
    pub(in crate::cli) tier: Tier,
    pub(in crate::cli) hosts: Vec<HostRestart>,
}

/// One Terminal Host's own tier, the terminals it runs, and whether the active version's Worker
/// moves them to its own Host: that version moves terminals, and its Worker shares a `worker-host`
/// generation with the Host, so it adopts it. A Host that offers no `handover` keeps them all the
/// same, since a query lists capabilities for the answering Server alone.
#[derive(Debug, Eq, PartialEq)]
pub(in crate::cli) struct HostRestart {
    pub(in crate::cli) pid: NonZeroU32,
    pub(in crate::cli) tier: Tier,
    pub(in crate::cli) terminals: u32,
    pub(in crate::cli) moves: bool,
}

impl Restart {
    pub(in crate::cli) fn json(&self) -> Value {
        let hosts = self
            .hosts
            .iter()
            .map(|host| {
                json!({
                    "processId": host.pid,
                    "tier": host.tier.token(),
                    "terminals": host.terminals,
                    "moves": host.moves,
                })
            })
            .collect::<Vec<_>>();
        json!({"tier": self.tier.token(), "hosts": hosts})
    }
}

/// The `restart` member `status server` and `doctor` carry for the Session `query` describes.
pub(crate) fn document(query: &QueryResult) -> Option<Value> {
    active(query).map(|restart| restart.json())
}

/// The decision for the Session `query` describes against the store's active release; `None` when
/// no release store or active release reads, as in a development build.
pub(in crate::cli) fn active(query: &QueryResult) -> Option<Restart> {
    let root = orchbee_local_runtime::release_store_root(|name| std::env::var_os(name)).ok()?;
    let active = orchbee_local_runtime::active_release(&root).ok()??;
    let release = super::installed::bound_release(&active.directory).ok()??;
    Some(evaluate(&release.components, query))
}

/// Server and Worker share the higher of their tiers: `required` when the Server predates the
/// compatibility window, shares no `client-server` generation with the active Client, or either is
/// below its security floor; `recommended` when either, or any Host, is below its recommended
/// floor. A Host is `required` below its security floor or without a `worker-host` generation in
/// common with the active Worker. Only a process older than its active component gets a tier; a
/// floor or range the manifest omits imposes nothing.
pub(super) fn evaluate(components: &[Component], query: &QueryResult) -> Restart {
    let processes = query.processes();
    let of = |role| processes.iter().filter(move |entry| entry.role == role);
    let client = component(components, "client");
    let server = component(components, "server");
    let worker = component(components, "worker");
    let host = component(components, "terminal-host");
    let mut tier = Tier::None;
    for entry in of(ProcessRole::Server) {
        let Some(server) = server.filter(|server| older(entry, server)) else {
            continue;
        };
        let joins = served(entry).is_some_and(|served| served.contains(COMPATIBILITY_WINDOW))
            && shares(
                listed(entry, ConnectionKind::ClientServer),
                client.and_then(|client| client.connections.client_server),
            );
        tier = tier.max(if joins {
            floors(entry, server)
        } else {
            Tier::Required
        });
    }
    for entry in of(ProcessRole::Worker) {
        if let Some(worker) = worker.filter(|worker| older(entry, worker)) {
            tier = tier.max(floors(entry, worker));
        }
    }
    let mut hosts = Vec::new();
    for entry in of(ProcessRole::TerminalHost) {
        let connects = shares(
            listed(entry, ConnectionKind::WorkerHost),
            worker.and_then(|worker| worker.connections.worker_host),
        );
        let own = host
            .filter(|host| older(entry, host))
            .map_or(Tier::None, |host| {
                if below(entry, host.recommended_floor.as_deref()) {
                    tier = tier.max(Tier::Recommended);
                }
                if !connects || below(entry, host.security_floor.as_deref()) {
                    Tier::Required
                } else {
                    Tier::None
                }
            });
        hosts.push(HostRestart {
            pid: entry.pid,
            tier: own,
            terminals: entry.terminals,
            moves: connects && orchbee_local_runtime::moves_live(),
        });
    }
    Restart { tier, hosts }
}

fn component<'a>(components: &'a [Component], id: &str) -> Option<&'a Component> {
    components.iter().find(|component| component.id == id)
}

fn older(entry: &ProcessEntry, component: &Component) -> bool {
    compare_product_versions(entry.product_version.as_str(), &component.version) == Ordering::Less
}

fn below(entry: &ProcessEntry, floor: Option<&str>) -> bool {
    floor.is_some_and(|floor| {
        compare_product_versions(entry.product_version.as_str(), floor) == Ordering::Less
    })
}

fn floors(entry: &ProcessEntry, component: &Component) -> Tier {
    if below(entry, component.security_floor.as_deref()) {
        Tier::Required
    } else if below(entry, component.recommended_floor.as_deref()) {
        Tier::Recommended
    } else {
        Tier::None
    }
}

fn listed(entry: &ProcessEntry, kind: ConnectionKind) -> Option<GenerationRange> {
    entry
        .connections
        .iter()
        .find(|connection| connection.kind.as_str() == kind.name())
        .map(|connection| connection.generations)
}

/// The capabilities the answering Server lists for its Client endpoint.
fn served(entry: &ProcessEntry) -> Option<&orchbee_connection_handshake::CapabilitySet> {
    entry
        .connections
        .iter()
        .find(|connection| connection.kind.as_str() == ConnectionKind::ClientServer.name())
        .and_then(|connection| connection.capabilities.as_ref())
}

/// Whether two ranges share a generation; an unlisted side imposes nothing.
fn shares(left: Option<GenerationRange>, right: Option<GenerationRange>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.highest_common(right).is_some(),
        _ => true,
    }
}
