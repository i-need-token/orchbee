# Automatic Confirmation

| Field             | Value                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Accepted by the product owner on 2026-11-20. Decided restates the owner's decision. Chosen marks this page's own pick, which the owner may veto. Nothing here is implemented until its slice lands.                                                                                                                                                                                                                                                                                                                                                                  |
| Owner             | Maintainers of `apps/orchbee-server`, `apps/terminal-host`, `apps/orchbee-worker`, `crates/terminal-protocol`, `crates/capability-node-protocol`, `crates/workbench-protocol`, `crates/local-runtime`, `apps/orchbee`, and `apps/web`                                                                                                                                                                                                                                                                                                                                |
| Current authority | [Agent screen detection](../../apps/orchbee-server/docs/agent-screen-model.md) owns how a screen becomes an Agent state. [Prompt-gated input](../../apps/terminal-host/docs/runtime-model.md#prompt-gated-input) and the [Worker send transaction](../../apps/orchbee-worker/docs/worker-send-transaction.md) own the existing gated write this design copies. The [terminal protocol](../../schemas/terminal/references/protocol.md) owns Host message kinds. Neither Client offers anything about confirmations today: a waiting Agent shows as needing attention. |
| Exit condition    | Delete this proposal once every slice ships and each rule lives in its owning package reference or standard.                                                                                                                                                                                                                                                                                                                                                                                                                                                         |

An Agent that stops on a routine confirmation, such as "run `cargo test`?", keeps a person at the
keyboard for an answer that is almost always yes. With automatic confirmation turned on, Orchbee
presses the Agent's own one-time accept key for the person. It never answers a confirmation it
cannot read completely, and it never answers a destructive one.

The feature types a key into a program that can run commands. This page therefore decides first
when a key may be sent, and only then what a person sees.

## What a person gets

Decided, unless marked otherwise.

- **Off by default.** Nothing is pressed until a person turns it on.
- **One offer.** The first time a supported Agent waits on a confirmation Orchbee would have
  allowed, the Client shows one unobtrusive offer with three answers: this Agent only, always for
  this Agent type, or no. "This Agent only" lasts until that Agent run exits. "Always" is saved for
  the Agent type. "No" stops the offer for every Agent for good; the feature stays reachable in
  Settings and on the Agent row. If the person answers the Agent's confirmation themselves, the
  offer disappears; one Agent run offers at most twice.
- **Visible and changeable.** The Agent row's menu shows the current choice (off, this Agent only,
  always for this type) and changes it. Settings lists one switch per supported Agent type and says
  that more Agents can be requested in a GitHub issue.
- **After a press** the status line says `Allowed: <command>` for a short time.
- **When Orchbee does not press**, nothing changes from today: the Agent needs attention and the
  notification fires. Resting the pointer on the row's state gives the reason, such as
  "destructive command, waiting for you".
- **Destructive confirmations always wait.** No setting changes that, and there is no list a person
  can edit in the first cut (Chosen).
- **File changes inside the Workspace are allowed too.** A file outside the Workspace, or a
  protected file, always waits.
- **First Agents:** Claude Code, Codex, OpenCode, Gemini CLI and Hermes Agent, shipped one at a
  time, Claude Code first.

Chosen: the first cut adds no CLI command and no option beyond these.

## When a key is sent

Orchbee sends exactly one key, the layout's verified accept key, and only when every rule below
holds for the screen the key lands on.

| Rule            | Contract                                                                                                                                                                                                                                            |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Turned on       | Automatic confirmation is on for this Agent run: for the run, or for its Agent type. Turning it off also stops a key that was already decided ([one step](#check-and-write-are-one-step))                                                           |
| Same Agent      | Two process samples around the screen capture select the same Agent process, by process id, creation time and provider, and the Terminal Host finds that process among the terminal's processes when it prepares the key                            |
| Exact layout    | A frame the Server captured for this decision matches one bundled layout of that Agent completely, from the dialog's first row to the last row of the screen. A state the Agent reports about itself never stands in for the capture                |
| One-time accept | The highlighted row is the Agent's one-time accept. Orchbee never moves the highlight and never picks a row that says always, session, or do not ask again                                                                                          |
| Readable        | The whole command, or the whole file path, is on screen and contains only plain characters                                                                                                                                                          |
| Not marked      | The Agent shows no danger marker of its own                                                                                                                                                                                                         |
| Passes check    | The command passes the [command check](#the-command-check); a file change passes the [path check](#file-changes)                                                                                                                                    |
| Settled         | The same prompt has been observed, unchanged in its layout, its command or path, and its highlighted row, for at least one second across at least three captures                                                                                    |
| Person is away  | No person typed or clicked in this terminal at or after the revision at which the prompt was first seen, and none in the three seconds before the key. The Terminal Host owns both facts                                                            |
| Once            | No key was written for this prompt before. A send whose outcome is unknown counts as written; a key the Host refused does not                                                                                                                       |
| Still the same  | The Terminal Host writes the key only if the screen is still the one the rules read, in the same step as the write ([one step](#check-and-write-are-one-step))                                                                                      |
| Not repeating   | The same prompt has not come back unchanged after three presses in this Agent run, counting written and unknown sends. The fourth time Orchbee stops pressing for the run, says so once, and the row's menu shows it off; the person may turn it on |

Any rule that does not hold means the prompt waits for the person, exactly as today. A layout that
does not match, a command that is only partly visible or paged, a command shape the check cannot
read, an Agent danger marker, and a prompt that is not a tool confirmation (trust, login, update,
MCP, plan, question) all wait. An Agent release that changes its wording stops matching by itself
and goes back to waiting; nothing falls back to a looser match.

Orchbee never sends digits, Tab, arrows, or text, and never changes an Agent's own configuration.
The Host message that carries the key can name only keys from a closed list, which holds Enter
alone until an Agent needs another verified key.

Windows and macOS are admitted, the platforms Orchbee ships. No rule depends on the platform: the
Terminal Host owns the person record and the process check everywhere. Linux gets the same
behavior when the product ships there and is not verified with real Agents before that.

## Check and write are one step

Chosen. The Server reads the screen and decides; the Terminal Host writes. A decision made on a
captured screen is stale by the time its key reaches the Host, and the stretch between the two
grows under load. The key therefore travels as a new **screen-gated key**, which the Host writes
only if the screen it holds is still the captured one. This copies
[prompt-gated input](../../apps/terminal-host/docs/runtime-model.md#prompt-gated-input): the Host
checks at publication, holding the terminal lock and then the input lock, so no output batch and
no other input can land between the check and the write.

The message names the key, the frame the Server matched by its generation and content revision,
the revision of the frame in which the Server first saw this prompt, and the Agent process by its
process id and creation time. The Host requires all of:

1. The terminal runs, and its client input is open.
2. The named process is among the terminal's processes. The Host samples them when it prepares
   the key, immediately before the locked step, because a process sample is too slow to take under
   the terminal lock.
3. Generation and content revision equal the named frame's, and no reader batch is being
   committed. The revision advances with every committed output batch, so equality means the
   screen has not changed since the capture.
4. No screen-gated key was already written on this revision.
5. No other client input is queued.
6. No person input was published at or after the first-seen revision.
7. No person input was published in the last three seconds, by the Host's own clock. The quiet
   time is a constant of the Host in the protocol's constant registry; a sender cannot shorten it.

Rules 3 to 7 are checked when the key is prepared and again at publication, in the same locked
step as the write.

**Person input** is every client input except a focus report, pointer motion without a button,
and a screen-gated key. An input that encodes to no bytes counts, such as a click in a terminal
whose program does not track the pointer: it shows a person is at this terminal. The Host owns
this definition and the record of the last person input, its revision and its time. The Server
keeps no record of its own and learns that a person was there from the refusal.

A refusal writes nothing, reserves nothing, and names the first rule that failed. The Server never
resends a refused key. What it does next depends on the rule:

| Refusal                                      | The Server then                                                                                          |
| -------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Screen changed, input queued, already typed  | Decides again on its next capture, so a prompt that redraws is pressed between two redraws or not at all |
| Person input since the prompt was first seen | Leaves this prompt to the person and shows why                                                           |
| Person input within the quiet time           | Decides again once the quiet time has passed                                                             |
| Agent process gone                           | Decides again on its next capture, which no longer finds the Agent                                       |

**Off stops a decided key.** Turning automatic confirmation off for a run, by the row menu,
Settings, the answer "no", or the repeat stop, sends a **hold**: the same message with no key. The
Host types nothing and records a person's action on the current screen. Every key decided earlier
names a first-seen revision at or before that screen, so rule 6 refuses it however late it
arrives. The Client shows Off when the Host has answered the hold. If the Host does not answer
within the request deadline the Client shows Off anyway: a late key would have to arrive on the
same ordered path that did not carry the hold.

This refines the announced pick "the Server decides and sends": the Server still decides, and the
Host alone is where deciding and writing meet. A Host, Worker, or Server that does not offer the
gate refuses the request, and Orchbee does not press. Nothing falls back to an ungated send. Each
carrier gains one capability name, and neither changes generation.

**Host handover.** The gate identifies a frame by generation and revision, and a terminal that
moves to another Host keeps both. A key decided before the move must not match the moved
terminal. The [Host handover](../../schemas/terminal/references/protocol.md#host-handover) design
therefore has to start a restored terminal with a person-input record at its arrival revision, or
with a new generation. Until it does, a terminal built from a handover record starts with an
unknown record and refuses every screen-gated key until a person has typed there.

Three limits remain, and no observer outside the Agent can remove them:

- The Host knows what the Agent drew, not what the Agent decided and has not drawn yet. If
  something other than a key resolves the prompt, such as the Agent's own timeout or an answer
  from the Agent's phone or IDE companion, in the same instant, the key can reach the next screen
  before that screen is drawn. Rule 3 bounds the window to the Agent's own redraw delay, and the
  one-second settle makes the coincidence unlikely, but it is not zero.
- A program inside the terminal that paints a perfect copy of an Agent's dialog, ending exactly at
  the last row while the Agent process is still the terminal's foreground Agent, receives one
  Enter.
- A Server that starts while a prompt is on screen first sees the prompt then. If a person typed
  on that prompt earlier and the Agent has redrawn since, only the quiet time protects it.

## Model

The decision is a concurrent protocol between four parties that do not share a clock: the Agent
redrawing and reading keys, the person typing, the Server sampling and deciding, and the Host
publishing input. `apps/orchbee-server/formal/tla/AgentConfirmationPress.tla` models them with
every interleaving of redraw, person input, capture, decision and delivery. Messages arrive in any
order, late, or never, and the Server may stop waiting for one that is still on its way. The model
checks:

| Property          | Statement                                                                                                                                                                              |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Matched screen    | Every written key was written while the Host's screen was the routine prompt the Server matched, at the matched revision                                                               |
| Once per prompt   | At most one key is written for one prompt the Agent raised                                                                                                                             |
| Person first      | No key is written once a person typed or clicked after the prompt was first seen, or behind a person's key the Agent has not read                                                      |
| Accept key only   | Every key Orchbee wrote is the accept key of a matched prompt                                                                                                                          |
| Lands where aimed | A key is read by a prompt other than the one it was written for only when that prompt was resolved without a key while the key was unread: the first limit above, stated as a property |
| Off means off     | Once the Host has confirmed Off, no key is written until it is turned on again, whatever was decided before and however late it arrives; and nothing is decided while it is off        |
| Repeats stop      | At most three keys are written for one unchanged prompt identity in one run                                                                                                            |

The model runs as four bounded variants. Three allow one press per prompt identity under every
fault: a person typing and clicking, a key that goes astray between two prompts, and turning off
with keys on their way. The fourth counts presses up to the implementation's limit of three in a
run without faults. Removing the first-seen rule or the quiet time makes Person first fail, and
removing the hold makes Off means off fail. Rule 4 of the gate is a backstop against a faulty
sender; the modeled Server never needs it.

The model treats layout matching, the command check and the path check as one pure function from
a screen to routine, hold, or not a prompt. Those are deterministic transformations and are
covered by rule tests over captured screens and command tables, as
[Modeling and Correctness](../../docs/MODELING-AND-CORRECTNESS.md) assigns them. Durations are
reduced to what they guard: the settle to a count of captures, and the quiet time to a flag that
clears only under the second assumption below.

The model writes two assumptions about Agents into its actions, and a layout is admitted only for
an Agent that keeps them:

- An Agent removes a prompt it drew only by resolving it. The Server therefore starts a new
  prompt only after it has seen the terminal without the pressed prompt, and gives a prompt whose
  identity equals the last pressed one three seconds to settle instead of one.
- Within the quiet time an Agent reads a pending key and draws what that key changed. Without
  this, a person's key that the Agent has read but not yet shown could be followed by Orchbee's.

## Layouts

Chosen. A layout describes one confirmation dialog of one Agent as that Agent draws it. Layouts
live beside the detection manifests, in `apps/orchbee-server/resources/agent-confirmation/`, and
are compiled into the Server. Unlike detection manifests they are never read from a local override
or a downloaded catalog: a rule that can press a key ships only inside a signed release.

- A layout is one anchored pattern over the dialog region: from the Agent's own top border of the
  dialog to the last row of the screen. It names the dialog kind (command, file create, file
  edit), the captured command or path, the row that must carry the highlight, and the accept key.
- Everything the pattern does not describe is a mismatch. Extra rows, a missing footer, another
  highlighted row, a pager hint, a truncation mark, and a danger marker all fail the match.
- Text the Agent takes from the model, such as the command, sits in the dialog's indented body.
  The Agent's own chrome starts in a column that body text cannot reach, so a command cannot draw
  a fake border or option row. Captured text that contains a control character, a box-drawing or
  block character, an invisible or direction-changing character, or one of the Agent's chrome
  glyphs waits.
- Layouts are matched exactly and not pinned to Agent versions (Decided as announced). Evidence
  for each layout is a set of screens captured from the real Agent, kept as test fixtures with the
  Agent version they came from.
- A prompt's identity is its layout, its captured command or path, and its highlighted row. Parts
  an Agent animates are matched but are not identity.

## The command check

The check is the only guard for Codex, OpenCode and Gemini CLI, which mark nothing as dangerous.
It is also needed for Claude Code: its captured `rm -rf` confirmations carried no danger marker.
The check reads text an Agent controls, so it is a trust boundary: it is a pure function with no
I/O, bounded input, and no panic path, and anything it cannot read is a hold.

Decided: the check is a list of what must wait, not a list of what may run. A command word it does
not list is routine; a list of allowed commands would hold nearly every real command.

The check never resolves quoting, escapes, or expansion, so it never has to agree with a shell
about them. It reads the text the same way for POSIX shells, PowerShell and `cmd`:

1. **Characters.** Only printable ASCII and letters or digits of other scripts pass.
2. **Line breaks.** A dialog wraps long commands, so a line break on screen may be a wrap inside a
   word, a wrap at a space, or a real new line that starts another command. Every reading of every
   break is checked, and one failing reading holds the command. More than five breaks hold.
3. **Structure.** The text is split into simple commands at `;`, `&&`, `||`, `|`, `&` and new
   lines wherever they appear, inside quotes too, and each simple command into words at spaces.
   Command substitution, process substitution, a here-document, a trailing line continuation and
   an unclosed quote hold.
4. **Command words.** A command word that contains a quote, a backtick, a caret, a backslash or a
   variable holds: the check cannot say which program it names. A leading assignment such as
   `FOO=1` holds too.
5. **Code given inline** holds: a shell or interpreter with its inline-code option (`sh`, `bash`,
   `zsh`, `dash` or `fish` with `-c`; `pwsh` or `powershell` with `-Command`, `-EncodedCommand`,
   `-File` or any abbreviation of them; `cmd` with `/c` or `/k`; `python`, `python3` or `py` with
   `-c`; `node` with `-e` or `-p`; `perl -e`, `ruby -e`, `php -r`), `Invoke-Expression`, `iex`,
   `eval`, `source`, `.`, `xargs`, `wsl` and `ssh`. A word that runs another command holds as
   well: `env`, `nohup`, `time`, `timeout`, `nice`, `command`, `exec`, `builtin`, `start`, and
   `find` with `-delete`, `-exec`, `-execdir` or `-ok`.
6. **Destructive families** hold: recursive or forced delete, disk overwrite or format,
   `git reset --hard`, `git clean`, a forced push, branch or tag deletion, discarding changes,
   `sudo` and other elevation, piping a download into a shell, `DROP` and `TRUNCATE`, recursive
   permission or owner changes, stopping processes, registry or system changes, and well-known
   tools' own destructive or outward subcommands, such as `docker rm`, `kubectl delete`,
   `terraform destroy`, `gh repo delete`, `aws s3 rm` and `npm publish`. A family names a command
   word and the subcommands and options that make it destructive, wherever they stand, so
   `git -C dir reset --hard` holds. In a family's command, a quote, a caret and a backslash are
   removed from each word before it is compared, so `git "reset" --hard` and `de^l /s` hold while
   `git commit -m "fix"` and `git add src\main.rs` pass. A word with a backtick or a variable
   holds, because those can spell a character the check would not see.
7. **Paths.** An argument or redirection target that names a place outside the Workspace, a
   protected file, a home or variable form such as `~`, `$HOME` or `%USERPROFILE%`, or a `..` step
   holds. The null device is the one exception. Creating a link holds.

Every spelling the families and rule 5 cover is a row of one fixture beside the check: a command,
its shell, and whether it waits. The fixture holds the aliases (`ri`, `rd`, `rmdir`, `del`,
`erase`, `iex`), abbreviated and bundled options, option-before-subcommand forms, and the quoted
and escaped spellings above. The check must agree with every row, and a new spelling is added as
a row first.

What the check can never catch, which product copy must not claim otherwise:

- A script, Makefile target, package script, or test the Agent wrote or changed and then runs.
- An alias, shell function, or program that deletes or overwrites by itself, and a destructive
  command of a tool the fixture does not list.
- A command that reaches outside the Workspace through a link that already exists, or through the
  shell's current directory after an earlier `cd`.
- An Agent that draws a command other than the one it runs.

So the feature is described as "answers routine confirmations" and "destructive commands always
wait", never as making an Agent safe.

## File changes

A file confirmation is allowed when its path stays inside the Workspace and is not protected.

- The path is resolved by its spelling alone, in Windows and POSIX forms, against the working
  directory of the Agent process from the same process sample, and compared with the Workspace's
  directory. A Workspace without a directory, an Agent whose working directory cannot be read, and
  a path only partly on screen hold.
- Protected, wherever they are: anything under `.git`; `.env` and `.env.*`; an Agent's own
  configuration and instruction files (`.claude`, `.codex`, `.gemini`, `.opencode`, `.hermes`,
  `.mcp.json`, `opencode.json`, `CLAUDE.md`, `AGENTS.md`, `GEMINI.md`); and shell start files
  (`.bashrc`, `.bash_profile`, `.profile`, `.zshrc`, `.zprofile`, `.zshenv`, `config.fish`, and
  PowerShell profiles).
- The content of the change is not judged. A change to a file the Workspace then runs is the
  first item of what the check cannot catch.

## State and wire

The Server owns every decision. Clients offer, show, and configure.

**Per Agent run (Server memory, never persisted).** The choice for the run, the count of offers,
the current prompt (identity, first-seen revision, captures, whether a key was sent, whether the
Host reported a person there), and the count of presses per identity. A run ends with its Agent
process, so "this Agent only" ends there, and a Server restart forgets it, which fails closed.

**Saved (user settings).** One Boolean per supported Agent type, `agents.auto_allow.<agent>`,
default false, shown in Settings and effective at once; and `agents.auto_allow.offer`, default
true, set false by the answer "no" and kept in the settings file only.

**Snapshot.** A Presentation whose terminal runs a supported Agent carries one runtime-only field
with the provider, the effective choice (off, run, type; off once the Host answered the hold),
and, when present: an open offer, the reason the current confirmation waits, the last press (kind
and bounded text), and that pressing stopped for the run. Absent means no supported Agent runs
there, so Clients show no entry.

**Requests.** One Workbench command sets the choice for the Agent in a Panel to off, run, or type.
The Server applies the run state and the saved setting together, so the row's three states never
disagree. One command answers an offer, including "no". Both need the grant that navigation
changes need. Settings switches are ordinary setting edits.

**Wait reasons** are a closed list, each with one localized sentence: destructive command, leaves
the Workspace, protected file, command not fully understood, command not fully visible, marked
dangerous by the Agent, not a prompt Orchbee answers, you typed or clicked in this terminal, the
screen keeps changing, the same confirmation keeps coming back, and this terminal's host is too
old to check the screen.

**Failure containment.** A layout that fails to compile fails the build. At run time a screen,
rule, or Client message that cannot be handled yields "wait" and one diagnostic line of the form
`component=agent-confirmation outcome=<kind>`, without command text. The sampler turn that decides
has a fixed time budget; text sent to Clients is bounded and sanitized as every external string
is. No path in this feature ends a process or blocks a terminal's input or output.

## Interface

Both Clients use the same words. English and Simplified Chinese ship together.

| Place                | English                                                                                         | Chinese                                                                      |
| -------------------- | ----------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| Offer                | Let Orchbee allow routine {agent} confirmations?                                                | 让 Orchbee 自动允许 {agent} 的常规确认？                                     |
| Offer answers        | This Agent only · Always for {agent} · No                                                       | 仅此智能体 · {agent} 始终允许 · 不用                                         |
| Row menu             | Allow confirmations: Off · This Agent only · Always for {agent}                                 | 自动允许确认：关闭 · 仅此智能体 · {agent} 始终允许                           |
| After a press        | Allowed: {command}                                                                              | 已允许：{command}                                                            |
| Stopped              | Stopped allowing: the same confirmation keeps coming back.                                      | 已停止自动允许：同一个确认反复出现。                                         |
| Hover reason         | Needs attention · destructive command, waiting for you                                          | 需要处理 · 破坏性命令，等你确认                                              |
| Settings group       | Allow routine confirmations                                                                     | 自动允许常规确认                                                             |
| Settings description | Orchbee presses the Agent's one-time Yes. Destructive commands always wait for you.             | Orchbee 替你按下智能体的单次允许。破坏性命令始终等你确认。                   |
| Settings second line | Orchbee checks the command shown, not what a script does. Request more Agents in a GitHub issue | Orchbee 只检查显示的命令，不检查脚本内容。可在 GitHub issue 中申请更多智能体 |

- **Offer.** A status notice, never a dialog: the person may be typing, and a surface that takes
  the keyboard could turn an Enter meant for the Agent into an answer. The GUI notice carries the
  three answers as buttons. The TUI notice names the key that opens the same three answers as a
  menu, and the Agent row's menu holds them too. The notice stays until answered or until the
  Server withdraws the offer.
- **Row menu.** A submenu with three single-choice items and a mark on the current one, shown only
  for a supported Agent.
- **After a press.** A timed status notice, shown by a Client only while that terminal is in its
  active Tab (Chosen). File changes read `Allowed: create {path}` and `Allowed: edit {path}`.
- **Hover reason.** Added to the state's tooltip, only while automatic confirmation is on for the
  Agent and a confirmation waits.
- **Settings.** In the Agents category, one group with a switch per supported Agent type, the
  description above, and a way to open the issue page where the Client can open a link. Settings
  has no footer in either Client, so the request line is the group's description.

## Slices

Main is released often, so nothing a person can reach changes until slice 5.

| Slice | Content                                                                                                                            | Visible |
| ----- | ---------------------------------------------------------------------------------------------------------------------------------- | ------- |
| 0     | This proposal and the model                                                                                                        | No      |
| 1     | Terminal Host: screen-gated key, its capability, protocol bindings and fixtures                                                    | No      |
| 2     | Worker and Server-to-Worker protocol: the gated send and its outcomes                                                              | No      |
| 3     | Server: layouts, command and path checks, the per-run decision, rule tests over captured Claude Code screens; no way to turn it on | No      |
| 4     | Settings keys, snapshot field, and the two requests, with schemas and bindings; no Client entry                                    | No      |
| 5     | Both Clients together: offer, row menu, notices, hover reason, Settings, both locales. Claude Code only                            | Yes     |
| 6–9   | Codex, OpenCode, Gemini CLI, Hermes Agent: layouts, captured screens, Settings switch, one merge each                              | Yes     |

Each slice updates its owning reference in the same change: the Host runtime model and terminal
protocol reference (1), the Worker send transaction (2), a new Server reference for confirmation
(3), the Workbench schema and settings references (4), and each Client's guide (5).

## Evidence

- The model above, registered with the other protocol models.
- Rule tests: every captured screen of every supported Agent maps to its expected outcome,
  including each declined destructive capture; the command fixture, with the quoted, escaped and
  wrapped spellings; a path table for both path forms.
- Host tests for each refusal rule of the gate, through the real input queue.
- Server tests of the per-run decision with a controlled clock: settle, each refusal and what
  follows it, an unknown outcome, turning off with a key on its way, the repeat stop, and the
  offer count.
- An end-to-end run in a development build for each Agent, in both Clients, in an empty scratch
  folder: a routine command is allowed, a destructive one waits with its reason, and typing stops
  a press.
