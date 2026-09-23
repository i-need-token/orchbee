//! `agent doctor`: offline provider-hook diagnostics. It answers *why*
//! Agent observations may not arrive using bounded filesystem facts only —
//! it never executes provider commands, never mutates setup or
//! configuration, never persists, and never connects to sockets for
//! liveness. `fs` is the module's only IO boundary; `probes` is the pure
//! classification half.

#[cfg(any(target_os = "linux", windows))]
mod fs;
#[cfg(any(target_os = "linux", windows))]
mod probes;
#[cfg(all(test, target_os = "linux"))]
mod tests;
#[cfg(all(test, windows))]
mod windows_tests;

/// The closed `--provider` vocabulary, in canonical row order. The grammar
/// validates against this list on every platform — selector validity is a
/// parse fact, never platform-dependent; `probes::PROVIDERS` carries exactly
/// these ids in this order (the tests pin that agreement).
pub(crate) const PROVIDER_IDS: &[&str] = &[
    "claude-code",
    "codex",
    "gemini",
    "github-copilot",
    "grok",
    "cline",
    "opencode",
    "pi",
    "omp",
    "qwen-code",
];

#[cfg(any(target_os = "linux", windows))]
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[cfg(any(target_os = "linux", windows))]
use serde_json::{Map, Value, json};

#[cfg(not(any(target_os = "linux", windows)))]
use crate::cli::stage_reason_details;
use crate::cli::{Failure, FailureKind, Output, failure_output};
#[cfg(any(target_os = "linux", windows))]
use crate::cli::{
    config::{local_runtime_source, path_detail},
    output, stage_details,
};
#[cfg(any(target_os = "linux", windows))]
use crate::termination_signal::{TerminationSignal, TerminationSource};
use crate::{invocation::OutputFormat, localization::Locale};

#[cfg(any(target_os = "linux", windows))]
use fs::{CapsuleWalk, DirEntries, EntryKind, FileRead, FingerprintWalk, ListKind, Lstat};
#[cfg(any(target_os = "linux", windows))]
use probes::{
    BootstrapVerdict, ClinePackage, HookReason, HookState, MarkerParse, ModuleUrlRoot, ProbeKind,
    Provider, RootState,
};

/// The cooperative run budget: checked between probe items — synchronous
/// filesystem operations promise no hard IO deadline.
#[cfg(any(target_os = "linux", windows))]
const RUN_BUDGET: Duration = Duration::from_secs(5);
/// Total probe items a run admits: ≤10 provider targets + ≤11 executable
/// scans (10 providers + node) + ≤20 roots + env + socket = ≤43, under 64.
#[cfg(any(target_os = "linux", windows))]
const MAX_PROBE_ITEMS: usize = 64;
/// Aggregate bytes all probes may read in one run.
#[cfg(any(target_os = "linux", windows))]
const MAX_TOTAL_READ_BYTES: u64 = 16 * 1024 * 1024;
/// `asset.roots` row cap: ≤4 supplied + ≤16 recorded.
#[cfg(any(target_os = "linux", windows))]
const MAX_ROOTS: usize = 20;
/// Recorded roots admitted across all providers.
#[cfg(any(target_os = "linux", windows))]
const MAX_RECORDED_ROOTS: usize = 16;
/// `hints` row cap.
#[cfg(any(target_os = "linux", windows))]
const MAX_HINTS: usize = 16;

#[cfg(any(target_os = "linux", windows))]
const CODEX_CONFIG_BYTES: u64 = 512 * 1024;
#[cfg(any(target_os = "linux", windows))]
const GUARDED_MARKER_BYTES: u64 = 16 * 1024;
#[cfg(any(target_os = "linux", windows))]
const CLINE_MANIFEST_BYTES: u64 = 16 * 1024;
#[cfg(any(target_os = "linux", windows))]
const CLINE_MAX_INSTALLS: usize = 128;
#[cfg(any(target_os = "linux", windows))]
const CAPSULE_MANIFEST_BYTES: u64 = 1024;
#[cfg(any(target_os = "linux", windows))]
const BOOTSTRAP_BYTES: u64 = 64 * 1024;

/// The bounded read/item/clock budget one run holds. `fs` consumes read
/// bytes through it; the orchestrator reserves probe items through it.
#[cfg(any(target_os = "linux", windows))]
pub(crate) struct Budget {
    deadline: Instant,
    items: usize,
    bytes: u64,
}

#[cfg(any(target_os = "linux", windows))]
impl Budget {
    fn new() -> Self {
        Self {
            deadline: Instant::now() + RUN_BUDGET,
            items: 0,
            bytes: 0,
        }
    }

    pub(crate) fn remaining_bytes(&self) -> u64 {
        MAX_TOTAL_READ_BYTES.saturating_sub(self.bytes)
    }

    pub(crate) fn consume(&mut self, bytes: u64) {
        self.bytes = self.bytes.saturating_add(bytes);
    }

    /// Reserves one probe item — `false` when the item cap or the
    /// cooperative deadline is already exhausted. The reservation is the
    /// count: an item that returns `false` was never started.
    fn take_item(&mut self) -> bool {
        if self.items >= MAX_PROBE_ITEMS || Instant::now() >= self.deadline {
            return false;
        }
        self.items += 1;
        true
    }
}

#[cfg(any(target_os = "linux", windows))]
/// A recorded root candidate declared inside an admitted provider file.
struct RootCandidate {
    /// The provider whose file declared the root.
    provider: &'static str,
    /// The declaring field: `source` or `moduleUrl`.
    field: &'static str,
    /// The declared absolute path bytes — never lossy-decoded.
    path: Vec<u8>,
}

#[cfg(any(target_os = "linux", windows))]
/// One probed root row — the `asset.roots` entry plus the state the
/// declaring provider's hook mapping consumes.
struct RootOutcome {
    state: RootState,
    detail: Map<String, Value>,
}

#[cfg(any(target_os = "linux", windows))]
/// The hook outcome a consistent recorded root unlocks.
enum ConsistentOutcome {
    /// Codex: `enabled=false` → `disabled`; `true` → `present` with
    /// `enabledFlag:true`; absent → `present` with `enabledFlag:"absent"`.
    Codex { enabled_flag: Option<bool> },
    /// A self-consistent declaration — `present` with the carried reason.
    Present { reason: HookReason },
}

#[cfg(any(target_os = "linux", windows))]
/// The target probe's raw result before the recorded-root fold.
enum HookProbe {
    /// A finished verdict with its detail object.
    Final {
        state: HookState,
        reason: HookReason,
        detail: Map<String, Value>,
    },
    /// The declaration's recorded root governs (or, for an explicit
    /// `enabled=false`, still contributes its fact row). `root_index`
    /// indexes the recorded-roots list; `None` means the declared root
    /// could not be admitted.
    AwaitsRoot {
        root_index: Option<usize>,
        consistent: ConsistentOutcome,
        detail: Map<String, Value>,
    },
}

#[cfg(any(target_os = "linux", windows))]
fn final_verdict(state: HookState, reason: HookReason) -> HookProbe {
    HookProbe::Final {
        state,
        reason,
        detail: Map::new(),
    }
}

/// `orchbee agent doctor [--provider <id>]... [--asset-root <path>]...
/// [--format text|json]` — one offline diagnostic document; exit 0 whenever
/// a complete or partial document was produced.
#[cfg(any(target_os = "linux", windows))]
pub(crate) fn run(
    providers: &[String],
    asset_roots: &[String],
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let signal = match TerminationSignal::install() {
        Ok(signal) => signal,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    match inspect(providers, asset_roots, &signal) {
        Err(failure) => failure_output(failure, Some(format), locale),
        Ok(run) => emit(run, format, locale),
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
pub(crate) fn run(
    _providers: &[String],
    _asset_roots: &[String],
    format: OutputFormat,
    locale: Locale,
) -> Output {
    // C-4: non-Linux is a command-level refusal — the failure envelope
    // carries the stage detail; no provider rows ever exist. The `command`
    // detail names this command so the shared platform arm never credits
    // the refusal to `config check`.
    let mut details = stage_reason_details("platform", "unsupported-platform");
    details.insert("command".to_owned(), serde_json::json!("agent doctor"));
    failure_output(
        Failure::new(FailureKind::CheckFailed).details(details),
        Some(format),
        locale,
    )
}

#[cfg(any(target_os = "linux", windows))]
/// The facts the document assembles — provider rows in canonical order,
/// root rows in flag-then-declaration order, the shared environment and
/// endpoint facts, and the completeness bit.
struct DoctorRun {
    providers: Vec<Value>,
    roots: Vec<Value>,
    endpoint: Value,
    environment: Value,
    complete: bool,
}

/// The bounded probe sequence: provider targets and executable scans in
/// canonical order, the single node scan, roots (supplied in flag order,
/// recorded in declaration order), then environment and endpoint — every
/// item behind a budget checkpoint and the pending termination signal.
#[cfg(any(target_os = "linux", windows))]
fn inspect(
    selected: &[String],
    supplied: &[String],
    signal: &TerminationSignal,
) -> Result<DoctorRun, Failure> {
    let mut budget = Budget::new();
    let mut complete = true;
    let home = fs::home_directory();
    let path_env = fs::inherited_path();

    let providers: Vec<&Provider> = probes::PROVIDERS
        .iter()
        .filter(|provider| selected.is_empty() || selected.iter().any(|id| id == provider.id))
        .collect();

    let mut recorded: Vec<RootCandidate> = Vec::new();
    let mut pending: Vec<HookProbe> = Vec::with_capacity(providers.len());
    let mut executable_facts: Vec<Value> = Vec::with_capacity(providers.len());

    for provider in &providers {
        if signal.requested() {
            return Err(Failure::new(FailureKind::Cancelled).details(stage_details("agent-doctor")));
        }
        // One provider target probe is one item; a failed checkpoint leaves
        // an honest `budget-exhausted` fact instead of a guess.
        pending.push(if budget.take_item() {
            match home.as_deref() {
                Some(home) => probe_provider(provider, home, &mut recorded, &mut budget),
                None => match provider.kind {
                    ProbeKind::ProviderCommand => final_verdict(
                        HookState::Unsupported,
                        HookReason::ProviderCommandInspectionRequired,
                    ),
                    _ => HookProbe::Final {
                        state: HookState::Unknown,
                        reason: HookReason::Unreadable,
                        detail: Map::from_iter([("home".to_owned(), json!("unset"))]),
                    },
                },
            }
        } else {
            complete = false;
            final_verdict(HookState::Unknown, HookReason::BudgetExhausted)
        });

        if signal.requested() {
            return Err(Failure::new(FailureKind::Cancelled).details(stage_details("agent-doctor")));
        }
        executable_facts.push(if budget.take_item() {
            executable_fact(fs::scan_path_for(
                provider.executable,
                path_env_or(&path_env),
            ))
        } else {
            complete = false;
            json!({ "state": "unknown", "reason": "budget-exhausted" })
        });
    }

    // The one node scan serves every command-hook provider — item 11 of
    // the ≤11 executable-scan domain.
    if signal.requested() {
        return Err(Failure::new(FailureKind::Cancelled).details(stage_details("agent-doctor")));
    }
    let node_fact = if budget.take_item() {
        executable_fact(fs::scan_path_for("node", path_env_or(&path_env)))
    } else {
        complete = false;
        json!({ "state": "unknown", "reason": "budget-exhausted" })
    };

    // Root probes: supplied roots in flag order, then recorded roots in
    // declaration order — each root is one probe item.
    let supplied_admitted = supplied.len().min(MAX_ROOTS);
    let mut ordered: Vec<(Vec<u8>, String)> = Vec::new();
    for path in supplied.iter().take(supplied_admitted) {
        ordered.push((path.as_bytes().to_vec(), "supplied".to_owned()));
    }
    let recorded_admitted = recorded.len().min(MAX_ROOTS - supplied_admitted);
    for candidate in recorded.iter().take(recorded_admitted) {
        ordered.push((
            candidate.path.clone(),
            format!("recorded:{}:{}", candidate.provider, candidate.field),
        ));
    }
    if supplied.len() > supplied_admitted || recorded.len() > recorded_admitted {
        complete = false;
    }

    let mut roots: Vec<Value> = Vec::with_capacity(ordered.len());
    let mut root_states: Vec<RootState> = Vec::with_capacity(ordered.len());
    for (path_bytes, origin) in &ordered {
        if signal.requested() {
            return Err(Failure::new(FailureKind::Cancelled).details(stage_details("agent-doctor")));
        }
        let outcome = if budget.take_item() {
            probe_root(path_bytes, &mut budget)
        } else {
            complete = false;
            RootOutcome {
                state: RootState::BudgetExhausted,
                detail: Map::new(),
            }
        };
        root_states.push(outcome.state);
        let mut row = path_bytes_field(path_bytes);
        row.insert("origin".to_owned(), json!(origin));
        row.insert("state".to_owned(), json!(outcome.state.as_str()));
        if !outcome.detail.is_empty() {
            row.insert("detail".to_owned(), Value::Object(outcome.detail));
        }
        roots.push(Value::Object(row));
    }

    // Provider rows finalize once every recorded root's state is known.
    let mut provider_rows: Vec<Value> = Vec::with_capacity(providers.len());
    for (index, provider) in providers.iter().enumerate() {
        let hook_probe = std::mem::replace(
            &mut pending[index],
            final_verdict(HookState::Unknown, HookReason::NotInspected),
        );
        let (state, reason, detail) =
            finalize_hook(hook_probe, &recorded, supplied_admitted, &root_states);
        let mut hook = Map::from_iter([
            ("state".to_owned(), json!(state.as_str())),
            ("reason".to_owned(), json!(reason.as_str())),
        ]);
        if !detail.is_empty() {
            hook.insert("detail".to_owned(), Value::Object(detail));
        }
        let node = if provider.node_applicable {
            node_fact.clone()
        } else {
            json!({ "state": "not-applicable" })
        };
        provider_rows.push(json!({
            "id": provider.id,
            "executable": executable_facts[index],
            "runtime": { "node": node },
            "hook": hook,
        }));
    }

    // Shared observations: environment then endpoint, one item each.
    if signal.requested() {
        return Err(Failure::new(FailureKind::Cancelled).details(stage_details("agent-doctor")));
    }
    let environment = if budget.take_item() {
        json!({ "observationSocketEnv": environment_fact() })
    } else {
        complete = false;
        json!({ "observationSocketEnv": { "state": "unknown", "reason": "budget-exhausted" } })
    };
    if signal.requested() {
        return Err(Failure::new(FailureKind::Cancelled).details(stage_details("agent-doctor")));
    }
    let endpoint = if budget.take_item() {
        json!({ "socket": socket_fact() })
    } else {
        complete = false;
        json!({ "socket": { "state": "unknown", "reason": "budget-exhausted" } })
    };

    Ok(DoctorRun {
        providers: provider_rows,
        roots,
        endpoint,
        environment,
        complete,
    })
}

/// The inherited `PATH` value as raw bytes for the scanner — an unset PATH
/// scans as empty (every directory admission fails).
#[cfg(any(target_os = "linux", windows))]
fn path_env_or(path_env: &Option<std::ffi::OsString>) -> &std::ffi::OsStr {
    path_env
        .as_deref()
        .unwrap_or_else(|| std::ffi::OsStr::new(""))
}

/// Folds the recorded-root outcome into a pending hook verdict. The root's
/// observed state is always retained as a bounded detail fact — an explicit
/// `disabled` verdict never erases an observed root problem.
#[cfg(any(target_os = "linux", windows))]
fn finalize_hook(
    probe: HookProbe,
    recorded: &[RootCandidate],
    supplied_admitted: usize,
    root_states: &[RootState],
) -> (HookState, HookReason, Map<String, Value>) {
    let HookProbe::AwaitsRoot {
        root_index,
        consistent,
        mut detail,
    } = probe
    else {
        let HookProbe::Final {
            state,
            reason,
            detail,
        } = probe
        else {
            unreachable!();
        };
        return (state, reason, detail);
    };
    // `root_index` indexes `recorded`; the probe order is supplied-then-
    // recorded, so the probed position is `supplied_admitted + index`.
    let (root_state, root_path) = root_index
        .and_then(|index| {
            let ordered = supplied_admitted.checked_add(index)?;
            root_states
                .get(ordered)
                .copied()
                .map(|state| (state, recorded.get(index).map(|r| r.path.as_slice())))
        })
        .unwrap_or((RootState::NotInspected, None));
    if let Some(path) = root_path {
        let mut recorded_detail = path_bytes_field(path);
        recorded_detail.insert("state".to_owned(), json!(root_state.as_str()));
        detail.insert("recordedRoot".to_owned(), Value::Object(recorded_detail));
    }
    match consistent {
        // An explicit `enabled = false` is terminal: the verdict is
        // `disabled` regardless of the root outcome — the root's state
        // survives only as the detail fact attached above.
        ConsistentOutcome::Codex {
            enabled_flag: Some(false),
        } => (HookState::Disabled, HookReason::DisabledFlag, detail),
        ConsistentOutcome::Codex { .. } | ConsistentOutcome::Present { .. } => {
            match root_state.recorded_hook_outcome() {
                Some((state, reason)) => (state, reason, detail),
                None => match consistent {
                    ConsistentOutcome::Present { reason } => (HookState::Present, reason, detail),
                    ConsistentOutcome::Codex { .. } => (
                        HookState::Present,
                        HookReason::ConfigDeclaresInstall,
                        detail,
                    ),
                },
            }
        }
    }
}

/// One provider's hook-target probe — dispatches on the declared
/// inspection kind.
#[cfg(any(target_os = "linux", windows))]
fn probe_provider(
    provider: &Provider,
    home: &Path,
    recorded: &mut Vec<RootCandidate>,
    budget: &mut Budget,
) -> HookProbe {
    match provider.kind {
        ProbeKind::ProviderCommand => HookProbe::Final {
            state: HookState::Unsupported,
            reason: HookReason::ProviderCommandInspectionRequired,
            detail: provider
                .manual_command
                .map(|command| Map::from_iter([("manualCommand".to_owned(), json!(command))]))
                .unwrap_or_default(),
        },
        ProbeKind::CodexConfig => probe_codex(provider, home, recorded, budget),
        ProbeKind::GuardedDirectory => probe_guarded(provider, home, budget),
        ProbeKind::Bootstrap => probe_bootstrap(provider, home, recorded, budget),
        ProbeKind::Cline => probe_cline(provider, home, budget),
    }
}

/// `~/.codex/config.toml`: the line-scanned declarations in Root43's total
/// order — uninspectable → `unknown`; recognized contradictions →
/// `conflict`; a non-boolean `enabled` → `unknown`/`malformed` (scanner
/// syntax is not proof of contradiction); no plugin → `absent`; explicit
/// `enabled = false` → `disabled` (the recorded root still contributes its
/// detail row); `true`/unspecified → the recorded-root outcome.
#[cfg(any(target_os = "linux", windows))]
fn probe_codex(
    provider: &Provider,
    home: &Path,
    recorded: &mut Vec<RootCandidate>,
    budget: &mut Budget,
) -> HookProbe {
    #[cfg(target_os = "linux")]
    let target = home.join(provider.home_target);
    #[cfg(windows)]
    let target = match fs::codex_configuration_path(home) {
        Some(path) => path,
        None => {
            return HookProbe::Final {
                state: HookState::Unknown,
                reason: HookReason::Unreadable,
                detail: Map::from_iter([(
                    "configurationRoot".to_owned(),
                    json!("invalid-CODEX_HOME"),
                )]),
            };
        }
    };
    if !fs::path_admitted(&target) {
        return target_too_long(&target);
    }
    let bytes = match fs::read_regular(&target, CODEX_CONFIG_BYTES, budget) {
        FileRead::Bytes(bytes) => bytes,
        FileRead::Absent => return final_verdict(HookState::Absent, HookReason::TargetMissing),
        read => return read_uninspectable(read),
    };
    let scan = probes::scan_codex_config(&bytes);
    // Recognized structural contradictions — the only `conflict` arms.
    if scan.duplicate_key {
        return final_verdict(HookState::Conflict, HookReason::DuplicateKey);
    }
    if scan.duplicate_section {
        return final_verdict(HookState::Conflict, HookReason::DuplicateSection);
    }
    if scan.plugin_present && !scan.marketplace_present {
        return final_verdict(HookState::Conflict, HookReason::PluginWithoutMarketplace);
    }
    let mut root_index = None;
    if scan.marketplace_present {
        if scan.source_type.as_deref() != Some("local") {
            return final_verdict(HookState::Conflict, HookReason::NonLocalSource);
        }
        match &scan.source {
            Some(source) if Path::new(source).is_absolute() => {
                root_index =
                    recorded_root(recorded, provider, "source", source.clone().into_bytes());
            }
            _ => return final_verdict(HookState::Conflict, HookReason::SourceNotAbsolute),
        }
    }
    // `enabled` present but not a boolean: malformed input, honestly
    // `unknown` — never `conflict` (unrecognized syntax is not a proof of
    // contradiction, and never a `disabled` guess).
    if scan.enabled == Some(None) {
        return final_verdict(HookState::Unknown, HookReason::Malformed);
    }
    if !scan.plugin_present {
        return final_verdict(HookState::Absent, HookReason::NoManagedInstall);
    }
    let enabled_flag = scan.enabled.flatten();
    let detail = Map::from_iter([(
        "enabledFlag".to_owned(),
        match enabled_flag {
            Some(flag) => json!(flag),
            None => json!("absent"),
        },
    )]);
    // The recorded root is awaited for both arms: `true`/unspecified let it
    // decide; explicit `false` keeps the verdict `disabled` while the
    // root's observed state still lands in `detail.recordedRoot`.
    HookProbe::AwaitsRoot {
        root_index,
        consistent: ConsistentOutcome::Codex { enabled_flag },
        detail,
    }
}

/// A guarded extension directory (`gemini`, `qwen-code`): directory →
/// marker → fingerprint self-consistency. `disabled` is unreachable — the
/// format carries no enablement field.
#[cfg(any(target_os = "linux", windows))]
fn probe_guarded(provider: &Provider, home: &Path, budget: &mut Budget) -> HookProbe {
    let target = home.join(provider.home_target);
    if !fs::path_admitted(&target) {
        return target_too_long(&target);
    }
    match fs::lstat(&target) {
        Lstat::Absent => return final_verdict(HookState::Absent, HookReason::TargetMissing),
        Lstat::Symlink => {
            return final_verdict(HookState::Unknown, HookReason::SymlinkUninspected);
        }
        Lstat::Unreadable => {
            return final_verdict(HookState::Unknown, HookReason::Unreadable);
        }
        Lstat::Entry { kind, .. } if kind != EntryKind::Directory => {
            return final_verdict(HookState::Conflict, HookReason::ForeignContent);
        }
        Lstat::Entry { .. } => {}
    }
    let marker_path = target.join(probes::GUARDED_MARKER_NAME);
    if !fs::path_admitted(&marker_path) {
        return target_too_long(&marker_path);
    }
    let bytes = match fs::read_regular(&marker_path, GUARDED_MARKER_BYTES, budget) {
        FileRead::Bytes(bytes) => bytes,
        // A directory at our managed path without our marker is a foreign
        // occupant — an observable contradiction, not an uninspectable one.
        FileRead::Absent => {
            return final_verdict(HookState::Conflict, HookReason::ForeignContent);
        }
        read => return read_uninspectable(read),
    };
    let (contract_version, declared) = match probes::parse_guarded_marker(&bytes, provider.id) {
        MarkerParse::Valid {
            contract_version,
            files,
        } => (contract_version, files),
        MarkerParse::UnrecognizedVersion(version) => {
            return HookProbe::Final {
                state: HookState::Unknown,
                reason: HookReason::ContractVersionUnrecognized,
                detail: Map::from_iter([("declaredContractVersion".to_owned(), json!(version))]),
            };
        }
        MarkerParse::NameMismatch => {
            return final_verdict(HookState::Conflict, HookReason::NameMismatch);
        }
        MarkerParse::Invalid => {
            return final_verdict(HookState::Conflict, HookReason::MarkerMismatch);
        }
    };
    let mut computed = match fs::fingerprint_walk(&target, budget) {
        FingerprintWalk::Entries(computed) => computed,
        FingerprintWalk::UnsafeKind => {
            return final_verdict(HookState::Unknown, HookReason::UnsafeFileKind);
        }
        FingerprintWalk::OverLimit => {
            return final_verdict(HookState::Unknown, HookReason::NotInspected);
        }
        FingerprintWalk::BudgetExhausted => {
            return final_verdict(HookState::Unknown, HookReason::BudgetExhausted);
        }
        // The directory was `lstat`-admitted moments ago; a vanished or
        // racing target is uninspectable, never a proof of mismatch.
        FingerprintWalk::Absent
        | FingerprintWalk::Symlink
        | FingerprintWalk::NotDirectory
        | FingerprintWalk::Unreadable => {
            return final_verdict(HookState::Unknown, HookReason::Unreadable);
        }
    };
    if !probes::fingerprint_matches(&mut computed, &declared) {
        return final_verdict(HookState::Conflict, HookReason::MarkerMismatch);
    }
    if contract_version == probes::SUPPORTED_CONTRACT_VERSION {
        final_verdict(HookState::Present, HookReason::MarkerConsistent)
    } else {
        final_verdict(
            HookState::UpdateRequired,
            HookReason::ContractVersionBelowSupported,
        )
    }
}

/// A bootstrap module file (`opencode`, `pi`, `omp`): marker + exact
/// template self-derivation, then the embedded `moduleUrl`'s recorded
/// root. `disabled` is unreachable — the format carries no enablement
/// field.
#[cfg(any(target_os = "linux", windows))]
fn probe_bootstrap(
    provider: &Provider,
    home: &Path,
    recorded: &mut Vec<RootCandidate>,
    budget: &mut Budget,
) -> HookProbe {
    let Some(shape) = &provider.bootstrap else {
        return final_verdict(HookState::Unknown, HookReason::NotInspected);
    };
    let target = home.join(provider.home_target);
    if !fs::path_admitted(&target) {
        return target_too_long(&target);
    }
    let bytes = match fs::read_regular(&target, BOOTSTRAP_BYTES, budget) {
        FileRead::Bytes(bytes) => bytes,
        FileRead::Absent => return final_verdict(HookState::Absent, HookReason::TargetMissing),
        read => return read_uninspectable(read),
    };
    let scan = probes::scan_bootstrap(&bytes, provider.id, shape);
    // A parseable `moduleUrl` contributes its recorded-root candidate even
    // when the body mismatches — the declaration's root is still a bounded
    // observable fact.
    if let Some(url) = &scan.module_url
        && let ModuleUrlRoot::Root(root) = probes::derive_module_root(url, shape.suffix)
    {
        let _ = recorded_root(recorded, provider, "moduleUrl", root);
    }
    match scan.verdict {
        BootstrapVerdict::ForeignContent => {
            final_verdict(HookState::Conflict, HookReason::ForeignContent)
        }
        BootstrapVerdict::NameMismatch => {
            final_verdict(HookState::Conflict, HookReason::NameMismatch)
        }
        BootstrapVerdict::TemplateMismatch => {
            final_verdict(HookState::Conflict, HookReason::TemplateMismatch)
        }
        BootstrapVerdict::Present => {
            let url = scan.module_url.as_deref().unwrap_or_default();
            match probes::derive_module_root(url, shape.suffix) {
                ModuleUrlRoot::Root(_) => HookProbe::AwaitsRoot {
                    // The root was registered just above — its index is the
                    // last recorded entry for this provider.
                    root_index: recorded.iter().rposition(|root| {
                        root.provider == provider.id && root.field == "moduleUrl"
                    }),
                    consistent: ConsistentOutcome::Present {
                        reason: HookReason::BootstrapMarkerConsistent,
                    },
                    detail: Map::new(),
                },
                failure => HookProbe::Final {
                    state: HookState::Unknown,
                    reason: HookReason::RecordedRootUninspectable,
                    detail: Map::from_iter([(
                        "recordedRoot".to_owned(),
                        json!({
                            "state": "uninspectable",
                            "reason": failure.failure_reason().unwrap_or("module-url-invalid"),
                        }),
                    )]),
                },
            }
        }
    }
}

/// `~/.cline/plugins`: foreign direct-plugin names conflict; the managed
/// `_installed/local` tree scans each `package/` for the entry payload and
/// its `package.json` marker. `disabled` is unreachable — the format
/// carries no enablement field.
#[cfg(any(target_os = "linux", windows))]
fn probe_cline(provider: &Provider, home: &Path, budget: &mut Budget) -> HookProbe {
    let root = home.join(provider.home_target);
    if !fs::path_admitted(&root) {
        return target_too_long(&root);
    }
    // Direct-plugin names under the plugins root are foreign occupants of
    // the managed namespace — the managed format installs under
    // `_installed/local` only.
    for name in [
        "orchbee-agent-observation.js",
        "orchbee-agent-observation.ts",
        "orchbee-agent-observation",
    ] {
        match fs::lstat(&root.join(name)) {
            Lstat::Absent => {}
            Lstat::Symlink => {
                return final_verdict(HookState::Unknown, HookReason::SymlinkUninspected);
            }
            Lstat::Unreadable => {
                return final_verdict(HookState::Unknown, HookReason::Unreadable);
            }
            Lstat::Entry { .. } => {
                return final_verdict(HookState::Conflict, HookReason::ForeignContent);
            }
        }
    }
    let installed = root.join("_installed/local");
    if !fs::path_admitted(&installed) {
        return target_too_long(&installed);
    }
    let entries = match fs::list_dir(&installed, CLINE_MAX_INSTALLS) {
        DirEntries::Entries(entries) => entries,
        DirEntries::Absent => {
            return final_verdict(HookState::Absent, HookReason::NoManagedInstall);
        }
        DirEntries::Symlink => {
            return final_verdict(HookState::Unknown, HookReason::SymlinkUninspected);
        }
        DirEntries::NotDirectory => {
            return final_verdict(HookState::Conflict, HookReason::ForeignContent);
        }
        DirEntries::OverLimit => {
            return final_verdict(HookState::Unknown, HookReason::NotInspected);
        }
        DirEntries::Unreadable => {
            return final_verdict(HookState::Unknown, HookReason::Unreadable);
        }
    };
    let mut current = 0usize;
    let mut update_required = 0usize;
    for entry in entries {
        if entry.kind == ListKind::Symlink {
            return final_verdict(HookState::Unknown, HookReason::SymlinkUninspected);
        }
        if entry.kind != ListKind::Directory {
            continue;
        }
        let package = installed.join(&entry.name).join("package");
        if !fs::path_admitted(&package) {
            return target_too_long(&package);
        }
        // The managed payload's entry file decides whether this install
        // belongs to the integration at all.
        match fs::lstat(&package.join(probes::CLINE_ENTRY_PATH)) {
            Lstat::Absent => continue,
            Lstat::Symlink => {
                return final_verdict(HookState::Unknown, HookReason::SymlinkUninspected);
            }
            Lstat::Unreadable => {
                return final_verdict(HookState::Unknown, HookReason::Unreadable);
            }
            Lstat::Entry { .. } => {}
        }
        let manifest = package.join("package.json");
        if !fs::path_admitted(&manifest) {
            return target_too_long(&manifest);
        }
        let bytes = match fs::read_regular(&manifest, CLINE_MANIFEST_BYTES, budget) {
            FileRead::Bytes(bytes) => bytes,
            // The payload entry exists but the declaring manifest does
            // not — a proven contradiction, not an uninspectable file.
            FileRead::Absent => {
                return final_verdict(HookState::Conflict, HookReason::MarkerMismatch);
            }
            read => return read_uninspectable(read),
        };
        match probes::parse_cline_package(&bytes) {
            ClinePackage::Current => current += 1,
            ClinePackage::UpdateRequired => update_required += 1,
            ClinePackage::UnrecognizedVersion => {
                return final_verdict(HookState::Unknown, HookReason::ContractVersionUnrecognized);
            }
            ClinePackage::Conflict => {
                return final_verdict(HookState::Conflict, HookReason::MarkerMismatch);
            }
        }
    }
    match (current, update_required) {
        (1, 0) => final_verdict(HookState::Present, HookReason::MarkerConsistent),
        (0, 1) => final_verdict(
            HookState::UpdateRequired,
            HookReason::ContractVersionBelowSupported,
        ),
        (0, 0) => final_verdict(HookState::Absent, HookReason::NoManagedInstall),
        // Coexisting managed installs contradict the single-install
        // declaration shape.
        _ => final_verdict(HookState::Conflict, HookReason::Malformed),
    }
}

/// Registers a recorded-root candidate; `Some(index)` when admitted, `None`
/// when the ≤16 recorded cap is already full.
#[cfg(any(target_os = "linux", windows))]
fn recorded_root(
    recorded: &mut Vec<RootCandidate>,
    provider: &Provider,
    field: &'static str,
    path: Vec<u8>,
) -> Option<usize> {
    if recorded.len() >= MAX_RECORDED_ROOTS {
        return None;
    }
    let index = recorded.len();
    recorded.push(RootCandidate {
        provider: provider.id,
        field,
        path,
    });
    Some(index)
}

/// A target path exceeding the byte admission cap — honestly
/// `not-inspected` with the offending path attached, never truncated.
#[cfg(any(target_os = "linux", windows))]
fn target_too_long(path: &Path) -> HookProbe {
    let (key, value) = path_detail(path);
    HookProbe::Final {
        state: HookState::Unknown,
        reason: HookReason::NotInspected,
        detail: Map::from_iter([(key, value)]),
    }
}

/// The uniform uninspectable-file mapping — every bounded-IO failure is
/// `unknown`, never a guessed contradiction.
#[cfg(any(target_os = "linux", windows))]
fn read_uninspectable(read: FileRead) -> HookProbe {
    let reason = match read {
        FileRead::Unreadable => HookReason::Unreadable,
        FileRead::Symlink => HookReason::SymlinkUninspected,
        FileRead::UnsafeKind => HookReason::UnsafeFileKind,
        // A file past its byte cap reads as `unreadable` like the source
        // adapters' own bounded read (CDX-02) — collection caps are the
        // `not-inspected` arm, file caps are not.
        FileRead::OverLimit => HookReason::Unreadable,
        FileRead::BudgetExhausted => HookReason::BudgetExhausted,
        FileRead::Absent | FileRead::Bytes(_) => unreachable!(),
    };
    final_verdict(HookState::Unknown, reason)
}

/// One asset-root probe: `lstat` → bounded manifest → bounded payload walk
/// → digest equality. Only a proven non-conforming artifact is
/// `inconsistent`; every bound/race/IO failure is an explicit
/// uninspectable state.
#[cfg(any(target_os = "linux", windows))]
fn probe_root(path_bytes: &[u8], budget: &mut Budget) -> RootOutcome {
    if path_bytes.is_empty() || path_bytes.len() > fs::MAX_PATH_BYTES {
        return RootOutcome {
            state: RootState::PathTooLong,
            detail: Map::new(),
        };
    }
    let path = fs::path_from_bytes(path_bytes);
    match fs::lstat(&path) {
        Lstat::Absent => {
            return RootOutcome {
                state: RootState::Absent,
                detail: Map::new(),
            };
        }
        Lstat::Symlink => {
            return RootOutcome {
                state: RootState::Unsafe,
                detail: Map::new(),
            };
        }
        Lstat::Entry { kind, .. } if kind != EntryKind::Directory => {
            return RootOutcome {
                state: RootState::Unsafe,
                detail: Map::new(),
            };
        }
        Lstat::Unreadable => {
            return RootOutcome {
                state: RootState::Unreadable,
                detail: Map::new(),
            };
        }
        Lstat::Entry { .. } => {}
    }
    let manifest_path = path.join(probes::CAPSULE_MANIFEST_NAME);
    if !fs::path_admitted(&manifest_path) {
        return RootOutcome {
            state: RootState::PathTooLong,
            detail: Map::new(),
        };
    }
    let bytes = match fs::read_regular(&manifest_path, CAPSULE_MANIFEST_BYTES, budget) {
        FileRead::Bytes(bytes) => bytes,
        // The directory exists but the required manifest artifact does
        // not — a proven non-conforming root.
        FileRead::Absent => {
            return RootOutcome {
                state: RootState::Inconsistent,
                detail: Map::new(),
            };
        }
        FileRead::Symlink | FileRead::UnsafeKind => {
            return RootOutcome {
                state: RootState::Unsafe,
                detail: Map::new(),
            };
        }
        FileRead::OverLimit => {
            return RootOutcome {
                state: RootState::NotInspected,
                detail: Map::new(),
            };
        }
        FileRead::BudgetExhausted => {
            return RootOutcome {
                state: RootState::BudgetExhausted,
                detail: Map::new(),
            };
        }
        FileRead::Unreadable => {
            return RootOutcome {
                state: RootState::Unreadable,
                detail: Map::new(),
            };
        }
    };
    let Some(scan) = probes::scan_capsule_manifest(&bytes) else {
        // Bounded parse proved the manifest is not a conforming object.
        return RootOutcome {
            state: RootState::Inconsistent,
            detail: Map::new(),
        };
    };
    let mut detail = Map::new();
    if let Some(version) = scan.declared_contract_version {
        detail.insert("declaredContractVersion".to_owned(), json!(version));
    }
    if let Some(version) = &scan.declared_payload_version {
        detail.insert("declaredPayloadVersion".to_owned(), json!(version));
    }
    if scan.version_mismatch {
        // A differently-versioned capsule format cannot be evaluated by
        // this build — uninspectable, never broken.
        return RootOutcome {
            state: RootState::Unreadable,
            detail,
        };
    }
    if !scan.valid_shape {
        return RootOutcome {
            state: RootState::Inconsistent,
            detail,
        };
    }
    let mut entries = match fs::capsule_walk(&path, budget) {
        CapsuleWalk::Entries(entries) => entries,
        CapsuleWalk::UnsafeKind => {
            return RootOutcome {
                state: RootState::Unsafe,
                detail,
            };
        }
        CapsuleWalk::OverLimit => {
            return RootOutcome {
                state: RootState::NotInspected,
                detail,
            };
        }
        CapsuleWalk::BudgetExhausted => {
            return RootOutcome {
                state: RootState::BudgetExhausted,
                detail,
            };
        }
        CapsuleWalk::Absent
        | CapsuleWalk::Symlink
        | CapsuleWalk::NotDirectory
        | CapsuleWalk::Unreadable => {
            return RootOutcome {
                state: RootState::Unreadable,
                detail,
            };
        }
    };
    let digest = probes::capsule_payload_digest(&mut entries);
    let consistent = scan.declared_payload_sha256.as_deref() == Some(digest.as_str());
    RootOutcome {
        state: if consistent {
            RootState::Consistent
        } else {
            RootState::Inconsistent
        },
        detail,
    }
}

/// One executable fact row: `present` carries the found path, `absent`
/// carries the scan's truncation bit.
#[cfg(any(target_os = "linux", windows))]
fn executable_fact(scan: fs::PathScan) -> Value {
    match &scan.found {
        Some(bytes) => {
            let mut detail = path_bytes_field(bytes);
            if scan.truncated {
                detail.insert("truncated".to_owned(), json!(true));
            }
            json!({ "state": "present", "detail": detail })
        }
        None => {
            if scan.truncated {
                json!({ "state": "absent", "detail": { "truncated": true } })
            } else {
                json!({ "state": "absent" })
            }
        }
    }
}

/// The `ORCHBEE_AGENT_OBSERVATION_SOCKET_V3` fact — the override hook
/// payloads read; only its shape is classified, never interpreted.
#[cfg(any(target_os = "linux", windows))]
fn environment_fact() -> Value {
    match fs::env_value(probes::OBSERVATION_SOCKET_ENV) {
        fs::EnvValue::Unset => json!({ "state": "unset" }),
        fs::EnvValue::Value(bytes) => {
            if bytes.is_empty() {
                json!({ "state": "empty" })
            } else if bytes.len() > fs::MAX_PATH_BYTES {
                json!({ "state": "path-too-long" })
            } else if !fs::path_from_bytes(&bytes).is_absolute() {
                json!({ "state": "non-absolute" })
            } else {
                json!({ "state": "absolute-path" })
            }
        }
    }
}

#[cfg(windows)]
fn socket_fact() -> Value {
    // The shipped hook payload cannot discover/deliver to the Windows Site
    // endpoint. Offline inspection must not connect or invent its liveness.
    json!({ "state": "not-inspected", "reason": "windows-hook-transport-unavailable" })
}

/// The endpoint fact — `lstat` of the derived path only; never a connect.
#[cfg(target_os = "linux")]
fn socket_fact() -> Value {
    use fs::SocketFact;
    let euid = nix::unistd::geteuid().as_raw();
    let path = fs::observation_socket_path(euid);
    let path_state = || {
        let (key, value) = path_detail(&path);
        Map::from_iter([(key, value)])
    };
    if !fs::path_admitted(&path) {
        return json!({ "state": "path-too-long", "detail": path_state() });
    }
    match fs::observation_socket(&path, euid) {
        SocketFact::Present { mode } => {
            let mut detail = path_state();
            detail.insert("mode".to_owned(), json!(mode));
            json!({ "state": "present", "detail": detail })
        }
        SocketFact::Absent => json!({ "state": "absent", "detail": path_state() }),
        SocketFact::NotASocket { kind } => {
            let mut detail = path_state();
            detail.insert("kind".to_owned(), json!(entry_kind(kind)));
            json!({ "state": "not-a-socket", "detail": detail })
        }
        SocketFact::WrongOwner { owner, mode } => {
            let mut detail = path_state();
            detail.insert("owner".to_owned(), json!(owner));
            detail.insert("mode".to_owned(), json!(mode));
            json!({ "state": "wrong-owner", "detail": detail })
        }
        SocketFact::UnsafeSymlink => {
            json!({ "state": "unsafe-symlink", "detail": path_state() })
        }
        SocketFact::Unreadable => {
            json!({ "state": "unreadable", "detail": path_state() })
        }
    }
}

#[cfg(target_os = "linux")]
fn entry_kind(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Regular => "regular",
        EntryKind::Directory => "directory",
        EntryKind::Socket => "socket",
        EntryKind::Other => "other",
    }
}

/// `{path}` or `{pathBytesHex}` — the byte-preserving field pair, never a
/// lossy render.
#[cfg(any(target_os = "linux", windows))]
fn path_bytes_field(bytes: &[u8]) -> Map<String, Value> {
    let (key, value) = path_detail(&fs::path_from_bytes(bytes));
    Map::from_iter([(key, value)])
}

/// Deterministic observation-only hints — `{id, evidence, text}` rows:
/// `evidence` names the concrete fact rows the hint cites, `text` is the
/// canonical `en-US` statement of the observed condition and its direct
/// consequence only — never a remedy, never a trust or currentness claim.
/// The machine document stays locale-neutral; text output re-localizes the
/// sentence from `id` and the subjects behind `evidence`. Capped at
/// `MAX_HINTS`.
#[cfg(any(target_os = "linux", windows))]
fn build_hints(run: &DoctorRun) -> Vec<Value> {
    let mut hints: Vec<Value> = Vec::new();
    let mut push = |id: &str, evidence: Vec<String>, subjects: Vec<String>| {
        if hints.len() >= MAX_HINTS || (evidence.is_empty() && subjects.is_empty()) {
            return;
        }
        hints.push(json!({
            "id": id,
            "evidence": evidence,
            "text": Locale::EnUs.cli_agent_doctor_hint_text(id, &subjects),
        }));
    };

    let mut conflict = Vec::new();
    let mut unknown = Vec::new();
    let mut disabled = Vec::new();
    let mut update_required = Vec::new();
    let mut unsupported = Vec::new();
    let mut present_exe_absent = Vec::new();
    let mut absent_exe_present = Vec::new();
    let mut present_exe_absent_rows = Vec::new();
    let mut absent_exe_present_rows = Vec::new();
    let mut hook_present = Vec::new();
    let mut node_missing = Vec::new();
    let mut node_missing_rows = Vec::new();
    let fact = |id: &str, field: &str| format!("providers.{id}.{field}");
    for provider in &run.providers {
        let id = provider["id"].as_str().unwrap_or_default();
        let hook_state = provider["hook"]["state"].as_str().unwrap_or_default();
        let exe_state = provider["executable"]["state"].as_str().unwrap_or_default();
        let hook_row = fact(id, "hook");
        match hook_state {
            "conflict" => conflict.push((id, hook_row)),
            "unknown" => unknown.push((id, hook_row)),
            "disabled" => disabled.push((id, hook_row)),
            "update-required" => update_required.push((id, hook_row)),
            "unsupported" => unsupported.push((id, hook_row)),
            "present" => {
                hook_present.push(id.to_owned());
                if exe_state == "absent" {
                    present_exe_absent.push(id.to_owned());
                    present_exe_absent_rows.push(fact(id, "executable"));
                }
            }
            "absent" if exe_state == "present" => {
                absent_exe_present.push(id.to_owned());
                absent_exe_present_rows.push(fact(id, "executable"));
            }
            _ => {}
        }
        if hook_state == "present"
            && provider["runtime"]["node"]["state"].as_str() == Some("absent")
        {
            node_missing.push(id.to_owned());
            node_missing_rows.push(fact(id, "runtime.node"));
        }
    }
    let split = |rows: Vec<(&str, String)>| -> (Vec<String>, Vec<String>) {
        rows.into_iter()
            .map(|(id, row)| (id.to_owned(), row))
            .unzip()
    };
    let (subjects, evidence) = split(conflict);
    push("hook-conflict", evidence, subjects);
    let (subjects, evidence) = split(unknown);
    push("hook-unknown", evidence, subjects);
    let (subjects, evidence) = split(disabled);
    push("hook-disabled", evidence, subjects);
    let (subjects, evidence) = split(update_required);
    push("hook-update-required", evidence, subjects);
    let (subjects, evidence) = split(unsupported);
    push("hook-unsupported", evidence, subjects);
    push(
        "hook-present-executable-absent",
        present_exe_absent_rows,
        present_exe_absent,
    );
    push(
        "hook-absent-executable-present",
        absent_exe_present_rows,
        absent_exe_present,
    );
    push("node-absent", node_missing_rows, node_missing);

    let socket_state = run.endpoint["socket"]["state"].as_str().unwrap_or_default();
    if run.endpoint["socket"]["reason"] == "windows-hook-transport-unavailable" {
        push(
            "windows-hook-transport-unavailable",
            vec!["endpoint.socket".to_owned()],
            Vec::new(),
        );
    }
    if socket_state == "absent" && !hook_present.is_empty() {
        push(
            "endpoint-absent",
            vec!["endpoint.socket".to_owned()],
            Vec::new(),
        );
    } else if matches!(
        socket_state,
        "not-a-socket" | "wrong-owner" | "unsafe-symlink" | "unreadable"
    ) {
        push(
            "endpoint-anomaly",
            vec!["endpoint.socket".to_owned()],
            Vec::new(),
        );
    }
    let env_state = run.environment["observationSocketEnv"]["state"]
        .as_str()
        .unwrap_or_default();
    if matches!(env_state, "empty" | "non-absolute" | "path-too-long") {
        push(
            "env-override-invalid",
            vec!["environment.observationSocketEnv".to_owned()],
            Vec::new(),
        );
    }
    for (index, root) in run.roots.iter().enumerate() {
        let state = root["state"].as_str().unwrap_or_default();
        let row = format!("asset.roots[{index}]");
        let subject = root["path"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| root["origin"].as_str().unwrap_or_default().to_owned());
        match state {
            "inconsistent" | "absent" => {
                push("root-anomaly", vec![row], vec![subject]);
            }
            "unsafe" | "unreadable" | "path-too-long" | "not-inspected" | "budget-exhausted" => {
                push("root-uninspectable", vec![row], vec![subject]);
            }
            _ => {}
        }
    }
    if !run.complete {
        push("partial-document", vec!["complete".to_owned()], Vec::new());
    }
    hints
}

/// Assembles and emits the document — exit 0 for complete and partial
/// documents alike (a partial document is itself the answer).
#[cfg(any(target_os = "linux", windows))]
fn emit(run: DoctorRun, format: OutputFormat, locale: Locale) -> Output {
    let result = json!({
        "complete": run.complete,
        "providers": run.providers,
        "endpoint": run.endpoint,
        "asset": { "roots": run.roots },
        "environment": run.environment,
        "hints": build_hints(&run),
    });
    let too_large = || {
        failure_output(
            Failure::new(FailureKind::OutputTooLarge).details(Map::from_iter([(
                "limitBytes".to_owned(),
                json!(output::LIMIT),
            )])),
            Some(format),
            locale,
        )
    };
    match format {
        OutputFormat::Json => {
            let document = json!({
                "schema": "orchbee.agent.doctor",
                "version": if cfg!(windows) { 2 } else { 1 },
                "source": local_runtime_source(),
                "result": result,
            });
            match output::encode_document(&document) {
                Ok(stdout) => Output {
                    status: 0,
                    stdout,
                    stderr: String::new(),
                    finalization: None,
                },
                Err(output::TooLarge) => too_large(),
            }
        }
        OutputFormat::Text => {
            match output::render_text(|out| crate::cli::present::agent_doctor(out, &result, locale))
            {
                Ok(stdout) => Output {
                    status: 0,
                    stdout,
                    stderr: String::new(),
                    finalization: None,
                },
                Err(output::TooLarge) => too_large(),
            }
        }
    }
}
