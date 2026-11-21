//! Agent session resume across real managed restarts through the sources Windows keeps: the CLI,
//! Server, Worker and Zig Host under test, and a default Session whose default program is cmd.
//! Each row has its own Workspace and work directory, where every provider command is an npm-style
//! `.cmd` shim to a Node script at a path process identity recognises as that provider: a package
//! entry, or an alias-named script, under the provider's install root where the alias is shared.
//! No native stub binary exists.
//!
//! Windows withdrew the Agent observation channel, so no hook names a session here; the writer
//! crate's README records why. A stub takes a fresh session id in its provider's shape, or the id
//! its start or resume arguments carry, and only the Agent's own process names it. The Claude Code
//! row's session is named by Claude Code's own run table entry, which the witness writes for each
//! of its runs with that process's exact creation time. The other rows are named only by the
//! Agent's own command line: a Grok started the way an Agent start types it, with a fresh
//! `--session-id`, a hand-typed `codex resume` with a leading option, and a hand-typed
//! `mimo --session <id>`. Every run records its arguments, directory and session in the fixture
//! root and stays alive like an interactive Agent until it reads `q`. Every wait reads the Agent
//! list, as a Client showing it does.
//!
//! The table restates the Server's argument rule as typed text, so the screen and the recorded
//! arguments are the only oracle; nothing imports Server internals. The flow captures every row,
//! resumes each once after a restart with its exact cmd line, captures again, from the delivered
//! reference carried forward with the run it started, and resumes each again. One row's Agent then
//! exits, so its reference drops, and a last restart with the preference off types nothing. A Panel
//! whose directory was deleted, or that a Client typed into first, receives nothing; the deleted one
//! stays held in its place and keeps its reference until the preference is off.
//!
//! Every resume is an idle-prompt send that the Host types only at cmd's marked prompt, so each
//! delivery seen here passed the Host's prompt gate. A refused send, its retry and the notice of a
//! give-up cannot be provoked or read through the CLI, and stay with the Server's decision tables.
//!
//! A restored terminal carries no launch environment: the Host projects `PATH` from the registry,
//! where a real provider may be installed. cmd resolves a command from its current directory
//! first, a resume is typed only in the retained directory, and every work directory holds every
//! provider's shim, so the line always reaches a stub. PowerShell resolves only through `PATH`, so
//! it is never this witness's shell. cmd's prompt marks pass the system ConHost, so the witness
//! runs under it, or under the product's ConPTY when `ORCHBEE_TEST_CONPTY_DIR` names a verified
//! bundle, and only when asked for.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

use orchbee_workbench_protocol::UserSettingsValues;
use serde_json::{Value, json};
use session::{Control, Fixture, system_program};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, Instant},
};

/// Collective waits scale with the table: 30 s plus 1 s per row bounds every prompt, start,
/// listing and round of resumes.
const OBSERVED: Duration = Duration::from_secs(30 + TABLE.len() as u64);
/// Well past the settle, first readiness reads and spacing of a resume that cmd's marked prompt
/// admits at once. Every case waited out holds or consumes its reference without a retry, so
/// nothing typed by then means nothing will be.
const QUIET: Duration = Duration::from_secs(6);
/// Controller and creation geometry, wide enough that no typed line in the long temporary path
/// wraps.
const COLUMNS: u16 = 200;
const LINES: u16 = 30;
/// The launch profile that makes cmd the default program before any terminal starts.
const CMD_PROFILE: &str = "cmd";
/// The row whose Agent exits before the last capture.
const EXITING: &str = "mimo";

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
    /// The typed resume arguments, `{id}` standing for the session id.
    resume: &'static [&'static str],
    /// How the first run is typed.
    start: Start,
}

/// How a row's first run is typed after its program.
#[derive(Clone, Copy)]
enum Start {
    /// Bare: the stub takes a fresh id, which this JavaScript expression gives in the provider's
    /// shape.
    Fresh(&'static str),
    /// These tokens, `{id}` standing for this fixed id, which the stub takes as its session.
    With(&'static [&'static str], &'static str),
}

impl Row {
    /// The tokens typed after the program for the first run.
    fn start_tokens(&self) -> Vec<String> {
        match self.start {
            Start::Fresh(_) => Vec::new(),
            Start::With(pattern, id) => pattern
                .iter()
                .map(|token| token.replace("{id}", id))
                .collect(),
        }
    }

    /// The line typed for the first run.
    fn start_line(&self) -> String {
        std::iter::once(self.program.to_owned())
            .chain(self.start_tokens())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// The rows: Claude Code named by its own run table, and Grok, Codex and MiMo Code named only by
/// their own command lines.
const TABLE: [Row; 4] = [
    Row {
        label: "claude-run-table",
        provider: "claude-code",
        program: "claude",
        entry: "node_modules/@anthropic-ai/claude-code/cli.js",
        resume: &["--resume={id}"],
        // Each run's entry, which Claude Code writes for itself, is written by the witness.
        start: Start::Fresh("crypto.randomUUID()"),
    },
    Row {
        label: "mimo",
        provider: "mimo",
        program: "mimo",
        entry: "node_modules/@mimo-ai/cli/mimo.js",
        resume: &["--session={id}"],
        start: Start::With(&["--session", "{id}"], "ses_0123456789abCdEfGhIjKlMnOp"),
    },
    Row {
        label: "grok-launch",
        provider: "grok",
        program: "grok",
        entry: "grok.js",
        resume: &["--resume={id}"],
        start: Start::With(
            &["--session-id", "{id}"],
            "5f0c3e2a-8b1d-4c6e-9a7f-2d4b6e8c0a13",
        ),
    },
    Row {
        label: "codex-command-line",
        provider: "codex",
        program: "codex",
        entry: "node_modules/@openai/codex/bin/codex.js",
        resume: &["resume", "{id}"],
        start: Start::With(
            &["--search", "resume", "{id}"],
            "8d2e4f60-1a3b-4c5d-8e7f-9a0b1c2d3e4f",
        ),
    },
];

/// A fresh run takes a new id, or the fixed one its start tokens carry, and a resumed one keeps its
/// own. Written as one async function with dynamic imports, so the same source runs as CommonJS
/// and as a module.
const STUB: &str = r"
(async () => {
  const crypto = await import('node:crypto');
  const { appendFileSync } = await import('node:fs');
  const argv = process.argv.slice(2);
  const fresh = () => (/*FRESH*/);
  const matched = (patterns) => {
    if (argv.length !== patterns.length) return null;
    let id = null;
    for (const [index, pattern] of patterns.entries()) {
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
  };
  const id = matched(RESUME) ?? (START.length === 0 ? (argv.length === 0 ? fresh() : null) : matched(START));
  appendFileSync(CALLS, JSON.stringify({ label: LABEL, script: process.argv[1], pid: process.pid, argv, cwd: process.cwd(), id }) + '\n');
  process.stdout.write(`${LABEL} stub running\n`);
  process.stdin.on('data', (data) => { if (String(data).trim() === 'q') process.exit(0); });
  setInterval(() => {}, 1 << 30);
})();
";

/// One Panel running one row's stub, with the session its fresh run took.
struct Instance {
    row: &'static Row,
    label: &'static str,
    panel: String,
    id: String,
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
        (self.row.provider.to_owned(), self.id.clone())
    }
}

/// What the Agent list must show for a Panel before a capture.
#[derive(Clone, Copy)]
enum Listed {
    /// Seen in its terminal's processes as its row's provider.
    Process,
    /// Not seen in its terminal's processes.
    Gone,
}

#[test]
#[ignore = "drives real Session restarts with stub Agents for about a minute; run explicitly"]
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
    witness.write_run_table(&instances);
    witness.until_listed(&listed(&instances, None));

    witness.stop();
    assert_eq!(
        witness.retained(),
        sorted(instances.iter()),
        "capture retains each live stub's own reference"
    );
    // A Panel whose directory is gone is held unlaunched in its place, is never resumed, and keeps
    // its reference while it waits for a retry.
    witness.remove(&gone.directory());
    witness.start_session();
    // One Client opens the gate by taking control of a Panel and types into it within the settle;
    // it then views another Panel. The rest stay unviewed, at their restored geometry.
    let mut touching = witness.control(&touched.panel);
    touching.input(b"x");
    let viewing = witness.control(&rows[0].panel);
    witness.until_resumed(&resumable, 1);
    witness.write_run_table(rows);
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
    assert!(
        !terminals.contains_key(&gone.panel),
        "a Panel without its directory launched instead of staying held"
    );
    touching.release();
    viewing.release();

    // Each row is captured again from the delivered reference bound to the run it started.
    // Resuming again keeps working.
    witness.until_listed(&listed(rows, None));
    witness.stop();
    assert_eq!(witness.retained(), sorted(rows.iter().chain([gone])));
    witness.start_session();
    let viewing = witness.control(&rows[0].panel);
    witness.until_resumed(&resumable, 2);
    witness.write_run_table(rows);
    witness.quiet();
    witness.assert_runs(&resumable, 2);
    witness.assert_runs(&[gone, touched], 0);
    viewing.release();

    // An Agent that exits takes its carried reference with it.
    let exiting = rows
        .iter()
        .find(|instance| instance.label == EXITING)
        .unwrap();
    witness.send(&witness.terminals()[&exiting.panel], "q");
    witness.until_listed(&listed(rows, Some(exiting)));
    witness.stop();
    assert_eq!(
        witness.retained(),
        sorted(
            rows.iter()
                .filter(|instance| instance.label != EXITING)
                .chain([gone])
        )
    );

    // With the preference off at startup, the retained references are discarded unresumed, and
    // the next capture records none.
    witness.set_values(|values| values.resume_agents_on_restore = false);
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

/// Each of `instances` is seen in its terminal's processes, `exited` in none.
fn listed<'a>(instances: &'a [Instance], exited: Option<&Instance>) -> Vec<(&'a Instance, Listed)> {
    instances
        .iter()
        .map(|instance| {
            let listed = if exited.is_some_and(|exited| exited.label == instance.label) {
                Listed::Gone
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
        witness.fixture.set_values_live(|values| {
            values.launch_profiles = serde_json::from_value(json!({
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
            }))
            .unwrap();
        });
        witness.place_stubs(labels);
        witness
    }

    /// Writes each Panel's stub under its own root and, in every work directory, one shim per
    /// provider command: to that directory's own stub for its command, else to the first Panel's
    /// stub with that command.
    fn place_stubs(&self, labels: &[&'static str]) {
        let literal = |value: &str| serde_json::to_string(value).unwrap();
        let mut entries: Vec<(&'static str, &'static str, PathBuf)> = Vec::new();
        for &label in labels {
            let row = Self::row(label);
            let entry = PathBuf::from(self.directory(&format!("stubs/{label}"))).join(row.entry);
            fs::create_dir_all(entry.parent().unwrap()).unwrap();
            let (start, fresh) = match row.start {
                Start::Fresh(fresh) => (&[][..], fresh),
                Start::With(pattern, _) => (pattern, "null"),
            };
            let stub = format!(
                "const LABEL = {};\nconst CALLS = {};\nconst RESUME = {};\nconst START = {};\n{}",
                literal(label),
                literal(&self.directory("calls.jsonl")),
                serde_json::to_string(row.resume).unwrap(),
                serde_json::to_string(start).unwrap(),
                STUB.replace("/*FRESH*/", fresh),
            );
            fs::write(&entry, stub).unwrap();
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

    /// A Panel's row: the gone and touched Panels each play the Claude Code row, whose bare start
    /// gives each its own session.
    fn row(label: &str) -> &'static Row {
        let label = match label {
            "gone" | "touched" => "claude-run-table",
            label => label,
        };
        TABLE.iter().find(|row| row.label == label).unwrap()
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
        created.document["result"]["workspace"]["result"]["panelId"]
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

    /// Waits for cmd's prompt in each Panel's directory, types its row's start line, and returns
    /// each Panel with the session its run took there.
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
                    self.send(&terminals[panel], &row.start_line());
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
                .map(|(row, label, _)| {
                    calls.iter().find(|call| {
                        call["label"] == *label
                            && call["argv"] == json!(row.start_tokens())
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

    /// Writes Claude Code's run table entry, as Claude Code 2.1.322 writes it for itself, naming the
    /// session of the newest run of each Claude Code Panel among `instances`, under the Server's
    /// home. The stubs cannot read their own creation time, and a reader started inside a Panel
    /// would disturb its processes, so the witness writes each entry from outside.
    fn write_run_table(&self, instances: &[Instance]) {
        let calls = self.calls();
        let table = PathBuf::from(self.directory("home/.claude/sessions"));
        fs::create_dir_all(&table).unwrap();
        for instance in instances
            .iter()
            .filter(|instance| instance.row.program == "claude")
        {
            let call = calls
                .iter()
                .rev()
                .find(|call| call["label"] == instance.label)
                .unwrap();
            let pid = call["pid"].as_u64().unwrap();
            let entry = json!({
                "pid": pid,
                "sessionId": call["id"],
                "cwd": call["cwd"],
                "startedAt": 0,
                "procStart": process_start(pid),
                "version": "2.1.322",
                "kind": "interactive",
                "entrypoint": "cli",
                "status": "idle",
            });
            fs::write(table.join(format!("{pid}.json")), entry.to_string()).unwrap();
        }
    }

    /// Reads the Agent list once, as a Client showing it does, so every wait runs under a viewing
    /// Client. A read refused because the list changed under it still counts.
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

    /// Each Panel's stub ran once as its row starts and then exactly `resumes` times with its own
    /// resume arguments, each resume in its own directory and its own session.
    fn assert_runs(&self, instances: &[&Instance], resumes: usize) {
        let calls = self.calls();
        for instance in instances {
            let own: Vec<&Value> = calls
                .iter()
                .filter(|call| call["label"] == instance.label)
                .collect();
            let start = json!(instance.row.start_tokens());
            let fresh = own.iter().filter(|call| call["argv"] == start).count();
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
                assert_eq!(call["id"], instance.id.as_str(), "{call}");
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

    /// Starts the default Session again once its settings file still selects cmd: an unreadable
    /// file would restore PowerShell, which resolves only through `PATH`.
    fn start_session(&self) {
        let values = serde_json::to_value(self.fixture.settings_values()).unwrap();
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
    /// own records and scripts, Claude Code's run table and the staged cohort are not the
    /// runtime's. The Server writes its diagnostic lines to standard error, not to a file here, so
    /// they are not read.
    fn assert_ids_only_in_store(&self, instances: &[Instance]) {
        let root = PathBuf::from(self.directory(""));
        let skipped = ["stubs", "cohort", "host", r"home\.claude"].map(|name| root.join(name));
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
                let id = instance.id.as_bytes();
                assert!(
                    !bytes.windows(id.len()).any(|window| window == id),
                    "{}: a session id is in {}",
                    instance.label,
                    path.display()
                );
            }
        }
    }

    fn set_values(&self, change: impl FnOnce(&mut UserSettingsValues)) {
        self.fixture.set_values(change);
    }
}

/// A live process's creation time as Claude Code's native Windows build writes `procStart`: the
/// decimal count of 100 ns units since 1601 UTC that `GetProcessTimes` gives.
fn process_start(pid: u64) -> String {
    let read = Command::new(system_program(r"WindowsPowerShell\v1.0\powershell.exe"))
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!("(Get-Process -Id {pid}).StartTime.ToFileTimeUtc()"),
        ])
        .output()
        .unwrap();
    let start = String::from_utf8(read.stdout).unwrap().trim().to_owned();
    assert!(
        read.status.success()
            && !start.is_empty()
            && start.bytes().all(|byte| byte.is_ascii_digit()),
        "no creation time for process {pid}: {start}"
    );
    start
}

fn sorted<'a>(instances: impl Iterator<Item = &'a Instance>) -> Vec<(String, String)> {
    let mut sessions: Vec<_> = instances.map(Instance::retained).collect();
    sessions.sort();
    sessions
}
