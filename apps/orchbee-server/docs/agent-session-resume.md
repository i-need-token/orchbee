# Agent session resume

Status: implemented for the managed Windows Session: the `agents.resume_on_restore` setting, the retained format, capture,
delivery after a restart at a prompt the Host confirms, the one-shot notice when a resume is given
up, carry-forward of a delivered reference, and two [process sources](#process-source) that need
no hook: Claude Code's own run table, and the conversation id bound on the Agent process's own
command line.

## Outcome and scope

A user runs supported Agents in Panels, and the managed Server restarts from its checkpoint. Each
Panel's supported Agent Session should reopen in that Panel once. The managed Windows Session is
the only platform with checkpoint restore; POSIX restore will reuse this format and these rules.

Non-goals: replaying a command, relaunching an arbitrary program, restoring managed Agent names,
and restoring a terminal screen or history. A resumed Agent appears without its former name.

Agent-reported resume commands are never accepted: persisting command text from a report would let
any process in a Panel schedule execution at the next restart.

## Provider table

The table is closed and lives in Server code: `AgentResumeProvider` with its id validator, the
program builder of delivery, and the conversation id table, which holds both the typed resume
arguments and the [command-line bindings](#command-line) in one place.

### Admission

A provider joins when all four hold:

1. Its resume form is source-verified or documented.
2. The typed id is bound to a value-taking option or a resume subcommand, so the documented grammar
   cannot read it as a positional prompt.
3. A strict per-provider validator accepts the id, and every accepted id is inert in cmd and
   PowerShell and cannot start with `-`.
4. A wrong or stale id can only cause an error, or an empty or new session.

Same-id evidence from source is preferred. Documentation or Herdr precedent is enough under rules
2–4, because a wrong id then costs one failed resume and never sends a prompt. Each provider id is
both its native process provider id and its hook-contract provider id; a test asserts both, since a
mismatch would silently drop all of that provider's candidates.

### Rows

U is a hyphenated UUID: 8-4-4-4-12 hex digits in either case, 36 characters. Evidence: S is source,
D is documentation, H is Herdr precedent. "Directory" says whether resuming outside the session's
own directory changes the outcome; the retained-directory gate applies to every row regardless.
"Re-reports" says when a resumed run reports its session again; until then
[carry-forward](#carry-forward) keeps the delivered reference.

| Provider id      | Program        | Typed arguments (cmd)                                                       | Valid id, max length                                             | Directory          | Wrong or stale id                           | Re-reports                   | Evidence   |
| ---------------- | -------------- | --------------------------------------------------------------------------- | ---------------------------------------------------------------- | ------------------ | ------------------------------------------- | ---------------------------- | ---------- |
| `claude-code`    | `claude`       | `--resume=<id>`                                                             | U, 36                                                            | yes                | error, exits                                | yes                          | S, witness |
| `codex`          | `codex`        | `resume <id>`                                                               | U, 36                                                            | gate               | error, exits                                | yes                          | S, witness |
| `grok`           | `grok`         | `--resume=<id>`                                                             | U, 36                                                            | no                 | error; UUIDs never match titles             | yes                          | S          |
| `opencode`       | `opencode`     | `--session=<id>`                                                            | `^ses_[0-9a-f]{12}[0-9A-Za-z]{14}$`, 30                          | 1.1.25 yes; 2.x no | 1.x exits 1; 2.x empty session with that id | 1.x on first prompt; 2.x yes | S          |
| `kilo`           | `kilo`         | `--session=<id>`                                                            | as OpenCode, 30                                                  | no                 | error, exits                                | on first prompt              | S          |
| `cursor`         | `cursor-agent` | `--resume=<id>`                                                             | U, 36                                                            | probably           | error or new chat                           | probably no                  | D, H       |
| `qodercli`       | `qodercli`     | `--resume=<id>`                                                             | U, 36                                                            | yes                | error or new session                        | yes                          | D, bundle  |
| `antigravity`    | `agy`          | `--conversation <id>`                                                       | U, 36                                                            | undocumented       | error or new conversation                   | on first prompt              | D          |
| `droid`          | `droid`        | `--resume=<id>`                                                             | U, 36                                                            | probably           | error or new session                        | yes                          | D, H       |
| `kimi`           | `kimi`         | `--session=<id>`                                                            | `session_` and a version 4 U, 44                                 | strict spelling    | error, exits                                | yes                          | S          |
| `letta`          | `letta`        | `--conversation <id>`; `default:<a>` → `--conversation default --agent <a>` | `^(?:local-)?conv-U$`, `^default:agent-(?:local-)?U$`, 56        | no                 | error or new conversation                   | yes                          | S          |
| `devin`          | `devin`        | `--resume <id>`                                                             | `^[A-Za-z0-9]+(?:-[A-Za-z0-9]+){0,7}$`, 64 (provisional)         | yes (else prompts) | error or new session                        | unknown                      | D, H       |
| `hermes`         | `hermes`       | `--resume <id>`                                                             | `^[0-9]{8}_[0-9]{6}_[0-9a-f]{6,12}$`, `^local-[0-9a-f]{64}$`, 70 | no                 | error, exits                                | yes                          | S          |
| `pi`             | `pi`           | `--session <id>`                                                            | U, 36                                                            | yes (else forks)   | error, exits                                | yes                          | S          |
| `omp`            | `omp`          | `--resume <id>`                                                             | U, 36                                                            | no                 | error, exits                                | yes                          | S          |
| `gemini`         | `gemini`       | `--resume=<id>`                                                             | U, 36                                                            | yes                | error, exits                                | yes                          | S          |
| `github-copilot` | `copilot`      | `--resume=<id>`                                                             | U, 36                                                            | no                 | new empty session with that id              | unknown                      | D          |
| `qwen-code`      | `qwen`         | `--resume=<id>`                                                             | U, 36 (arena `-agent-…` ids fail closed)                         | yes                | "No saved session found", exits 1           | yes                          | S          |
| `cline`          | `cline`        | `--id <id>`                                                                 | `^[0-9]{13}_[0-9a-z]{5}$`, 19                                    | no                 | empty session with that id, or error        | on first run                 | S          |

Where a wrong id creates a new or empty session under that same id (Copilot, OpenCode 2.x, Cline),
that session reports the id again, so the reference stays stable and harmless. Cline's adapter keys
each session by the core session id its plugin setup receives, the id `cline --id` resumes, and
drops subagent runs.

OpenCode 1.3.4 through 1.18.30 and Kilo report their sessions from Orchbee's server plugin.
OpenCode 2.x runs server plugins in a process that never shares the Agent's console, so on Windows
it reports through Orchbee's TUI plugin instead, which runs in the TUI process that owns the
console, as the
[Windows integration model](../../orchbee/docs/windows-agent-integrations.md#opencode-automatic-discovery)
describes. Every OpenCode client shares one session service, so the TUI plugin reads only its own
TUI's route and session data and reports the root of the session that TUI displays, never another
Panel's session:

- a newly displayed root ends the root reported before it and starts itself;
- returning to a root the TUI already left ends the current one and reports nothing, because the
  Server never reopens an ended session;
- the home screen, a plugin page, or a session whose data has not loaded changes nothing;
- busy and idle follow the displayed root's status.

A 2.x Panel therefore holds the exact id of the root it shows, or no reference at all. An existing
2.x session opens in its own recorded directory whatever the current one is. The typed form stays
`--session=<id>`: `--continue` opens the most recently updated root session of the directory
across every client of the shared session store, which may be another Panel's.

Not in the table:

- **Mastracode.** Below 0.45.0, `mastracode resume <id>` runs a headless model turn whose prompt is
  `resume`, with every tool auto-approved; from 0.45.0 the same words open its interface. Neither
  its hook payload nor its hook environment proves which version runs, a package-version check
  races upgrades and `PATH` order, and no argument form resumes on 0.45.0 or later while failing
  harmlessly earlier. A typed resume could therefore send a prompt, which rule 2 forbids.
- **Amp, Kiro, Maki, Muse, AtomCode, CodeBuddy Code, Command Code, DevEco Code, MiMo Code,
  Reasonix, Snow CLI, MiniMax Code, MothX, J-Code, OpenClaw and Prime Agent** have no hook adapter, so no session key is ever observed.

### Tokens and quoting

- One builder turns a reference into argument tokens; the typed line and
  [carry-forward](#carry-forward)'s binding both use it. It sits beside the command-line binding
  table, and every typed form is one of that table's bindings, so a resumed run's command line
  names the id it was given.
- An option whose value is optional is typed `--name=<id>`, so the value stays glued to its option:
  no parser takes it as a positional prompt, and a parser without `=` support rejects it harmlessly.
  A required-value option or a subcommand takes a separate token. Claude Code's value is optional,
  so it takes `--resume=<id>`. OMP keeps `--resume <id>`, whose next-token consumption is
  source-verified while its `=` support is not.
- Tokens use only `[A-Za-z0-9_=-]`: never `'`, `:` or `.`. Letta's stored `default:<agent>` key is
  split into `--conversation default --agent <agent>` before it is typed.
- Programs resolve both in cmd and as a PowerShell application or external script, and none is a cmd
  internal command: `cursor-agent`, never the generic `agent`; `qodercli`, never the IDE dispatcher
  `qoder`; and `agy`.
- PowerShell single-quotes every argument token. A quoted token is a plain string, never a
  parameter, so it reaches a native program verbatim and an npm `.ps1` shim's `$args` intact. cmd
  takes the tokens bare.

## Inputs and trust

- Untrusted: hook envelopes (local and already admitted), checkpoint bytes (strictly parsed),
  Client input, and Claude Code's run table entries, which any process of the user can write
  (strictly parsed, and admitted only for the exact process they name).
- Trusted after admission: canonical Agent sessions, Workbench membership, and command and process
  evidence the Worker reports for one exact Terminal generation. That includes the command line of
  the published run's own process in the Host's sample, read only through the closed
  [binding table](#command-line); the Panel's reports and any reported command text never are.
- Absence: a Presentation without a reference restores a plain shell.

## Data

- `AgentResumeProvider` is the closed provider table. `RetainedAgentSession` pairs a provider with
  its native session id, which must be valid in that provider's own grammar (the
  [table](#rows)) and is kept verbatim, never case-folded. Every grammar admits only ASCII letters,
  digits, `_` and `-`, plus the `:` of Letta's stored key, which is never typed, and no id begins
  with `-`, so an id carries no option or command injection. The id is checked at capture and
  again at parse.
- The checkpoint `agentSessions` list holds one `presentationId`, `provider` and `sessionId` row per
  Presentation. The [checkpoint contract](workbench-checkpoint.md#per-presentation-agent-session-references)
  owns its validation, carry-over and order.
- `WorkbenchAuthority` keeps restored rows as pending restart intent keyed by Presentation.
  `pending_agent_session` reads an entry, `take_pending_agent_session` consumes it and
  `return_pending_agent_session` puts back one a refused send took; none changes the Workbench
  revision. `give_up_agent_resume` consumes an entry and publishes the
  [notice fact](#what-clients-show), one revision.
- The Session coordinator owns carried references: each delivered reference, with the exact
  Terminal generation its delivery reached, the tokens it typed and, once bound, its run anchor.
  They live only in memory, for one Server lifetime.
- A `RunTableEntry` is the session one exact process, its pid and creation time, named in its
  Agent's run table. It is read just before one capture and lives only for that capture.
- Each committed process sample keeps the published run's arguments after its program, read from
  that run's own row, for as long as the run stays published. They live only in memory and are
  never written; a conversation id bound in them is the [command-line](#command-line) source.

## Capture rule

Each checkpoint capture computes at most one live reference per bound Presentation. A carried
reference decides first ([carry-forward](#carry-forward)); without one, or once it drops, these
steps decide.

1. **Candidates.** The Agent cell supplies root hook sessions whose provider has a table row, whose
   canonical session is `Live` with no parent, and which hold a navigable attachment of this
   Presentation whose generation is the creation id of its current Terminal generation.
2. **Command evidence** of that generation:

   | Evidence                            | Candidates kept                                                  |
   | ----------------------------------- | ---------------------------------------------------------------- |
   | An open command with id X           | Those whose run token is X; if none match, the tokenless ones.   |
   | The latest command completed        | The tokenless ones only: a token names a command that has ended. |
   | No command ever observed, as in cmd | All.                                                             |

3. **Process identity** of that generation:

   | Identity                  | Effect                                                           |
   | ------------------------- | ---------------------------------------------------------------- |
   | `Agent { provider: p }`   | Keep candidates of provider `p` only.                            |
   | `None`                    | Drop all: the provider exited without a session-end observation. |
   | `Unknown` / `Unsupported` | No effect.                                                       |

4. **Result.** Exactly one distinct `(provider, native key)` gives a reference, and only if the key
   is a valid id. Two or more give none.
5. **Process source.** When no candidate remains, whether none was supplied or steps 2 and 3
   refuted all, the generation's published run decides through its own process while the
   generation's identity is `Agent` of that run's provider: the run table entry read for that exact
   run, else the one conversation id bound on that run's own command line, gives the reference
   ([process source](#process-source)). Otherwise none.
6. **Excluded sources.** Explicit Panel Agent reports, other report sources, process names alone,
   screen and scrollback content, a directory's most recent session and any reported command text
   never grant a reference.

The canonical attachment ties a session to its Panel, the generation ties it to the live terminal,
and run token and process identity refute stale or foreign sessions. A hook session therefore wins
over the process source, and ambiguity fails closed without consulting it.

The Session coordinator joins checkpoint targets to their Presentations through current
registration at the exact generation each target names, asks the Agent cell for candidates under
the registry, coordinator and Agent cell lock order, and applies steps 2–5. A stale target, an
unreadable registration or an unavailable Agent cell contributes no live reference, so pending
references still carry over. Capture then writes, per member Presentation in capture order, its
live reference, else its pending one, beside a directory row; the first Presentation keeps a shared
`(provider, sessionId)`.

Capture uses the existing triggers: the background saver after a layout change or every 30 seconds,
and the fresh final capture at clean stop. An Agent started less than 30 seconds before a crash may
be missed.

### Process source

When no hook candidate remains, the generation's published run names its session through its own
process, in this order:

1. its Agent's [run table](#run-table) entry, Claude Code's alone;
2. the conversation id bound on its own [command line](#command-line), for every provider in the
   table.

The run table wins because it follows the conversation the process holds now, while the command
line names the one the process started in. Both admit only the published run's exact process, its
pid and creation time, while the generation's identity is `Agent` of that run's provider. The order
is decided in one place, `select` in the Session coordinator's `agent_resume` module.

#### Run table

Claude Code keeps one entry per running process in its own run table, `sessions/<pid>.json` in its
configuration directory, naming the conversation that process holds. Version 2.1.322 writes `pid`,
`sessionId`, `procStart` and other fields, removes the entry when it exits normally, and leaves it
behind when it is killed. No other provider has a run table source yet.

- **Admission.** The entry of a generation's published run counts only when all of these hold: it
  is the regular file named for the run's pid, at most 64 KiB, holding one JSON object without a
  repeated field; its `pid` is the run's pid; its `procStart` is a string equal, digit for digit, to
  the run's creation time as the Host sampled it; and its `sessionId` is valid in the `claude-code`
  grammar. Other fields are ignored. A missing, unreadable, oversized, malformed or mismatched entry
  is no entry, never an error.
- **Units.** The Windows Host samples a creation time with `GetProcessTimes`: 100 ns units since
  1601 UTC. Claude Code's native Windows build writes `procStart` as that same count in decimal.
  Its PowerShell fallback writes CIM creation ticks, local and microsecond-precise, and its POSIX
  build writes `ps -o lstart` text; neither ever equals a sampled value, so those entries give none.
- **Reuse.** A later process that reuses a pid has another creation time, so an entry a killed
  process left behind never names it.
- **Location.** The directory is `.claude/sessions` in the Server's home directory, Claude Code's
  default. Process samples carry no environment, so a `CLAUDE_CONFIG_DIR` set for Claude Code alone
  moves its entries out of view, and that Panel resumes only through a hook session.
- **Reading.** Before each capture the Server lists, under the coordinator lock, the published runs
  whose identity is `Agent` of Claude Code, then reads one entry per run with no lock held. The
  capture admits an entry only while the generation still publishes that exact run, pid and
  creation time, so a run that changed in between gains nothing. Restore never reads it, and the
  entries are kept only for that capture.
- **Coverage.** A `claude` the person typed is covered like a board start, since identity and the
  published run do not depend on who started it. Only the Windows Host samples processes, so the
  source gives none elsewhere.

A capture that meets an entry while Claude Code rewrites it may read nothing for that Presentation
at that capture. Whether Claude Code updates `sessionId` when it switches conversation inside the
process (`/clear`, `/resume`) is unverified; if it does, later captures follow it, once no carried
reference decides first.

#### Command line

The Host's process sample carries each process's command line. At each committed sample the Session
coordinator keeps the published run's arguments after its program, read from the row with that
run's pid and creation time, and only when process identity finds that row interactive for the
run's provider: `interactive_agent_arguments` in `orchbee-agent-observation-protocol` gives exactly
the arguments the provider's mode grammar classified, so no runtime, launcher or wrapper token is
ever read. A capture admits a conversation id from them only when all of these hold:

- the arguments come from the generation's published run, as above;
- the option or subcommand that binds the id is in the provider's row below;
- exactly one id is bound;
- the id is valid in the provider's grammar.

| Provider id        | Binds the id                                     | Binds nothing                        |
| ------------------ | ------------------------------------------------ | ------------------------------------ |
| `claude-code`      | `--resume`, `-r`, `--session-id`                 | `-c`, `--continue`, `--fork-session` |
| `codex`            | the positional after a first positional `resume` | `--last`                             |
| `grok`             | `--resume`, `-r`, `--session-id`, `-s`           | `-c`, `--continue`, `--fork-session` |
| `opencode`, `kilo` | `--session`, `-s`                                | `-c`, `--continue`                   |
| `cursor`, `droid`  | `--resume`                                       |                                      |
| `qodercli`         | `--resume`, `-r`                                 | `-c`, `--continue`                   |
| `antigravity`      | `--conversation`                                 |                                      |
| `kimi`             | `--session`, `-S`                                |                                      |
| `letta`            | `--conversation`, `--conv`, `-C`                 |                                      |
| `devin`, `hermes`  | `--resume`, `-r`                                 |                                      |
| `pi`               | `--session`, `--session-id`                      | `-c`, `--continue`, `-r`, `--resume` |
| `omp`              | `--resume`                                       | `-c`, `--continue`                   |
| `gemini`           | `--resume`, `-r`, `--session-id`                 |                                      |
| `github-copilot`   | `--resume`, `-r`, `--session-id`                 | `--continue`                         |
| `qwen-code`        | `--resume`, `-r`, `--session-id`                 | `-c`, `--continue`                   |
| `cline`            | `--id`                                           |                                      |

"Binds nothing" lists flags that continue the latest conversation, fork a new one or open a picker;
any of them gives none, since the process may hold another conversation than any bound id.

- **Forms.** A long option binds as `--name <id>` or `--name=<id>`, a short one as `-n <id>` only;
  `-n=<id>` gives none. A value that begins with `-` is no id. Every other option skips the value
  its provider's mode grammar gives it, so another option's value is never read as an id, and
  nothing after `--` is read. Codex's `fork <id>` binds nothing, since it opens a new conversation.
- **Letta.** A bound `default` with exactly one `--agent` or `-a` value is the stored
  `default:<agent>` key.
- **Ambiguity.** Two bindings give none, even of one id. An unknown or ambiguous form gives none,
  never an error.
- **Coverage.** A command line the person typed counts like an Agent start's: a hand-typed
  `codex resume <id>` gives that id. An Agent start gives Grok, Gemini, Qwen Code, Copilot and Pi a
  fresh `--session-id <uuid>` unless the caller's arguments already choose a conversation
  ([Windows Agent start](../../orchbee/docs/windows-agent-start.md#launch-time-conversation-ids)),
  so those runs carry an id from their first sample; Orchbee stores it nowhere else. Claude Code
  starts without one, because its run table follows the conversation it holds.
- **Optional values.** An option whose value is optional and written with a space, such as
  `grok --resume <id>`, leaves the id where the provider's mode grammar reads a positional
  argument. Grok, Qoder, Kimi, Droid and Copilot classify that as unclassified, so identity stays
  `Unknown` and nothing is read. The `=` form, which delivery types, is read.
- **Stale ids.** The command line names the conversation the process started in. When the person
  switches conversation inside the Agent (`/resume`, `/new`, `/clear` and the like) and no hook
  session reports the switch, the next restart reopens the conversation the process started in,
  which still exists: stale, never a prompt. Orchbee does not detect the switch.
- **Platform.** Only the Windows Host samples processes, so the source gives none elsewhere.

## Carry-forward

Several providers stay silent after a resume until a prompt or run starts (the table's
"Re-reports" column), so the capture rule alone would find no candidate and the next restart would
lose the session. A delivered reference therefore stays capturable while, and only while, the
process its delivery started lives in that Terminal generation. It applies to every provider: where
the provider re-reports, the report takes over, and the anchor binding keeps it safe without a
per-provider flag.

- **Record.** When the send's admission takes the pending reference under its locks, the Session
  coordinator records it as carried for that exact Terminal generation, with the tokens the
  delivery types and no anchor. The pending reference is consumed as before.
- **Bind.** At each committed process sample of that generation, the first published run decides.
  The reference binds to that run's anchor, its process id and creation time, when the run's
  identity is `Agent` of the carried provider and the anchor row's decoded arguments end with the
  typed tokens. Any other first run drops it, so a manual start of the same provider never
  inherits a failed resume.
- **Capture**, per Presentation:
  1. Any candidate at that generation drops the carried reference for good, and the capture rule
     decides. That covers a re-report of the same key, a new key and ambiguity.
  2. Otherwise, a bound reference whose anchor is still the generation's published run, as `Agent`
     of its provider, is written as the live reference.
  3. Otherwise a bound reference is dropped. An unbound reference is kept for its first run but
     never written, so a resume that fails fast is never typed again.
- **Other drops.** A binding or generation change, or the Presentation going away, drops it. With
  the setting off it is not written. The creation time defeats process id reuse, and a run that
  hands off to a new anchor no longer matches, so its reference drops.

One pure decision, `carried_outcome`, gives each moment's `Bind`, `Write`, `Keep` or `Drop`.

Stale ids stay harmless:

- If the Agent exits and a new session starts without a report, the anchor is gone, so the
  reference drops.
- If the conversation switches inside the process with no report at all (Cursor `/new` without
  `sessionStart`, or an untouched switch in Antigravity or OpenCode 1.x), the next restart reopens
  an older conversation that still exists. That is never a prompt.
- A later report drops the reference at the next capture, at most 30 seconds away. A report that
  appears and ends between two captures can leave the old id, which is stale but harmless.

## Delivery

Orchbee types the provider's own resume command into the restored default shell, the way Herdr
does, instead of launching the Agent as the terminal's program. The shell survives the Agent's
exit, profile `PATH` and `.cmd` or `.ps1` shims resolve as when the user types the command, and
the run prefix gives the resumed run a token, so the next capture finds it like any other run, or
keeps the delivered reference while the provider stays silent ([carry-forward](#carry-forward)).
Launch programs and the restored-program policy stay unchanged.

The Host's own launch record for the terminal's root, the `launchKind` of its process sample,
chooses the line; the root's command line is never read.

| Launch kind                                                  | Typed line                                                                                                                                          |
| ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Managed PowerShell: `powershell` or `pwsh` with prompt marks | `<#orchbee-run:<token>#> & (Get-Command '<program>' -CommandType Application,ExternalScript -ErrorAction Stop \| Select-Object -First 1) '<arg>' …` |
| Interactive cmd with prompt marks                            | `<program> <arg> …`                                                                                                                                 |
| Any other shell, unmanaged PowerShell included               | Nothing: the reference is given up.                                                                                                                 |
| None: the Host predates the prompt gate                      | Nothing: the reference is given up as `terminal-too-old`.                                                                                           |

The line is an idle-prompt send, as an Agent start's is: the Host types it only while the root
shell waits at a fresh, empty, marked prompt, and otherwise refuses with `prompt-unconfirmed`,
having typed nothing ([prompt-gated input](../../orchbee/docs/windows-agent-start.md#prompt-gated-input)).
The records are one paste of the line, then Right for PowerShell, then Enter, as an Agent start
types them. The token is 16 bytes of operating-system entropy as 32 lowercase hex digits; without
entropy the reference is skipped. Managed PowerShell strips the prefix, echoes the token in the
command's start and finish markers and keeps the line out of history, as for an Agent start
([Windows Agent start](../../orchbee/docs/windows-agent-start.md)).

### Startup

Startup reads `agents.resume_on_restore` once, from the settings the Server loaded. Off discards
every pending reference and starts no driver. A Presentation whose directory is unavailable is
held without launching and keeps its reference
([held restore](workbench-checkpoint.md#held-restore)); see [Retry](#retry). The remaining
Presentations with a pending reference, in restoration order with their retained directories, are
the driver's candidates. The driver starts after the restored terminals launch and the process
observer starts, before any Client listener opens.

### Retry

While the setting is on, a Retry that starts a held Panel's shell in its saved directory hands a
still pending reference to the driver under the registry and coordinator locks it committed the
new binding under. The candidate queues behind the running driver, which takes it after its other
candidates, or, when the driver already ended in this Server lifetime, starts a new run. A run
ends in the same critical section that finds no queued candidate, so a handed-over reference is
never left unattempted while the Server runs. Its attempt waits one second from the Retry so the
Client can size the new terminal, then follows the same gate, spacing, readiness, guards and
consumption as a startup candidate. Start in home folder consumes the reference instead. A
stopping Server refuses the handover, and the final capture carries the reference.

### Gate, order and spacing

- **Gate.** The first successful Client viewer attachment of this Server lifetime opens the gate;
  the process observer's own viewers never do. The driver then waits one second so Clients can size
  what they show. With no Client, nothing is typed and the references carry over.
- **Order.** Terminals a Client viewer attached to go first, then the rest, each group in
  restoration order. Deliveries are at least 250 ms apart.
- **Readiness.** Each candidate's attempt has a 10-second deadline from its start. The driver reads
  the terminal every 250 ms through the process observer's on-demand lane, which keeps every
  temporary viewer serialized with the periodic samples: one fresh frame for the shell's report and
  reported directory, then one process sample, whose root directory stands in when the shell
  reports none. Every wait, a refused send's included, sleeps on the driver's own thread, which
  holds no lock and no shared channel, so neither the Server's dispatch, the Worker channel nor
  anyone's typing waits for it; stop wakes it.
- **Not viewed.** A Panel no Client viewed resumes at its current geometry, 80x24 if no Client sized
  it. The Agent redraws its live region at the next resize; transcript already printed stays
  wrapped.

### Guards

Before every read, under the registry and coordinator locks, the reference must still be pending,
the Presentation must keep the binding it had when the driver took the candidate (at the driver's
start, or at the Retry) and the started Terminal generation of its first attempt, no Client input
may have reached that terminal since then, and its shell must not have echoed any command. Client input is typed bytes, key and text
input and pastes on a controller, and every Client send; focus, mouse reports and resizes are not.
Notes about Client views and input are taken while their paths hold the coordinator lock.

Each read then decides:

| Shell report       | Process sample                                        | Decision                                  |
| ------------------ | ----------------------------------------------------- | ----------------------------------------- |
| Running            | Any                                                   | Give up: not idle.                        |
| Any                | No sample                                             | Wait.                                     |
| Any                | Another program beside the shell and its console host | Give up: not idle.                        |
| Any                | Launch kind is another shell                          | Give up: unsupported shell.               |
| Any                | No launch kind                                        | Give up: terminal too old.                |
| Any                | Row limit reached or an unreadable row                | Wait.                                     |
| At prompt          | Shell only                                            | Try the gated send.                       |
| Silent or no frame | Shell only                                            | Wait: only the Host can confirm a prompt. |

A shell's report is only a sign that a send is worth trying; the Host's gate decides. Silence never
admits a send, since a silent shell may be a program waiting for input.

The shell-only rule is the strict idle-shell rule an Agent start uses, shared in
`orchbee-agent-observation-protocol`. The directory, the reported one else the root's, must equal
the retained one, compared ASCII case-insensitively with one trailing separator trimmed, except a
drive root's. Another directory skips: resuming elsewhere fails for Claude Code, whose session
lookup is per project, misplaces Codex, and for other providers changes the outcome where the
[table](#rows) says so. No directory evidence waits.

On a try, the send's own admission checks every guard again under its locks and takes the
pending reference before any record leaves, so an input admitted before that point refutes the
delivery and one admitted after it follows the typed line.

The Host answers the gated send:

| Answer                                 | Then                                                                                                     |
| -------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Accepted                               | Sent; the reference is carried.                                                                          |
| `prompt-unconfirmed`, `settling`       | The reference goes back to pending; after the Host's 100 ms quiet period the guards and reads run again. |
| `prompt-unconfirmed`, any other state  | The reference goes back to pending; after 250 ms the guards and reads run again.                         |
| The Host lacks the gate                | Given up as `terminal-too-old` at once; never retried and never sent ungated.                            |
| Any other failure, or partial progress | Skipped and consumed.                                                                                    |

A refusal typed nothing, so retrying within the deadline is safe. A deadline that a refusal reached
gives up as `prompt-unconfirmed`; one that no read decided gives up as not ready.

### Consumption

A reference is consumed, without retry in this Server lifetime, by Client input, an observed
command, a changed binding or generation, a Presentation that is gone, a directory mismatch, a
busy terminal, an unsupported or too-old shell, a deadline, unavailable entropy, and every
attempted send except a gate refusal that typed nothing, which puts it back for the next read
within the deadline. It stays pending, for the next checkpoint to carry, when its terminal has no
started physical target or the Server stops first. The driver
stops and joins early in clean stop and startup abort, while the Worker still runs, so the final
capture carries every reference not yet attempted.

### Failures

| Case                                       | Behavior                                                                          |
| ------------------------------------------ | --------------------------------------------------------------------------------- |
| Retained directory missing at restore      | Held with its reference; Retry resumes it, Start in home folder consumes it.      |
| Directory differs when ready               | Skipped and consumed.                                                             |
| Provider not on `PATH`                     | The shell prints its own error and stays; no run binds, so no capture writes it.  |
| Session gone, such as a deleted transcript | The provider errors and exits, or opens an empty or new session; the shell stays. |
| Busy, unknown or touched terminal          | Skipped and consumed.                                                             |
| Prompt never confirmed within the deadline | Given up with a notice; nothing typed.                                            |
| Terminal started by an older Orchbee       | Given up with a notice; nothing typed.                                            |
| No Client before stop                      | Nothing typed; the references carry over.                                         |

### What Clients show

A success shows nothing new: the Panel shows the typed line and the provider's own output or error,
and sidebar Agent rows appear through ordinary hook observation once the Agent starts.

A give-up the person could not tell from the screen publishes a one-shot fact on the Presentation,
`agentNotResumed { at, providerId, reason }` in the
[Workbench snapshot](../../../schemas/workbench/README.md): the give-up's Unix milliseconds, the
provider and the reason. Both Clients show it as a 4-second status notice, the TUI's status line
and the GUI's notice with the same words, an identical notice already showing keeping its time:

| Reason               | Given up for                                                                | Notice                                                                                                   |
| -------------------- | --------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `prompt-unconfirmed` | A gate refusal at the deadline, a busy terminal, another shell, no decision | "{Agent} conversation not resumed: the shell was not at its prompt, so nothing was typed."               |
| `terminal-too-old`   | No launch kind, or a Host without the gate                                  | "{Agent} conversation not resumed: this terminal was started by an older Orchbee, so nothing was typed." |

Client input, an observed command, a changed binding, a gone Presentation, a directory mismatch,
unavailable entropy and a failed send stay quiet: the person caused or sees them, or no notice can
help. The fact is runtime-only, never checkpointed, and leaves only with its binding: a rebind,
the Presentation's removal or a Server restart. No timer clears it, so each published fact advances
the revision exactly once and a refusal or a returned reference never does. A Client shows each fact
once, keyed by Presentation and time, and only within 60 seconds of `at` by its own clock, so a
Client that connects later says nothing.

## Setting

`agents.resume_on_restore` defaults to on. The Server reads it from the Session's last valid
settings before every capture, outside every lock, and once at startup to decide resuming. Off
writes no row, pending ones included, and at startup discards every pending reference and types
nothing. A `config.toml` that stops parsing keeps the last valid value; a Session before startup
installs its configuration reads as off. The
[settings registry](../../../crates/local-runtime/README.md#published-key-file) owns the key.

## Invariants

- At most one delivery per retained reference per Server lifetime, only into the restored Terminal
  generation of its own Presentation.
- Rows are unique by Presentation and by `(provider, sessionId)`; each has a directory row; every id
  is valid in its provider's grammar.
- A typed resume binds the id to a value-taking option or a resume subcommand, so it never sends a
  prompt.
- A live reference comes only from the capture rule, or from a delivered reference bound to the
  live process its delivery started. The capture rule's process source admits only the run table
  entry, or the one conversation id bound on the command line, of the published run's exact
  process.
- With the setting off, no row is written and nothing is typed.
- Delivery happens only when all of these hold: the setting was on at startup, the binding and
  Terminal generation are unchanged, no Client input reached that terminal, no command was
  observed, the directory equals the retained one, the strict idle-shell rule holds, the shell
  reports its prompt, and the Host confirms the prompt and types the line itself. No resume is
  ever sent ungated.
- A give-up with a notice costs one Workbench revision; a quiet give-up, a refusal, a retry and a
  returned reference cost none. The notice names the provider and the reason, never the session.
- Native session ids never reach the wire, logs or notices. They live only in the private
  checkpoint store and in the line typed into the user's own Panel. Each attempt writes one
  diagnostic line, `component=agent-resume outcome=sent|skipped:<reason>|kept:<reason>`, with no
  id.

## Evidence

Deterministic tests cover the checkpoint rows (every fault, the omitted list, carry-over, take, live
override, the directory-row requirement, first-Presentation dedupe and recording off), each
provider's id grammar (its accepted forms and alternates, a leading `-`, whitespace, a wrong prefix,
wrong case where it matters, overlong ids and an embedded `'`), that every provider id is a native
and a hook-contract provider id, the process identity matrix for every typed form, and the
capture decision table (run token match, mismatch and tokenless; open, completed and unmarked
command evidence; same provider, other provider, `None`, `Unknown` and `Unsupported` identities;
ambiguity and invalid ids), the process source decision table (the published run's own entry
without a hook session; a hook session over an entry naming another; hook ambiguity and a sole
invalid key staying none; candidates refuted by provider or by an ended command leaving the entry;
an entry of a reused pid or another process, no published run, and `Unknown`, other-provider,
`None` and `Unsupported` identities giving none), the command-line source decision table (an id
alone; a hand-typed resume; Claude Code's own command line without an entry; its run table entry
over its command line; a hook session over both; two bound ids, one id bound twice, a continue flag,
another provider's id format, the command line of a reused pid's process, no sampled command line,
another provider's identity and `Unknown` giving none), the command-line binding table (every typed
resume reading back its own id for every provider, every binding option in each written form, and
look-alike options, glued short options, foreign ids, repeated bindings, options after `--`,
dash-led values and voiding flags giving none, with Codex's subcommand and Letta's default key
forms), the identity matrix and argument reader for each launch-time session option, the run table
reader (the exact pid and creation
time admitted; a reused pid's later or earlier creation time, no entry, an entry filed under
another pid and a missing directory giving none; an empty, truncated, BOM-prefixed, non-JSON or
array entry, a numeric, zero-padded, signed or .NET-ticks `procStart`, a textual pid, a missing
field, an invalid, option-like, repeated or non-string `sessionId`, trailing text, an entry past
64 KiB and a directory in its place each giving none), the carry-forward decision table (its own run binds; another
provider, a manual start, unreadable arguments and a run not yet an `Agent` drop; a candidate, a
gone anchor, a reused process id with a new creation time and a run that is no longer `Agent` of
the provider drop; a living bound run is written; an unbound reference is kept but never written)
and the delivery readiness table (a running command, another program, another launch kind, no
launch kind, every undecidable sample, each marked shell at its prompt, silence and no frame for
each, and directory equality), the Clients' notice selection (once per fact, only while fresh),
and the Retry handover (a reference handed over while the driver runs is taken before
it ends, one handed over after it ended starts a new run, and a stopping Server refuses it and
leaves it pending). Candidate joining, lock order, the gate, ordering, spacing, the observer lane, saver
wiring, the run table read outside the locks, carry-forward's record, bind and prune wiring, the
refusal retry and the give-up's commit are not claimed by unit tests.

`apps/orchbee/tests/cli_windows_agent_resume.rs` witnesses them across real stops and restarts in
one Session, with one Panel per [table](#rows) row: every provider, Letta's conversation and
default forms, Claude Code once more without hooks, and two more hookless rows named only by their
command lines, a Grok started with `--session-id <id>` as an Agent start types it and a hand-typed
`codex --search resume <id>`, 23 rows. Each row's stub is a Node script at a path process identity recognises as
that provider, and it reports its session as the provider's adapter does: a command-hook payload
through the shipped hook script, or the shipped OpenCode, Kilo, Cline, Pi or OMP module driven as
its provider drives it. The Kilo stub first admits the module's plugin definition, the one the
installed bootstrap default-exports, as the OpenCode 2.2.5 loader does: a plain object with a
string id and a setup function whose setup resolves. It then admits it as OpenCode 1.3.4+ and Kilo
do and reports through the hooks its `server` returns. The OpenCode stub plays a 2.2.5 TUI: it
admits the client definition the same way as a server plugin whose setup observes nothing, then
sets it up with a TUI context, typed as the 2.2.5 TUI plugin API types it, whose route shows a
child session of the stub's root beside another root, and the client reports that root. A row the
table marks as
re-reporting only "on first prompt" or "on first run", "probably no" or "unknown" stays silent after
a resume. The hookless Claude Code row reports nothing: the witness writes, from outside the
Panel, a Claude Code 2.1.322 run table entry for each of its runs, carrying the run's creation time
as `Get-Process` reads it, and writes one for each hooked Claude Code run naming another session.
Per row, the witness proves:

- capture of the fresh session without ambiguity, with Kilo's child session and the OpenCode
  TUI's displayed child session and other root present; the hookless Claude Code row's from its run
  table entry alone, the hooked one's from its hook session over its entry, and the hookless Grok
  and Codex rows' from their command lines alone;
- after a restart, the exact cmd line on screen and the exact recorded argument tokens, Claude
  Code's `--resume=<id>` and Letta's split `default:<agent>` key included, in the retained
  directory;
- exactly one delivery per Panel per Server lifetime, across two restarts;
- capture after a resume: from the report where the provider re-reports, and from the carried
  reference bound to its run where it stays silent, which then survives the second restart;
- a silent Agent that exits drops its carried reference from the next checkpoint;
- with the setting off at startup, nothing is typed and the next capture writes no row.

It also keeps the earlier cases: a missing directory restores a plain shell in home, Client input
during the settle refutes the delivery, every run is a stub under the fixture root, and no session
id appears in any file the runtime writes other than checkpoint frames.

Waits read the Agent list as a Client showing it does, because a hook report joins its Panel only
when a Client's Agent read drains it within 10 seconds. The witness runs only on request, with the
pinned ConPTY that carries the hook's correlation, in about 90 seconds. Its shell is cmd: a
restored terminal's `PATH` comes from the registry, where the user's real provider may sit, and
only cmd resolves a command from its current directory first, where every provider's stub sits.

Every delivery it sees passes the Host's prompt gate at cmd's marked prompt.

It does not prove:

- the managed PowerShell line, which has no end-to-end witness that keeps a real provider out of
  reach;
- a refused send's retry, a give-up or its notice, which no CLI read can provoke or observe; they
  rest on the decision tables and the end batch's real-window checks;
- any real provider: that its hook key is the id it resumes, that it parses the typed form, or
  where it looks up a session. Same-id and directory evidence rests on the table's sources, and
  where that is documentation only (Cursor, Droid, Devin, Copilot and Antigravity) a resume may
  fail until a real run confirms the id;
- that a real Grok, Gemini, Qwen Code, Copilot or Pi accepts `--session-id <uuid>` and opens a new
  conversation under it that its resume form reopens, or that any real provider keeps the bound id
  on its command line: the forms follow each provider's documentation or source, unrun here;
- a real Claude Code's run table: that the build a person runs, native or launched through npm and
  Node, writes `procStart` as the sampled creation time, and whether it rewrites `sessionId` on
  `/clear` or `/resume`. The format and the native build's equality with the Host's sample were
  read from one 2.1.322 run;
- that a real OpenCode 2.x loads both plugins without a plugin error, or that its TUI context
  behaves as the 2.2.5 TUI plugin API types it: the stub follows those sources, and a real 2.2.5
  run is still pending;
- native provider images, which identity classifies by pure rule in its matrix, nor Hermes's
  Python plugin, whose payload a Node stub sends through the same hook script;
- the Server's diagnostic lines, which go to standard error rather than to a file the witness can
  read.
