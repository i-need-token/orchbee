//! `terminal.launch.read`: one read-only launch-fact lookup for a single
//! terminal. The request carries only correlation and the typed target; the
//! `found` outcome echoes the routing the Server resolved against its current
//! committed Workbench membership, plus the retained launch fact under the same
//! coordinator lock. The read dispatches no Worker or Host RPC, attaches no
//! viewer, holds no grant, and mutates nothing.

use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

use crate::{
    CreateOutcome, Geometry, PanelId, PresentationId, ProtocolError, RequestId, SiteId, TabId,
    TerminalContextId, TerminalId, WorkerId,
};

/// `terminal.launch.read` — resolve `terminal_id` against the current visible
/// Workbench binding and report the retained launch fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalLaunchReadRequest {
    pub(crate) request_id: RequestId,
    pub(crate) terminal_id: TerminalId,
}

impl TerminalLaunchReadRequest {
    #[must_use]
    pub const fn new(request_id: RequestId, terminal_id: TerminalId) -> Self {
        Self {
            request_id,
            terminal_id,
        }
    }

    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    #[must_use]
    pub const fn terminal_id(&self) -> &TerminalId {
        &self.terminal_id
    }
}

/// The presentation/panel/tab/context path a visible binding sits at — the same
/// routing tuple `terminal list`/`get` documents carry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalLaunchBoundTo {
    pub(crate) presentation_id: PresentationId,
    pub(crate) panel_id: PanelId,
    pub(crate) tab_id: TabId,
    pub(crate) terminal_context_id: TerminalContextId,
}

impl TerminalLaunchBoundTo {
    #[must_use]
    pub const fn new(
        presentation_id: PresentationId,
        panel_id: PanelId,
        tab_id: TabId,
        terminal_context_id: TerminalContextId,
    ) -> Self {
        Self {
            presentation_id,
            panel_id,
            tab_id,
            terminal_context_id,
        }
    }

    #[must_use]
    pub const fn presentation_id(&self) -> &PresentationId {
        &self.presentation_id
    }

    #[must_use]
    pub const fn panel_id(&self) -> &PanelId {
        &self.panel_id
    }

    #[must_use]
    pub const fn tab_id(&self) -> &TabId {
        &self.tab_id
    }

    #[must_use]
    pub const fn terminal_context_id(&self) -> &TerminalContextId {
        &self.terminal_context_id
    }
}

/// The resolved routing row: the binding pair plus where it is bound.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalLaunchRoute {
    pub(crate) terminal_id: TerminalId,
    pub(crate) worker_id: WorkerId,
    pub(crate) bound_to: TerminalLaunchBoundTo,
}

impl TerminalLaunchRoute {
    #[must_use]
    pub const fn new(
        terminal_id: TerminalId,
        worker_id: WorkerId,
        bound_to: TerminalLaunchBoundTo,
    ) -> Self {
        Self {
            terminal_id,
            worker_id,
            bound_to,
        }
    }

    #[must_use]
    pub const fn terminal_id(&self) -> &TerminalId {
        &self.terminal_id
    }

    #[must_use]
    pub const fn worker_id(&self) -> &WorkerId {
        &self.worker_id
    }

    #[must_use]
    pub const fn bound_to(&self) -> &TerminalLaunchBoundTo {
        &self.bound_to
    }
}

/// The retained launch phase verbatim, plus `not-tracked`: the visible binding
/// has no uniquely attributable consistent coordinator record. `not-tracked` is
/// an observation about retained facts — never the `unknown` phase and never
/// process liveness.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalLaunchState {
    Creating,
    AwaitingStart,
    Starting,
    Started,
    Unknown,
    Unavailable,
    NotTracked,
}

/// The retained launch fact for one resolved binding. `worker_generation` is
/// `null` exactly for `not-tracked` (no record means no generation to report);
/// `geometry` is non-null exactly for `started`; `create_outcome` echoes the
/// retained `CreateOutcome` and is `null` for `creating` and `not-tracked`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalLaunchFact {
    pub(crate) state: TerminalLaunchState,
    #[serde(serialize_with = "crate::number::positive_decimal_nullable::serialize")]
    pub(crate) worker_generation: Option<NonZeroU64>,
    pub(crate) create_outcome: Option<CreateOutcome>,
    pub(crate) geometry: Option<Geometry>,
}

impl TerminalLaunchFact {
    /// Constructs only the consistent combinations: the decoder enforces the
    /// same rules on the wire shape.
    pub fn new(
        state: TerminalLaunchState,
        worker_generation: Option<NonZeroU64>,
        create_outcome: Option<CreateOutcome>,
        geometry: Option<Geometry>,
    ) -> Result<Self, ProtocolError> {
        let fact = Self {
            state,
            worker_generation,
            create_outcome,
            geometry,
        };
        fact.validate()?;
        Ok(fact)
    }

    pub(crate) fn validate(&self) -> Result<(), ProtocolError> {
        let tracked = !matches!(self.state, TerminalLaunchState::NotTracked);
        if self.worker_generation.is_some() != tracked
            || self.geometry.is_some() != matches!(self.state, TerminalLaunchState::Started)
            || self.create_outcome.is_some()
                != !matches!(
                    self.state,
                    TerminalLaunchState::Creating | TerminalLaunchState::NotTracked
                )
        {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(())
    }

    #[must_use]
    pub const fn state(&self) -> TerminalLaunchState {
        self.state
    }

    #[must_use]
    pub const fn worker_generation(&self) -> Option<NonZeroU64> {
        self.worker_generation
    }

    #[must_use]
    pub const fn create_outcome(&self) -> Option<CreateOutcome> {
        self.create_outcome
    }

    #[must_use]
    pub const fn geometry(&self) -> Option<Geometry> {
        self.geometry
    }
}

/// `server-busy` is the only typed unavailability a read can report.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalLaunchReadUnavailableReason {
    ServerBusy,
}

/// The retained provenance of a profile-launched terminal: the opaque
/// Site-scoped profile identity and the catalog-private revision captured
/// when the launch command resolved the profile. It records history — the
/// entry may since have been deleted — and is absent for launches that never
/// named a profile.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    deny_unknown_fields,
    try_from = "WireLaunchProfileProvenance"
)]
pub struct LaunchProfileProvenance {
    pub(crate) profile_id: String,
    #[serde(
        serialize_with = "crate::number::decimal::serialize",
        deserialize_with = "crate::number::decimal::deserialize"
    )]
    pub(crate) catalog_revision: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireLaunchProfileProvenance {
    profile_id: String,
    #[serde(deserialize_with = "crate::number::decimal::deserialize")]
    catalog_revision: u64,
}

impl TryFrom<WireLaunchProfileProvenance> for LaunchProfileProvenance {
    type Error = ProtocolError;
    fn try_from(value: WireLaunchProfileProvenance) -> Result<Self, Self::Error> {
        Self::new(value.profile_id, value.catalog_revision)
    }
}

impl LaunchProfileProvenance {
    /// `profile_id` is the opaque `<sessionHex>:<counterHex>` grammar minted by
    /// the Server; the boundary re-checks the shared identity envelope (at
    /// most 128 bytes, non-empty) without re-parsing the opaque interior.
    pub fn new(profile_id: String, catalog_revision: u64) -> Result<Self, ProtocolError> {
        if profile_id.is_empty() || profile_id.len() > 128 {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self {
            profile_id,
            catalog_revision,
        })
    }

    #[must_use]
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    #[must_use]
    pub const fn catalog_revision(&self) -> u64 {
        self.catalog_revision
    }
}

/// `terminal.launch.read.result` — `found` answers with the resolved routing
/// and retained fact; `not-found` covers both an absent binding and a site with
/// no Workbench at all.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum TerminalLaunchReadOutcome {
    Found {
        #[serde(rename = "sessionId")]
        site_id: SiteId,
        #[serde(serialize_with = "crate::number::decimal::serialize")]
        workbench_revision: u64,
        terminal: Box<TerminalLaunchRoute>,
        launch: Box<TerminalLaunchFact>,
        /// Sibling of `launch`: the profile provenance retained at prepare,
        /// present exactly when the terminal was launched through a named
        /// profile. Absent members stay off the wire entirely.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        launch_profile: Option<LaunchProfileProvenance>,
        /// Immutable program and argv captured at creation after profile resolution.
        /// Default-shell and untracked bindings omit this member.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        launch_selection: Option<Box<crate::LaunchSelection>>,
    },
    NotFound,
    Unavailable {
        reason: TerminalLaunchReadUnavailableReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalLaunchReadResult {
    pub(crate) request_id: RequestId,
    pub(crate) outcome: TerminalLaunchReadOutcome,
}

impl TerminalLaunchReadResult {
    #[must_use]
    pub const fn new(request_id: RequestId, outcome: TerminalLaunchReadOutcome) -> Self {
        Self {
            request_id,
            outcome,
        }
    }

    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    #[must_use]
    pub const fn outcome(&self) -> &TerminalLaunchReadOutcome {
        &self.outcome
    }
}
