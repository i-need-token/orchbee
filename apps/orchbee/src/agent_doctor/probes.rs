//! Deterministic probe classification — the pure half of `agent doctor`.
//! Every function takes bytes/typed adapter outcomes and returns typed
//! facts; no path, no IO, no clock. The mappings are direct ports of the
//! owning Desktop adapters' check contracts (each cites its source), with
//! the contract's own divergences noted inline.

use std::cmp::Ordering;

use serde_json::Value;
use sha2::{Digest, Sha256};

/// The contract version this build can evaluate — `ENTRY_CONTRACT_VERSION`,
/// `CLINE_CONTRACT_VERSION`, and `CAPSULE_CONTRACT_VERSION` all pin `3` in
/// the Desktop sources. A declared newer version is an honest
/// `unrecognized`, never "invalid"; the doctor build may be the stale side.
pub(crate) const SUPPORTED_CONTRACT_VERSION: u64 = 3;

/// The capsule payload version the manifest contract requires —
/// `AGENT_OBSERVATION_CAPSULE_VERSION`.
pub(crate) const CAPSULE_PAYLOAD_VERSION: &str = "0.4.17";

// ---------------------------------------------------------------------------
// Provider binding table — the literals of
// `createDesktopAgentIntegrationAdapters.ts`.
// ---------------------------------------------------------------------------

/// The inspection mechanism a provider's hook declaration admits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProbeKind {
    /// The formats carry nothing offline-derivable; honest inspection needs
    /// the provider's own command surface — a deferred surface, reported
    /// `unsupported`/`provider-command-inspection-required`.
    ProviderCommand,
    /// `~/.codex/config.toml` marketplace+plugin declarations (line scanner).
    CodexConfig,
    /// A guarded extension directory: marker + fingerprint self-consistency.
    GuardedDirectory,
    /// A bootstrap module file: marker + exact template self-derivation.
    Bootstrap,
    /// The `~/.cline/plugins` install scan.
    Cline,
    ConfigurationHooks,
}

/// The bootstrap template tail a provider's module derives: everything after
/// `const orchbeeAgentObservationModuleUrl = <json-literal>;\n`, plus the
/// anchored payload-relative suffix the embedded `file:` URL must end with.
pub(crate) struct BootstrapShape {
    pub(crate) suffix: &'static str,
    pub(crate) body: &'static [u8],
}

/// One provider's offline binding: canonical id, divergent executable name,
/// probe kind, node applicability, and the managed target path under `$HOME`.
pub(crate) struct Provider {
    pub(crate) id: &'static str,
    pub(crate) executable: &'static str,
    pub(crate) kind: ProbeKind,
    /// The hook payload needs `node` on PATH — command-hook providers only.
    pub(crate) node_applicable: bool,
    /// The managed target under `$HOME`; `""` for provider-command kinds.
    pub(crate) home_target: &'static str,
    pub(crate) bootstrap: Option<BootstrapShape>,
    /// The provider's own manual inspection command for text hints —
    /// `Some` only on provider-command kinds (TA-01).
    pub(crate) manual_command: Option<&'static str>,
}

const OPENCODE_BODY: &[u8] = b"export async function OrchbeeAgentObservation(...arguments_) {\n\ttry {\n\t\tconst loaded = await import(orchbeeAgentObservationModuleUrl);\n\t\treturn typeof loaded.OrchbeeAgentObservation === 'function'\n\t\t\t? await loaded.OrchbeeAgentObservation(...arguments_)\n\t\t\t: {};\n\t} catch {\n\t\treturn {};\n\t}\n}\n";

const CJS_BODY: &[u8] = b"module.exports = async function orchbeeAgentObservation(api) {\n\ttry {\n\t\tconst loaded = await import(orchbeeAgentObservationModuleUrl);\n\t\tif (typeof loaded.orchbeeAgentObservation === 'function') {\n\t\t\tawait loaded.orchbeeAgentObservation(api);\n\t\t}\n\t} catch {}\n};\n";

/// The native diagnostic providers, in canonical order.
pub(crate) const PROVIDERS: &[Provider] = &[
    Provider {
        id: "claude-code",
        executable: "claude",
        kind: ProbeKind::ProviderCommand,
        node_applicable: true,
        home_target: "",
        bootstrap: None,
        manual_command: Some("claude plugin list --json; claude plugin marketplace list --json"),
    },
    Provider {
        id: "codex",
        executable: "codex",
        kind: ProbeKind::CodexConfig,
        node_applicable: true,
        home_target: ".codex/config.toml",
        bootstrap: None,
        manual_command: None,
    },
    Provider {
        id: "gemini",
        executable: "gemini",
        kind: ProbeKind::GuardedDirectory,
        node_applicable: true,
        home_target: ".gemini/extensions/orchbee-agent-observation",
        bootstrap: None,
        manual_command: None,
    },
    Provider {
        id: "github-copilot",
        executable: "copilot",
        kind: ProbeKind::ProviderCommand,
        node_applicable: true,
        home_target: "",
        bootstrap: None,
        manual_command: Some(if cfg!(windows) {
            "copilot plugin list --json; copilot plugin marketplace list --json"
        } else {
            "copilot plugins list --kind plugin --json; copilot plugins marketplace list --json"
        }),
    },
    Provider {
        id: "grok",
        executable: "grok",
        kind: ProbeKind::ProviderCommand,
        node_applicable: true,
        home_target: "",
        bootstrap: None,
        manual_command: Some("grok plugin list --json"),
    },
    Provider {
        id: "cline",
        executable: "cline",
        kind: ProbeKind::Cline,
        node_applicable: false,
        home_target: ".cline/plugins",
        bootstrap: None,
        manual_command: None,
    },
    Provider {
        id: "opencode",
        executable: "opencode",
        kind: ProbeKind::Bootstrap,
        node_applicable: false,
        home_target: if cfg!(windows) {
            ".config/opencode/plugins/orchbee-agent-observation.js"
        } else {
            ".config/opencode/plugins/orchbee-agent-observation.mjs"
        },
        bootstrap: Some(BootstrapShape {
            suffix: if cfg!(windows) {
                "/bundle/opencode/orchbee-agent-observation.mjs"
            } else {
                "/opencode/orchbee-agent-observation.mjs"
            },
            body: OPENCODE_BODY,
        }),
        manual_command: None,
    },
    Provider {
        id: "kilo",
        executable: "kilo",
        kind: ProbeKind::Bootstrap,
        node_applicable: false,
        home_target: ".config/kilo/plugin/orchbee-agent-observation.js",
        bootstrap: Some(BootstrapShape {
            suffix: "/bundle/kilo/orchbee-agent-observation.mjs",
            body: OPENCODE_BODY,
        }),
        manual_command: None,
    },
    Provider {
        id: "mastracode",
        executable: "mastracode",
        kind: ProbeKind::ConfigurationHooks,
        node_applicable: true,
        home_target: ".mastracode/hooks.json",
        bootstrap: None,
        manual_command: None,
    },
    Provider {
        id: "hermes",
        executable: "hermes",
        kind: ProbeKind::ProviderCommand,
        node_applicable: true,
        home_target: "",
        bootstrap: None,
        manual_command: Some("hermes plugins list --json"),
    },
    Provider {
        id: "devin",
        executable: "devin",
        kind: ProbeKind::ConfigurationHooks,
        node_applicable: true,
        home_target: "devin/config.json",
        bootstrap: None,
        manual_command: None,
    },
    Provider {
        id: "letta",
        executable: "letta",
        kind: ProbeKind::ConfigurationHooks,
        node_applicable: true,
        home_target: ".letta/settings.json",
        bootstrap: None,
        manual_command: None,
    },
    Provider {
        id: "kimi",
        executable: "kimi",
        kind: ProbeKind::ConfigurationHooks,
        node_applicable: true,
        home_target: ".kimi-code/config.toml",
        bootstrap: None,
        manual_command: None,
    },
    Provider {
        id: "droid",
        executable: "droid",
        kind: ProbeKind::ConfigurationHooks,
        node_applicable: true,
        home_target: ".factory/hooks.json",
        bootstrap: None,
        manual_command: None,
    },
    Provider {
        id: "antigravity-cli",
        executable: "agy",
        kind: ProbeKind::ConfigurationHooks,
        node_applicable: true,
        home_target: ".gemini/config/hooks.json",
        bootstrap: None,
        manual_command: None,
    },
    Provider {
        id: "qodercli",
        executable: "qoder",
        kind: ProbeKind::ConfigurationHooks,
        node_applicable: true,
        home_target: ".qoder/settings.json",
        bootstrap: None,
        manual_command: None,
    },
    Provider {
        id: "cursor",
        executable: "agent",
        kind: ProbeKind::ConfigurationHooks,
        node_applicable: true,
        home_target: ".cursor/hooks.json",
        bootstrap: None,
        manual_command: None,
    },
    Provider {
        id: "pi",
        executable: "pi",
        kind: ProbeKind::Bootstrap,
        node_applicable: false,
        home_target: ".pi/agent/extensions/orchbee-agent-observation.cjs",
        bootstrap: Some(BootstrapShape {
            suffix: if cfg!(windows) {
                "/bundle/pi/orchbee-agent-observation.mjs"
            } else {
                "/pi/orchbee-agent-observation.cjs"
            },
            body: CJS_BODY,
        }),
        manual_command: None,
    },
    Provider {
        id: "omp",
        executable: "omp",
        kind: ProbeKind::Bootstrap,
        node_applicable: false,
        home_target: ".omp/agent/extensions/orchbee-agent-observation.cjs",
        bootstrap: Some(BootstrapShape {
            suffix: if cfg!(windows) {
                "/bundle/omp/orchbee-agent-observation.mjs"
            } else {
                "/omp/orchbee-agent-observation.cjs"
            },
            body: CJS_BODY,
        }),
        manual_command: None,
    },
    Provider {
        id: "qwen-code",
        executable: "qwen",
        kind: ProbeKind::GuardedDirectory,
        node_applicable: true,
        home_target: ".qwen/extensions/orchbee-agent-observation",
        bootstrap: None,
        manual_command: None,
    },
];

/// The bootstrap marker line prefix — `BOOTSTRAP_MARKER_PREFIX`.
pub(crate) const BOOTSTRAP_MARKER_PREFIX: &[u8] = b"// orchbee-agent-observation-bootstrap-v3:";
const MODULE_URL_LINE_PREFIX: &[u8] = b"const orchbeeAgentObservationModuleUrl = ";
/// The guarded-directory marker filename — `DIRECTORY_MARKER_NAME`.
pub(crate) const GUARDED_MARKER_NAME: &str = ".orchbee-agent-observation.json";
/// The capsule manifest filename inside an asset root.
pub(crate) const CAPSULE_MANIFEST_NAME: &str = "capsule.json";
/// The cline managed package name — `CLINE_PLUGIN_NAME`.
pub(crate) const CLINE_PACKAGE_NAME: &str = "orchbee-agent-observation";
/// The cline payload entry path inside a managed `package/` directory.
pub(crate) const CLINE_ENTRY_PATH: &str = "cline/orchbee-agent-observation.js";
/// The own-process environment variable hook payloads read.
pub(crate) const OBSERVATION_SOCKET_ENV: &str = "ORCHBEE_AGENT_OBSERVATION_SOCKET_V3";

// ---------------------------------------------------------------------------
// Shared outcome vocabulary — the contract's closed state/reason sets.
// ---------------------------------------------------------------------------

/// The uniform hook-declaration state vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HookState {
    Present,
    Absent,
    Disabled,
    Conflict,
    UpdateRequired,
    Unknown,
    Unsupported,
}

impl HookState {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Absent => "absent",
            Self::Disabled => "disabled",
            Self::Conflict => "conflict",
            Self::UpdateRequired => "update-required",
            Self::Unknown => "unknown",
            Self::Unsupported => "unsupported",
        }
    }
}

/// The closed hook-state reason vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HookReason {
    TargetMissing,
    NoManagedInstall,
    DisabledFlag,
    MarkerConsistent,
    BootstrapMarkerConsistent,
    ConfigDeclaresInstall,
    ForeignContent,
    MarkerMismatch,
    TemplateMismatch,
    NameMismatch,
    DuplicateKey,
    DuplicateSection,
    NonLocalSource,
    SourceNotAbsolute,
    PluginWithoutMarketplace,
    RecordedRootInconsistent,
    RecordedRootAbsent,
    ContractVersionBelowSupported,
    ProviderCommandInspectionRequired,
    Unreadable,
    Malformed,
    UnsafeFileKind,
    SymlinkUninspected,
    BudgetExhausted,
    NotInspected,
    ContractVersionUnrecognized,
    RecordedRootUninspectable,
}

impl HookReason {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::TargetMissing => "target-missing",
            Self::NoManagedInstall => "no-managed-install",
            Self::DisabledFlag => "disabled-flag",
            Self::MarkerConsistent => "marker-consistent",
            Self::BootstrapMarkerConsistent => "bootstrap-marker-consistent",
            Self::ConfigDeclaresInstall => "config-declares-install",
            Self::ForeignContent => "foreign-content",
            Self::MarkerMismatch => "marker-mismatch",
            Self::TemplateMismatch => "template-mismatch",
            Self::NameMismatch => "name-mismatch",
            Self::DuplicateKey => "duplicate-key",
            Self::DuplicateSection => "duplicate-section",
            Self::NonLocalSource => "non-local-source",
            Self::SourceNotAbsolute => "source-not-absolute",
            Self::PluginWithoutMarketplace => "plugin-without-marketplace",
            Self::RecordedRootInconsistent => "recorded-root-inconsistent",
            Self::RecordedRootAbsent => "recorded-root-absent",
            Self::ContractVersionBelowSupported => "contract-version-below-supported",
            Self::ProviderCommandInspectionRequired => "provider-command-inspection-required",
            Self::Unreadable => "unreadable",
            Self::Malformed => "malformed",
            Self::UnsafeFileKind => "unsafe-file-kind",
            Self::SymlinkUninspected => "symlink-uninspected",
            Self::BudgetExhausted => "budget-exhausted",
            Self::NotInspected => "not-inspected",
            Self::ContractVersionUnrecognized => "contract-version-unrecognized",
            Self::RecordedRootUninspectable => "recorded-root-uninspectable",
        }
    }
}

/// An asset-root probe state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RootState {
    Consistent,
    Inconsistent,
    Unreadable,
    Unsafe,
    Absent,
    PathTooLong,
    BudgetExhausted,
    NotInspected,
}

impl RootState {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Consistent => "consistent",
            Self::Inconsistent => "inconsistent",
            Self::Unreadable => "unreadable",
            Self::Unsafe => "unsafe",
            Self::Absent => "absent",
            Self::PathTooLong => "path-too-long",
            Self::BudgetExhausted => "budget-exhausted",
            Self::NotInspected => "not-inspected",
        }
    }

    /// The uniform recorded-root mapping: only a proven content/digest
    /// contradiction or a positively-missing referent is a conflict; every
    /// uninspectable outcome is `unknown` — never falsely "broken".
    /// `Consistent` yields `None`: the caller's own verdict proceeds.
    pub(crate) fn recorded_hook_outcome(self) -> Option<(HookState, HookReason)> {
        match self {
            Self::Consistent => None,
            Self::Inconsistent => Some((HookState::Conflict, HookReason::RecordedRootInconsistent)),
            Self::Absent => Some((HookState::Conflict, HookReason::RecordedRootAbsent)),
            Self::Unreadable
            | Self::Unsafe
            | Self::PathTooLong
            | Self::BudgetExhausted
            | Self::NotInspected => {
                Some((HookState::Unknown, HookReason::RecordedRootUninspectable))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Bounded JSON — C-2 structural bounds, folded into every JSON parse.
// ---------------------------------------------------------------------------

/// Proposed structural bounds for admitted JSON documents (C-2): nesting
/// depth and per-container member counts are capped independently of the
/// byte caps — over-limit documents classify as malformed, never truncated.
pub(crate) const MAX_JSON_DEPTH: usize = 32;
pub(crate) const MAX_JSON_MEMBERS: usize = 256;

/// Parses one bounded JSON document: valid JSON within the structural caps,
/// else `None` — the caller's classification decides malformed vs
/// unreadable per context.
pub(crate) fn bounded_json(bytes: &[u8]) -> Option<Value> {
    fn within(value: &Value, depth: usize) -> bool {
        if depth > MAX_JSON_DEPTH {
            return false;
        }
        match value {
            Value::Object(map) => {
                map.len() <= MAX_JSON_MEMBERS && map.values().all(|inner| within(inner, depth + 1))
            }
            Value::Array(items) => {
                items.len() <= MAX_JSON_MEMBERS
                    && items.iter().all(|inner| within(inner, depth + 1))
            }
            _ => true,
        }
    }
    let value = serde_json::from_slice::<Value>(bytes).ok()?;
    within(&value, 1).then_some(value)
}

/// JavaScript `Number.isSafeInteger` on a JSON number — accepts `3.0` the
/// way `=== 3` does.
fn safe_integer(value: &Value) -> Option<u64> {
    let number = value.as_f64()?;
    if number.fract() == 0.0 && (0.0..=9_007_199_254_740_991.0).contains(&number) {
        Some(number as u64)
    } else {
        None
    }
}

/// `^[0-9a-f]{64}$` — the digest syntax gate shared by marker and manifest.
pub(crate) fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

/// `validRelativePath` — nonempty, ≤256 UTF-16 units (the JS `.length`
/// domain), relative, no empty/`.`/`..` segments.
fn valid_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.encode_utf16().count() <= 256
        && !value.starts_with('/')
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

/// JavaScript string `<`/`>=` order — UTF-16 code-unit comparison, the same
/// ordering `sort` and `>=` apply to `files[].path`.
fn utf16_cmp(left: &str, right: &str) -> Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}

// ---------------------------------------------------------------------------
// Codex config — byte-level port of `parseCodexAgentIntegrationConfiguration`
// (all grammar tokens are ASCII; non-UTF-8 bytes elsewhere simply do not
// match — never a lossy decode).
// ---------------------------------------------------------------------------

/// The codex `~/.codex/config.toml` line-scan outcome — the declared facts
/// the precedence mapping consumes.
#[derive(Clone, Debug, Default)]
pub(crate) struct CodexScan {
    /// A `[plugins."orchbee-agent-observation@orchbee-local"]` section
    /// appeared exactly once.
    pub(crate) plugin_present: bool,
    /// A `[marketplaces.orchbee-local]` section appeared exactly once.
    pub(crate) marketplace_present: bool,
    /// A counted section appeared more than once.
    pub(crate) duplicate_section: bool,
    /// A section-declared key repeated (the scan's early-return condition).
    pub(crate) duplicate_key: bool,
    /// `enabled`: `None` = key absent; `Some(Some(b))` = a boolean;
    /// `Some(None)` = present but not `true`/`false`.
    pub(crate) enabled: Option<Option<bool>>,
    /// The marketplace `source_type` basic string when it parsed as one.
    pub(crate) source_type: Option<String>,
    /// The marketplace `source` basic string when it parsed as one.
    pub(crate) source: Option<String>,
}

/// Byte-level line scanner for `~/.codex/config.toml`: section headers are
/// byte-exact, assignments match `[A-Za-z][A-Za-z0-9_]*\s*=\s*.+`, duplicate
/// keys stop the scan exactly like the source's early return.
pub(crate) fn scan_codex_config(bytes: &[u8]) -> CodexScan {
    const MARKETPLACE: &[u8] = b"[marketplaces.orchbee-local]";
    const PLUGIN: &[u8] = b"[plugins.\"orchbee-agent-observation@orchbee-local\"]";
    #[derive(Clone, Copy, Eq, PartialEq)]
    enum Section {
        Other,
        Marketplace,
        Plugin,
    }
    let mut section = Section::Other;
    let mut marketplace_count = 0u32;
    let mut plugin_count = 0u32;
    let mut marketplace_values: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    let mut plugin_values: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    let mut duplicate_key = false;

    'lines: for raw_line in bytes.split(|byte| *byte == b'\n') {
        let line = trim_ascii(raw_line);
        if line.is_empty() || line[0] == b'#' {
            continue;
        }
        if line[0] == b'[' {
            if line == MARKETPLACE {
                marketplace_count += 1;
                section = Section::Marketplace;
            } else if line == PLUGIN {
                plugin_count += 1;
                section = Section::Plugin;
            } else {
                section = Section::Other;
            }
            continue;
        }
        let target = match section {
            Section::Other => continue,
            Section::Marketplace => &mut marketplace_values,
            Section::Plugin => &mut plugin_values,
        };
        let Some((key, value)) = parse_assignment(line) else {
            continue;
        };
        if target.iter().any(|(existing, _)| existing == key) {
            duplicate_key = true;
            break 'lines;
        }
        target.push((key.to_vec(), value.to_vec()));
    }

    fn lookup<'a>(values: &'a [(Vec<u8>, Vec<u8>)], key: &[u8]) -> Option<&'a [u8]> {
        values
            .iter()
            .find(|(existing, _)| existing == key)
            .map(|(_, value)| value.as_slice())
    }
    let plugin_present = plugin_count == 1;
    let enabled = if plugin_present {
        match lookup(&plugin_values, b"enabled") {
            None => None,
            Some(b"true") => Some(Some(true)),
            Some(b"false") => Some(Some(false)),
            Some(_) => Some(None),
        }
    } else {
        None
    };
    CodexScan {
        plugin_present,
        marketplace_present: marketplace_count == 1,
        duplicate_section: marketplace_count > 1 || plugin_count > 1,
        duplicate_key,
        enabled,
        source_type: lookup(&marketplace_values, b"source_type").and_then(basic_string),
        source: lookup(&marketplace_values, b"source").and_then(basic_string),
    }
}

/// ASCII whitespace trim — the scanner works on bytes; `trim()` in the
/// source covers ASCII whitespace which is all a config line can carry at
/// the edges for our tokens.
fn trim_ascii(bytes: &[u8]) -> &[u8] {
    fn is_space(byte: u8) -> bool {
        matches!(byte, b' ' | b'\t' | b'\r' | 0x0b | 0x0c)
    }
    let mut start = 0;
    let mut end = bytes.len();
    while start < end && is_space(bytes[start]) {
        start += 1;
    }
    while end > start && is_space(bytes[end - 1]) {
        end -= 1;
    }
    &bytes[start..end]
}

/// `[A-Za-z][A-Za-z0-9_]*\s*=\s*(.+)` on one trimmed line — returns the key
/// and the already-trimmed raw value.
fn parse_assignment(line: &[u8]) -> Option<(&[u8], &[u8])> {
    if line.is_empty() || !line[0].is_ascii_alphabetic() {
        return None;
    }
    let mut index = 1;
    while index < line.len() && (line[index].is_ascii_alphanumeric() || line[index] == b'_') {
        index += 1;
    }
    let key = &line[..index];
    while index < line.len() && matches!(line[index], b' ' | b'\t' | 0x0b | 0x0c) {
        index += 1;
    }
    if index >= line.len() || line[index] != b'=' {
        return None;
    }
    index += 1;
    while index < line.len() && matches!(line[index], b' ' | b'\t' | 0x0b | 0x0c) {
        index += 1;
    }
    let value = &line[index..];
    if value.is_empty() {
        return None;
    }
    Some((key, value))
}

/// `parseBasicString` — `JSON.parse` of the raw value producing a string,
/// else `None`.
fn basic_string(raw: &[u8]) -> Option<String> {
    // Codex serializes Windows paths as TOML single-line literal strings.
    #[cfg(windows)]
    if let Some(literal) = raw
        .strip_prefix(b"'")
        .and_then(|raw| raw.strip_suffix(b"'"))
    {
        if literal
            .iter()
            .any(|byte| *byte == b'\'' || *byte < 0x20 || *byte == 0x7f)
        {
            return None;
        }
        return std::str::from_utf8(literal).ok().map(str::to_owned);
    }
    serde_json::from_slice::<String>(raw).ok()
}

// ---------------------------------------------------------------------------
// Module-URL derivation — C-1 hardened `file:` admission.
// ---------------------------------------------------------------------------

/// The outcome of deriving a candidate root from an embedded `moduleUrl`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ModuleUrlRoot {
    /// The decoded absolute root bytes.
    Root(Vec<u8>),
    /// Not a `file:` URL with empty authority — `module-url-non-file`.
    NonFile,
    /// Query/fragment present, malformed escapes, NUL bytes, or a decoded
    /// non-absolute path — `module-url-invalid`.
    Invalid,
    /// Decoded path lacks the anchored provider suffix (or its remainder
    /// would be empty/the filesystem root) — `module-url-suffix-unrecognized`.
    SuffixUnrecognized,
}

impl ModuleUrlRoot {
    /// The detail reason spelling when no candidate root was derived.
    pub(crate) fn failure_reason(&self) -> Option<&'static str> {
        match self {
            Self::Root(_) => None,
            Self::NonFile => Some("module-url-non-file"),
            Self::Invalid => Some("module-url-invalid"),
            Self::SuffixUnrecognized => Some("module-url-suffix-unrecognized"),
        }
    }
}

/// `file:` admission and root derivation: exact `file://` scheme with empty
/// authority (the owning source emits `pathToFileURL` only — `localhost` is
/// rejected like any authority), query/fragment rejection, exactly one
/// percent-decode preserving raw bytes, NUL rejection, absoluteness, then the
/// anchored provider suffix strip. No ancestor guessing — a failed URL
/// contributes no candidate.
pub(crate) fn derive_module_root(module_url: &str, suffix: &str) -> ModuleUrlRoot {
    let Some(rest) = module_url.strip_prefix("file://") else {
        return ModuleUrlRoot::NonFile;
    };
    // Authority is everything up to the first '/'; it must be empty — the
    // source only ever emits `file:///…`. `rest` itself is the encoded path.
    if !rest.starts_with('/') {
        return ModuleUrlRoot::NonFile;
    }
    if rest.contains('?') || rest.contains('#') {
        return ModuleUrlRoot::Invalid;
    }
    let path = rest;
    let Some(decoded) = percent_decode(path.as_bytes()) else {
        return ModuleUrlRoot::Invalid;
    };
    if decoded.contains(&0) || !decoded.starts_with(b"/") {
        return ModuleUrlRoot::Invalid;
    }
    #[cfg(windows)]
    let decoded = if decoded.len() >= 4
        && decoded[1].is_ascii_alphabetic()
        && decoded[2] == b':'
        && decoded[3] == b'/'
    {
        decoded[1..].to_vec()
    } else {
        return ModuleUrlRoot::Invalid;
    };
    let Some(root) = decoded
        .strip_suffix(suffix.as_bytes())
        .map(|root| root.to_vec())
    else {
        return ModuleUrlRoot::SuffixUnrecognized;
    };
    if root.is_empty() || root == b"/" {
        return ModuleUrlRoot::SuffixUnrecognized;
    }
    ModuleUrlRoot::Root(root)
}

/// Exactly one percent-decode: `%XX` hex pairs decode, everything else is
/// literal (including `+`, which file URLs never use for space). Malformed
/// escapes fail the whole URL.
fn percent_decode(bytes: &[u8]) -> Option<Vec<u8>> {
    fn hex(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let (Some(high), Some(low)) = (
                bytes.get(index + 1).copied().and_then(hex),
                bytes.get(index + 2).copied().and_then(hex),
            ) else {
                return None;
            };
            decoded.push(high << 4 | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    Some(decoded)
}

// ---------------------------------------------------------------------------
// Bootstrap module — marker + exact template self-derivation.
// ---------------------------------------------------------------------------

/// The bootstrap-file scan verdict.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BootstrapVerdict {
    /// Marker and body equal the template derived from the file's own
    /// embedded `moduleUrl` literal.
    Present,
    /// No owned marker line — the file's content is not our declaration.
    ForeignContent,
    /// The marker prefix is ours but names a different integration id.
    NameMismatch,
    /// The marker is ours but the body differs from the derived template.
    TemplateMismatch,
}

/// The bootstrap scan result: the verdict plus the embedded `moduleUrl`
/// string whenever its literal parsed — recorded roots derive from the
/// declaration even when the body mismatches.
#[derive(Clone, Debug)]
pub(crate) struct BootstrapScan {
    pub(crate) verdict: BootstrapVerdict,
    pub(crate) module_url: Option<String>,
}

/// Scans a bootstrap module file: first line must carry our marker prefix
/// and integration id, second must be the `moduleUrl` assignment whose JSON
/// string literal is bounded-parsed (C-1d), and the whole file must equal
/// the template reconstructed from its own raw literal.
pub(crate) fn scan_bootstrap(
    bytes: &[u8],
    provider_id: &str,
    shape: &BootstrapShape,
) -> BootstrapScan {
    let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') else {
        return BootstrapScan {
            verdict: BootstrapVerdict::ForeignContent,
            module_url: None,
        };
    };
    let marker_line = &bytes[..newline];
    let Some(marker_id) = marker_line.strip_prefix(BOOTSTRAP_MARKER_PREFIX) else {
        return BootstrapScan {
            verdict: BootstrapVerdict::ForeignContent,
            module_url: None,
        };
    };
    if marker_id != provider_id.as_bytes() {
        return BootstrapScan {
            verdict: BootstrapVerdict::NameMismatch,
            module_url: None,
        };
    }
    let rest = &bytes[newline + 1..];
    let Some(after_prefix) = rest.strip_prefix(MODULE_URL_LINE_PREFIX) else {
        return BootstrapScan {
            verdict: BootstrapVerdict::TemplateMismatch,
            module_url: None,
        };
    };
    // The literal ends at the first unescaped quote; bounded JSON-string
    // parse per C-1d — never a regex substring, never raw line bytes.
    let Some(literal_end) = json_literal_end(after_prefix) else {
        return BootstrapScan {
            verdict: BootstrapVerdict::TemplateMismatch,
            module_url: None,
        };
    };
    let literal = &after_prefix[..literal_end];
    let module_url = serde_json::from_slice::<String>(literal).ok();
    let suffix = &after_prefix[literal_end..];
    let Some(body) = suffix.strip_prefix(b";\n".as_slice()) else {
        return BootstrapScan {
            verdict: BootstrapVerdict::TemplateMismatch,
            module_url,
        };
    };
    if body != shape.body || module_url.is_none() {
        return BootstrapScan {
            verdict: BootstrapVerdict::TemplateMismatch,
            module_url,
        };
    }
    BootstrapScan {
        verdict: BootstrapVerdict::Present,
        module_url,
    }
}

/// The byte offset just past a JSON string literal at `bytes[0]` — requires
/// the opening quote and honors backslash escapes; `None` when unterminated
/// or not a string literal.
fn json_literal_end(bytes: &[u8]) -> Option<usize> {
    if bytes.first() != Some(&b'"') {
        return None;
    }
    let mut index = 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => return Some(index + 1),
            b'\\' => index += 2,
            _ => index += 1,
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Guarded-directory marker — `parseDirectoryMarker` port with the doctor's
// version>3 divergence (unrecognized, not conflict).
// ---------------------------------------------------------------------------

/// One `files[]` entry of a guarded marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MarkerFileEntry {
    pub(crate) path: String,
    pub(crate) sha256: String,
}

/// The guarded-marker parse outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MarkerParse {
    /// Structurally valid marker for a supported contract version (1..=3).
    Valid {
        contract_version: u64,
        files: Vec<MarkerFileEntry>,
    },
    /// Structurally valid marker declaring a newer contract version — the
    /// doctor build is the stale side.
    UnrecognizedVersion(u64),
    /// The marker names a different integration id.
    NameMismatch,
    /// Any other shape contradiction.
    Invalid,
}

/// `parseDirectoryMarker` port: `kind == "guarded-extension"`, matching
/// `integrationId`, safe-integer `contractVersion`, `files[]` with strict
/// `{path, sha256}` entry shapes and strict ascending order — extra
/// top-level keys tolerated like the source, numeric `3.0` accepted like
/// JS `=== 3`. Divergence: `contractVersion > 3` reports
/// `UnrecognizedVersion` instead of the source's parse failure.
pub(crate) fn parse_guarded_marker(bytes: &[u8], integration_id: &str) -> MarkerParse {
    const MAX_ENTRY_FILES: usize = 64;
    let Some(value) = bounded_json(bytes) else {
        return MarkerParse::Invalid;
    };
    let Some(record) = value.as_object() else {
        return MarkerParse::Invalid;
    };
    if record.get("kind") != Some(&Value::String("guarded-extension".to_owned())) {
        return MarkerParse::Invalid;
    }
    match record.get("integrationId").and_then(Value::as_str) {
        Some(id) if id == integration_id => {}
        Some(_) => return MarkerParse::NameMismatch,
        None => return MarkerParse::Invalid,
    }
    let Some(contract_version) = record.get("contractVersion").and_then(safe_integer) else {
        return MarkerParse::Invalid;
    };
    if contract_version < 1 {
        return MarkerParse::Invalid;
    }
    if contract_version > SUPPORTED_CONTRACT_VERSION {
        return MarkerParse::UnrecognizedVersion(contract_version);
    }
    let Some(files) = record.get("files").and_then(Value::as_array) else {
        return MarkerParse::Invalid;
    };
    if files.len() > MAX_ENTRY_FILES {
        return MarkerParse::Invalid;
    }
    let mut parsed = Vec::with_capacity(files.len());
    for entry in files {
        let Some(record) = entry.as_object() else {
            return MarkerParse::Invalid;
        };
        // E-6: strict {path, sha256} entry shapes — deny extra keys.
        if record.len() != 2 {
            return MarkerParse::Invalid;
        }
        let (Some(path), Some(sha256)) = (
            record.get("path").and_then(Value::as_str),
            record.get("sha256").and_then(Value::as_str),
        ) else {
            return MarkerParse::Invalid;
        };
        if !valid_relative_path(path) || !valid_digest(sha256) {
            return MarkerParse::Invalid;
        }
        parsed.push(MarkerFileEntry {
            path: path.to_owned(),
            sha256: sha256.to_owned(),
        });
    }
    for pair in parsed.windows(2) {
        if utf16_cmp(&pair[0].path, &pair[1].path) != Ordering::Less {
            return MarkerParse::Invalid;
        }
    }
    MarkerParse::Valid {
        contract_version,
        files: parsed,
    }
}

/// Fingerprint comparison: the walk's computed `{path, sha256}` set versus
/// the marker's declared list — compared as UTF-16-sorted sequences so the
/// declared order's own collation never decides consistency.
pub(crate) fn fingerprint_matches(
    computed: &mut Vec<MarkerFileEntry>,
    declared: &[MarkerFileEntry],
) -> bool {
    let by_path = |entries: &mut Vec<MarkerFileEntry>| {
        entries.sort_by(|left, right| utf16_cmp(&left.path, &right.path));
    };
    let mut declared_sorted = declared.to_vec();
    by_path(computed);
    by_path(&mut declared_sorted);
    *computed == declared_sorted
}

// ---------------------------------------------------------------------------
// Cline package — `inspectManagedClinePackage` port.
// ---------------------------------------------------------------------------

/// The managed `package/` inspection outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ClinePackage {
    /// `name` matches and `orchbeeAgentObservation.contractVersion === 3`.
    Current,
    /// Safe-integer `contractVersion` in 1..=2.
    UpdateRequired,
    /// Safe-integer `contractVersion` above the supported version.
    UnrecognizedVersion,
    /// Anything else — `conflict`.
    Conflict,
}

/// `inspectManagedClinePackage` port: `package.json` must be a bounded JSON
/// record with `name == "orchbee-agent-observation"` and a numeric
/// `orchbeeAgentObservation.contractVersion` — `=== 3` is current, another
/// positive safe integer is stale (`< 3`) or unrecognized (`> 3`), anything
/// else is a contradiction.
pub(crate) fn parse_cline_package(bytes: &[u8]) -> ClinePackage {
    let Some(value) = bounded_json(bytes) else {
        return ClinePackage::Conflict;
    };
    let Some(record) = value.as_object() else {
        return ClinePackage::Conflict;
    };
    if record.get("name").and_then(Value::as_str) != Some(CLINE_PACKAGE_NAME) {
        return ClinePackage::Conflict;
    }
    let contract_version = record
        .get("orchbeeAgentObservation")
        .and_then(Value::as_object)
        .and_then(|marker| marker.get("contractVersion"))
        .and_then(safe_integer);
    match contract_version {
        Some(SUPPORTED_CONTRACT_VERSION) => ClinePackage::Current,
        Some(version) if version > SUPPORTED_CONTRACT_VERSION => ClinePackage::UnrecognizedVersion,
        Some(version) if version >= 1 => ClinePackage::UpdateRequired,
        _ => ClinePackage::Conflict,
    }
}

// ---------------------------------------------------------------------------
// Capsule — `readManifest` + `computeAgentObservationCapsulePayloadDigest`.
// ---------------------------------------------------------------------------

/// The strict capsule manifest check: exactly the three declared keys,
/// `contractVersion === 3`, `payloadVersion === "0.4.17"`, and a lowercase
/// `payloadSha256` — plus the declared values carried for detail.
#[derive(Clone, Debug)]
pub(crate) struct CapsuleManifestScan {
    /// The manifest's declared `contractVersion` when it parsed as a number.
    pub(crate) declared_contract_version: Option<f64>,
    /// The manifest's declared `payloadVersion` when it parsed as a string.
    pub(crate) declared_payload_version: Option<String>,
    /// The manifest's declared `payloadSha256` when it parsed as a string.
    pub(crate) declared_payload_sha256: Option<String>,
    /// The manifest declares a different contract/payload version — the
    /// payload contract cannot be evaluated under this build.
    pub(crate) version_mismatch: bool,
    /// The manifest satisfies the strict three-key shape and version gate.
    pub(crate) valid_shape: bool,
}

/// `readManifest` port: bounded JSON record, strict three-key set —
/// `version_mismatch` (declared ≠ supported) is reported separately from a
/// shape contradiction so a newer-format capsule reads as uninspectable,
/// never broken.
pub(crate) fn scan_capsule_manifest(bytes: &[u8]) -> Option<CapsuleManifestScan> {
    let value = bounded_json(bytes)?;
    let record = value.as_object()?;
    let declared_contract_version = record.get("contractVersion").and_then(Value::as_f64);
    let declared_payload_version = record
        .get("payloadVersion")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let declared_payload_sha256 = record
        .get("payloadSha256")
        .and_then(Value::as_str)
        .map(str::to_owned);
    // A version mismatch requires both fields to be present and declaring a
    // different version; missing or wrongly-typed fields are shape
    // contradictions, not "a different format we might not understand".
    let version_declared =
        declared_contract_version.is_some() && declared_payload_version.is_some();
    let version_matches = declared_contract_version == Some(3.0)
        && declared_payload_version.as_deref() == Some(CAPSULE_PAYLOAD_VERSION);
    let version_mismatch = version_declared && !version_matches;
    let valid_shape = !version_mismatch
        && version_declared
        && version_matches
        && record.len() == 3
        && declared_payload_sha256.as_deref().is_some_and(valid_digest);
    Some(CapsuleManifestScan {
        declared_contract_version,
        declared_payload_version,
        declared_payload_sha256,
        version_mismatch,
        valid_shape,
    })
}

/// One payload entry of the capsule walk — serialized in declaration order
/// exactly like the TypeScript objects.
#[derive(Clone, Debug)]
pub(crate) struct CapsuleEntry {
    pub(crate) directory: bool,
    pub(crate) path: String,
    pub(crate) sha256: Option<String>,
    pub(crate) size: Option<u64>,
}

/// `sha256(JSON.stringify(sortedEntries))` — the exact serialization:
/// `{kind,path}` for directories, `{kind,path,sha256,size}` for files,
/// entries sorted by UTF-16 code-unit path order like the source's `<`/`>`.
pub(crate) fn capsule_payload_digest(entries: &mut [CapsuleEntry]) -> String {
    entries.sort_by(|left, right| utf16_cmp(&left.path, &right.path));
    let mut serialized = String::from("[");
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            serialized.push(',');
        }
        let escaped = json_escape(&entry.path);
        match (entry.directory, &entry.sha256, entry.size) {
            (true, _, _) => {
                serialized.push_str(&format!("{{\"kind\":\"directory\",\"path\":{escaped}}}"));
            }
            (false, Some(sha256), Some(size)) => {
                serialized.push_str(&format!(
                    "{{\"kind\":\"file\",\"path\":{escaped},\"sha256\":\"{sha256}\",\"size\":{size}}}"
                ));
            }
            _ => {}
        }
    }
    serialized.push(']');
    hex_digest(&serialized)
}

/// JSON string escaping identical to `JSON.stringify`/`serde_json` for the
/// characters payload paths can carry: quote, backslash, and the short +
/// `\u00XX` control escapes.
fn json_escape(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization cannot fail")
}

/// Lowercase hex SHA-256 of `bytes`.
pub(crate) fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    let digest = Sha256::digest(bytes.as_ref());
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}
