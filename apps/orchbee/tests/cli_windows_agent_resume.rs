//! Agent session resume across real managed restarts for every provider in the resume table: the
//! CLI, Server, Worker and Zig Host under test with the pinned ConPTY, and a default Session whose
//! default program is cmd. Each table row has its own Workspace and work directory, where every
//! provider command is an npm-style `.cmd` shim to a Node script at a path process identity
//! recognises as that provider: a package entry, or an alias-named script, under the provider's
//! install root where the alias is shared. No native stub binary exists.
//!
//! A stub takes a fresh session id in its provider's shape, or the id its resume arguments carry,
//! and reports that session the way the provider's adapter does: a command-hook payload through
//! the shipped `forward-hook.mjs`, or the shipped module driven as the provider drives it (OpenCode
//! and Kilo plugin events with one child session, Cline's plugin setup and a run, Pi and OMP
//! extension events). A row whose provider stays silent after a resume reports nothing then. Every
//! run records its arguments, directory and session in the fixture root and stays alive like an
//! interactive Agent until it reads `q`. Every wait reads the Agent list, as a Client showing it
//! does: a hook report joins its Panel only when a Client's Agent read drains it within 10 s.
//!
//! The table restates the Server's argument rule as typed text, so the screen and the recorded
//! arguments are the only oracle; nothing imports Server internals. The flow captures every row,
//! resumes each once after a restart with its exact cmd line, captures again, which for silent rows
//! is the delivered reference carried forward, and resumes each again. One silent row's Agent then
//! exits, so its reference drops, and a last restart with the preference off types nothing. A Panel
//! whose directory was deleted, or that a Client typed into first, receives nothing.
//!
//! A restored terminal carries no launch environment: the Host projects `PATH` from the registry,
//! where a real provider may be installed. cmd resolves a command from its current directory
//! first, a resume is typed only in the retained directory, and every work directory holds every
//! provider's shim, so the line always reaches a stub. PowerShell resolves only through `PATH`, so
//! it is never this witness's shell. The hook's correlation reaches the Host as a private APC, which
//! only the pinned ConPTY carries, so the witness needs `ORCHBEE_TEST_CONPTY_DIR` naming a
//! manifest-verified bundle and runs only when asked for.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

use orchbee_local_runtime::{UserPreferencesScope, UserPreferencesStore};
use orchbee_workbench_protocol::{
    PreferenceKey, UserPreferencesPatch, UserPreferencesRequest, UserPreferencesResponse,
    UserPreferencesSnapshot,
};
use serde_json::{Value, json};
use session::{Control, Fixture, system_program};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

/// Collective waits scale with the table: 30 s plus 1 s per row bounds every prompt, start,
/// listing and round of resumes.
const OBSERVED: Duration = Duration::from_secs(30 + TABLE.len() as u64);
/// Well past the settle, readiness reads and spacing of any resume, so nothing typed by then
/// means nothing will be.
const QUIET: Duration = Duration::from_secs(6);
/// Controller and creation geometry, wide enough that no typed line in the long temporary path
/// wraps.
const COLUMNS: u16 = 200;
const LINES: u16 = 30;
/// The launch profile that makes cmd the default program before any terminal starts.
const CMD_PROFILE: &str = "cmd";
/// The silent row whose Agent exits before the last capture.
const EXITING: &str = "cursor";

/// One resume table row as its stub plays it.
struct Row {
    /// Workspace label and work directory name.
    label: &'static str,
    /// The checkpoint's provider id, which is also the Agent list's provider.
    provider: &'static str,
    /// The command a user types; the Server types it too.
    program: &'static str,
    /// The stub script under the row's stub root, at a path identity recognises.
    entry: &'static str,
    /// A sibling script the entry runs as `node --expose-gc <relay> …`, which then holds the stub.
    relay: Option<&'static str>,
    /// A JavaScript expression giving a fresh id in the provider's shape.
    fresh: &'static str,
    /// The typed resume arguments, `{id}` standing for the id.
    resume: &'static [&'static str],
    /// The session key the adapter reports and the checkpoint retains, `{id}` standing for the id.
    key: &'static str,
    /// A JavaScript `async (session, id, resumed)` function reporting the session as the
    /// provider's adapter does.
    report: &'static str,
    /// Whether a resumed run reports again; unknown is false, the stricter path.
    reports_on_resume: bool,
}

const UUID: &str = "crypto.randomUUID()";
const OPENCODE_ID: &str = "`ses_${hex(12)}${alnum(14)}`";

/// The resume table: every resumable provider, with Letta's two forms.
const TABLE: [Row; 20] = [
    Row {
        label: "claude-code",
        provider: "claude-code",
        program: "claude",
        entry: "node_modules/@anthropic-ai/claude-code/cli.js",
        relay: None,
        fresh: UUID,
        resume: &["--resume={id}"],
        key: "{id}",
        report: "async (session, _id, resumed) => hook('claude-code', { session_id: session, \
                 hook_event_name: 'SessionStart', source: resumed ? 'resume' : 'startup', \
                 cwd: process.cwd() })",
        reports_on_resume: true,
    },
    Row {
        label: "codex",
        provider: "codex",
        program: "codex",
        entry: "node_modules/@openai/codex/bin/codex.js",
        relay: None,
        fresh: UUID,
        resume: &["resume", "{id}"],
        key: "{id}",
        report: "async (session, _id, resumed) => hook('codex', { session_id: session, \
                 hook_event_name: 'SessionStart', source: resumed ? 'resume' : 'startup', \
                 cwd: process.cwd() })",
        reports_on_resume: true,
    },
    Row {
        label: "grok",
        provider: "grok",
        program: "grok",
        entry: "grok.js",
        relay: None,
        fresh: UUID,
        resume: &["--resume={id}"],
        key: "{id}",
        report: "async (session) => hook('grok', { sessionId: session, hookEventName: 'SessionStart' })",
        reports_on_resume: true,
    },
    Row {
        label: "opencode",
        provider: "opencode",
        program: "opencode",
        entry: "opencode.js",
        relay: None,
        fresh: OPENCODE_ID,
        resume: &["--session={id}"],
        key: "{id}",
        report: "async (session) => {
            const hooks = await (await bundle('opencode/orchbee-agent-observation.mjs')).OrchbeeAgentObservation({});
            await hooks.event({ event: { type: 'session.created', properties: { info: { id: session } } } });
            await hooks.event({ event: { type: 'session.created', properties: { info: { id: `ses_${hex(12)}${alnum(14)}`, parentID: session } } } });
        }",
        reports_on_resume: false,
    },
    Row {
        label: "kilo",
        provider: "kilo",
        program: "kilo",
        entry: "node_modules/@kilocode/cli/kilo.js",
        relay: None,
        fresh: OPENCODE_ID,
        resume: &["--session={id}"],
        key: "{id}",
        report: "async (session) => {
            const hooks = await (await bundle('kilo/orchbee-agent-observation.mjs')).OrchbeeAgentObservation({});
            await hooks.event({ event: { type: 'session.created', properties: { info: { id: session } } } });
            await hooks.event({ event: { type: 'session.created', properties: { info: { id: `ses_${hex(12)}${alnum(14)}`, parentID: session } } } });
        }",
        reports_on_resume: false,
    },
    Row {
        label: "cursor",
        provider: "cursor",
        program: "cursor-agent",
        entry: "cursor-agent.js",
        relay: None,
        fresh: UUID,
        resume: &["--resume={id}"],
        key: "{id}",
        report: "async (session) => hook('cursor', { hook_event_name: 'sessionStart', conversation_id: session })",
        reports_on_resume: false,
    },
    Row {
        label: "qodercli",
        provider: "qodercli",
        program: "qodercli",
        entry: "qodercli.js",
        relay: None,
        fresh: UUID,
        resume: &["--resume={id}"],
        key: "{id}",
        report: "async (session) => hook('qodercli', { hook_event_name: 'SessionStart', session_id: session })",
        reports_on_resume: true,
    },
    Row {
        label: "antigravity",
        provider: "antigravity",
        program: "agy",
        entry: "agy.js",
        relay: None,
        fresh: UUID,
        resume: &["--conversation", "{id}"],
        key: "{id}",
        report: "async (session) => hook('antigravity-cli', { hook_event_name: 'PreInvocation', conversationId: session })",
        reports_on_resume: false,
    },
    Row {
        label: "droid",
        provider: "droid",
        program: "droid",
        entry: "droid.js",
        relay: None,
        fresh: UUID,
        resume: &["--resume={id}"],
        key: "{id}",
        report: "async (session) => hook('droid', { hook_event_name: 'SessionStart', session_id: session })",
        reports_on_resume: true,
    },
    Row {
        label: "kimi",
        provider: "kimi",
        program: "kimi",
        entry: "node_modules/@moonshot-ai/kimi-code/dist/main.mjs",
        relay: None,
        fresh: "`session_${crypto.randomUUID()}`",
        resume: &["--session={id}"],
        key: "{id}",
        report: "async (session) => hook('kimi', { hook_event_name: 'SessionStart', session_id: session })",
        reports_on_resume: true,
    },
    Row {
        label: "letta-conversation",
        provider: "letta",
        program: "letta",
        entry: "node_modules/@letta-ai/letta-code/letta.js",
        relay: None,
        fresh: "`conv-${crypto.randomUUID()}`",
        resume: &["--conversation", "{id}"],
        key: "{id}",
        report: "async (session, _id, resumed) => hook('letta', { event_type: 'SessionStart', \
                 is_new_session: !resumed, conversation_id: session })",
        reports_on_resume: true,
    },
    Row {
        label: "letta-default",
        provider: "letta",
        program: "letta",
        entry: "node_modules/@letta-ai/letta-code/letta.js",
        relay: None,
        fresh: "`agent-${crypto.randomUUID()}`",
        resume: &["--conversation", "default", "--agent", "{id}"],
        key: "default:{id}",
        report: "async (_session, id, resumed) => hook('letta', { event_type: 'SessionStart', \
                 is_new_session: !resumed, conversation_id: 'default', agent_id: id })",
        reports_on_resume: true,
    },
    Row {
        label: "devin",
        provider: "devin",
        program: "devin",
        entry: "devin.js",
        relay: None,
        fresh: "`devin-${hex(32)}`",
        resume: &["--resume", "{id}"],
        key: "{id}",
        report: "async (session) => hook('devin', { hook_event_name: 'SessionStart', session_id: session })",
        reports_on_resume: false,
    },
    Row {
        label: "hermes",
        provider: "hermes",
        program: "hermes",
        entry: "hermes-agent/hermes.js",
        relay: None,
        fresh: "(() => { const t = new Date().toISOString().replace(/[-:]/g, ''); \
                return `${t.slice(0, 8)}_${t.slice(9, 15)}_${hex(8)}`; })()",
        resume: &["--resume", "{id}"],
        key: "{id}",
        report: "async (session) => hook('hermes', { event: 'on_session_start', platform: 'cli', session_id: session })",
        reports_on_resume: true,
    },
    Row {
        label: "pi",
        provider: "pi",
        program: "pi",
        entry: "node_modules/@earendil-works/pi-coding-agent/dist/cli.js",
        relay: None,
        fresh: UUID,
        resume: &["--session", "{id}"],
        key: "{id}",
        report: "async (session) => {
            const handlers = new Map();
            await (await bundle('pi/orchbee-agent-observation.cjs')).default({ on: (name, handler) => handlers.set(name, handler) });
            handlers.get('session_start')({}, { sessionManager: { getSessionId: () => session } });
        }",
        reports_on_resume: true,
    },
    Row {
        label: "omp",
        provider: "omp",
        program: "omp",
        entry: "omp.js",
        relay: None,
        fresh: UUID,
        resume: &["--resume", "{id}"],
        key: "{id}",
        report: "async (session) => {
            const handlers = new Map();
            await (await bundle('omp/orchbee-agent-observation.cjs')).default({ on: (name, handler) => handlers.set(name, handler) });
            handlers.get('session_start')({}, { sessionManager: { getSessionId: () => session } });
        }",
        reports_on_resume: true,
    },
    Row {
        label: "gemini",
        provider: "gemini",
        program: "gemini",
        entry: "node_modules/@google/gemini-cli/dist/index.js",
        relay: None,
        fresh: UUID,
        resume: &["--resume={id}"],
        key: "{id}",
        report: "async (session) => hook('gemini', { hook_event_name: 'SessionStart', session_id: session })",
        reports_on_resume: true,
    },
    Row {
        label: "github-copilot",
        provider: "github-copilot",
        program: "copilot",
        entry: "node_modules/@github/copilot/index.js",
        relay: None,
        fresh: UUID,
        resume: &["--resume={id}"],
        key: "{id}",
        report: "async (session) => hook('github-copilot', { hook_event_name: 'SessionStart', session_id: session })",
        reports_on_resume: false,
    },
    Row {
        label: "qwen-code",
        provider: "qwen-code",
        program: "qwen",
        entry: "node_modules/@qwen-code/qwen-code/cli-entry.js",
        relay: Some("cli.js"),
        fresh: UUID,
        resume: &["--resume={id}"],
        key: "{id}",
        report: "async (session, _id, resumed) => hook('qwen-code', { hook_event_name: 'SessionStart', \
                 session_id: session, source: resumed ? 'resume' : 'startup' })",
        reports_on_resume: true,
    },
    Row {
        label: "cline",
        provider: "cline",
        program: "cline",
        entry: "cline.js",
        relay: None,
        fresh: "`${Date.now()}_${pick('0123456789abcdefghijklmnopqrstuvwxyz', 5)}`",
        resume: &["--id", "{id}"],
        key: "{id}",
        report: "async (session) => {
            const { plugin } = await bundle('cline/orchbee-agent-observation.js');
            plugin.setup({}, { session: { sessionId: session } });
            plugin.hooks.onEvent({ type: 'run-started', snapshot: { conversationId: `conv_${hex(8)}`, runId: `run_${hex(8)}` } });
        }",
        reports_on_resume: false,
    },
];

/// A fresh run takes a new id and a resumed one keeps its own. A command hook runs as the provider
/// runs one: a child with the payload on standard input, inheriting the environment and console. A
/// module adapter runs in the stub's own process, as in the provider's. Written as one async
/// function with dynamic imports, so the same source runs as CommonJS and as a module.
const STUB: &str = r"
(async () => {
  const { spawn } = await import('node:child_process');
  const crypto = await import('node:crypto');
  const { appendFileSync } = await import('node:fs');
  const path = await import('node:path');
  const { pathToFileURL } = await import('node:url');
  const argv = process.argv.slice(2);
  const bundle = (relative) => import(pathToFileURL(path.join(BUNDLE, relative)).href);
  const hook = (adapter, payload) => new Promise((resolve) => {
    const child = spawn(process.execPath, [path.join(BUNDLE, 'scripts', 'forward-hook.mjs'), adapter], { stdio: ['pipe', 'ignore', 'ignore'] });
    child.on('error', resolve);
    child.on('exit', resolve);
    child.stdin.end(JSON.stringify(payload));
  });
  const pick = (alphabet, length) => Array.from(crypto.randomBytes(length), (byte) => alphabet[byte % alphabet.length]).join('');
  const hex = (length) => pick('0123456789abcdef', length);
  const alnum = (length) => pick('0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz', length);
  const fresh = () => (/*FRESH*/);
  const report = /*REPORT*/;
  const resumed = (() => {
    if (argv.length !== RESUME.length) return null;
    let id = null;
    for (const [index, pattern] of RESUME.entries()) {
      const token = argv[index];
      const [prefix, suffix] = pattern.split('{id}');
      if (suffix === undefined) {
        if (token !== pattern) return null;
        continue;
      }
      if (!token.startsWith(prefix) || !token.endsWith(suffix) || token.length <= prefix.length + suffix.length) return null;
      id = token.slice(prefix.length, token.length - suffix.length);
    }
    return id;
  })();
  const id = resumed ?? (argv.length === 0 ? fresh() : null);
  const session = id === null ? null : KEY.replace('{id}', id);
  const reported = session !== null && (resumed === null || REPORTS_ON_RESUME);
  if (reported) await report(session, id, resumed !== null);
  appendFileSync(CALLS, JSON.stringify({ label: LABEL, script: process.argv[1], argv, cwd: process.cwd(), id, session, reported }) + '\n');
  process.stdout.write(`${LABEL} stub running\n`);
  process.stdin.on('data', (data) => { if (String(data).trim() === 'q') process.exit(0); });
  setInterval(() => {}, 1 << 30);
})();
";

/// Qwen's package entry relaunches its CLI under the same runtime with `--expose-gc`.
const RELAY: &str = r"
(async () => {
  const { spawn } = await import('node:child_process');
  const path = await import('node:path');
  const target = path.join(path.dirname(process.argv[1]), RELAY);
  const child = spawn(process.execPath, ['--expose-gc', target, ...process.argv.slice(2)], { stdio: 'inherit' });
  child.on('exit', (code) => process.exit(code ?? 1));
})();
";

/// One Panel running one row's stub, with the session its fresh run took.
struct Instance {
    row: &'static Row,
    label: &'static str,
    panel: String,
    id: String,
    session: String,
}

impl Instance {
    fn directory(&self) -> String {
        format!("work/{}", self.label)
    }

    /// The tokens the Server types after the program.
    fn tokens(&self) -> Vec<String> {
        self.row
            .resume
            .iter()
            .map(|token| token.replace("{id}", &self.id))
            .collect()
    }

    fn retained(&self) -> (String, String) {
        (self.row.provider.to_owned(), self.session.clone())
    }
}

/// What the Agent list must show for a Panel before a capture.
#[derive(Clone, Copy)]
enum Listed {
    /// Seen through its hook and in its terminal's processes.
    Both,
    /// Seen only in its terminal's processes, with no hook report.
    Process,
    /// Not seen in its terminal's processes.
    Gone,
}

#[test]
#[ignore = "requires ORCHBEE_TEST_CONPTY_DIR: only the pinned ConPTY carries the hook's correlation; run explicitly"]
fn every_retained_agent_resumes_once_per_restart_in_its_own_shell_and_never_in_a_moved_or_touched_one()
 {
    let labels: Vec<&'static str> = TABLE
        .iter()
        .map(|row| row.label)
        .chain(["gone", "touched"])
        .collect();
    let witness = Witness::new("orchbee-agent-resume-", &labels);
    let pending: Vec<(&'static Row, &'static str, String)> = labels
        .iter()
        .map(|&label| (Witness::row(label), label, witness.workspace(label)))
        .collect();
    let instances = witness.start_all(pending);
    let (rows, extra) = instances.split_at(TABLE.len());
    let [gone, touched] = extra else {
        unreachable!()
    };
    let resumable: Vec<&Instance> = rows.iter().collect();
    witness.until_listed(
        &instances
            .iter()
            .map(|i| (i, Listed::Both))
            .collect::<Vec<_>>(),
    );

    witness.stop();
    assert_eq!(
        witness.retained(),
        sorted(instances.iter()),
        "capture retains each live stub's own reference"
    );
    // A Panel whose directory is gone restores in home and is never resumed.
    witness.remove(&gone.directory());
    witness.start_session();
    // One Client opens the gate by taking control of a Panel and types into it within the settle;
    // it then views another Panel. The rest stay unviewed, at their restored geometry.
    let mut touching = witness.control(&touched.panel);
    touching.input(b"x");
    let viewing = witness.control(&rows[0].panel);
    witness.until_resumed(&resumable, 1);
    witness.quiet();
    witness.assert_runs(&resumable, 1);
    witness.assert_runs(&[gone, touched], 0);
    let terminals = witness.terminals();
    for instance in rows {
        let typed = witness.screen(&terminals[&instance.panel]);
        let line = format!("{} {}", instance.row.program, instance.tokens().join(" "));
        assert!(
            typed.contains(&line),
            "{}: cmd did not receive `{line}`:\n{typed}",
            instance.label
        );
    }
    let touched_screen = witness.screen(&terminals[&touched.panel]);
    assert!(
        !touched_screen.contains(&touched.id),
        "a touched Panel received resume text:\n{touched_screen}"
    );
    let home = witness.directory("home").to_lowercase();
    let moved = witness.screen(&terminals[&gone.panel]);
    assert!(
        moved.to_lowercase().contains(&format!("{home}>")) && !moved.contains(&gone.id),
        "a Panel without its directory did not restore as a plain shell in home:\n{moved}"
    );
    touching.release();
    viewing.release();

    // A row that reports again is captured from its report; a silent row is captured from the
    // delivered reference bound to the run it started. Resuming again keeps working.
    witness.until_listed(&listed_after_resume(rows, None));
    witness.stop();
    assert_eq!(witness.retained(), sorted(rows.iter()));
    witness.start_session();
    let viewing = witness.control(&rows[0].panel);
    witness.until_resumed(&resumable, 2);
    witness.quiet();
    witness.assert_runs(&resumable, 2);
    witness.assert_runs(&[gone, touched], 0);
    viewing.release();

    // A silent Agent that exits takes its carried reference with it.
    let exiting = rows
        .iter()
        .find(|instance| instance.label == EXITING)
        .unwrap();
    assert!(!exiting.row.reports_on_resume);
    witness.send(&witness.terminals()[&exiting.panel], "q");
    witness.until_listed(&listed_after_resume(rows, Some(exiting)));
    witness.stop();
    assert_eq!(
        witness.retained(),
        sorted(rows.iter().filter(|instance| instance.label != EXITING))
    );

    // With the preference off at startup, the retained references are discarded unresumed, and
    // the next capture records none.
    witness.set_preference(PreferenceKey::ResumeAgentsOnRestore, json!(false));
    witness.start_session();
    let runs = witness.calls().len();
    let viewing = witness.control(&rows[0].panel);
    witness.quiet();
    assert_eq!(
        witness.calls().len(),
        runs,
        "a run started with the preference off: {:#?}",
        witness.calls()
    );
    witness.assert_runs(&resumable, 2);
    let terminals = witness.terminals();
    for instance in rows {
        let screen = witness.screen(&terminals[&instance.panel]);
        assert!(
            !screen.contains(&instance.id),
            "{}: resume text typed with the preference off:\n{screen}",
            instance.label
        );
    }
    viewing.release();
    witness.stop();
    assert_eq!(witness.retained(), Vec::<(String, String)>::new());
    witness.assert_ids_only_in_store(&instances);
    witness.start_session();
}

/// After a resume, a reporting row is seen through its hook and its processes; a silent row only
/// in its processes; `exited` in neither.
fn listed_after_resume<'a>(
    rows: &'a [Instance],
    exited: Option<&Instance>,
) -> Vec<(&'a Instance, Listed)> {
    rows.iter()
        .map(|instance| {
            let listed = if exited.is_some_and(|exited| exited.label == instance.label) {
                Listed::Gone
            } else if instance.row.reports_on_resume {
                Listed::Both
            } else {
                Listed::Process
            };
            (instance, listed)
        })
        .collect()
}

/// One real local runtime whose default program is cmd, with every provider's stub in each of its
/// work directories.
struct Witness {
    fixture: Fixture,
    node: PathBuf,
}

impl Witness {
    fn new(prefix: &'static str, labels: &[&'static str]) -> Self {
        assert!(
            std::env::var_os("ORCHBEE_TEST_CONPTY_DIR").is_some(),
            "set ORCHBEE_TEST_CONPTY_DIR to a ConPTY bundle verified against \
             apps/terminal-host/windows-conpty.json"
        );
        // With this registry value cmd skips its current directory and could reach a real
        // provider on the registry `PATH` a restored terminal receives.
        for key in [
            r"HKCU\Environment",
            r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
        ] {
            let query = Command::new(system_program("reg.exe"))
                .args(["query", key, "/v", "NoDefaultCurrentDirectoryInExePath"])
                .output()
                .unwrap();
            assert!(
                !query.status.success(),
                "NoDefaultCurrentDirectoryInExePath is set in {key}; cmd would not reach the stubs"
            );
        }
        // A version-manager shim on `PATH` would run inside the scratch profile, where it can
        // try to install tools; the stubs call the interpreter's own executable instead.
        let shim = std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|path| path.join("node.exe"))
            .find(|path| path.is_file())
            .expect("node.exe on PATH");
        let resolved = Command::new(&shim)
            .args(["-p", "process.execPath"])
            .output()
            .unwrap();
        assert!(resolved.status.success(), "{}", shim.display());
        let node = PathBuf::from(String::from_utf8(resolved.stdout).unwrap().trim());
        assert!(node.is_file(), "{}", node.display());
        let directories: Vec<String> = labels.iter().map(|label| format!("work/{label}")).collect();
        let directories: Vec<&str> = directories.iter().map(String::as_str).collect();
        let witness = Self {
            fixture: Fixture::new(prefix, &directories),
            node,
        };
        witness.set_preference(
            PreferenceKey::LaunchProfiles,
            json!({
                "version": 1,
                "defaultProfileId": CMD_PROFILE,
                "profiles": [
                    {"id": "system-default", "name": null, "command": {"kind": "system-shell", "login": false}},
                    {"id": CMD_PROFILE, "name": "Command Prompt", "command": {
                        "kind": "custom-executable",
                        "executable": system_program("cmd.exe"),
                        "arguments": ["/D"],
                    }},
                ],
            }),
        );
        witness.place_stubs(labels);
        witness
    }

    /// Writes each Panel's stub under its own root and, in every work directory, one shim per
    /// provider command: to that directory's own stub for its command, else to the first Panel's
    /// stub with that command.
    fn place_stubs(&self, labels: &[&'static str]) {
        let bundle = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/agent-integration-writer/agent-observation-adapters/bundle");
        let bundle = fs::canonicalize(&bundle)
            .unwrap_or_else(|error| panic!("{}: {error}", bundle.display()));
        let bundle = bundle.to_str().unwrap().trim_start_matches(r"\\?\");
        let literal = |value: &str| serde_json::to_string(value).unwrap();
        let mut entries: Vec<(&'static str, &'static str, PathBuf)> = Vec::new();
        for &label in labels {
            let row = Self::row(label);
            let entry = PathBuf::from(self.directory(&format!("stubs/{label}"))).join(row.entry);
            fs::create_dir_all(entry.parent().unwrap()).unwrap();
            let stub = format!(
                "const LABEL = {};\nconst CALLS = {};\nconst BUNDLE = {};\nconst RESUME = {};\n\
                 const KEY = {};\nconst REPORTS_ON_RESUME = {};\n{}",
                literal(label),
                literal(&self.directory("calls.jsonl")),
                literal(bundle),
                serde_json::to_string(row.resume).unwrap(),
                literal(row.key),
                row.reports_on_resume,
                STUB.replace("/*FRESH*/", row.fresh)
                    .replace("/*REPORT*/", row.report),
            );
            match row.relay {
                Some(relay) => {
                    fs::write(entry.with_file_name(relay), stub).unwrap();
                    fs::write(
                        &entry,
                        format!("const RELAY = {};\n{RELAY}", literal(relay)),
                    )
                    .unwrap();
                }
                None => fs::write(&entry, stub).unwrap(),
            }
            entries.push((label, row.program, entry));
        }
        for &(label, _, _) in &entries {
            let mut programs: Vec<&str> = entries.iter().map(|(_, program, _)| *program).collect();
            programs.sort_unstable();
            programs.dedup();
            for program in programs {
                let (_, _, entry) = entries
                    .iter()
                    .find(|(own, own_program, _)| *own == label && *own_program == program)
                    .or_else(|| entries.iter().find(|(_, other, _)| *other == program))
                    .unwrap();
                fs::write(
                    PathBuf::from(self.directory(&format!("work/{label}")))
                        .join(format!("{program}.cmd")),
                    format!(
                        "@\"{}\" \"{}\" %*\r\n",
                        self.node.display(),
                        entry.display()
                    ),
                )
                .unwrap();
            }
        }
    }

    /// A Panel's row: the gone Panel plays Claude Code and the touched one Codex.
    fn row(label: &str) -> &'static Row {
        match label {
            "gone" => &TABLE[0],
            "touched" => &TABLE[1],
            _ => TABLE.iter().find(|row| row.label == label).unwrap(),
        }
    }

    /// A fixture path as Windows spells it.
    fn directory(&self, relative: &str) -> String {
        self.fixture.path(relative).replace('/', "\\")
    }

    /// Opens a Workspace whose default-program terminal starts in `label`'s work directory and
    /// returns its Panel.
    fn workspace(&self, label: &str) -> String {
        let cwd = self.directory(&format!("work/{label}"));
        let columns = COLUMNS.to_string();
        let lines = LINES.to_string();
        let mut words = vec![
            "workspace",
            "create",
            "--label",
            label,
            "--cwd",
            &cwd,
            "--no-focus",
            "--cols",
            &columns,
            "--rows",
            &lines,
            "--cell-width",
            "8",
            "--cell-height",
            "16",
        ];
        let environment = self.fixture.terminal_environment();
        words.extend(environment.iter().map(String::as_str));
        let created = self.fixture.run(&words);
        assert_eq!(created.code, Some(0), "{created}");
        created.document["workspace"]["result"]["panelId"]
            .as_str()
            .unwrap_or_else(|| panic!("{created}"))
            .to_owned()
    }

    /// The typed handle of each bound terminal by its Panel.
    fn terminals(&self) -> HashMap<String, String> {
        let listed = self.fixture.run(&["terminal", "list"]);
        listed.document["result"]["terminals"]
            .as_array()
            .unwrap_or_else(|| panic!("{listed}"))
            .iter()
            .filter_map(|terminal| {
                Some((
                    terminal["boundTo"]["panelId"].as_str()?.to_owned(),
                    format!(
                        "@terminal:0x{}",
                        terminal["terminalId"].as_str()?.trim_start_matches('0')
                    ),
                ))
            })
            .collect()
    }

    /// The recent unwrapped text of `terminal`. A read is a Client view.
    fn screen(&self, terminal: &str) -> String {
        let read =
            self.fixture
                .run(&["terminal", "read", terminal, "--source", "recent-unwrapped"]);
        read.document["result"]["content"]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("{read}"))
            .to_owned()
    }

    /// Types `text` and Enter into `terminal`.
    fn send(&self, terminal: &str, text: &str) {
        let sent = self
            .fixture
            .run(&["terminal", "send", terminal, "--text", text, "--enter"]);
        assert_eq!(sent.code, Some(0), "{sent}");
    }

    /// A Client controlling and viewing the terminal bound to `panel`.
    fn control(&self, panel: &str) -> Control {
        Control::hold_in(
            &self.fixture,
            "default",
            &self.terminals()[panel],
            COLUMNS,
            LINES,
        )
    }

    /// Waits for cmd's prompt in each Panel's directory, types its row's command, and returns each
    /// Panel with the session its run took there.
    fn start_all(&self, panels: Vec<(&'static Row, &'static str, String)>) -> Vec<Instance> {
        let terminals = self.terminals();
        let deadline = Instant::now() + OBSERVED;
        let mut waiting: Vec<&(&'static Row, &'static str, String)> = panels.iter().collect();
        while !waiting.is_empty() {
            waiting.retain(|(row, label, panel)| {
                let prompt = format!(
                    "{}>",
                    self.directory(&format!("work/{label}")).to_lowercase()
                );
                let screen = self.screen(&terminals[panel]);
                if screen.to_lowercase().contains(&prompt) && !screen.contains("PS ") {
                    self.send(&terminals[panel], row.program);
                    return false;
                }
                assert!(
                    Instant::now() < deadline,
                    "no cmd prompt in {label}:\n{screen}"
                );
                true
            });
            self.view_agents();
        }
        loop {
            let calls = self.calls();
            let started: Vec<Option<&Value>> = panels
                .iter()
                .map(|(_, label, _)| {
                    calls.iter().find(|call| {
                        call["label"] == *label
                            && call["argv"] == json!([])
                            && call["reported"] == true
                            && self.same_directory(&call["cwd"], &format!("work/{label}"))
                    })
                })
                .collect();
            if started.iter().all(Option::is_some) {
                return panels
                    .iter()
                    .zip(started)
                    .map(|((row, label, panel), call)| {
                        let call = call.unwrap();
                        Instance {
                            row,
                            label,
                            panel: panel.clone(),
                            id: call["id"].as_str().unwrap().to_owned(),
                            session: call["session"].as_str().unwrap().to_owned(),
                        }
                    })
                    .collect();
            }
            assert!(
                Instant::now() < deadline,
                "not every stub started: {calls:#?}"
            );
            self.view_agents();
        }
    }

    /// Waits until the Agent list shows each Panel as `expected`, with its row's provider.
    fn until_listed(&self, expected: &[(&Instance, Listed)]) {
        let deadline = Instant::now() + OBSERVED;
        loop {
            let listed = self.fixture.run(&["agent", "list"]);
            let items = listed.document["result"]["items"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            // A row seen in the Panel's processes, as its provider when `provider` is given.
            let seen = |instance: &Instance, provider: bool, source: Option<&str>| {
                items.iter().any(|item| {
                    item["placement"]["panelId"] == instance.panel.as_str()
                        && item["terminal"].is_object()
                        && (!provider || item["provider"] == instance.row.provider)
                        && source.is_none_or(|source| item["source"] == source)
                })
            };
            let missing: Vec<&str> = expected
                .iter()
                .filter(|(instance, listed)| match listed {
                    Listed::Both => !seen(instance, true, Some("both")),
                    Listed::Process => !seen(instance, true, Some("terminal")),
                    Listed::Gone => seen(instance, false, None),
                })
                .map(|(instance, _)| instance.label)
                .collect();
            if missing.is_empty() {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the Session never listed {missing:?} as expected: {listed}"
            );
            thread::sleep(Duration::from_millis(250));
        }
    }

    /// Every run the stubs recorded, in order. Each must be a stub this test placed.
    fn calls(&self) -> Vec<Value> {
        let root = self.directory("").to_lowercase();
        let calls: Vec<Value> = fs::read_to_string(self.directory("calls.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        for call in &calls {
            assert!(
                call["script"]
                    .as_str()
                    .is_some_and(|script| script.to_lowercase().starts_with(&root)),
                "a run outside this fixture: {call}"
            );
        }
        calls
    }

    /// The runs of `instance`'s stub that resumed its own session.
    fn resumes(&self, instance: &Instance) -> usize {
        self.calls()
            .iter()
            .filter(|call| {
                call["label"] == instance.label && call["argv"] == json!(instance.tokens())
            })
            .count()
    }

    fn until_resumed(&self, instances: &[&Instance], count: usize) {
        let deadline = Instant::now() + OBSERVED;
        while let Some(late) = instances
            .iter()
            .find(|instance| self.resumes(instance) < count)
        {
            assert!(
                Instant::now() < deadline,
                "{} never resumed {count} times: {:#?}",
                late.label,
                self.calls()
            );
            self.view_agents();
        }
    }

    /// Reads the Agent list once, as a Client showing it does. Hook reports join their Panel only
    /// when a Client's Agent read drains the Worker's correlation hints, and an unjoined report
    /// expires after 10 s, so every wait reads it. A read refused because the list changed under
    /// it has still drained.
    fn view_agents(&self) {
        self.fixture.run(&["agent", "list"]);
    }

    /// Waits out `QUIET` while viewing the Agent list.
    fn quiet(&self) {
        let deadline = Instant::now() + QUIET;
        while Instant::now() < deadline {
            self.view_agents();
            thread::sleep(Duration::from_millis(250));
        }
    }

    /// Each Panel's stub ran once fresh and then exactly `resumes` times with its own resume
    /// arguments, each resume in its own directory and its own session, reporting only where its
    /// provider does.
    fn assert_runs(&self, instances: &[&Instance], resumes: usize) {
        let calls = self.calls();
        for instance in instances {
            let own: Vec<&Value> = calls
                .iter()
                .filter(|call| call["label"] == instance.label)
                .collect();
            let fresh = own.iter().filter(|call| call["argv"] == json!([])).count();
            let resumed: Vec<&&Value> = own
                .iter()
                .filter(|call| call["argv"] == json!(instance.tokens()))
                .collect();
            assert!(
                fresh == 1 && resumed.len() == resumes && own.len() == 1 + resumes,
                "{} ran {} times, not once fresh and {resumes} resumed with {:?}: {own:#?}",
                instance.label,
                own.len(),
                instance.tokens()
            );
            // The fresh run's directory was checked when it started; a gone Panel's no longer exists.
            for call in resumed {
                assert!(
                    self.same_directory(&call["cwd"], &instance.directory()),
                    "{} resumed outside its directory: {call}",
                    instance.label
                );
                assert_eq!(call["session"], instance.session.as_str(), "{call}");
                assert_eq!(
                    call["reported"], instance.row.reports_on_resume,
                    "{}: {call}",
                    instance.label
                );
            }
        }
    }

    fn same_directory(&self, actual: &Value, directory: &str) -> bool {
        let Some(actual) = actual.as_str() else {
            return false;
        };
        let expected = fs::canonicalize(self.directory(directory));
        expected.is_ok() && fs::canonicalize(actual).ok() == expected.ok()
    }

    fn stop(&self) {
        let stopped = self
            .fixture
            .run(&["server", "stop", "--confirm", "--session", "default"]);
        assert_eq!(stopped.code, Some(0), "{stopped}");
    }

    /// Starts the default Session again once its preference record still selects cmd: an
    /// unreadable record would restore PowerShell, which resolves only through `PATH`.
    fn start_session(&self) {
        let values = serde_json::to_value(&self.preferences().values).unwrap();
        assert_eq!(
            values["launchProfiles"]["defaultProfileId"], CMD_PROFILE,
            "{values}"
        );
        let started = self
            .fixture
            .run(&["server", "start", "--session", "default"]);
        assert_eq!(started.code, Some(0), "{started}");
    }

    /// Removes a fixture directory once the stopped Session's shells have released it.
    fn remove(&self, directory: &str) {
        let path = self.directory(directory);
        let deadline = Instant::now() + OBSERVED;
        while let Err(error) = fs::remove_dir_all(&path) {
            assert!(Instant::now() < deadline, "remove {path}: {error}");
            thread::sleep(Duration::from_millis(100));
        }
    }

    /// The `(provider, sessionId)` rows the newest published checkpoint retains, sorted.
    fn retained(&self) -> Vec<(String, String)> {
        let (_, checkpoint) = self
            .checkpoints()
            .into_iter()
            .max_by_key(|(generation, _)| *generation)
            .expect("the Session published no checkpoint");
        let checkpoint: Value = serde_json::from_slice(&checkpoint).unwrap();
        let rows = checkpoint["agentSessions"]
            .as_array()
            .unwrap_or_else(|| panic!("no agentSessions in the checkpoint: {checkpoint}"));
        let mut sessions: Vec<_> = rows
            .iter()
            .map(|row| {
                (
                    row["provider"].as_str().unwrap().to_owned(),
                    row["sessionId"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        sessions.sort();
        sessions
    }

    /// Every file under the fixture root with its path, depth first.
    fn files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut directories = vec![PathBuf::from(self.directory(""))];
        while let Some(directory) = directories.pop() {
            for entry in fs::read_dir(&directory).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    directories.push(path);
                } else {
                    files.push(path);
                }
            }
        }
        files
    }

    /// Each published checkpoint frame's generation and payload: a five-byte magic, a
    /// little-endian generation and a digest precede it.
    fn checkpoints(&self) -> Vec<(u64, Vec<u8>)> {
        self.files()
            .into_iter()
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("checkpoint") && name.ends_with(".json"))
            })
            .filter_map(|path| {
                let bytes = fs::read(&path).unwrap();
                (bytes.len() >= 45 && bytes.starts_with(b"OBCK\x01")).then(|| {
                    (
                        u64::from_le_bytes(bytes[5..13].try_into().unwrap()),
                        bytes[45..].to_vec(),
                    )
                })
            })
            .collect()
    }

    /// No session id appears in any file the runtime wrote other than checkpoint frames; the stubs'
    /// own records and scripts and the staged cohort are not the runtime's. The Server writes its
    /// diagnostic lines to standard error, not to a file here, so they are not read.
    fn assert_ids_only_in_store(&self, instances: &[Instance]) {
        let root = PathBuf::from(self.directory(""));
        let skipped = ["stubs", "cohort", "host"].map(|name| root.join(name));
        for path in self.files() {
            if skipped.iter().any(|skipped| path.starts_with(skipped))
                || path == root.join("calls.jsonl")
            {
                continue;
            }
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            if bytes.starts_with(b"OBCK\x01") {
                continue;
            }
            for instance in instances {
                for id in [&instance.id, &instance.session] {
                    assert!(
                        !bytes
                            .windows(id.len())
                            .any(|window| window == id.as_bytes()),
                        "{}: a session id is in {}",
                        instance.label,
                        path.display()
                    );
                }
            }
        }
    }

    fn store(&self) -> UserPreferencesStore {
        let roaming = self.fixture.path("roaming");
        UserPreferencesStore::new(
            UserPreferencesScope::from_environment(|name| {
                (name == "APPDATA").then(|| roaming.clone().into())
            })
            .unwrap(),
        )
    }

    fn preferences(&self) -> UserPreferencesSnapshot {
        match self.store().execute(&UserPreferencesRequest::Read {}) {
            UserPreferencesResponse::Snapshot { snapshot, .. }
                if snapshot.source != "unreadable" =>
            {
                *snapshot
            }
            other => panic!("the preference record is unreadable: {other:?}"),
        }
    }

    fn set_preference(&self, key: PreferenceKey, value: Value) {
        let response = self.store().execute(&UserPreferencesRequest::Update {
            source: self.preferences().source,
            changes: UserPreferencesPatch::set(key, value).unwrap(),
        });
        assert!(
            matches!(
                response,
                UserPreferencesResponse::Snapshot { changed: true, .. }
            ),
            "{response:?}"
        );
    }
}

fn sorted<'a>(instances: impl Iterator<Item = &'a Instance>) -> Vec<(String, String)> {
    let mut sessions: Vec<_> = instances.map(Instance::retained).collect();
    sessions.sort();
    sessions
}
