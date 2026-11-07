# Agent session resume

Status: implemented for the managed Windows Session: the preference, the retained format, capture,
delivery after a restart and carry-forward of a delivered reference.

## Outcome and scope

A user runs supported Agents in Panels, and the managed Server restarts from its checkpoint. Each
Panel's supported Agent Session should reopen in that Panel once. The managed Windows Session is
the only platform with checkpoint restore; POSIX restore will reuse this format and these rules.

Non-goals: replaying a command, relaunching an arbitrary program, restoring managed Agent names,
and restoring a terminal screen or history. A resumed Agent appears without its former name.

Agent-reported resume commands are never accepted: persisting command text from a report would let
any process in a Panel schedule execution at the next restart.

## Provider table

The table is closed and lives in Server code: `AgentResumeProvider` with its id validator, and the
program and argument builder of delivery.

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
- **Amp, Kiro, Maki and Muse** have no hook adapter, so no session key is ever observed.

### Tokens and quoting

- One builder turns a reference into argument tokens; the typed line and
  [carry-forward](#carry-forward)'s binding both use it.
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

- Untrusted: hook envelopes (local and already admitted), checkpoint bytes (strictly parsed) and
  Client input.
- Trusted after admission: canonical Agent sessions, Workbench membership, and command and process
  evidence the Worker reports for one exact Terminal generation.
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
  `pending_agent_session` reads an entry and `take_pending_agent_session` consumes it; neither
  changes the Workbench revision.
- The Session coordinator owns carried references: each delivered reference, with the exact
  Terminal generation its delivery reached, the tokens it typed and, once bound, its run anchor.
  They live only in memory, for one Server lifetime.

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
   is a valid id. Zero, or two or more, give none.
5. **Excluded sources.** Explicit Panel Agent reports, other report sources, process names and
   screen detection never grant a reference.

The canonical attachment ties a session to its Panel, the generation ties it to the live terminal,
and run token and process identity refute stale or foreign sessions. Ambiguity fails closed.

The Session coordinator joins checkpoint targets to their Presentations through current
registration at the exact generation each target names, asks the Agent cell for candidates under
the registry, coordinator and Agent cell lock order, and applies steps 2–4. A stale target, an
unreadable registration or an unavailable Agent cell contributes no live reference, so pending
references still carry over. Capture then writes, per member Presentation in capture order, its
live reference, else its pending one, beside a directory row; the first Presentation keeps a shared
`(provider, sessionId)`.

Capture uses the existing triggers: the background saver after a layout change or every 30 seconds,
and the fresh final capture at clean stop. An Agent started less than 30 seconds before a crash may
be missed.

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
  the preference off it is not written. The creation time defeats process id reuse, and a run that
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

| Restored shell                                                                                      | Typed line                                                                                                                                          |
| --------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Managed PowerShell: `powershell` or `pwsh` whose last argument is the Worker's fixed initialization | `<#orchbee-run:<token>#> & (Get-Command '<program>' -CommandType Application,ExternalScript -ErrorAction Stop \| Select-Object -First 1) '<arg>' …` |
| Other interactive PowerShell                                                                        | The same without the prefix.                                                                                                                        |
| Interactive cmd                                                                                     | `<program> <arg> …`                                                                                                                                 |
| Any other shell                                                                                     | Nothing: the reference is skipped.                                                                                                                  |

The records are one paste of the line, then Right for PowerShell, then Enter, as an Agent start
types them. The token is 16 bytes of operating-system entropy as 32 lowercase hex digits; without
entropy the reference is skipped. Managed PowerShell strips the prefix, echoes the token in the
command's start and finish markers and keeps the line out of history, as for an Agent start
([Windows Agent start](../../orchbee/docs/windows-agent-start.md)).

### Startup

Startup reads the preference once, after the Session's preference scope is frozen. Off discards
every pending reference and starts no driver. A pending reference whose retained directory is
missing, or not a directory, is discarded without a notice: the shell starts in the Worker's home,
as any restore does. The remaining Presentations, in restoration order with their retained
directories, are the driver's candidates. The driver starts after the restored terminals launch
and the process observer starts, before any Client listener opens.

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
  reports none.
- **Not viewed.** A Panel no Client viewed resumes at its current geometry, 80x24 if no Client sized
  it. The Agent redraws its live region at the next resize; transcript already printed stays
  wrapped.

### Guards

Before every read, under the registry and coordinator locks, the reference must still be pending,
the Presentation must keep the binding it had when the driver started and the started Terminal
generation of its first attempt, no Client input may have reached that terminal since the driver
started, and its shell must not have echoed any command. Client input is typed bytes, key and text
input and pastes on a controller, and every Client send; focus, mouse reports and resizes are not.
Notes about Client views and input are taken while their paths hold the coordinator lock.

Each read then decides:

| Shell report | Process sample                                             | Decision                                     |
| ------------ | ---------------------------------------------------------- | -------------------------------------------- |
| Running      | Any                                                        | Skip: not idle.                              |
| Any          | Another program beside the shell and its console host      | Skip: not idle.                              |
| Any          | Root is not an interactive cmd, powershell or pwsh         | Skip: unsupported shell.                     |
| Any          | Row limit reached, an unreadable row or root, or no sample | Wait.                                        |
| At prompt    | Shell only                                                 | Deliver.                                     |
| Silent       | Shell only, managed PowerShell                             | Wait: it marks its prompt.                   |
| Silent       | Shell only, cmd or other PowerShell                        | Deliver on the second consecutive such read. |
| No frame     | Shell only                                                 | Wait.                                        |

The shell-only rule is the strict idle-shell rule an Agent start uses, shared in
`orchbee-agent-observation-protocol`. The directory, the reported one else the root's, must equal
the retained one, compared ASCII case-insensitively with one trailing separator trimmed, except a
drive root's. Another directory skips: resuming elsewhere fails for Claude Code, whose session
lookup is per project, misplaces Codex, and for other providers changes the outcome where the
[table](#rows) says so. No directory evidence waits.

On a deliver, the send's own admission checks every guard again under its locks and takes the
pending reference before any record leaves, so an input admitted before that point refutes the
delivery and one admitted after it follows the typed line.

### Consumption

A reference is consumed, without retry in this Server lifetime, by Client input, an observed
command, a changed binding or generation, a Presentation that is gone, a directory mismatch, a
busy terminal, an unsupported shell, a read still undecided at the deadline, unavailable entropy,
and every attempted send, whatever its outcome. It stays pending, for the next checkpoint to
carry, when its terminal has no started physical target or the Server stops first. The driver
stops and joins early in clean stop and startup abort, while the Worker still runs, so the final
capture carries every reference not yet attempted.

### Failures

| Case                                       | Behavior                                                                          |
| ------------------------------------------ | --------------------------------------------------------------------------------- |
| Retained directory missing at restore      | Discarded; the shell starts in the Worker's home; nothing typed; no notice.       |
| Directory differs when ready               | Skipped and consumed.                                                             |
| Provider not on `PATH`                     | The shell prints its own error and stays; no run binds, so no capture writes it.  |
| Session gone, such as a deleted transcript | The provider errors and exits, or opens an empty or new session; the shell stays. |
| Busy, unknown or touched terminal          | Skipped and consumed.                                                             |
| No Client before stop                      | Nothing typed; the references carry over.                                         |

### What Clients show

Nothing new: no notice, toast or pending status on success or failure. The Panel shows the typed
line and the provider's own output or error, and sidebar Agent rows appear through ordinary hook
observation once the Agent starts.

## Preference

`resumeAgentsOnRestore` defaults to on. The Server reads it before every capture, outside every
lock, and once at startup to decide resuming. Off writes no row, pending ones included, and at
startup discards every pending reference and types nothing. A refused preference record reads as
on, the built-in default; a Session without a preference store reads as off. The
[preference contract](../../../crates/workbench-protocol/docs/user-preferences.md) owns the key.

## Invariants

- At most one delivery per retained reference per Server lifetime, only into the restored Terminal
  generation of its own Presentation.
- Rows are unique by Presentation and by `(provider, sessionId)`; each has a directory row; every id
  is valid in its provider's grammar.
- A typed resume binds the id to a value-taking option or a resume subcommand, so it never sends a
  prompt.
- A live reference comes only from the capture rule, or from a delivered reference bound to the
  live process its delivery started.
- With the preference off, no row is written and nothing is typed.
- Delivery happens only when all of these hold: the preference was on at startup, the binding and
  Terminal generation are unchanged, no Client input reached that terminal, no command was
  observed, the directory equals the retained one, the strict idle-shell rule holds, and the shell
  is at its prompt or, for an unmarked shell, silent and shell-only on two consecutive reads.
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
ambiguity and invalid ids), the carry-forward decision table (its own run binds; another
provider, a manual start, unreadable arguments and a run not yet an `Agent` drop; a candidate, a
gone anchor, a reused process id with a new creation time and a run that is no longer `Agent` of
the provider drop; a living bound run is written; an unbound reference is kept but never written)
and the delivery readiness table (a running command, another program, unsupported roots, every
undecidable sample, each shell at its prompt, silence for marked and unmarked shells, and directory
equality). Candidate joining, lock order, the gate, ordering, spacing, the observer lane, saver
wiring and carry-forward's record, bind and prune wiring are not claimed by unit tests.

`apps/orchbee/tests/cli_windows_agent_resume.rs` witnesses them across real stops and restarts in
one Session, with one Panel per [table](#rows) row: every provider, and Letta's conversation and
default forms, 20 rows. Each row's stub is a Node script at a path process identity recognises as
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
a resume. Per row, the witness proves:

- capture of the fresh session without ambiguity, with Kilo's child session and the OpenCode
  TUI's displayed child session and other root present;
- after a restart, the exact cmd line on screen and the exact recorded argument tokens, Claude
  Code's `--resume=<id>` and Letta's split `default:<agent>` key included, in the retained
  directory;
- exactly one delivery per Panel per Server lifetime, across two restarts;
- capture after a resume: from the report where the provider re-reports, and from the carried
  reference bound to its run where it stays silent, which then survives the second restart;
- a silent Agent that exits drops its carried reference from the next checkpoint;
- with the preference off at startup, nothing is typed and the next capture writes no row.

It also keeps the earlier cases: a missing directory restores a plain shell in home, Client input
during the settle refutes the delivery, every run is a stub under the fixture root, and no session
id appears in any file the runtime writes other than checkpoint frames.

Waits read the Agent list as a Client showing it does, because a hook report joins its Panel only
when a Client's Agent read drains it within 10 seconds. The witness runs only on request, with the
pinned ConPTY that carries the hook's correlation, in about 90 seconds. Its shell is cmd: a
restored terminal's `PATH` comes from the registry, where the user's real provider may sit, and
only cmd resolves a command from its current directory first, where every provider's stub sits.

It does not prove:

- the managed PowerShell line, which has no end-to-end witness that keeps a real provider out of
  reach;
- any real provider: that its hook key is the id it resumes, that it parses the typed form, or
  where it looks up a session. Same-id and directory evidence rests on the table's sources, and
  where that is documentation only (Cursor, Droid, Devin, Copilot and Antigravity) a resume may
  fail until a real run confirms the id;
- that a real OpenCode 2.x loads both plugins without a plugin error, or that its TUI context
  behaves as the 2.2.5 TUI plugin API types it: the stub follows those sources, and a real 2.2.5
  run is still pending;
- native provider images, which identity classifies by pure rule in its matrix, nor Hermes's
  Python plugin, whose payload a Node stub sends through the same hook script;
- the Server's diagnostic lines, which go to standard error rather than to a file the witness can
  read.
