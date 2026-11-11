use super::*;
use crate::AgentReportProvider;
use AgentInstallShape as S;
use AgentReportProvider as P;
use AgentUnresolvedProcess as U;

fn row(pid: u32, parent: Option<u32>, recognition: AgentProcessRecognition) -> AgentProcessRow {
    AgentProcessRow {
        pid,
        parent_pid: parent,
        creation_time: u64::from(pid),
        opaque: false,
        detached: false,
        recognition,
    }
}
fn anchor(pid: u32, provider: P, evidence: AgentAnchorEvidence) -> AgentSampleVerdict {
    AgentSampleVerdict::Anchor(AgentAnchor {
        pid,
        creation_time: u64::from(pid),
        provider,
        evidence,
    })
}
#[test]
fn sample_verdict_requires_one_outer_chain_and_no_unresolved_row_outside_it() {
    use AgentAnchorEvidence as E;
    use AgentIdentityUnknownReason as R;
    let shell = row(1, None, AgentProcessRecognition::NotAgent);
    let codex = row(2, Some(1), interactive(P::Codex, S::NativeImage));
    let claude = row(3, Some(2), interactive(P::Claude, S::NativeImage));
    let ambiguous = AgentSampleVerdict::Unresolved {
        anchor: None,
        reason: R::AmbiguousCandidates,
    };
    let with = |edit: &dyn Fn(&mut Vec<AgentProcessRow>)| {
        let mut rows = vec![shell, codex, claude];
        edit(&mut rows);
        evaluate_agent_sample(&rows, false)
    };
    // The outer Agent of a nested chain is the anchor.
    assert_eq!(with(&|_| {}), anchor(2, P::Codex, E::Interactive));
    // Siblings, a parent created after its child, cycles, and duplicate identifiers.
    assert_eq!(with(&|r| r[2].parent_pid = Some(1)), ambiguous);
    assert_eq!(with(&|r| r[1].creation_time = 4), ambiguous);
    assert_eq!(
        with(&|r| {
            r[1].creation_time = 3;
            r[1].parent_pid = Some(3);
        }),
        ambiguous
    );
    assert_eq!(with(&|r| r.push(r[0])), ambiguous);
    // Unclassified, unreadable, and detached anchors keep their provider and their reason.
    let unclassified = AgentProcessRecognition::Unresolved {
        provider: Some(P::Claude),
        reason: U::UnclassifiedArguments,
    };
    assert_eq!(
        with(&|r| {
            r.truncate(2);
            r[1].recognition = unclassified;
        }),
        anchor(2, P::Claude, E::UnclassifiedArguments)
    );
    assert_eq!(
        with(&|r| r[1].detached = true),
        anchor(2, P::Codex, E::DetachedConsole)
    );
    // Opaque or unresolved rows inside the anchor's subtree never hold an outer Agent.
    assert_eq!(
        with(&|r| r[2].opaque = true),
        anchor(2, P::Codex, E::Interactive)
    );
    // Outside it they leave the sample unresolved, still naming the anchor.
    let outside = AgentSampleVerdict::Unresolved {
        anchor: match anchor(2, P::Codex, E::Interactive) {
            AgentSampleVerdict::Anchor(a) => Some(a),
            _ => None,
        },
        reason: R::CommandLineUnreadable,
    };
    assert_eq!(
        with(&|r| r.push(AgentProcessRow {
            opaque: true,
            ..row(9, Some(1), AgentProcessRecognition::NotAgent)
        })),
        outside
    );
    let unproven = AgentProcessRecognition::Unresolved {
        provider: Some(P::Cursor),
        reason: U::UnverifiedProvenance,
    };
    assert_eq!(
        with(&|r| {
            r.truncate(1);
            r.push(row(4, Some(1), unproven));
        }),
        AgentSampleVerdict::Unresolved {
            anchor: None,
            reason: R::UnverifiedProvenance
        }
    );
    // A full sample may have omitted members.
    assert_eq!(
        evaluate_agent_sample(&[shell], true),
        AgentSampleVerdict::Unresolved {
            anchor: None,
            reason: R::SampleLimit
        }
    );
    // No candidate: none, or remote while a remote client runs.
    assert_eq!(
        evaluate_agent_sample(&[shell], false),
        AgentSampleVerdict::None
    );
    assert_eq!(evaluate_agent_sample(&[], false), AgentSampleVerdict::None);
    assert_eq!(
        evaluate_agent_sample(
            &[
                shell,
                row(5, Some(1), AgentProcessRecognition::RemoteClient)
            ],
            false
        ),
        AgentSampleVerdict::Remote
    );
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
const ID: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";
const OPENCODE_ID: &str = "ses_0123456789abCdEfGhIjKlMnOp";

/// One fixture per Windows support matrix row and proposal counterexample:
/// (row, image, decoded argv or `None` when unreadable, expected recognition).
/// Console attachment and opaque rows need Host facts and belong to the sampler.
#[test]
fn recognition_matrix_rows_and_counterexamples() {
    let f = |s: &str| {
        s.replace("{home}", HOME)
            .replace("{npm}", NPM)
            .replace("{cursor}", CURSOR)
            .replace("{id}", ID)
            .replace("{ses}", OPENCODE_ID)
    };
    let mut rows: Vec<(&str, &str, Option<Vec<&str>>, AgentProcessRecognition)> = vec![
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
        // Each typed resume form is interactive for its provider.
        (
            "resume",
            "claude.exe",
            Some(vec!["claude", "--resume", "{id}"]),
            interactive(P::Claude, S::NativeImage),
        ),
        (
            "resume",
            "claude.exe",
            Some(vec!["claude", "--resume={id}"]),
            interactive(P::Claude, S::NativeImage),
        ),
        (
            "resume",
            "codex.exe",
            Some(vec!["codex", "resume", "{id}"]),
            interactive(P::Codex, S::NativeImage),
        ),
        (
            "resume",
            "grok.exe",
            Some(vec!["grok", "--resume={id}"]),
            interactive(P::Grok, S::NativeImage),
        ),
        (
            "resume",
            "opencode.exe",
            Some(vec!["opencode", "--session={ses}"]),
            interactive(P::OpenCode, S::NativeImage),
        ),
        (
            "resume",
            r"{npm}\node_modules\@kilocode\cli\bin\kilo.exe",
            Some(vec!["kilo", "--session={ses}"]),
            interactive(P::Kilo, S::NativeImage),
        ),
        (
            "resume",
            r"{cursor}\cursor-agent.exe",
            Some(vec!["cursor-agent", "--resume={id}"]),
            interactive(P::Cursor, S::NativeImage),
        ),
        (
            "resume",
            "qodercli.exe",
            Some(vec!["qodercli", "--resume={id}"]),
            interactive(P::Qoder, S::NativeImage),
        ),
        (
            "resume",
            "agy.exe",
            Some(vec!["agy", "--conversation", "{id}"]),
            interactive(P::Antigravity, S::NativeImage),
        ),
        (
            "resume",
            "droid.exe",
            Some(vec!["droid", "--resume={id}"]),
            interactive(P::Droid, S::NativeImage),
        ),
        (
            "resume",
            "kimi.exe",
            Some(vec!["kimi", "--session=session_{id}"]),
            interactive(P::Kimi, S::NativeImage),
        ),
        (
            "resume",
            "letta.exe",
            Some(vec!["letta", "--conversation", "conv-{id}"]),
            interactive(P::Letta, S::NativeImage),
        ),
        (
            "resume",
            "letta.exe",
            Some(vec![
                "letta",
                "--conversation",
                "default",
                "--agent",
                "agent-{id}",
            ]),
            interactive(P::Letta, S::NativeImage),
        ),
        (
            "resume",
            "devin.exe",
            Some(vec!["devin", "--resume", "{id}"]),
            interactive(P::Devin, S::NativeImage),
        ),
        (
            "resume",
            HERMES,
            Some(vec!["hermes", "--resume", "20250101_120000_abcdef"]),
            interactive(P::Hermes, S::NativeImage),
        ),
        (
            "resume",
            "node.exe",
            Some(vec![
                "node.exe",
                r"{npm}\node_modules\@earendil-works\pi-coding-agent\dist\cli.js",
                "--session",
                "{id}",
            ]),
            interactive(P::Pi, S::PackageEntry),
        ),
        (
            "resume",
            "omp.exe",
            Some(vec!["omp", "--resume", "{id}"]),
            interactive(P::Omp, S::NativeImage),
        ),
        (
            "resume",
            "gemini.exe",
            Some(vec!["gemini", "--resume={id}"]),
            interactive(P::Gemini, S::NativeImage),
        ),
        (
            "resume",
            "node.exe",
            Some(vec![
                "node.exe",
                r"{npm}\node_modules\@github\copilot\index.js",
                "--resume={id}",
            ]),
            interactive(P::Copilot, S::PackageEntry),
        ),
        (
            "resume",
            "qwen.exe",
            Some(vec!["qwen", "--resume={id}"]),
            interactive(P::Qwen, S::NativeImage),
        ),
        (
            "resume",
            "cline.exe",
            Some(vec!["cline", "--id", "1730000000000_a1b2c"]),
            interactive(P::Cline, S::NativeImage),
        ),
        // Each launch-time session option Orchbee starts a provider with is interactive for it;
        // OMP, which does not document the option, stays unclassified.
        (
            "launch session",
            "grok.exe",
            Some(vec!["grok", "--session-id", "{id}"]),
            interactive(P::Grok, S::NativeImage),
        ),
        (
            "launch session",
            "grok.exe",
            Some(vec!["grok", "-s", "{id}"]),
            interactive(P::Grok, S::NativeImage),
        ),
        (
            "launch session",
            "gemini.exe",
            Some(vec!["gemini", "--session-id", "{id}"]),
            interactive(P::Gemini, S::NativeImage),
        ),
        (
            "launch session",
            "qwen.exe",
            Some(vec!["qwen", "--session-id", "{id}"]),
            interactive(P::Qwen, S::NativeImage),
        ),
        (
            "launch session",
            "node.exe",
            Some(vec![
                "node.exe",
                r"{npm}\node_modules\@github\copilot\index.js",
                "--session-id",
                "{id}",
            ]),
            interactive(P::Copilot, S::PackageEntry),
        ),
        (
            "launch session",
            "node.exe",
            Some(vec![
                "node.exe",
                r"{npm}\node_modules\@earendil-works\pi-coding-agent\dist\cli.js",
                "--session-id",
                "{id}",
            ]),
            interactive(P::Pi, S::PackageEntry),
        ),
        (
            "launch session counterexample",
            "omp.exe",
            Some(vec!["omp", "--session-id", "{id}"]),
            unresolved(Some(P::Omp), U::UnclassifiedArguments),
        ),
        // Bare launches of the providers that gained a grammar are unchanged.
        (
            "bare",
            "grok.exe",
            Some(vec!["grok"]),
            interactive(P::Grok, S::NativeImage),
        ),
        (
            "bare",
            "qodercli.exe",
            Some(vec!["qodercli"]),
            interactive(P::Qoder, S::NativeImage),
        ),
        (
            "bare",
            "kimi.exe",
            Some(vec!["kimi"]),
            interactive(P::Kimi, S::NativeImage),
        ),
        (
            "bare",
            "agy.exe",
            Some(vec!["agy"]),
            interactive(P::Antigravity, S::NativeImage),
        ),
        (
            "bare",
            "devin.exe",
            Some(vec!["devin"]),
            interactive(P::Devin, S::NativeImage),
        ),
        (
            "bare",
            "cline.exe",
            Some(vec!["cline"]),
            interactive(P::Cline, S::NativeImage),
        ),
        // A bare optional option never consumes a positional; `exec` keeps its own `-r`.
        (
            "resume counterexample",
            "node.exe",
            Some(vec![
                "node.exe",
                r"{npm}\node_modules\@github\copilot\index.js",
                "--resume",
                "foo",
            ]),
            unresolved(Some(P::Copilot), U::UnclassifiedArguments),
        ),
        (
            "resume counterexample",
            "droid.exe",
            Some(vec!["droid", "exec", "-r", "high", "task"]),
            noninteractive(P::Droid, S::NativeImage),
        ),
        // Qwen's entry re-executes `cli.js` with `--expose-gc`; the outer `cli-entry` anchors.
        (
            "qwen launcher",
            "node.exe",
            Some(vec![
                "node.exe",
                r"{npm}\node_modules\@qwen-code\qwen-code\cli-entry.js",
                "--resume={id}",
            ]),
            interactive(P::Qwen, S::PackageEntry),
        ),
        (
            "qwen launcher",
            "node.exe",
            Some(vec![
                "node.exe",
                "--expose-gc",
                r"{npm}\node_modules\@qwen-code\qwen-code\cli.js",
                "--resume={id}",
            ]),
            interactive(P::Qwen, S::PackageEntry),
        ),
        (
            "decoy",
            "node.exe",
            Some(vec!["node.exe", "--expose-gc", "ordinary.js"]),
            NOT,
        ),
        // Native images that npm launchers, uv tools and installers run.
        (
            "native",
            r"{npm}\node_modules\@atomgit.com\atomcode\node_modules\@atomgit.com\atomcode-win32-x64\bin\atomcode.exe",
            Some(vec!["atomcode"]),
            interactive(P::AtomCode, S::NativeImage),
        ),
        (
            "native",
            r"{npm}\node_modules\@deveco\deveco-code\bin\deveco.exe",
            Some(vec!["deveco"]),
            interactive(P::DevEco, S::NativeImage),
        ),
        (
            "native",
            r"{npm}\node_modules\reasonix\node_modules\@reasonix\cli-win32-x64\bin\reasonix.exe",
            Some(vec!["reasonix"]),
            interactive(P::Reasonix, S::NativeImage),
        ),
        (
            "native",
            r"{npm}\node_modules\mothx-installer\node_modules\mothx-installer-win32-x64\bin\mothx.exe",
            Some(vec!["mothx"]),
            interactive(P::Mothx, S::NativeImage),
        ),
        (
            "native",
            r"{home}\.local\bin\jcode.exe",
            Some(vec!["jcode"]),
            interactive(P::Jcode, S::NativeImage),
        ),
        (
            "native",
            r"{home}\.local\share\prime-agent\prime-agent.exe",
            Some(vec!["prime-agent"]),
            interactive(P::PrimeAgent, S::NativeImage),
        ),
        (
            "native",
            r"{home}\.local\bin\cmdc.exe",
            Some(vec!["cmdc"]),
            interactive(P::CommandCode, S::NativeImage),
        ),
        // Command Code's `cmd` command is the system command processor's name, never Command Code.
        (
            "command processor",
            r"{home}\AppData\Local\mise\shims\cmd.exe",
            Some(vec!["cmd"]),
            NOT,
        ),
        // npm launchers at their package entries.
        (
            "package entry",
            "node.exe",
            Some(vec![
                "node.exe",
                r"{npm}\node_modules\@tencent-ai\codebuddy-code\bin\codebuddy",
            ]),
            interactive(P::CodeBuddy, S::PackageEntry),
        ),
        (
            "package entry",
            "node.exe",
            Some(vec![
                "node.exe",
                r"{npm}\node_modules\command-code\dist\index.mjs",
            ]),
            interactive(P::CommandCode, S::PackageEntry),
        ),
        (
            "package entry",
            "node.exe",
            Some(vec!["node.exe", r"{npm}\node_modules\@mimo-ai\cli\mimo.js"]),
            interactive(P::Mimo, S::PackageEntry),
        ),
        (
            "package entry",
            "node.exe",
            Some(vec![
                "node.exe",
                r"{npm}\node_modules\snow-ai\bundle\cli.mjs",
            ]),
            interactive(P::Snow, S::PackageEntry),
        ),
        (
            "package entry",
            "node.exe",
            Some(vec![
                "node.exe",
                r"{npm}\node_modules\@minimax-ai\code\cli.js",
            ]),
            interactive(P::MinimaxCode, S::PackageEntry),
        ),
        // Short shared and generic names need their install root.
        (
            "shared alias",
            r"{npm}\node_modules\@mimo-ai\cli\vendor\mimo.exe",
            Some(vec!["mimo"]),
            interactive(P::Mimo, S::NativeImage),
        ),
        (
            "shared alias",
            r"C:\tools\mimo.exe",
            Some(vec!["mimo"]),
            unresolved(Some(P::Mimo), U::UnverifiedProvenance),
        ),
        (
            "shared alias",
            r"{home}\AppData\Local\mise\shims\mcode.exe",
            Some(vec!["mcode"]),
            unresolved(Some(P::MinimaxCode), U::UnverifiedProvenance),
        ),
        (
            "shared alias",
            r"C:\coin-or\bin\cbc.exe",
            Some(vec!["cbc"]),
            unresolved(Some(P::CodeBuddy), U::UnverifiedProvenance),
        ),
        (
            "generic name",
            r"C:\tools\snow.exe",
            Some(vec!["snow"]),
            unresolved(Some(P::Snow), U::UnverifiedProvenance),
        ),
        (
            "generic name",
            "node.exe",
            Some(vec!["node.exe", r"C:\tools\penguin.js"]),
            unresolved(Some(P::Penguin), U::UnverifiedProvenance),
        ),
        // Kiro Crew is not Kiro CLI.
        (
            "distinct product",
            r"{home}\.local\bin\kirocrew.exe",
            Some(vec!["kirocrew", "chat"]),
            interactive(P::KiroCrew, S::NativeImage),
        ),
        // Agents whose chat is a subcommand: only that subcommand is interactive.
        (
            "required command",
            r"{home}\.local\bin\kirocrew.exe",
            Some(vec!["kirocrew"]),
            unresolved(Some(P::KiroCrew), U::UnclassifiedArguments),
        ),
        (
            "required command",
            r"{home}\.local\bin\qwenpaw.exe",
            Some(vec!["qwenpaw", "tui"]),
            interactive(P::QwenPaw, S::NativeImage),
        ),
        (
            "required command",
            r"{home}\.local\bin\qwenpaw.exe",
            Some(vec!["qwenpaw"]),
            unresolved(Some(P::QwenPaw), U::UnclassifiedArguments),
        ),
        (
            "required command",
            r"{home}\.local\bin\qwenpaw.exe",
            Some(vec!["qwenpaw", "task", "x"]),
            noninteractive(P::QwenPaw, S::NativeImage),
        ),
        (
            "required command",
            "node.exe",
            Some(vec![
                "node.exe",
                r"{home}\AppData\Local\Programs\OrchbeeAgentCLIs\node24\node_modules\openclaw\openclaw.mjs",
                "chat",
            ]),
            interactive(P::OpenClaw, S::PackageEntry),
        ),
        (
            "required command",
            "openclaw.exe",
            Some(vec!["openclaw", "--local"]),
            unresolved(Some(P::OpenClaw), U::UnclassifiedArguments),
        ),
        (
            "required command",
            "node.exe",
            Some(vec![
                "node.exe",
                r"{npm}\node_modules\@prismshadow\penguin-cli\dist\penguin.js",
                "chat",
                "--resume",
            ]),
            interactive(P::Penguin, S::PackageEntry),
        ),
        (
            "required command",
            "node.exe",
            Some(vec![
                "node.exe",
                r"{npm}\node_modules\@prismshadow\penguin-cli\dist\penguin.js",
                "run",
                "task",
            ]),
            noninteractive(P::Penguin, S::PackageEntry),
        ),
    ];
    // A bare optional option followed by a noninteractive option stays noninteractive.
    for (image, program, option, provider) in [
        ("claude.exe", "claude", "-r", P::Claude),
        ("claude.exe", "claude", "--resume", P::Claude),
        ("droid.exe", "droid", "--resume", P::Droid),
        ("grok.exe", "grok", "-r", P::Grok),
        ("grok.exe", "grok", "--resume", P::Grok),
        ("qodercli.exe", "qodercli", "-r", P::Qoder),
        ("qodercli.exe", "qodercli", "--resume", P::Qoder),
        ("kimi.exe", "kimi", "-S", P::Kimi),
        ("kimi.exe", "kimi", "--session", P::Kimi),
    ] {
        rows.push((
            "optional option",
            image,
            Some(vec![program, option, "-v"]),
            noninteractive(provider, S::NativeImage),
        ));
    }
    for option in ["-r", "--resume"] {
        rows.push((
            "optional option",
            "node.exe",
            Some(vec![
                "node.exe",
                r"{npm}\node_modules\@github\copilot\index.js",
                option,
                "-v",
            ]),
            noninteractive(P::Copilot, S::PackageEntry),
        ));
    }
    for (row, image, argv, want) in rows {
        let image = f(image);
        let argv: Option<Vec<String>> = argv.map(|a| a.into_iter().map(f).collect());
        let image = (!image.is_empty()).then_some(image.as_str());
        assert_eq!(
            classify_agent_process(image, argv.as_deref()),
            want,
            "{row}: {image:?} {argv:?}"
        );
        // The arguments a command-line reader gets are those of an interactive row only, and
        // they are the row's own trailing arguments, which its provider's grammar classified.
        let arguments = crate::interactive_agent_arguments(image, argv.as_deref());
        assert_eq!(
            arguments.as_ref().map(|(provider, _)| *provider),
            want.interactive_provider(),
            "{row}: {image:?} {argv:?}"
        );
        if let (Some((provider, arguments)), Some(argv)) = (arguments, &argv) {
            assert!(
                argv.join(" ").ends_with(&arguments.join(" "))
                    && provider.classify_invocation(&arguments) == AgentInvocationMode::Interactive,
                "{row}: {argv:?} gave {arguments:?}"
            );
        }
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

#[test]
fn identity_wire_form_is_closed_and_round_trips_every_reason() {
    use serde_json::{from_value, json, to_value};
    let mut cases = vec![
        (
            AgentProcessIdentity::Agent {
                provider: P::Claude,
                grade: AgentIdentityGrade::Confirmed,
            },
            json!({"result":"agent","providerId":"claude-code","grade":"confirmed"}),
        ),
        (
            AgentProcessIdentity::Agent {
                provider: P::Codex,
                grade: AgentIdentityGrade::Recognized,
            },
            json!({"result":"agent","providerId":"codex","grade":"recognized"}),
        ),
        (AgentProcessIdentity::None, json!({"result":"none"})),
    ];
    cases.extend(AgentIdentityUnknownReason::ALL.map(|reason| {
        (
            AgentProcessIdentity::Unknown(reason),
            json!({"result":"unknown","reason":reason.as_str()}),
        )
    }));
    cases.extend(AgentIdentityUnsupportedReason::ALL.map(|reason| {
        (
            AgentProcessIdentity::Unsupported(reason),
            json!({"result":"unsupported","reason":reason.as_str()}),
        )
    }));
    for (identity, wire) in cases {
        assert_eq!(to_value(identity).unwrap(), wire);
        assert_eq!(from_value::<AgentProcessIdentity>(wire).unwrap(), identity);
    }
    for invalid in [
        json!({"result":"agent","providerId":"claude","grade":"confirmed"}),
        json!({"result":"agent","providerId":"codex"}),
        json!({"result":"none","reason":"settling"}),
        json!({"result":"unknown","reason":"other"}),
        json!({"result":"unknown"}),
        json!({"result":"unsupported","reason":"settling"}),
        json!({"kind":"unsupported"}),
    ] {
        assert!(from_value::<AgentProcessIdentity>(invalid).is_err());
    }
}
