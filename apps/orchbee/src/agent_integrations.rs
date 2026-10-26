//! `agent integration` — the CLI-owned provider onboarding surface. This
//! module is the private owner of the accepted contract: the canonical
//! adapter semantics are ported verbatim from
//! `apps/desktop/src/main/agent-integration-management`, with the accepted
//! divergences (ambient-PATH resolution, no-replace publication, and the
//! bounded process-group runner).
//!
//! The payload source is the native cohort's `agent-observation-adapters`
//! sidecar — declared by the schema-2 manifest, admitted lazily here, and
//! never touched by Session startup. The managed capsule materializes at
//! `~/.orchbee/agent-observation-adapters/<payloadSha256>` only as an
//! explicit `enable` effect.

#[cfg(any(unix, windows))]
mod capsule;
/// Checks a distribution sidecar against its cohort's payload identity.
#[cfg(windows)]
pub(crate) fn verify_distribution_capsule(root: &std::path::Path, expected: &str) -> bool {
    capsule::verify_distribution(root, expected)
}
#[cfg(any(unix, windows))]
mod copilot;
#[cfg(any(unix, windows))]
#[cfg_attr(windows, path = "agent_integrations/exec_windows.rs")]
mod exec;
#[cfg(any(unix, windows))]
mod exec_budget;
#[cfg(any(unix, windows))]
#[cfg_attr(windows, path = "agent_integrations/files_windows.rs")]
mod files;
#[cfg(windows)]
mod hermes_windows;
#[cfg(windows)]
mod hook_configuration_windows;
#[cfg(any(unix, windows))]
mod inspect;
#[cfg(windows)]
pub(crate) use hook_configuration_windows::{HookConfigurationScan, scan_hook_configuration};
/// The closed result vocabularies stay available on every platform — the
/// parser names them — while platform adapters own filesystem and process effects.
pub(crate) mod model;
#[cfg(any(unix, windows))]
mod mutation;
#[cfg(any(unix, windows))]
mod providers;

#[cfg(test)]
mod tests;

pub(crate) use model::IntegrationId;
#[cfg(any(unix, windows))]
pub(crate) use model::{Issue, SnapshotItem, State};

#[cfg(any(unix, windows))]
mod imp {
    use std::{
        ffi::OsString,
        path::{Path, PathBuf},
        time::{Duration, Instant},
    };

    use orchbee_local_runtime::Cohort;

    use super::model::{ManagementMode, Provenance};
    use super::{IntegrationId, SnapshotItem, capsule, mutation, providers};

    pub(crate) use super::mutation::{
        AttemptedStep, Direction, FailureCode, MutationReport, PlannedStep, Refusal, StepKind,
        Verdict,
    };
    pub(crate) use super::providers::InspectionDetail;

    /// The one monotonic cooperative budget for an integration invocation —
    /// the accepted 40s default. It is a cooperative bound, not a real-time
    /// guarantee: a provider's own detached descendants sit outside it.
    const TOTAL_BUDGET: Duration = Duration::from_millis(40_000);

    /// Where the adapter payload comes from for this invocation.
    #[derive(Debug)]
    pub(crate) enum AdapterSource {
        /// The cohort sidecar — path plus the manifest-declared payload digest
        /// the capsule must match.
        Cohort {
            directory: PathBuf,
            declared_payload_sha256: String,
        },
        /// The `--adapter-source` development/test override — no manifest
        /// binding, the capsule's own validation still applies.
        Override { directory: PathBuf },
        /// No cohort could be loaded and no override was given.
        Absent,
    }

    impl AdapterSource {
        pub(crate) fn directory(&self) -> Option<&Path> {
            match self {
                Self::Cohort { directory, .. } | Self::Override { directory } => Some(directory),
                Self::Absent => None,
            }
        }

        /// The manifest-declared digest binding — `None` for the override.
        pub(crate) fn declared_payload_sha256(&self) -> Option<&str> {
            match self {
                Self::Cohort {
                    declared_payload_sha256,
                    ..
                } => Some(declared_payload_sha256),
                _ => None,
            }
        }

        pub(crate) fn kind_str(&self) -> &'static str {
            match self {
                Self::Cohort { .. } => "cohort-sidecar",
                Self::Override { .. } => "adapter-source-override",
                Self::Absent => "absent",
            }
        }
    }

    /// The assembled invocation environment.
    pub(crate) struct IntegrationContext {
        pub(crate) home: PathBuf,
        pub(crate) capsule_root: PathBuf,
        pub(crate) source: AdapterSource,
        pub(crate) deadline: Instant,
        path_env: Option<OsString>,
        /// The armed termination source — `None` when the caller never
        /// installed one (unit tests, non-unix fallbacks).
        pub(crate) cancel: Option<std::sync::Arc<dyn crate::termination_signal::TerminationSource>>,
    }

    impl IntegrationContext {
        fn provider_context(&self) -> providers::ProviderContext<'_> {
            providers::ProviderContext {
                home: &self.home,
                capsule_root: &self.capsule_root,
                adapter_source: self.source.directory(),
                path_env: self.path_env.as_deref(),
                deadline: self.deadline,
                cancel: self.cancel.as_deref(),
                // The provider registry further restricts platform capabilities;
                // Provider policy admits only the supported Windows entries.
                management_mode: ManagementMode::ManagedLocal,
            }
        }
    }

    /// The capsule-facing view a snapshot/explain carries: whether the managed
    /// capsule validates and stays in step with its source, plus the source's
    /// own admission state.
    #[derive(Debug)]
    pub(crate) struct CapsuleStatus {
        pub(crate) ready: bool,
        pub(crate) root: PathBuf,
        pub(crate) source_kind: &'static str,
        /// `Some` when the source's declared digest does not match the capsule
        /// contract — a binding failure, surfaced separately from readiness.
        pub(crate) binding_failure: bool,
        /// Identity of the current source payload, never a semantic version.
        pub(crate) payload_sha256: String,
    }

    /// Resolve the adapter source: the explicit `--adapter-source` override
    /// wins; otherwise the cohort's declared sidecar. A cohort that cannot be
    /// loaded is an absent source — the integration command answers a typed
    /// unavailable rather than failing Session-style.
    fn resolve_adapter_source(override_path: Option<&Path>, deadline: Instant) -> AdapterSource {
        if let Some(path) = override_path {
            return AdapterSource::Override {
                directory: path.to_path_buf(),
            };
        }
        match Cohort::current(deadline) {
            Ok(cohort) => {
                let sidecar = cohort.observation_adapters_sidecar();
                AdapterSource::Cohort {
                    directory: sidecar.directory().to_path_buf(),
                    declared_payload_sha256: sidecar.declared_payload_sha256().to_owned(),
                }
            }
            Err(_) => AdapterSource::Absent,
        }
    }

    /// The cohort-declared digest must equal the source capsule's own declared
    /// payloadSha256 — the binding the schema-2 manifest asserts.
    fn source_binding_ok(source: &AdapterSource) -> bool {
        let Some(declared) = source.declared_payload_sha256() else {
            return true;
        };
        let Some(directory) = source.directory() else {
            return true;
        };
        match capsule::manifest_declared_digest(directory) {
            Some(actual) => actual == declared,
            None => false,
        }
    }

    /// Assemble the invocation context. `home`/`path_env` come from the ambient
    /// environment — the tests isolate both. `cancel` is the armed termination
    /// source the frontier checks at every boundary.
    pub(crate) fn context(
        override_path: Option<&Path>,
        cancel: Option<std::sync::Arc<dyn crate::termination_signal::TerminationSource>>,
    ) -> Result<IntegrationContext, ContextError> {
        context_before(override_path, cancel, Instant::now() + TOTAL_BUDGET)
    }

    /// `context` under a caller's earlier ceiling: a read-only consumer that
    /// owns a shorter budget never inherits the full integration budget.
    pub(crate) fn context_before(
        override_path: Option<&Path>,
        cancel: Option<std::sync::Arc<dyn crate::termination_signal::TerminationSource>>,
        ceiling: Instant,
    ) -> Result<IntegrationContext, ContextError> {
        let deadline = ceiling.min(Instant::now() + TOTAL_BUDGET);
        let home = std::env::home_dir().ok_or(ContextError::NoHome)?;
        #[cfg(windows)]
        if !super::files::path_admitted(&home) {
            return Err(ContextError::NoHome);
        }
        let source = resolve_adapter_source(override_path, deadline);
        let payload_sha256 = source
            .directory()
            .and_then(capsule::manifest_declared_digest)
            .filter(|digest| {
                digest.len() == 64
                    && digest
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
            .unwrap_or_else(capsule::bundled_payload_sha256);
        let capsule_root = home
            .join(".orchbee")
            .join("agent-observation-adapters")
            .join(payload_sha256);
        Ok(IntegrationContext {
            home,
            capsule_root,
            source,
            deadline,
            path_env: std::env::var_os("PATH"),
            cancel,
        })
    }

    #[derive(Debug)]
    pub(crate) enum ContextError {
        NoHome,
    }

    fn capsule_status(context: &IntegrationContext) -> CapsuleStatus {
        let binding_failure = !source_binding_ok(&context.source);
        let ready = !binding_failure
            && capsule::is_ready(context.capsule_root.as_path(), context.source.directory());
        CapsuleStatus {
            ready,
            root: context.capsule_root.clone(),
            source_kind: context.source.kind_str(),
            binding_failure,
            payload_sha256: context
                .capsule_root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_owned(),
        }
    }

    /// `agent integration list` — inspect all providers inside one deadline.
    pub(crate) fn list(
        context: &IntegrationContext,
        provider: Option<IntegrationId>,
    ) -> (Vec<SnapshotItem>, CapsuleStatus) {
        let provider_context = context.provider_context();
        (
            match provider {
                Some(id) => vec![providers::inspect_provider(id, &provider_context)],
                None => providers::snapshot(&provider_context),
            },
            capsule_status(context),
        )
    }

    /// `agent integration explain --provider <id>` — the snapshot row, the
    /// provider's guard dimensions, and the guard-evaluated enable vector.
    pub(crate) fn explain(
        context: &IntegrationContext,
        id: IntegrationId,
    ) -> (
        SnapshotItem,
        InspectionDetail,
        CapsuleStatus,
        Vec<PlannedStep>,
    ) {
        let provider_context = context.provider_context();
        let (item, detail) = providers::inspect_provider_detail(id, &provider_context);
        let plan = mutation::plan_enable(id, &provider_context, &item, &detail);
        (item, detail, capsule_status(context), plan)
    }

    /// `agent integration enable|disable --provider <id>` — the guarded
    /// mutation.
    pub(crate) fn mutate(
        context: &IntegrationContext,
        id: IntegrationId,
        direction: Direction,
    ) -> MutationReport {
        // A cohort-declared digest that does not match the source capsule is a
        // binding failure — refuse before any effect.
        if !source_binding_ok(&context.source) {
            let provider_context = context.provider_context();
            let observed = providers::inspect_provider(id, &provider_context);
            return MutationReport {
                integration_id: id,
                direction,
                attempted: Vec::new(),
                residuals: Vec::new(),
                observed: Some(observed),
                provenance: Provenance::PreEffect,
                effect_begun: false,
                verdict: Verdict::Refused(Refusal::Conflict),
            };
        }
        let provider_context = context.provider_context();
        mutation::mutate(id, direction, &provider_context)
    }
}

#[cfg(any(target_os = "linux", windows))]
pub(crate) use capsule::bundled_payload_sha256;
#[cfg(windows)]
pub(crate) use imp::context_before;
#[cfg(any(unix, windows))]
pub(crate) use imp::{
    AttemptedStep, CapsuleStatus, Direction, FailureCode, InspectionDetail, MutationReport,
    PlannedStep, Refusal, StepKind, Verdict, context, explain, list, mutate,
};
