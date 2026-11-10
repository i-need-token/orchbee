//! Owns explicit provider integration inspection, plans, and enable or disable effects using
//! the cohort-admitted observation-adapter sidecar and retained mutation outcomes. Each
//! environment installs its Capsules in its own data root and changes only the Agent entries that
//! name them. Session startup does not materialize adapter capsules, and provider integration does
//! not own Agent session state.

#[cfg(any(unix, windows))]
mod capsule;
#[cfg(windows)]
mod configuration_windows;
/// Checks a distribution sidecar against its cohort's payload identity.
#[cfg(windows)]
pub fn verify_distribution_capsule(root: &std::path::Path, expected: &str) -> bool {
    capsule::verify_distribution(root, expected)
}
#[cfg(any(unix, windows))]
mod copilot;
#[cfg(any(unix, windows))]
#[cfg_attr(windows, path = "exec_windows.rs")]
mod exec;
#[cfg(any(unix, windows))]
mod exec_budget;
#[cfg(any(unix, windows))]
#[cfg_attr(windows, path = "files_windows.rs")]
mod files;
#[cfg(windows)]
mod hermes_windows;
mod hook_configuration;
#[cfg(windows)]
mod hook_configuration_windows;
#[cfg(any(unix, windows))]
mod inspect;
pub use hook_configuration::{
    CapsuleLocation, HookConfigurationKind, HookConfigurationScan, RootPolicy, capsule_location,
    configuration_kinds, owned_by_another_environment, scan_hook_configuration,
};
#[cfg(windows)]
pub use hook_configuration_windows::ROOT_POLICY as CONFIGURATION_ROOT_POLICY;
/// The closed result vocabularies stay available on every platform — the
/// parser names them — while platform adapters own filesystem and process effects.
pub(crate) mod model;
#[cfg(any(unix, windows))]
mod mutation;
#[cfg(any(unix, windows))]
mod providers;

#[cfg(all(any(test, feature = "test-support"), unix))]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;

pub use model::IntegrationId;
#[cfg(any(unix, windows))]
pub use model::{Issue, SnapshotItem, State};

#[cfg(any(unix, windows))]
mod imp {
    use std::{
        ffi::OsString,
        path::{Path, PathBuf},
        time::{Duration, Instant},
    };

    use orchbee_local_runtime::{Cohort, DataRoot};

    use super::model::{Availability, ManagementMode, Provenance, State};
    use super::{IntegrationId, SnapshotItem, capsule, files, mutation, providers};

    pub use super::mutation::{
        AttemptedStep, Direction, FailureCode, MutationReport, PlannedStep, Refusal, StepKind,
        Verdict,
    };
    pub use super::providers::InspectionDetail;

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
    pub struct IntegrationContext {
        pub(crate) home: PathBuf,
        /// This payload's folder among the environment's Capsules,
        /// `internal/integrations/agent-observation/<version>` in its data root.
        pub(crate) capsule_root: PathBuf,
        /// The payload identity the Capsule folder holds.
        pub(crate) payload_sha256: String,
        /// `~/.orchbee/agent-observation-adapters`, the folder every environment shared before
        /// each kept its own Capsules, in release only: release owns the entries naming it.
        pub(crate) legacy_capsules: Option<PathBuf>,
        pub(crate) source: AdapterSource,
        pub(crate) deadline: Instant,
        path_env: Option<OsString>,
        /// The armed termination source — `None` when the caller never
        /// installed one (unit tests, non-unix fallbacks).
        pub(crate) cancel: Option<std::sync::Arc<dyn crate::TerminationSource>>,
    }

    impl IntegrationContext {
        fn provider_context(&self) -> providers::ProviderContext<'_> {
            providers::ProviderContext {
                home: &self.home,
                capsule_root: &self.capsule_root,
                legacy_capsules: self.legacy_capsules.as_deref(),
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

    pub use orchbee_agent_integration::CapsuleStatus;

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
    /// payloadSha256 — the binding the admitted cohort manifest asserts.
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
    pub fn context(
        override_path: Option<&Path>,
        cancel: Option<std::sync::Arc<dyn crate::TerminationSource>>,
    ) -> Result<IntegrationContext, ContextError> {
        context_before(override_path, cancel, Instant::now() + TOTAL_BUDGET)
    }

    /// `context` under a caller's earlier ceiling: a read-only consumer that
    /// owns a shorter budget never inherits the full integration budget.
    pub fn context_before(
        override_path: Option<&Path>,
        cancel: Option<std::sync::Arc<dyn crate::TerminationSource>>,
        ceiling: Instant,
    ) -> Result<IntegrationContext, ContextError> {
        let deadline = ceiling.min(Instant::now() + TOTAL_BUDGET);
        let home = std::env::home_dir().ok_or(ContextError::NoHome)?;
        let data_root =
            DataRoot::resolve(|name| std::env::var_os(name)).map_err(|_| ContextError::NoHome)?;
        #[cfg(windows)]
        if !files::path_admitted(&home) || !files::path_admitted(data_root.path()) {
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
        let version = source
            .directory()
            .and_then(capsule::declared_version)
            .unwrap_or_else(capsule::bundled_version);
        let capsule_root = capsule::installation_root(
            &data_root.agent_observation_capsules(),
            &version,
            &payload_sha256,
        );
        let legacy_capsules = legacy_capsules(&home);
        Ok(IntegrationContext {
            home,
            capsule_root,
            payload_sha256,
            legacy_capsules,
            source,
            deadline,
            path_env: std::env::var_os("PATH"),
            cancel,
        })
    }

    #[derive(Debug)]
    pub enum ContextError {
        NoHome,
    }

    /// `~/.orchbee/agent-observation-adapters`, the folder every environment's Capsules shared
    /// before each kept its own; only release Orchbee owns it.
    fn legacy_capsules(home: &Path) -> Option<PathBuf> {
        orchbee_local_runtime::is_release()
            .then(|| home.join(".orchbee").join("agent-observation-adapters"))
    }

    /// Cheap local evidence, without running provider commands or reading their configuration,
    /// that an Agent integration may have been enabled for this environment: enabling installs a
    /// Capsule in its data root, and release Orchbee once installed them in the legacy folder.
    /// `false` means neither folder exists, so no provider entry can name this environment.
    /// `true` proves nothing about current entries; an unreadable location also answers `true`.
    pub fn may_have_been_enabled() -> bool {
        let present = |path: &Path| !matches!(path.try_exists(), Ok(false));
        let Ok(data_root) = DataRoot::resolve(|name| std::env::var_os(name)) else {
            return true;
        };
        present(&data_root.agent_observation_capsules())
            || std::env::home_dir().map_or(orchbee_local_runtime::is_release(), |home| {
                legacy_capsules(&home).is_some_and(|legacy| present(&legacy))
            })
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
            payload_sha256: context.payload_sha256.clone(),
        }
    }

    /// `agent integration list` — inspect all providers inside one deadline.
    pub fn list(
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
    pub fn explain(
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
    pub fn mutate(
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
        let report = mutation::mutate(id, direction, &provider_context);
        if matches!(report.verdict, Verdict::Completed) {
            remove_unnamed_capsules(context);
        }
        report
    }

    /// After a completed enable or disable, removes this environment's other Capsule versions
    /// once a fresh inspection shows that no Agent entry can name one: no provider reads
    /// `update-required`, and every provider whose command is on PATH was inspected. A provider
    /// whose command is missing runs no hook and holds no version. The current version stays even
    /// when nothing names it, since an enable in another Session may be about to name it;
    /// uninstall removes it. Removal is housekeeping outside the mutation receipt: a refused or
    /// failed removal leaves the folder for the next completed change.
    fn remove_unnamed_capsules(context: &IntegrationContext) {
        let Some(capsules) = context.capsule_root.parent() else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(capsules) else {
            return;
        };
        let current = context.capsule_root.file_name();
        let earlier: Vec<PathBuf> = entries
            .flatten()
            .filter(|entry| Some(entry.file_name().as_os_str()) != current)
            // Version names start with a digit; staging folders start with `.`.
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with(|first: char| first.is_ascii_digit()))
            })
            .map(|entry| entry.path())
            .filter(|folder| capsule::manifest_declared_digest(folder).is_some())
            .collect();
        if earlier.is_empty() {
            return;
        }
        let provider_context = context.provider_context();
        if providers::snapshot(&provider_context)
            .iter()
            .any(may_name_another_version)
        {
            return;
        }
        for folder in earlier {
            if Instant::now() >= context.deadline {
                return;
            }
            let _ = files::remove_owned(&folder, true);
        }
    }

    fn may_name_another_version(item: &SnapshotItem) -> bool {
        item.timed_out
            || match item.state {
                State::UpdateRequired => true,
                State::Unknown => item.availability != Availability::NotFound,
                _ => false,
            }
    }
}

#[cfg(any(unix, windows))]
pub use capsule::bundled_payload_sha256;
#[cfg(any(unix, windows))]
pub use imp::{
    AttemptedStep, CapsuleStatus, Direction, FailureCode, InspectionDetail, MutationReport,
    PlannedStep, Refusal, StepKind, Verdict, context, explain, list, mutate,
};
#[cfg(any(unix, windows))]
pub use imp::{context_before, may_have_been_enabled};

/// Cooperative termination the writer observes between bounded steps.
pub trait TerminationSource {
    fn requested(&self) -> bool;
}

#[cfg(windows)]
pub use configuration_windows::{
    extension_directory, hook_configuration_candidates_for, kilo_entry, opencode_entries,
};
#[cfg(all(unix, feature = "test-support"))]
pub use test_support::{provider_conformance64_replay, provider_effects52_conformance};
