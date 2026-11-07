# Agent session resume

Status: implemented for the managed Windows Session: the preference, the retained format, capture
and delivery after a restart.

## Outcome and scope

A user runs supported Agents in Panels, and the managed Server restarts from its checkpoint. Each
Panel's supported Agent Session should reopen in that Panel once. The managed Windows Session is
the only platform with checkpoint restore; POSIX restore will reuse this format and these rules.

Non-goals: replaying a command, relaunching an arbitrary program, restoring managed Agent names,
and restoring a terminal screen or history. A resumed Agent appears without its former name.

The provider table is closed and lives in Server code:

| Provider id   | Program  | Arguments       |
| ------------- | -------- | --------------- |
| `claude-code` | `claude` | `--resume <id>` |
| `codex`       | `codex`  | `resume <id>`   |

A provider joins the table only with a witness that its hook session key is the id its CLI
resumes. Agent-reported resume commands are never accepted: persisting command text from a report
would let any process in a Panel schedule execution at the next restart.

## Inputs and trust

- Untrusted: hook envelopes (local and already admitted), checkpoint bytes (strictly parsed) and
  Client input.
- Trusted after admission: canonical Agent sessions, Workbench membership, and command and process
  evidence the Worker reports for one exact Terminal generation.
- Absence: a Presentation without a reference restores a plain shell.

## Data

- `AgentResumeProvider` is the closed provider table. `RetainedAgentSession` pairs a provider with
  its native session id, which must be a hyphenated UUID (8-4-4-4-12 hex digits, either case) kept
  verbatim. The charset is inert in PowerShell argument mode and in cmd, and an id cannot begin
  with `-`, so it carries no option or command injection. The id is checked at capture and again
  at parse.
- The checkpoint `agentSessions` list holds one `presentationId`, `provider` and `sessionId` row per
  Presentation. The [checkpoint contract](workbench-checkpoint.md#per-presentation-agent-session-references)
  owns its validation, carry-over and order.
- `WorkbenchAuthority` keeps restored rows as pending restart intent keyed by Presentation.
  `pending_agent_session` reads an entry and `take_pending_agent_session` consumes it; neither
  changes the Workbench revision.

## Capture rule

Each checkpoint capture computes at most one live reference per bound Presentation.

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

## Delivery

Orchbee types the provider's own resume command into the restored default shell, the way Herdr
does, instead of launching the Agent as the terminal's program. The shell survives the Agent's
exit, profile `PATH` and `.cmd` or `.ps1` shims resolve as when the user types the command, and
the run prefix gives the resumed run a token, so the next capture finds it like any other run.
Launch programs and the restored-program policy stay unchanged.

| Restored shell                                                                                      | Typed line                                                                                                                                       |
| --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| Managed PowerShell: `powershell` or `pwsh` whose last argument is the Worker's fixed initialization | `<#orchbee-run:<token>#> & (Get-Command '<program>' -CommandType Application,ExternalScript -ErrorAction Stop \| Select-Object -First 1) <args>` |
| Other interactive PowerShell                                                                        | The same without the prefix.                                                                                                                     |
| Interactive cmd                                                                                     | `<program> <args>`                                                                                                                               |
| Any other shell                                                                                     | Nothing: the reference is skipped.                                                                                                               |

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
lookup is per project, and misplaces Codex. No directory evidence waits.

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

| Case                                       | Behavior                                                                        |
| ------------------------------------------ | ------------------------------------------------------------------------------- |
| Retained directory missing at restore      | Discarded; the shell starts in the Worker's home; nothing typed; no notice.     |
| Directory differs when ready               | Skipped and consumed.                                                           |
| Provider not on `PATH`                     | The shell prints its own error and stays; the next capture drops the reference. |
| Session gone, such as a deleted transcript | The provider prints its error and exits; the shell stays; the reference drops.  |
| Busy, unknown or touched terminal          | Skipped and consumed.                                                           |
| No Client before stop                      | Nothing typed; the references carry over.                                       |

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
  is a hyphenated UUID.
- A live reference comes only from the capture rule.
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
override, the directory-row requirement, first-Presentation dedupe and recording off) and the
capture decision table (run token match, mismatch and tokenless; open, completed and unmarked
command evidence; same provider, other provider, `None`, `Unknown` and `Unsupported` identities;
ambiguity and invalid ids) and the delivery readiness table (a running command, another program,
unsupported roots, every undecidable sample, each shell at its prompt, silence for marked and
unmarked shells, and directory equality). Candidate joining, lock order, the gate, ordering,
spacing, the observer lane and saver wiring are not claimed by unit tests;
`apps/orchbee/tests/cli_windows_agent_resume.rs` witnesses them on Windows with stub providers
reporting through the managed hook path across a real stop and restart.
