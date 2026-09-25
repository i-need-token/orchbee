//! The native provider registry and the canonical inspection orchestration.
//! Per-provider behavior is the verbatim port of the Desktop adapters'
//! `inspect()` paths — including which providers gate their state on the
//! executable being resolvable (the CLI-command adapters) and which inspect
//! the filesystem regardless (codex, cline, the guarded entries). The
//! capsule fold (`enabled` + capsule-not-ready → `update-required`) wraps
//! every provider exactly as `CapsuleAgentIntegrationAdapter` does.

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    time::Instant,
};

use super::capsule;
use super::copilot::Command as CopilotCommand;
use super::exec::{self, CommandResult};
use super::inspect::{
    self, BootstrapKind, ClinePackage, CodexInspection, Marketplace, PluginInspection,
    ResourceOptions, TargetState,
};
use super::model::{Availability, IntegrationId, Issue, ManagementMode, SnapshotItem, State};

/// Everything one provider needs from the invocation environment.
pub(crate) struct ProviderContext<'a> {
    /// The user's home directory (HOME-isolated in tests).
    pub(crate) home: &'a Path,
    /// The stable managed capsule root `~/.orchbee/agent-observation-adapters/<v>`.
    pub(crate) capsule_root: &'a Path,
    /// The declared adapter source: the cohort sidecar directory or the
    /// `--adapter-source` override — `None` when no source exists.
    pub(crate) adapter_source: Option<&'a Path>,
    /// The ambient PATH for executable resolution.
    pub(crate) path_env: Option<&'a OsStr>,
    /// The invocation's single monotonic deadline.
    pub(crate) deadline: Instant,
    /// The armed termination source — `None` in contexts that cannot be
    /// cancelled (tests drive their own flag through this same slot).
    pub(crate) cancel: Option<&'a dyn crate::termination_signal::TerminationSource>,
    /// The platform's management mode for this cohort — `manual-only` on
    /// platforms the managed flow does not support. A spec-level property,
    /// never an inspected value.
    pub(crate) management_mode: ManagementMode,
}

/// Build the classifier options: the caller owns the legacy-root Vec.
pub(crate) fn resource_options<'a>(
    context: &ProviderContext<'a>,
    legacy_roots: &'a [PathBuf],
) -> ResourceOptions<'a> {
    ResourceOptions {
        expected_version: capsule::PAYLOAD_VERSION,
        legacy_roots,
        resource_root: context.capsule_root,
    }
}

/// How a provider inspects its state.
#[derive(Clone, Copy)]
enum Kind {
    /// CLI-backed plugin commands — availability gates the inspection.
    Cli(CliKind),
    /// codex's owned TOML declaration — filesystem regardless of PATH.
    Codex,
    /// cline's managed-install scan plus the bundle source-readiness fold.
    Cline,
    /// Guarded filesystem entries (module files and extension directories).
    Guarded(GuardedSpec),
}

#[derive(Clone, Copy)]
enum CliKind {
    Claude,
    Grok,
    Copilot,
}

#[derive(Clone, Copy)]
pub(crate) struct GuardedSpec {
    pub(crate) kind: BootstrapKind,
    /// The source path inside the capsule, relative to the capsule root.
    pub(crate) source_relative: &'static str,
    /// The provider-managed target under the user's home.
    pub(crate) target_relative: &'static str,
    /// Extension directories also require the shared runtime files.
    pub(crate) runtime_relative: Option<&'static str>,
}

struct Spec {
    kind: Kind,
    executable: &'static str,
    /// Command slots one inspection consumes (post-resolution).
    inspect_slots: usize,
}

const fn spec(id: IntegrationId) -> Spec {
    match id {
        IntegrationId::ClaudeCode => Spec {
            kind: Kind::Cli(CliKind::Claude),
            executable: "claude",
            inspect_slots: 2,
        },
        IntegrationId::Codex => Spec {
            kind: Kind::Codex,
            executable: "codex",
            inspect_slots: 0,
        },
        IntegrationId::Grok => Spec {
            kind: Kind::Cli(CliKind::Grok),
            executable: "grok",
            inspect_slots: 1,
        },
        IntegrationId::Opencode => Spec {
            kind: Kind::Guarded(GuardedSpec {
                kind: BootstrapKind::Opencode,
                source_relative: "bundle/opencode/orchbee-agent-observation.mjs",
                target_relative: if cfg!(windows) {
                    ".config/opencode/plugins/orchbee-agent-observation.js"
                } else {
                    ".config/opencode/plugins/orchbee-agent-observation.mjs"
                },
                runtime_relative: None,
            }),
            executable: "opencode",
            inspect_slots: 0,
        },
        IntegrationId::Kilo => Spec {
            kind: Kind::Guarded(GuardedSpec {
                kind: BootstrapKind::Opencode,
                source_relative: "bundle/kilo/orchbee-agent-observation.mjs",
                target_relative: ".config/kilo/plugin/orchbee-agent-observation.js",
                runtime_relative: None,
            }),
            executable: "kilo",
            inspect_slots: 0,
        },
        IntegrationId::Pi => Spec {
            kind: Kind::Guarded(GuardedSpec {
                kind: BootstrapKind::Pi,
                source_relative: if cfg!(windows) {
                    "bundle/pi/orchbee-agent-observation.mjs"
                } else {
                    "bundle/pi/orchbee-agent-observation.cjs"
                },
                target_relative: ".pi/agent/extensions/orchbee-agent-observation.cjs",
                runtime_relative: None,
            }),
            executable: "pi",
            inspect_slots: 0,
        },
        IntegrationId::Omp => Spec {
            kind: Kind::Guarded(GuardedSpec {
                kind: BootstrapKind::Omp,
                source_relative: if cfg!(windows) {
                    "bundle/omp/orchbee-agent-observation.mjs"
                } else {
                    "bundle/omp/orchbee-agent-observation.cjs"
                },
                target_relative: ".omp/agent/extensions/orchbee-agent-observation.cjs",
                runtime_relative: None,
            }),
            executable: "omp",
            inspect_slots: 0,
        },
        IntegrationId::Gemini => Spec {
            kind: Kind::Guarded(GuardedSpec {
                kind: BootstrapKind::ExtensionDirectory,
                source_relative: "bundle/gemini",
                target_relative: ".gemini/extensions/orchbee-agent-observation",
                runtime_relative: Some("bundle/scripts"),
            }),
            executable: "gemini",
            inspect_slots: 0,
        },
        IntegrationId::GitHubCopilot => Spec {
            kind: Kind::Cli(CliKind::Copilot),
            executable: "copilot",
            inspect_slots: 2,
        },
        IntegrationId::Cline => Spec {
            kind: Kind::Cline,
            executable: "cline",
            inspect_slots: 0,
        },
        IntegrationId::QwenCode => Spec {
            kind: Kind::Guarded(GuardedSpec {
                kind: BootstrapKind::ExtensionDirectory,
                source_relative: "bundle/qwen-code",
                target_relative: ".qwen/extensions/orchbee-agent-observation",
                runtime_relative: Some("bundle/scripts"),
            }),
            executable: "qwen",
            inspect_slots: 0,
        },
    }
}

/// Command slots a single inspection consumes — the final re-inspection
/// reserve a mutation must hold back.
pub(crate) fn inspect_slots(id: IntegrationId) -> usize {
    spec(id).inspect_slots
}

/// One platform admission rule for both plans and effects. The JS module
/// entries use the existing Windows pipe/console transport without a shell.
pub(crate) const fn management_mode(id: IntegrationId, declared: ManagementMode) -> ManagementMode {
    if !cfg!(windows) && matches!(id, IntegrationId::Kilo) {
        return ManagementMode::ManualOnly;
    }
    if cfg!(windows)
        && !matches!(
            id,
            IntegrationId::ClaudeCode
                | IntegrationId::Grok
                | IntegrationId::Codex
                | IntegrationId::Opencode
                | IntegrationId::Kilo
                | IntegrationId::Pi
                | IntegrationId::Omp
                | IntegrationId::GitHubCopilot
                | IntegrationId::Gemini
                | IntegrationId::QwenCode
        )
    {
        ManagementMode::ManualOnly
    } else {
        declared
    }
}

/// The provider's registered executable name — the PATH-resolution key.
#[cfg(all(test, unix))]
pub(crate) const fn executable_name(id: IntegrationId) -> &'static str {
    spec(id).executable
}

pub(crate) fn guarded_spec(id: IntegrationId) -> Option<GuardedSpec> {
    match spec(id).kind {
        Kind::Guarded(guarded) => Some(guarded),
        _ => None,
    }
}

/// The obsolete Windows OpenCode target was never discovered by its loader.
pub(crate) fn obsolete_guarded_spec(id: IntegrationId) -> Option<GuardedSpec> {
    #[cfg(windows)]
    if id == IntegrationId::Opencode {
        return Some(GuardedSpec {
            target_relative: ".config/opencode/plugins/orchbee-agent-observation.mjs",
            ..guarded_spec(id)?
        });
    }
    let _ = id;
    None
}

pub(crate) fn guarded_target(id: IntegrationId, spec: GuardedSpec, home: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    if id == IntegrationId::Opencode {
        let paths = crate::agent_configuration_windows::opencode_entries(home)?;
        let target = if spec.target_relative.ends_with(".mjs") {
            paths.obsolete
        } else {
            paths.current
        };
        return super::files::path_admitted(&target).then_some(target);
    }
    #[cfg(windows)]
    if id == IntegrationId::Kilo {
        let target = crate::agent_configuration_windows::kilo_entry(home)?;
        return super::files::path_admitted(&target).then_some(target);
    }
    #[cfg(windows)]
    if matches!(id, IntegrationId::Gemini | IntegrationId::QwenCode) {
        let target = crate::agent_configuration_windows::extension_directory(id.as_str(), home)?;
        return super::files::path_admitted(&target).then_some(target);
    }
    let _ = id;
    Some(home.join(spec.target_relative))
}

fn inspect_guarded_spec(
    id: IntegrationId,
    spec: GuardedSpec,
    context: &ProviderContext,
    options: &ResourceOptions,
) -> TargetState {
    let Some(target) = guarded_target(id, spec, context.home) else {
        return TargetState::Unknown;
    };
    inspect::inspect_guarded_target(
        &target,
        spec.kind,
        id,
        &context.capsule_root.join(spec.source_relative),
        Path::new(spec.source_relative),
        options,
    )
}

fn combine_guarded_targets(current: TargetState, obsolete: Option<TargetState>) -> TargetState {
    use TargetState::*;
    let Some(obsolete) = obsolete else {
        return current;
    };
    if current == Newer || obsolete == Newer {
        return Newer;
    }
    if current == Conflict || obsolete == Conflict {
        return Conflict;
    }
    if current == Unknown || obsolete == Unknown {
        return Unknown;
    }
    if obsolete != Missing {
        UpdateRequired
    } else {
        current
    }
}

/// The three-way outcome of a provider read — `TimedOut` stays distinct so
/// a read that consumed the deadline never collapses into the generic
/// unclassifiable failure the `inspection-failed` issue would then report.
pub(crate) enum ReadOutcome {
    Stdout(Vec<u8>),
    Failed,
    TimedOut,
}

/// Run one provider read command inside a slot clipped to the invocation
/// deadline.
fn read(executable: &Path, arguments: &[&str], context: &ProviderContext) -> ReadOutcome {
    let argv: Vec<String> = arguments.iter().map(|arg| (*arg).to_owned()).collect();
    match exec::run_command(
        executable,
        &argv,
        context.home,
        context.deadline,
        context.cancel,
    ) {
        CommandResult::Completed { stdout } => ReadOutcome::Stdout(stdout),
        CommandResult::TimedOut => ReadOutcome::TimedOut,
        _ => ReadOutcome::Failed,
    }
}

/// The provider's registered executable resolves on PATH — the enable-side
/// availability gate, answerable without issuing a single provider command.
pub(crate) fn executable_available(id: IntegrationId, context: &ProviderContext) -> bool {
    exec::resolve_executable(spec(id).executable, context.path_env).is_some()
}

/// One provider's snapshot row — canonical `inspect()` semantics plus the
/// capsule fold. CLI-command providers report `unknown` (no issue) when the
/// executable is unresolvable; filesystem providers inspect regardless.
pub(crate) fn inspect_provider(
    id: IntegrationId,
    context: &ProviderContext,
    legacy_roots: &[PathBuf],
) -> SnapshotItem {
    inspect_full(id, context, legacy_roots).0
}

/// The single read pass every consumer shares: one scripted queue slot per
/// read, one inspection parse, both the snapshot row and the guard
/// dimensions derived from it — the mutation vectors plan against this
/// detail without ever re-issuing the provider's read commands.
pub(crate) fn inspect_full(
    id: IntegrationId,
    context: &ProviderContext,
    legacy_roots: &[PathBuf],
) -> (SnapshotItem, InspectionDetail) {
    let management_mode = management_mode(id, context.management_mode);
    let spec = spec(id);
    let executable = exec::resolve_executable(spec.executable, context.path_env);
    let availability = if executable.is_some() {
        Availability::Available
    } else {
        Availability::NotFound
    };
    let options = resource_options(context, legacy_roots);
    let bundle_root = context.capsule_root.join("bundle");

    // CLI-command adapters skip inspection entirely when the executable is
    // absent — state unknown; the platform issue still reports manual-only.
    if matches!(spec.kind, Kind::Cli(_)) && executable.is_none() {
        return (
            SnapshotItem {
                availability,
                integration_id: id,
                issue: (management_mode == ManagementMode::ManualOnly)
                    .then_some(Issue::UnsupportedPlatform),
                management_mode,
                state: State::Unknown,
                timed_out: false,
            },
            InspectionDetail::Cli {
                marketplace: None,
                plugin_present: None,
            },
        );
    }

    let mut timed_out = false;
    let mut read_slot =
        |executable: &Path, arguments: &[&str], context: &ProviderContext| -> ReadOutcome {
            let outcome = read(executable, arguments, context);
            if matches!(outcome, ReadOutcome::TimedOut) {
                timed_out = true;
            }
            outcome
        };

    let cli_detail = |inspection: Option<&PluginInspection>| InspectionDetail::Cli {
        marketplace: inspection.map(|found| found.marketplace),
        plugin_present: inspection.map(|found| found.plugin_present),
    };

    let (state, inspection_failed, detail) = match spec.kind {
        Kind::Cli(CliKind::Claude) => {
            let executable = executable.as_deref().expect("gated above");
            let plugin = read_slot(executable, &["plugin", "list", "--json"], context);
            // A timed-out read already consumed its whole slot — the sibling
            // read is still owed (it answers timed-out against the spent
            // deadline itself); only a quick determinate failure
            // short-circuits the way canonical does.
            let marketplace = match plugin {
                ReadOutcome::Failed => None,
                _ => Some(read_slot(
                    executable,
                    &["plugin", "marketplace", "list", "--json"],
                    context,
                )),
            };
            match (plugin, marketplace) {
                (ReadOutcome::Stdout(plugin), Some(ReadOutcome::Stdout(marketplace))) => {
                    match inspect::inspect_claude(&plugin, &marketplace, &options) {
                        Some(inspection) => {
                            (inspection.state, false, cli_detail(Some(&inspection)))
                        }
                        None => (State::Unknown, true, cli_detail(None)),
                    }
                }
                _ => (State::Unknown, true, cli_detail(None)),
            }
        }
        Kind::Cli(CliKind::Grok) => {
            let executable = executable.as_deref().expect("gated above");
            match read_slot(executable, &["plugin", "list", "--json"], context) {
                ReadOutcome::Stdout(stdout) => {
                    match inspect::inspect_grok(&stdout, &options, context.home) {
                        Some(inspection) => {
                            (inspection.state, false, cli_detail(Some(&inspection)))
                        }
                        None => (State::Unknown, true, cli_detail(None)),
                    }
                }
                _ => (State::Unknown, true, cli_detail(None)),
            }
        }
        Kind::Cli(CliKind::Copilot) => {
            let executable = executable.as_deref().expect("gated above");
            let plugin = read_slot(executable, &CopilotCommand::List.arguments(), context);
            let marketplace = match plugin {
                ReadOutcome::Failed => None,
                _ => Some(read_slot(
                    executable,
                    &CopilotCommand::MarketplaceList.arguments(),
                    context,
                )),
            };
            match (plugin, marketplace) {
                (ReadOutcome::Stdout(plugin), Some(ReadOutcome::Stdout(marketplace))) => {
                    match inspect::inspect_copilot(&plugin, &marketplace, &options) {
                        Some(inspection) => {
                            (inspection.state, false, cli_detail(Some(&inspection)))
                        }
                        None => (State::Unknown, true, cli_detail(None)),
                    }
                }
                _ => (State::Unknown, true, cli_detail(None)),
            }
        }
        Kind::Codex => {
            let inspection: CodexInspection = inspect::inspect_codex(context.home, &options);
            (
                inspection.state,
                inspection.state == State::Unknown,
                InspectionDetail::Codex {
                    marketplace: inspection.marketplace,
                    plugin_present: inspection.plugin_present,
                },
            )
        }
        Kind::Cline => {
            let raw = inspect::inspect_cline(context.home);
            let source_ready = matches!(
                inspect::cline_package_state(&bundle_root),
                ClinePackage::Current
            );
            let observed =
                if (raw == State::Enabled && !source_ready) || raw == State::UpdateRequired {
                    State::UpdateRequired
                } else {
                    raw
                };
            (observed, raw == State::Unknown, InspectionDetail::Plain)
        }
        Kind::Guarded(guarded) => {
            let source_path = context.capsule_root.join(guarded.source_relative);
            let runtime_source = guarded
                .runtime_relative
                .map(|relative| context.capsule_root.join(relative));
            let source_ready = inspect::guarded_source_ready(
                &source_path,
                guarded.kind,
                runtime_source.as_deref(),
            );
            let target_state = inspect_guarded_spec(id, guarded, context, &options);
            let obsolete_target_state = obsolete_guarded_spec(id)
                .map(|old| inspect_guarded_spec(id, old, context, &options));
            let combined_target_state =
                combine_guarded_targets(target_state, obsolete_target_state);
            let state = match combined_target_state {
                TargetState::Missing => State::NotInstalled,
                TargetState::Legacy | TargetState::UpdateRequired => State::UpdateRequired,
                TargetState::Current if !source_ready => State::UpdateRequired,
                TargetState::Current => State::Enabled,
                TargetState::Newer => State::StaleSide,
                TargetState::Conflict => State::Conflict,
                TargetState::Unknown => State::Unknown,
            };
            (
                state,
                combined_target_state == TargetState::Unknown,
                InspectionDetail::Guarded {
                    target_state,
                    source_ready,
                    obsolete_target_state,
                },
            )
        }
    };

    // The capsule fold — `enabled` downgrades to `update-required` when the
    // managed payload is not ready.
    let state = if state == State::Enabled
        && !capsule::is_ready(context.capsule_root, context.adapter_source)
    {
        State::UpdateRequired
    } else {
        state
    };

    // Issue precedence (canonical): the platform slot wins, then the
    // inspection failure, then the additive capsule-version issue (R-2).
    let issue = if management_mode == ManagementMode::ManualOnly {
        Some(Issue::UnsupportedPlatform)
    } else if inspection_failed {
        Some(Issue::InspectionFailed)
    } else {
        match context.adapter_source {
            Some(source)
                if matches!(
                    capsule::validate(source),
                    capsule::CapsuleValidity::UnsupportedVersion { .. }
                ) =>
            {
                Some(Issue::CapsuleUnsupportedVersion)
            }
            _ => None,
        }
    };

    (
        SnapshotItem {
            availability,
            integration_id: id,
            issue,
            management_mode,
            state,
            timed_out,
        },
        detail,
    )
}

/// The whole snapshot — canonical order, each provider inspected in turn
/// inside the shared deadline.
pub(crate) fn snapshot(context: &ProviderContext, legacy_roots: &[PathBuf]) -> Vec<SnapshotItem> {
    IntegrationId::ALL
        .iter()
        .map(|id| inspect_provider(*id, context, legacy_roots))
        .collect()
}

/// The detailed guard dimensions an `explain` surfaces — the canonical
/// per-provider inspection internals, captured in the same read pass as
/// the snapshot row.
#[derive(Clone, Copy)]
pub(crate) enum InspectionDetail {
    Cli {
        marketplace: Option<Marketplace>,
        plugin_present: Option<bool>,
    },
    Codex {
        marketplace: Marketplace,
        plugin_present: bool,
    },
    Guarded {
        target_state: TargetState,
        source_ready: bool,
        obsolete_target_state: Option<TargetState>,
    },
    Plain,
}

/// The richer inspection for `explain` — same verdict, same single read
/// pass, with the guard dimensions attached where the provider exposes
/// them.
pub(crate) fn inspect_provider_detail(
    id: IntegrationId,
    context: &ProviderContext,
    legacy_roots: &[PathBuf],
) -> (SnapshotItem, InspectionDetail) {
    inspect_full(id, context, legacy_roots)
}
