# Agent session resume

Status: the preference, the retained format and capture are implemented for the managed Windows
Session. Delivery after a restart is not implemented: a restored reference stays pending and
carries over into later checkpoints until a delivery owner takes it.

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

## Preference

`resumeAgentsOnRestore` defaults to on. The Server reads it before every capture, outside every
lock. Off writes no row, pending ones included. A refused preference record reads as on, the
built-in default; a Session without a preference store reads as off. The
[preference contract](../../../crates/workbench-protocol/docs/user-preferences.md) owns the key.

## Invariants

- At most one delivery per retained reference per Server lifetime, only into the restored Terminal
  generation of its own Presentation.
- Rows are unique by Presentation and by `(provider, sessionId)`; each has a directory row; every id
  is a hyphenated UUID.
- A live reference comes only from the capture rule.
- With the preference off, no row is written and nothing is typed.
- Native session ids never reach the wire, logs or notices. They live only in the private
  checkpoint store, and diagnostics carry counts and closed outcome kinds.

## Evidence

Deterministic tests cover the checkpoint rows (every fault, the omitted list, carry-over, take, live
override, the directory-row requirement, first-Presentation dedupe and recording off) and the
capture decision table (run token match, mismatch and tokenless; open, completed and unmarked
command evidence; same provider, other provider, `None`, `Unknown` and `Unsupported` identities;
ambiguity and invalid ids). Candidate joining, lock order and saver wiring are not claimed by
tests; a real Windows restart witness owns them together with delivery.
