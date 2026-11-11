# Agent start delivery

Status: implemented for the Windows Session: one candidate per creation that carried an Agent
start, the typed line for a marked cmd, the early check, and the one-shot `agentStartFailure` fact.
A POSIX Session ends each start `unsupported-shell` at once.

## Outcome and scope

`tab.create`, `panel.split`, `workspace.launch` and a Worktree create may carry
`options.agentStart`, a provider of the startable catalog. The Server resolves it into the new
shell's first command, and the Server alone makes sure that start happens: no Client types it. When
it did not happen, the Server says so once on the Presentation.

Non-goals: starting an Agent in an existing shell, reserving or binding an Agent name, typing a
command a request supplied, and replaying a line. A creation without `agentStart` is never a
candidate.

## Candidate

When a creation settles with its terminal started, the coordinator returns the start it carried,
`AgentStartLaunch`: the Presentation and binding the creation committed, the provider, and the
`AgentStartCommand` the launch attached, with the fresh conversation id it may hold. The
dispatcher offers it to the start delivery while it still holds the registry and coordinator locks
the creation settled under, so notes about the new terminal start before any Client input can reach
it. The Terminal generation the binding names at that moment is the launch's generation.

What the candidate does depends on the Host's launch kind of the new terminal, read from its first
process sample:

| Launch kind        | Delivery                                                                                       | Watch, then fact                                                                          |
| ------------------ | ---------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| Managed PowerShell | Nothing typed: the shell runs the first command itself.                                        | 10 seconds from the launch's acceptance.                                                  |
| Marked cmd         | One gated send of the bare line, retried after a refusal that typed nothing, up to 30 seconds. | 3 seconds from the accepted send; no accepted send by the deadline: `prompt-unconfirmed`. |
| Any other program  | Nothing typed.                                                                                 | `unsupported-shell` at once.                                                              |
| None (older Host)  | Nothing typed.                                                                                 | `terminal-too-old` at once.                                                               |

The module comment of `local_terminal_service::agent_start_delivery` holds the states and
transitions; its decision is a pure function of the launch kind, the reads and the send's answers,
with time given by the driver.

### Reads

One thread carries every candidate, each due at its own next read, so one start never waits behind
another's watch. Each read first checks, under the registry and coordinator locks, that the
Presentation keeps the launch's binding and generation and whether Client input or a shell command
reached the terminal. It then reads the terminal through the process observer's on-demand lane:
one fresh frame for the shell's report, then one process sample. Reads are 250 milliseconds apart.
A terminal that shows no sample for 30 seconds ends with no fact.

### Typed line

A marked cmd's line is the command and its arguments joined by single spaces, the same tokens
managed PowerShell runs; every token is plain ASCII without spaces, quotes or shell syntax, so cmd
reads none as code. The line is sent only when the readiness rule
[resume uses](agent-session-resume.md#guards), without its directory check, admits a shell at its
prompt with nothing else in its console, and only through the Host's prompt gate, as one idle-prompt
submission of a paste and Enter. A refusal that typed nothing waits 250 milliseconds, or the
prompt's quiet period when the Host found it settling, and reads again. A Host without the gate ends
`terminal-too-old`. Any other result ends with no fact and is never sent again, since part of the
line may have been typed. At most one line is ever typed per launch.

Client input before the send ends the candidate silently: typed bytes, key and text input and
pastes on a controller, and every Client send, as for resume; focus, mouse reports and resizes do
not. So does a shell command the Host observed before the send. Both are checked at every read and
again under the send's own locks, so an input either precedes the send's admission or follows its
send. After the line was typed, and in managed PowerShell, input and commands are the person using
the Agent and end nothing.

### Early check

The watch reads the terminal like a Client's early check. The shell idle again after a process of
the provider was seen ends `exited`. The shell idle at the end of the watch with none seen ends
`not-found`. Anything else, an unreadable sample included, ends with no fact. Idle means a complete
sample with nothing but the root shell and console hosts.

### Abnormal ends

| Case                                    | What ends the candidate                                                                                  |
| --------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Server stop                             | The stop joins the thread before the observer stops; every candidate is dropped, nothing typed, no fact. |
| Panel or Tab closed, Presentation moved | The next read's guard finds it gone or rebound; ends with no fact.                                       |
| Worker lost or terminal fenced          | The guard finds no started terminal of the launch's generation; ends with no fact.                       |
| Host never reports a prompt             | A marked cmd ends `prompt-unconfirmed` at 30 seconds; a shell never sampled ends with no fact.           |
| Thread not running                      | The offer is refused and the start is dropped; nothing is typed.                                         |

## Outcomes

A start that happened shows nothing new: the Agent's own screen appears, and the Agent run's
`opening` state comes from ordinary observation.

A start that did not happen publishes `agentStartFailure { at, providerId, reason }` on the
Presentation in the [Workbench snapshot](../../../schemas/workbench/README.md), in one revision and
only while the Presentation is still bound as the creation committed it:

| Reason               | Found when                                                                  |
| -------------------- | --------------------------------------------------------------------------- |
| `not-found`          | The shell came back to its prompt and no process of the provider was seen.  |
| `exited`             | A process of the provider was seen, then the shell came back to its prompt. |
| `unsupported-shell`  | The shell is neither managed PowerShell nor a cmd Orchbee marks.            |
| `prompt-unconfirmed` | No gated send was accepted within 30 seconds; nothing was typed.            |
| `terminal-too-old`   | The terminal's Host records no launch kind or has no prompt gate.           |

`not-found` also marks that provider's [catalog](../README.md#agent-catalog) row `not-found` until
the next completed lookup answers again; a row the catalog cannot check, or one no completed lookup
has answered, stays as it is.

### What Clients show

The fact follows `agentNotResumed`: runtime-only, never checkpointed, gone only with its binding,
no timer, and each fact advances the revision exactly once. A Client that shows the fact shows each
one once, keyed by Presentation and `at`, and only within 60 seconds of `at` by its own clock, so a
Client that connects later says nothing. The notices themselves belong to the Clients.

## Invariants

- At most one typed line per launch, only into the launch's Terminal generation of its own
  Presentation, only through the Host's prompt gate, and never after Client input or a shell
  command reached that terminal.
- Managed PowerShell and every unsupported shell receive nothing from the Server.
- Each candidate ends exactly once, with at most one fact; a quiet end costs no revision.
- Each candidate end writes one diagnostic line, `component=agent-start outcome=<kind>`, with no
  id.

## Evidence

A decision-table test drives the candidate on a controlled clock across launch kinds and read
sequences: PowerShell running, idle, exiting, busy and unread; cmd typing once, not found, exiting,
waiting for its prompt, never confirmed, retrying refusals, refused to the deadline, without the
gate and with a failed send; another shell, an older Host, a terminal never sampled and a
Presentation that leaves. A guard test shows that Client input seen by a read or by the send's
admission types nothing, while input after the line or into PowerShell's Agent ends nothing. The
gate's own refusal, settling and acceptance rules are resume's, and so is their evidence.
