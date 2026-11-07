//! Versioned provider registry data: process aliases with collision classes,
//! install provenance, package and bundled-runtime entries, and mode grammars.
//! Pure data and pure matching; nothing here reads the filesystem.

use super::mode::{AgentModeGrammar, grammar};
use crate::AgentReportProvider;

/// Registry data version; it changes whenever an entry's recognition meaning changes.
pub const AGENT_PROVIDER_REGISTRY_VERSION: u32 = 2;

/// How much a bare process name proves about its provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentAliasClass {
    /// Only this provider installs the name; the name alone identifies it.
    Unique,
    /// Unrelated programs use the name; a listed install root must prove it.
    Shared,
    /// Several providers or generic tools install the name; a listed install root must prove it.
    Generic,
}

/// One process name of a provider. A name ending in `#` matches a version suffix that starts
/// with an ASCII digit after the prefix, such as `muse-bin-1.2.3`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentAlias {
    pub name: &'static str,
    pub class: AgentAliasClass,
}

/// The install shape a recognized process was matched through.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentInstallShape {
    /// A native image named by an alias, including Python console launchers and version-manager shims.
    NativeImage,
    /// A runtime (`node`, `bun`, Python) running a listed package entry, an alias-named script, or an allowed Python module.
    PackageEntry,
    /// A runtime image inside a provider install root running that root's listed entry.
    BundledRuntime,
    /// `cmd /c` or PowerShell `-Command` resolving to the native command it launches.
    LauncherWrapper,
    /// A `.cmd`, `.bat`, `.ps1`, or POSIX shell script named by an alias.
    Shim,
}

/// Contiguous directory segments that prove an install location; matching ignores ASCII case
/// and separator style, and a segment ending in `*` matches by prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentInstallRoot(pub &'static [&'static str]);

/// A runtime image inside `root` running the script basename `entry` from the same root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentBundledRuntime {
    pub root: AgentInstallRoot,
    pub entry: &'static str,
}

/// One provider's recognition data. Version ranges are recorded only after a real corpus
/// verifies them; until then every entry is best effort.
#[derive(Clone, Copy, Debug)]
pub struct AgentProviderEntry {
    pub provider: AgentReportProvider,
    pub aliases: &'static [AgentAlias],
    /// Install roots that prove shared and generic aliases.
    pub install_roots: &'static [AgentInstallRoot],
    /// Script path suffixes, at a path-segment boundary, of package entries.
    pub package_entries: &'static [&'static str],
    pub bundled_runtimes: &'static [AgentBundledRuntime],
    /// Python modules whose `-m` launch is this provider.
    pub python_modules: &'static [&'static str],
    pub grammar: &'static AgentModeGrammar,
}

impl AgentReportProvider {
    /// This provider's registry entry; every provider has exactly one.
    pub fn registry_entry(self) -> &'static AgentProviderEntry {
        AGENT_PROVIDER_REGISTRY
            .iter()
            .find(|entry| entry.provider == self)
            .expect("every provider has a registry entry")
    }
}

const fn unique(name: &'static str) -> AgentAlias {
    AgentAlias {
        name,
        class: AgentAliasClass::Unique,
    }
}
const fn shared(name: &'static str) -> AgentAlias {
    AgentAlias {
        name,
        class: AgentAliasClass::Shared,
    }
}
const fn generic(name: &'static str) -> AgentAlias {
    AgentAlias {
        name,
        class: AgentAliasClass::Generic,
    }
}
const NONE: &[&str] = &[];

const fn entry(
    provider: AgentReportProvider,
    aliases: &'static [AgentAlias],
    package_entries: &'static [&'static str],
    grammar: &'static AgentModeGrammar,
) -> AgentProviderEntry {
    AgentProviderEntry {
        provider,
        aliases,
        install_roots: &[],
        package_entries,
        bundled_runtimes: &[],
        python_modules: NONE,
        grammar,
    }
}

/// The registry, in `AgentReportProvider::ALL` order. Desktop and IDE application names that
/// share a provider's report label (`cursor`, `kiro`, `qoder`, `antigravity`) are not process
/// aliases, so those applications are never candidates.
pub static AGENT_PROVIDER_REGISTRY: [AgentProviderEntry; 24] = {
    use AgentReportProvider as P;
    [
        AgentProviderEntry {
            install_roots: &[
                AgentInstallRoot(&["node_modules", "@earendil-works", "pi-coding-agent"]),
                AgentInstallRoot(&["node_modules", "@mariozechner", "pi-coding-agent"]),
            ],
            ..entry(
                P::Pi,
                &[shared("pi")],
                &[
                    "node_modules/@earendil-works/pi-coding-agent/dist/cli.js",
                    "node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js",
                    "node_modules/@mariozechner/pi-coding-agent/dist/cli.js",
                ],
                &grammar::PI,
            )
        },
        entry(
            P::Claude,
            &[unique("claude")],
            &["node_modules/@anthropic-ai/claude-code/cli.js"],
            &grammar::CLAUDE,
        ),
        entry(
            P::Codex,
            &[unique("codex")],
            &["node_modules/@openai/codex/bin/codex.js"],
            &grammar::CODEX,
        ),
        entry(
            P::Gemini,
            &[unique("gemini")],
            &["node_modules/@google/gemini-cli/dist/index.js"],
            &grammar::GEMINI,
        ),
        AgentProviderEntry {
            install_roots: &[AgentInstallRoot(&["cursor-agent"])],
            bundled_runtimes: &[AgentBundledRuntime {
                root: AgentInstallRoot(&["cursor-agent", "versions"]),
                entry: "index.js",
            }],
            ..entry(
                P::Cursor,
                &[unique("cursor-agent"), generic("agent")],
                NONE,
                &grammar::CURSOR,
            )
        },
        entry(P::Devin, &[unique("devin")], NONE, &grammar::DEVIN),
        entry(
            P::Antigravity,
            &[unique("agy"), unique("antigravity-cli")],
            NONE,
            &grammar::ANTIGRAVITY,
        ),
        entry(P::Cline, &[unique("cline")], NONE, &grammar::DEFAULT),
        entry(P::Omp, &[unique("omp")], NONE, &grammar::PI),
        entry(
            P::Mastracode,
            &[unique("mastracode")],
            &["node_modules/mastracode/dist/cli.js"],
            &grammar::DEFAULT,
        ),
        entry(P::OpenCode, &[unique("opencode")], NONE, &grammar::OPENCODE),
        AgentProviderEntry {
            install_roots: &[
                AgentInstallRoot(&["node_modules", "@github", "copilot"]),
                AgentInstallRoot(&["winget", "packages", "github.copilot_*"]),
            ],
            ..entry(
                P::Copilot,
                &[shared("copilot")],
                &["node_modules/@github/copilot/index.js"],
                &grammar::COPILOT,
            )
        },
        entry(
            P::Kimi,
            &[unique("kimi")],
            &["node_modules/@moonshot-ai/kimi-code/dist/main.mjs"],
            &grammar::KIMI,
        ),
        entry(P::Kiro, &[unique("kiro-cli")], NONE, &grammar::DEFAULT),
        entry(P::Droid, &[unique("droid")], NONE, &grammar::DROID),
        AgentProviderEntry {
            install_roots: &[
                AgentInstallRoot(&["node_modules", "@sourcegraph", "amp"]),
                AgentInstallRoot(&[".amp", "bin"]),
            ],
            ..entry(
                P::Amp,
                &[shared("amp")],
                &["node_modules/@sourcegraph/amp/dist/main.js"],
                &grammar::DEFAULT,
            )
        },
        AgentProviderEntry {
            install_roots: &[AgentInstallRoot(&[".grok", "bin"])],
            ..entry(
                P::Grok,
                &[unique("grok"), generic("agent")],
                NONE,
                &grammar::GROK,
            )
        },
        AgentProviderEntry {
            install_roots: &[AgentInstallRoot(&["hermes-agent"])],
            python_modules: &["hermes_cli"],
            ..entry(P::Hermes, &[shared("hermes")], NONE, &grammar::HERMES)
        },
        AgentProviderEntry {
            install_roots: &[AgentInstallRoot(&["node_modules", "@kilocode", "cli"])],
            ..entry(
                P::Kilo,
                &[shared("kilo"), unique("kilocode")],
                NONE,
                &grammar::OPENCODE,
            )
        },
        entry(
            P::Qoder,
            &[unique("qodercli"), unique("qoderclicn")],
            NONE,
            &grammar::QODER,
        ),
        entry(
            P::Qwen,
            &[unique("qwen")],
            &[
                "node_modules/@qwen-code/qwen-code/dist/index.js",
                "node_modules/@qwen-code/qwen-code/cli-entry.js",
                "node_modules/@qwen-code/qwen-code/cli.js",
            ],
            &grammar::GEMINI,
        ),
        entry(
            P::Letta,
            &[unique("letta")],
            &["node_modules/@letta-ai/letta-code/letta.js"],
            &grammar::LETTA,
        ),
        entry(P::Maki, &[unique("maki")], NONE, &grammar::DEFAULT),
        entry(
            P::Muse,
            &[unique("muse"), unique("muse-bin-#")],
            NONE,
            &grammar::DEFAULT,
        ),
    ]
};

impl AgentAlias {
    pub(super) fn matches(self, name: &str) -> bool {
        match self.name.strip_suffix('#') {
            Some(prefix) => name
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit())),
            None => self.name == name,
        }
    }
}

impl AgentInstallRoot {
    /// True when the directory part of `path` contains these segments contiguously.
    pub fn contains(self, path: &str) -> bool {
        let segments = directory_segments(path);
        !self.0.is_empty()
            && segments.windows(self.0.len()).any(|window| {
                window.iter().zip(self.0).all(|(segment, pattern)| {
                    match pattern.strip_suffix('*') {
                        Some(prefix) => segment.starts_with(&prefix.to_ascii_lowercase()),
                        None => *segment == pattern.to_ascii_lowercase(),
                    }
                })
            })
    }
}

/// Lowercased directory segments of a path in either separator style; the basename is dropped.
fn directory_segments(path: &str) -> Vec<String> {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    let mut segments: Vec<String> = lower
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    segments.pop();
    segments
}

/// True when `path` ends with `suffix` at a path-segment boundary, ignoring ASCII case and separators.
pub(super) fn ends_with_entry(path: &str, suffix: &str) -> bool {
    let path = path.replace('\\', "/").to_ascii_lowercase();
    path == suffix
        || path
            .strip_suffix(suffix)
            .is_some_and(|prefix| prefix.ends_with('/'))
}
