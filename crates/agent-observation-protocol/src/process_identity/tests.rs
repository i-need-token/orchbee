use super::*;
use AgentInstallShape as S;
use AgentReportProvider as P;
use AgentUnresolvedProcess as U;

fn p(
    pid: u32,
    parent: Option<u32>,
    provider: Option<AgentReportProvider>,
) -> AgentProcessCandidate {
    AgentProcessCandidate {
        pid,
        parent_pid: parent,
        creation_time: u64::from(pid),
        provider,
    }
}
#[test]
fn selection_requires_one_valid_outer_chain_and_creation_order() {
    let mut rows = vec![
        p(1, None, None),
        p(2, Some(1), Some(AgentReportProvider::Codex)),
        p(3, Some(2), Some(AgentReportProvider::Claude)),
    ];
    assert_eq!(
        select_agent_process(&rows),
        AgentProcessSelection::Selected {
            pid: 2,
            creation_time: 2,
            provider: AgentReportProvider::Codex
        }
    );
    rows[2].parent_pid = Some(1);
    assert_eq!(
        select_agent_process(&rows),
        AgentProcessSelection::Ambiguous
    );
    rows[2].parent_pid = Some(2);
    rows[1].creation_time = 4;
    assert_eq!(
        select_agent_process(&rows),
        AgentProcessSelection::Ambiguous
    );
    rows[1].creation_time = 3;
    rows[1].parent_pid = Some(3);
    assert_eq!(
        select_agent_process(&rows),
        AgentProcessSelection::Ambiguous
    );
    rows.truncate(1);
    assert_eq!(select_agent_process(&rows), AgentProcessSelection::None);
    rows.push(rows[0]);
    assert_eq!(
        select_agent_process(&rows),
        AgentProcessSelection::Ambiguous
    );
    assert_eq!(select_agent_process(&[]), AgentProcessSelection::None);
}

fn interactive(provider: P, shape: S) -> AgentProcessRecognition {
    AgentProcessRecognition::Interactive { provider, shape }
}
fn noninteractive(provider: P, shape: S) -> AgentProcessRecognition {
    AgentProcessRecognition::Noninteractive { provider, shape }
}
fn unresolved(provider: Option<P>, reason: U) -> AgentProcessRecognition {
    AgentProcessRecognition::Unresolved { provider, reason }
}
const NOT: AgentProcessRecognition = AgentProcessRecognition::NotAgent;
const REMOTE: AgentProcessRecognition = AgentProcessRecognition::RemoteClient;
const HOME: &str = r"C:\Users\u";
const NPM: &str = r"C:\Users\u\AppData\Roaming\npm";
const CURSOR: &str = r"C:\Users\u\AppData\Local\cursor-agent";
const HERMES: &str = r"C:\Users\u\AppData\Roaming\uv\tools\hermes-agent\Scripts\hermes.exe";

/// One fixture per Windows support matrix row and proposal counterexample:
/// (row, image, decoded argv or `None` when unreadable, expected recognition).
/// Console attachment and opaque rows need Host facts and belong to the sampler.
#[test]
fn recognition_matrix_rows_and_counterexamples() {
    let f = |s: &str| {
        s.replace("{home}", HOME)
            .replace("{npm}", NPM)
            .replace("{cursor}", CURSOR)
    };
    let rows: Vec<(&str, &str, Option<Vec<&str>>, AgentProcessRecognition)> = vec![
        // Native image named by an alias.
        (
            "native",
            r"{home}\.local\bin\claude.exe",
            Some(vec!["claude"]),
            interactive(P::Claude, S::NativeImage),
        ),
        (
            "native",
            r"C:\tools\CODEX.EXE",
            Some(vec!["forged.exe"]),
            interactive(P::Codex, S::NativeImage),
        ),
        (
            "native",
            r"{home}\.grok\bin\grok.exe",
            Some(vec!["grok"]),
            interactive(P::Grok, S::NativeImage),
        ),
        (
            "native versioned",
            r"C:\tools\muse-bin-1.2.3.exe",
            Some(vec!["muse-bin-1.2.3"]),
            interactive(P::Muse, S::NativeImage),
        ),
        // Native image with an unlisted name stays outside the matrix.
        (
            "unlisted name",
            r"C:\tools\codex-1.2.exe",
            Some(vec!["codex-1.2"]),
            NOT,
        ),
        (
            "unlisted name",
            r"C:\tools\muse-bin-x.exe",
            Some(vec!["muse-bin-x"]),
            NOT,
        ),
        // A generic `agent` needs provenance and is never Cursor by name alone.
        (
            "generic agent",
            r"{home}\.grok\bin\agent.exe",
            Some(vec!["agent"]),
            interactive(P::Grok, S::NativeImage),
        ),
        (
            "generic agent",
            r"{cursor}\agent.exe",
            Some(vec!["agent"]),
            interactive(P::Cursor, S::NativeImage),
        ),
        (
            "generic agent",
            r"C:\tools\agent.exe",
            Some(vec!["agent"]),
            unresolved(None, U::UnverifiedProvenance),
        ),
        (
            "generic agent",
            "agent.exe",
            Some(vec!["agent"]),
            unresolved(None, U::UnverifiedProvenance),
        ),
        // Desktop and IDE applications that share a report label are never candidates.
        (
            "ide",
            r"{home}\AppData\Local\Programs\cursor\Cursor.exe",
            Some(vec!["Cursor.exe", "."]),
            NOT,
        ),
        (
            "ide",
            r"{home}\AppData\Local\Programs\Kiro\Kiro.exe",
            Some(vec!["Kiro.exe"]),
            NOT,
        ),
        (
            "ide",
            r"{home}\AppData\Local\Programs\Qoder\Qoder.exe",
            Some(vec!["Qoder.exe"]),
            NOT,
        ),
        (
            "ide",
            r"{home}\AppData\Local\Programs\Antigravity\Antigravity.exe",
            Some(vec!["Antigravity.exe"]),
            NOT,
        ),
        // Unrelated programs sharing a short alias need provenance.
        (
            "shared alias",
            r"C:\Program Files\amp\amp.exe",
            Some(vec!["amp"]),
            unresolved(Some(P::Amp), U::UnverifiedProvenance),
        ),
        (
            "shared alias",
            r"C:\Program Files\kilo\kilo.exe",
            Some(vec!["kilo"]),
            unresolved(Some(P::Kilo), U::UnverifiedProvenance),
        ),
        (
            "shared alias",
            r"C:\hermes\hermes.exe",
            Some(vec!["hermes"]),
            unresolved(Some(P::Hermes), U::UnverifiedProvenance),
        ),
        (
            "shared alias",
            r"{home}\.amp\bin\amp.exe",
            Some(vec!["amp"]),
            interactive(P::Amp, S::NativeImage),
        ),
        // Runtime script named by an alias.
        (
            "runtime alias script",
            r"C:\Program Files\nodejs\node.exe",
            Some(vec![
                "node",
                r"C:\npm\node_modules\@openai\codex\bin\codex.js",
                "task",
            ]),
            interactive(P::Codex, S::PackageEntry),
        ),
        (
            "runtime alias script",
            "node.exe",
            Some(vec!["node.exe", r"C:\bin\amp.js"]),
            unresolved(Some(P::Amp), U::UnverifiedProvenance),
        ),
        // Runtime script at a listed package path.
        (
            "package entry",
            "node.exe",
            Some(vec![
                "node.exe",
                "C:/node_modules/@anthropic-ai/claude-code/cli.js",
            ]),
            interactive(P::Claude, S::PackageEntry),
        ),
        (
            "package entry",
            "bun.exe",
            Some(vec![
                "bun.exe",
                r"C:\node_modules\@qwen-code\qwen-code\dist\index.js",
            ]),
            interactive(P::Qwen, S::PackageEntry),
        ),
        (
            "package entry",
            "node.exe",
            Some(vec![
                "node.exe",
                r"C:\node_modules\@sourcegraph\amp\dist\main.js",
            ]),
            interactive(P::Amp, S::PackageEntry),
        ),
        // Bundled runtime inside a provider install root; an unlisted path stays missed.
        (
            "bundled runtime",
            r"{cursor}\versions\2025.10.1\node.exe",
            Some(vec!["node.exe", r"{cursor}\versions\2025.10.1\index.js"]),
            interactive(P::Cursor, S::BundledRuntime),
        ),
        (
            "bundled runtime",
            "node.exe",
            Some(vec!["node.exe", r"C:\project\index.js"]),
            NOT,
        ),
        // Listed runtime options are skipped; unlisted options stay outside.
        (
            "runtime options",
            "node.exe",
            Some(vec![
                "node.exe",
                "--max-old-space-size=8192",
                "--disable-warning",
                "ExperimentalWarning",
                "C:/node_modules/@anthropic-ai/claude-code/cli.js",
            ]),
            interactive(P::Claude, S::PackageEntry),
        ),
        (
            "runtime options",
            "node.exe",
            Some(vec![
                "node.exe",
                "--unknown-option",
                "C:/node_modules/@anthropic-ai/claude-code/cli.js",
            ]),
            NOT,
        ),
        // Package-manager shims through cmd resolve to what they launch.
        (
            "shim",
            r"C:\Windows\System32\cmd.exe",
            Some(vec!["cmd.exe", "/d", "/s", "/c", r"{npm}\codex.cmd"]),
            interactive(P::Codex, S::Shim),
        ),
        (
            "shim",
            "cmd.exe",
            Some(vec!["cmd.exe", "/c", r"{npm}\codex.cmd --version"]),
            noninteractive(P::Codex, S::Shim),
        ),
        (
            "shim",
            "cmd.exe",
            Some(vec!["cmd.exe", "/c", r"{cursor}\agent.cmd"]),
            interactive(P::Cursor, S::Shim),
        ),
        (
            "shim",
            "cmd.exe",
            Some(vec!["cmd.exe", "/c", "agent"]),
            unresolved(None, U::UnverifiedProvenance),
        ),
        // PowerShell script launch.
        (
            "powershell file",
            "pwsh.exe",
            Some(vec!["pwsh.exe", "-NoProfile", "-File", r"{npm}\codex.ps1"]),
            interactive(P::Codex, S::Shim),
        ),
        // PowerShell command, batch file, and cmd /c resolve only a plain launched command.
        (
            "wrapper",
            "pwsh.exe",
            Some(vec!["pwsh.exe", "-NoLogo", "-Command", "codex --model o3"]),
            interactive(P::Codex, S::LauncherWrapper),
        ),
        (
            "wrapper",
            "powershell.exe",
            Some(vec!["powershell.exe", "-Command", "claude", "-p", "x"]),
            noninteractive(P::Claude, S::LauncherWrapper),
        ),
        (
            "wrapper",
            "cmd.exe",
            Some(vec!["cmd.exe", "/c", r"C:\tools\codex.bat"]),
            interactive(P::Codex, S::Shim),
        ),
        (
            "wrapper",
            "cmd.exe",
            Some(vec!["cmd.exe", "/c", "echo", "codex.exe"]),
            NOT,
        ),
        (
            "wrapper",
            "cmd.exe",
            Some(vec!["cmd.exe", "/c", "codex & del x"]),
            NOT,
        ),
        (
            "wrapper",
            "pwsh.exe",
            Some(vec!["pwsh.exe", "-Command", "codex; Remove-Item x"]),
            NOT,
        ),
        (
            "wrapper",
            "pwsh.exe",
            Some(vec!["pwsh.exe", "-Command", "-"]),
            NOT,
        ),
        // Python console launcher and module launch.
        (
            "python launcher",
            HERMES,
            Some(vec!["hermes"]),
            interactive(P::Hermes, S::NativeImage),
        ),
        (
            "python launcher",
            "python.exe",
            Some(vec!["python.exe", HERMES, "chat"]),
            interactive(P::Hermes, S::PackageEntry),
        ),
        (
            "python module",
            "python3.12.exe",
            Some(vec!["python3.12.exe", "-u", "-m", "hermes_cli"]),
            interactive(P::Hermes, S::PackageEntry),
        ),
        (
            "python module",
            "python.exe",
            Some(vec!["python.exe", "-m", "http.server"]),
            NOT,
        ),
        // Version-manager shim images: unique names by name, shared names need provenance.
        (
            "version manager",
            r"{home}\.volta\bin\codex.exe",
            Some(vec!["codex"]),
            interactive(P::Codex, S::NativeImage),
        ),
        (
            "version manager",
            r"{home}\.volta\bin\amp.exe",
            Some(vec!["amp"]),
            unresolved(Some(P::Amp), U::UnverifiedProvenance),
        ),
        // Unreadable command lines never default to interactive.
        (
            "unreadable",
            r"C:\tools\claude.exe",
            None,
            unresolved(Some(P::Claude), U::CommandLineUnreadable),
        ),
        (
            "unreadable",
            "node.exe",
            None,
            unresolved(None, U::CommandLineUnreadable),
        ),
        ("unreadable", "pwsh.exe", None, NOT),
        ("opaque", "", None, NOT),
        // Noninteractive forms resolve to none; unlisted arguments stay unclassified.
        (
            "noninteractive",
            "claude.exe",
            Some(vec!["claude", "-p", "task"]),
            noninteractive(P::Claude, S::NativeImage),
        ),
        (
            "noninteractive",
            "claude.exe",
            Some(vec!["claude", "--version"]),
            noninteractive(P::Claude, S::NativeImage),
        ),
        (
            "noninteractive",
            "claude.exe",
            Some(vec!["claude", "mcp", "list"]),
            noninteractive(P::Claude, S::NativeImage),
        ),
        (
            "noninteractive",
            "codex.exe",
            Some(vec!["codex", "exec", "task"]),
            noninteractive(P::Codex, S::NativeImage),
        ),
        (
            "noninteractive",
            "codex.exe",
            Some(vec!["codex", "login"]),
            noninteractive(P::Codex, S::NativeImage),
        ),
        (
            "noninteractive",
            "codex.exe",
            Some(vec!["codex", "app-server"]),
            noninteractive(P::Codex, S::NativeImage),
        ),
        (
            "noninteractive",
            "gemini.exe",
            Some(vec!["gemini", "summarize"]),
            noninteractive(P::Gemini, S::NativeImage),
        ),
        (
            "noninteractive",
            r"{cursor}\cursor-agent.exe",
            Some(vec!["cursor-agent", "update"]),
            noninteractive(P::Cursor, S::NativeImage),
        ),
        (
            "noninteractive",
            "opencode.exe",
            Some(vec!["opencode", "serve"]),
            noninteractive(P::OpenCode, S::NativeImage),
        ),
        (
            "noninteractive",
            "letta.exe",
            Some(vec!["letta", "--print=task"]),
            noninteractive(P::Letta, S::NativeImage),
        ),
        (
            "noninteractive",
            "droid.exe",
            Some(vec!["droid", "--help"]),
            noninteractive(P::Droid, S::NativeImage),
        ),
        (
            "interactive form",
            "claude.exe",
            Some(vec!["claude", "--model", "opus", "-c", "explain this"]),
            interactive(P::Claude, S::NativeImage),
        ),
        (
            "interactive form",
            "codex.exe",
            Some(vec!["codex", "-p", "work", "resume", "--last"]),
            interactive(P::Codex, S::NativeImage),
        ),
        (
            "interactive form",
            "gemini.exe",
            Some(vec!["gemini", "-i", "summarize"]),
            interactive(P::Gemini, S::NativeImage),
        ),
        (
            "interactive form",
            "opencode.exe",
            Some(vec!["opencode", r"C:\project"]),
            interactive(P::OpenCode, S::NativeImage),
        ),
        (
            "unclassified",
            "claude.exe",
            Some(vec!["claude", "--unlisted"]),
            unresolved(Some(P::Claude), U::UnclassifiedArguments),
        ),
        (
            "unclassified",
            "claude.exe",
            Some(vec!["claude", "--model"]),
            unresolved(Some(P::Claude), U::UnclassifiedArguments),
        ),
        (
            "unclassified",
            "droid.exe",
            Some(vec!["droid", "task"]),
            unresolved(Some(P::Droid), U::UnclassifiedArguments),
        ),
        // Runtime decoys never borrow names from arguments, including a forged argv[0].
        (
            "decoy",
            "node.exe",
            Some(vec!["node.exe", "-e", "codex.js"]),
            NOT,
        ),
        ("decoy", "node.exe", Some(vec!["node.exe"]), NOT),
        (
            "decoy",
            "node.exe",
            Some(vec!["node.exe", "ordinary.js", "codex"]),
            NOT,
        ),
        (
            "decoy",
            "node.exe",
            Some(vec!["node.exe", "--require", "codex.js", "ordinary.js"]),
            NOT,
        ),
        (
            "decoy",
            "node.exe",
            Some(vec!["codex.exe", "codex.js"]),
            NOT,
        ),
        (
            "decoy",
            "python.exe",
            Some(vec!["python.exe", "-c", "import codex"]),
            NOT,
        ),
        (
            "decoy",
            "bash.exe",
            Some(vec!["bash.exe", "-c", "codex"]),
            NOT,
        ),
        // Remote execution clients are reported separately from Agents.
        (
            "remote",
            r"C:\Windows\System32\OpenSSH\ssh.exe",
            Some(vec!["ssh", "host"]),
            REMOTE,
        ),
        ("remote", "wsl.exe", None, REMOTE),
        (
            "remote",
            "docker.exe",
            Some(vec!["docker", "exec", "-it", "box", "codex"]),
            REMOTE,
        ),
        ("remote", "docker.exe", Some(vec!["docker", "ps"]), NOT),
    ];
    for (row, image, argv, want) in rows {
        let image = f(image);
        let argv: Option<Vec<String>> = argv.map(|a| a.into_iter().map(f).collect());
        let image = (!image.is_empty()).then_some(image.as_str());
        assert_eq!(
            classify_agent_process(image, argv.as_deref()),
            want,
            "{row}: {image:?} {argv:?}"
        );
    }
}

#[test]
fn registry_invariants_hold() {
    let mut unique = Vec::new();
    for (entry, provider) in AGENT_PROVIDER_REGISTRY.iter().zip(P::ALL) {
        assert_eq!(entry.provider, provider);
        assert_eq!(provider.registry_entry().provider, provider);
        assert_eq!(
            P::from_native_provider_id(provider.native_provider_id()),
            Some(provider)
        );
        assert!(!entry.aliases.is_empty(), "{provider:?}");
        for alias in entry.aliases {
            if alias.class == AgentAliasClass::Unique {
                unique.push(alias.name);
            } else {
                assert!(!entry.install_roots.is_empty(), "{provider:?} {alias:?}");
            }
        }
    }
    let count = unique.len();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        count,
        "a unique alias belongs to one provider"
    );
    for entry in &AGENT_PROVIDER_REGISTRY {
        for alias in entry.aliases {
            assert!(
                alias.class == AgentAliasClass::Unique || !unique.contains(&alias.name),
                "{alias:?}"
            );
        }
    }
    assert_eq!(P::from_native_provider_id("claude"), None);
    assert_eq!(P::from_native_provider_id("agent"), None);
}

#[test]
fn result_vocabulary_is_closed_and_round_trips() {
    assert_eq!(
        AgentIdentityUnknownReason::ALL.map(AgentIdentityUnknownReason::as_str),
        [
            "settling",
            "sample-failed",
            "sample-limit",
            "command-line-unreadable",
            "ambiguous-candidates",
            "unclassified-arguments",
            "unverified-provenance",
            "detached-console",
        ]
    );
    for reason in AgentIdentityUnknownReason::ALL {
        assert_eq!(
            AgentIdentityUnknownReason::parse(reason.as_str()),
            Some(reason)
        );
    }
    for reason in AgentIdentityUnsupportedReason::ALL {
        assert_eq!(
            AgentIdentityUnsupportedReason::parse(reason.as_str()),
            Some(reason)
        );
    }
    assert_eq!(AgentIdentityUnknownReason::parse("other"), None);
    assert_eq!(
        [
            U::CommandLineUnreadable,
            U::UnclassifiedArguments,
            U::UnverifiedProvenance
        ]
        .map(|u| u.reason().as_str()),
        [
            "command-line-unreadable",
            "unclassified-arguments",
            "unverified-provenance"
        ]
    );
    let agent = AgentProcessIdentity::Agent {
        provider: P::Codex,
        grade: AgentIdentityGrade::Recognized,
    };
    assert_eq!(
        [
            agent,
            AgentProcessIdentity::None,
            AgentProcessIdentity::Unknown(AgentIdentityUnknownReason::Settling),
            AgentProcessIdentity::Unsupported(AgentIdentityUnsupportedReason::Platform),
        ]
        .map(AgentProcessIdentity::kind),
        ["agent", "none", "unknown", "unsupported"]
    );
    assert_eq!(
        AgentIdentityGrade::ALL.map(AgentIdentityGrade::as_str),
        ["confirmed", "recognized"]
    );
}
