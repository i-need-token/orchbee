//! `agent integration` — the CLI-owned provider onboarding surface. This
//! module is the private owner of the accepted contract: the canonical
//! adapter semantics are ported verbatim from
//! `apps/desktop/src/main/agent-integration-management`, with the accepted
//! divergences (ambient-PATH resolution, no-replace publication, the
//! stale-side direction split, and the bounded process-group runner).
//!
//! The payload source is the native cohort's `agent-observation-adapters`
//! sidecar — declared by the schema-2 manifest, admitted lazily here, and
//! never touched by Site startup. The managed capsule materializes at
//! `~/.orchbee/agent-observation-adapters/<payloadVersion>` only as an
//! explicit `enable` effect.

#[cfg(unix)]
mod capsule;
#[cfg(unix)]
mod exec;
#[cfg(unix)]
mod files;
#[cfg(unix)]
mod inspect;
/// The closed result vocabularies stay available on every platform — the
/// parser names them — while every effectful owner stays unix-gated.
pub(crate) mod model;
#[cfg(unix)]
mod mutation;
#[cfg(unix)]
mod providers;

/// Test-only fault-injection seam — primitive errno injection, named step
/// boundaries, scripted provider commands, a skewed clock, and a shared
/// cancel flag. Production code consults it only under `#[cfg(test)]`.
#[cfg(all(test, unix))]
pub(crate) mod inject;

/// Root57 `provider-effects52` conformance harness — the frozen 47-case
/// pack driven against the real fs/machine paths under injected faults.
#[cfg(all(test, unix))]
mod effects52;

/// Report64 `provider-conformance64` harness — the frozen 244-case corpus
/// across all ten providers driven against the real inspect/mutate paths.
#[cfg(all(test, unix))]
mod conformance64;

/// The one real process-tree teardown witness for `exec::run_command` —
/// fake executables only, owned temporary roots, no scripted injection.
#[cfg(all(test, unix))]
mod exec_witness;

pub(crate) use model::{IntegrationId, Issue, SnapshotItem, State};

#[cfg(unix)]
mod imp {
    use std::{
        ffi::OsString,
        path::{Path, PathBuf},
        time::{Duration, Instant},
    };

    use orchbee_local_site_runtime::NativeCohort;

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
        pub(crate) legacy_roots: Vec<PathBuf>,
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
                // This build's cohort manages all ten providers locally —
                // non-unix management modes never reach this code path.
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
        /// `Some(declared)` when the capsule's payloadVersion exceeds support.
        pub(crate) unsupported_version: Option<String>,
        /// The source manifest's declared payloadVersion when one is legible —
        /// carried verbatim into the machine document.
        pub(crate) declared_payload_version: Option<String>,
    }

    /// Resolve the adapter source: the explicit `--adapter-source` override
    /// wins; otherwise the cohort's declared sidecar. A cohort that cannot be
    /// loaded is an absent source — the integration command answers a typed
    /// unavailable rather than failing Site-style.
    fn resolve_adapter_source(override_path: Option<&Path>, deadline: Instant) -> AdapterSource {
        if let Some(path) = override_path {
            return AdapterSource::Override {
                directory: path.to_path_buf(),
            };
        }
        match NativeCohort::current(deadline) {
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
        let deadline = Instant::now() + TOTAL_BUDGET;
        let home = std::env::home_dir().ok_or(ContextError::NoHome)?;
        let capsule_root = home
            .join(".orchbee")
            .join("agent-observation-adapters")
            .join(capsule::PAYLOAD_VERSION);
        let source = resolve_adapter_source(override_path, deadline);
        let legacy_roots = source
            .directory()
            .into_iter()
            .map(Path::to_path_buf)
            .collect();
        Ok(IntegrationContext {
            home,
            capsule_root,
            source,
            legacy_roots,
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
        let (unsupported_version, declared_payload_version) = context
            .source
            .directory()
            .map(|directory| {
                (
                    match capsule::validate(directory) {
                        capsule::CapsuleValidity::UnsupportedVersion { declared } => declared,
                        _ => None,
                    },
                    capsule::manifest_declared_version(directory),
                )
            })
            .unwrap_or((None, None));
        CapsuleStatus {
            ready,
            root: context.capsule_root.clone(),
            source_kind: context.source.kind_str(),
            binding_failure,
            unsupported_version,
            declared_payload_version,
        }
    }

    /// `agent integration list` — inspect all providers inside one deadline.
    pub(crate) fn list(context: &IntegrationContext) -> (Vec<SnapshotItem>, CapsuleStatus) {
        let provider_context = context.provider_context();
        (
            providers::snapshot(&provider_context, &context.legacy_roots),
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
        let (item, detail) =
            providers::inspect_provider_detail(id, &provider_context, &context.legacy_roots);
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
            let observed =
                providers::inspect_provider(id, &provider_context, &context.legacy_roots);
            return MutationReport {
                integration_id: id,
                direction,
                attempted: Vec::new(),
                residuals: Vec::new(),
                observed: Some(observed),
                provenance: Provenance::PreEffect,
                verdict: Verdict::Refused(Refusal::Conflict),
            };
        }
        let provider_context = context.provider_context();
        mutation::mutate(id, direction, &provider_context, &context.legacy_roots)
    }
}

#[cfg(unix)]
pub(crate) use capsule::PAYLOAD_VERSION;
#[cfg(unix)]
pub(crate) use imp::{
    AttemptedStep, CapsuleStatus, Direction, FailureCode, InspectionDetail, MutationReport,
    PlannedStep, Refusal, StepKind, Verdict, context, explain, list, mutate,
};
