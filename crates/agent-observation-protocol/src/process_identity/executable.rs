//! Per-process recognition from the actual image and its arguments. Names mentioned in
//! arbitrary arguments never identify a provider; wrappers resolve only to what they launch.

use super::native_agent_basename as basename;
use super::registry::{
    AGENT_PROVIDER_REGISTRY, AgentAliasClass, AgentInstallShape, ends_with_entry,
};
use super::{AgentIdentityUnknownReason, AgentInvocationMode};
use crate::AgentReportProvider;

/// What one sampled process proves on its own. Selection and settling belong to the identity state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentProcessRecognition {
    /// No provider shape matches.
    NotAgent,
    Interactive {
        provider: AgentReportProvider,
        shape: AgentInstallShape,
    },
    /// A provider shape in a noninteractive form; it never holds an interactive run.
    Noninteractive {
        provider: AgentReportProvider,
        shape: AgentInstallShape,
    },
    /// A row that could hold an Agent but cannot be resolved; `provider` is set only when one provider is implied.
    Unresolved {
        provider: Option<AgentReportProvider>,
        reason: AgentUnresolvedProcess,
    },
    /// A remote execution client such as SSH, WSL, or a container exec.
    RemoteClient,
}

/// Why a row that could hold an Agent stays unresolved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentUnresolvedProcess {
    CommandLineUnreadable,
    UnclassifiedArguments,
    UnverifiedProvenance,
}

impl AgentUnresolvedProcess {
    pub const fn reason(self) -> AgentIdentityUnknownReason {
        match self {
            Self::CommandLineUnreadable => AgentIdentityUnknownReason::CommandLineUnreadable,
            Self::UnclassifiedArguments => AgentIdentityUnknownReason::UnclassifiedArguments,
            Self::UnverifiedProvenance => AgentIdentityUnknownReason::UnverifiedProvenance,
        }
    }
}

impl AgentProcessRecognition {
    /// The provider of an interactive match only.
    pub const fn interactive_provider(self) -> Option<AgentReportProvider> {
        match self {
            Self::Interactive { provider, .. } => Some(provider),
            _ => None,
        }
    }

    /// Transitional candidate rule until the Server identity state owns the result: unclassified
    /// arguments, and a native image named by an alias whose command line is unreadable, still
    /// count as an interactive candidate, as before the mode grammar existed. Noninteractive forms
    /// and every other unresolved row do not.
    pub const fn candidate_provider(self) -> Option<AgentReportProvider> {
        match self {
            Self::Interactive { provider, .. }
            | Self::Unresolved {
                provider: Some(provider),
                reason:
                    AgentUnresolvedProcess::UnclassifiedArguments
                    | AgentUnresolvedProcess::CommandLineUnreadable,
            } => Some(provider),
            _ => None,
        }
    }
}

/// Classifies one process from its image path and decoded arguments; `None` arguments mean
/// the command line was unreadable, which never defaults to interactive. A missing image is an
/// opaque row that the sampler reports separately.
pub fn classify_agent_process(
    image: Option<&str>,
    argv: Option<&[String]>,
) -> AgentProcessRecognition {
    use AgentProcessRecognition as R;
    let Some(image) = image else {
        return R::NotAgent;
    };
    match resolve_name(image) {
        Name::Provider(provider) => {
            return with_mode(
                provider,
                AgentInstallShape::NativeImage,
                argv.map(|argv| argv.get(1..).unwrap_or_default()),
            );
        }
        Name::Unproven(provider) => return unproven(provider),
        Name::Absent => {}
    }
    let stem = basename(image);
    let Some(host) = Host::of(&stem) else {
        return R::NotAgent;
    };
    let Some(argv) = argv else {
        return match host {
            Host::Node | Host::Bun | Host::Python => R::Unresolved {
                provider: None,
                reason: AgentUnresolvedProcess::CommandLineUnreadable,
            },
            Host::Remote(_) => remote(&stem, &[]),
            _ => R::NotAgent,
        };
    };
    // A forged argv[0] must not change the actual executable's identity.
    if argv.first().is_none_or(|first| basename(first) != stem) {
        return R::NotAgent;
    }
    match host {
        Host::Remote(_) => remote(&stem, &argv[1..]),
        Host::Cmd => cmd(argv),
        Host::PowerShell => powershell(argv),
        Host::Node | Host::Bun | Host::Python | Host::PosixShell => runtime(host, image, argv),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Host {
    Node,
    Bun,
    Python,
    PosixShell,
    PowerShell,
    Cmd,
    Remote(bool),
}

impl Host {
    fn of(stem: &str) -> Option<Self> {
        Some(match stem {
            "node" => Self::Node,
            "bun" => Self::Bun,
            "sh" | "bash" | "zsh" | "fish" => Self::PosixShell,
            "pwsh" | "powershell" => Self::PowerShell,
            "cmd" => Self::Cmd,
            "ssh" | "mosh" | "wsl" => Self::Remote(true),
            "docker" | "podman" | "kubectl" => Self::Remote(false),
            python
                if python.strip_prefix("python").is_some_and(|version| {
                    version.chars().all(|c| c.is_ascii_digit() || c == '.')
                }) =>
            {
                Self::Python
            }
            _ => return None,
        })
    }
}

/// SSH, mosh, and WSL are remote clients outright; container tools only for exec, run, and attach.
fn remote(stem: &str, args: &[String]) -> AgentProcessRecognition {
    let direct = matches!(Host::of(stem), Some(Host::Remote(true)));
    let exec = args
        .iter()
        .find(|arg| !arg.starts_with('-'))
        .is_some_and(|command| matches!(command.as_str(), "exec" | "run" | "attach"));
    if direct || exec {
        AgentProcessRecognition::RemoteClient
    } else {
        AgentProcessRecognition::NotAgent
    }
}

enum Name {
    Provider(AgentReportProvider),
    Unproven(Option<AgentReportProvider>),
    Absent,
}

/// Resolves a path's basename through aliases; shared and generic aliases need a listed install root.
fn resolve_name(path: &str) -> Name {
    let name = basename(path);
    let mut proven = None;
    let mut proven_count = 0;
    let mut unproven = None;
    let mut unproven_count = 0;
    for entry in &AGENT_PROVIDER_REGISTRY {
        for alias in entry.aliases.iter().filter(|alias| alias.matches(&name)) {
            if alias.class == AgentAliasClass::Unique {
                return Name::Provider(entry.provider);
            }
            if entry.install_roots.iter().any(|root| root.contains(path)) {
                proven = Some(entry.provider);
                proven_count += 1;
            } else {
                unproven = Some(entry.provider);
                unproven_count += 1;
            }
        }
    }
    match (proven_count, unproven_count) {
        (1, _) => Name::Provider(proven.expect("counted")),
        (0, 0) => Name::Absent,
        (0, 1) => Name::Unproven(unproven),
        _ => Name::Unproven(None),
    }
}

fn unproven(provider: Option<AgentReportProvider>) -> AgentProcessRecognition {
    AgentProcessRecognition::Unresolved {
        provider,
        reason: AgentUnresolvedProcess::UnverifiedProvenance,
    }
}

fn with_mode(
    provider: AgentReportProvider,
    shape: AgentInstallShape,
    args: Option<&[String]>,
) -> AgentProcessRecognition {
    let Some(args) = args else {
        return AgentProcessRecognition::Unresolved {
            provider: Some(provider),
            reason: AgentUnresolvedProcess::CommandLineUnreadable,
        };
    };
    match provider.classify_invocation(args) {
        AgentInvocationMode::Interactive => {
            AgentProcessRecognition::Interactive { provider, shape }
        }
        AgentInvocationMode::Noninteractive => {
            AgentProcessRecognition::Noninteractive { provider, shape }
        }
        AgentInvocationMode::Unclassified => AgentProcessRecognition::Unresolved {
            provider: Some(provider),
            reason: AgentUnresolvedProcess::UnclassifiedArguments,
        },
    }
}

/// Resolves a launched command token and its arguments; a script suffix makes it a shim.
fn launched(token: &str, args: &[String], wrapper: AgentInstallShape) -> AgentProcessRecognition {
    let lower = token.to_ascii_lowercase();
    let shape = if [".cmd", ".bat", ".ps1"].iter().any(|s| lower.ends_with(s)) {
        AgentInstallShape::Shim
    } else {
        wrapper
    };
    match resolve_name(token) {
        Name::Provider(provider) => with_mode(provider, shape, Some(args)),
        Name::Unproven(provider) => unproven(provider),
        Name::Absent => AgentProcessRecognition::NotAgent,
    }
}

/// Splits a wrapper's command into tokens; text with shell syntax never resolves.
fn command_tokens(rest: &[String], syntax: &[char]) -> Option<Vec<String>> {
    if rest.iter().any(|arg| arg.contains(syntax)) {
        return None;
    }
    match rest {
        [single] => Some(single.split_whitespace().map(str::to_owned).collect()),
        _ => Some(rest.to_vec()),
    }
}

fn wrapped(rest: &[String], syntax: &[char]) -> AgentProcessRecognition {
    match command_tokens(rest, syntax).as_deref() {
        Some([token, args @ ..]) => launched(token, args, AgentInstallShape::LauncherWrapper),
        _ => AgentProcessRecognition::NotAgent,
    }
}

fn cmd(argv: &[String]) -> AgentProcessRecognition {
    for (index, arg) in argv.iter().enumerate().skip(1) {
        match arg.to_ascii_lowercase().as_str() {
            "/c" | "/k" => {
                return wrapped(
                    &argv[index + 1..],
                    &['&', '|', '<', '>', '^', '%', '(', ')'],
                );
            }
            "/d" | "/q" | "/s" | "/a" | "/u" | "/v:on" | "/v:off" | "/e:on" | "/e:off" => {}
            _ => break,
        }
    }
    AgentProcessRecognition::NotAgent
}

fn powershell(argv: &[String]) -> AgentProcessRecognition {
    let mut index = 1;
    while let Some(arg) = argv.get(index) {
        match arg.to_ascii_lowercase().as_str() {
            "-file" | "-f" => {
                return match argv.get(index + 1) {
                    Some(script) => launched(script, &argv[index + 2..], AgentInstallShape::Shim),
                    None => AgentProcessRecognition::NotAgent,
                };
            }
            "-command" | "-c" => {
                return wrapped(
                    &argv[index + 1..],
                    &[
                        ';', '|', '&', '$', '(', ')', '{', '}', '@', '\'', '"', '`', '<', '>',
                    ],
                );
            }
            "-noprofile" | "-nologo" | "-noninteractive" | "-noexit" => {}
            "-executionpolicy" | "-windowstyle" | "-workingdirectory" if index + 1 < argv.len() => {
                index += 1;
            }
            _ => break,
        }
        index += 1;
    }
    AgentProcessRecognition::NotAgent
}

/// `node`, `bun`, Python, and POSIX shells: bundled runtime, package entry, alias-named script, or Python module.
fn runtime(host: Host, image: &str, argv: &[String]) -> AgentProcessRecognition {
    let Some(entry) = entrypoint(host, argv) else {
        return AgentProcessRecognition::NotAgent;
    };
    let (target, args) = match entry {
        Entry::Script(index) => (&argv[index], &argv[index + 1..]),
        Entry::Module(index) => {
            let module = argv[index].as_str();
            return AGENT_PROVIDER_REGISTRY
                .iter()
                .find(|entry| entry.python_modules.contains(&module))
                .map_or(AgentProcessRecognition::NotAgent, |entry| {
                    with_mode(
                        entry.provider,
                        AgentInstallShape::PackageEntry,
                        Some(&argv[index + 1..]),
                    )
                });
        }
    };
    for entry in &AGENT_PROVIDER_REGISTRY {
        let bundled = entry.bundled_runtimes.iter().any(|bundled| {
            host != Host::PosixShell
                && bundled.root.contains(image)
                && bundled.root.contains(target)
                && basename_with_suffix(target) == bundled.entry
        });
        if bundled {
            return with_mode(
                entry.provider,
                AgentInstallShape::BundledRuntime,
                Some(args),
            );
        }
        if host != Host::PosixShell
            && entry
                .package_entries
                .iter()
                .any(|suffix| ends_with_entry(target, suffix))
        {
            return with_mode(entry.provider, AgentInstallShape::PackageEntry, Some(args));
        }
    }
    let shape = if host == Host::PosixShell {
        AgentInstallShape::Shim
    } else {
        AgentInstallShape::PackageEntry
    };
    launched(target, args, shape)
}

fn basename_with_suffix(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

enum Entry {
    Script(usize),
    Module(usize),
}

/// The index of the runtime's script or Python module after supported runtime options.
fn entrypoint(host: Host, argv: &[String]) -> Option<Entry> {
    let mut index = 1;
    while let Some(arg) = argv.get(index) {
        match arg.as_str() {
            "--" => return (index + 1 < argv.len()).then_some(Entry::Script(index + 1)),
            "-m" if host == Host::Python => {
                return (index + 1 < argv.len()).then_some(Entry::Module(index + 1));
            }
            "-r"
            | "--require"
            | "--loader"
            | "--import"
            | "--experimental-loader"
            | "--disable-warning"
            | "-W"
            | "-X" => {
                argv.get(index + 1)?;
                index += 1;
            }
            "--no-warnings"
            | "--no-deprecation"
            | "--trace-warnings"
            | "--enable-source-maps"
            | "--inspect"
            | "--inspect-brk"
            | "-u"
            | "-B"
            | "-E"
            | "-s"
            | "-I"
            | "-O"
            | "-OO" => {}
            value
                if [
                    "--inspect=",
                    "--inspect-brk=",
                    "--max-old-space-size=",
                    "--stack-size=",
                    "--disable-warning=",
                ]
                .iter()
                .any(|prefix| value.starts_with(prefix))
                    || (value.starts_with("--experimental-") && !value.contains('=')) => {}
            value if value.starts_with('-') => return None,
            _ => return Some(Entry::Script(index)),
        }
        index += 1;
    }
    None
}
