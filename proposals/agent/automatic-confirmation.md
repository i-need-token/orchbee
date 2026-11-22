# Automatic Confirmation

| Field             | Value                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Accepted by the product owner on 2026-11-20. Decided restates the owner's decision. Chosen marks this page's own pick, which the owner may veto. Nothing here is implemented until its slice lands.                                                                                                                                                                                                                                                                                                                                                                  |
| Owner             | Maintainers of `apps/orchbee-server`, `apps/terminal-host`, `apps/orchbee-worker`, `crates/terminal-protocol`, `crates/capability-node-protocol`, `crates/workbench-protocol`, `crates/local-runtime`, `apps/orchbee`, and `apps/web`                                                                                                                                                                                                                                                                                                                                |
| Current authority | [Agent screen detection](../../apps/orchbee-server/docs/agent-screen-model.md) owns how a screen becomes an Agent state. [Prompt-gated input](../../apps/terminal-host/docs/runtime-model.md#prompt-gated-input) and the [Worker send transaction](../../apps/orchbee-worker/docs/worker-send-transaction.md) own the existing gated write this design copies. The [terminal protocol](../../schemas/terminal/references/protocol.md) owns Host message kinds. Neither Client offers anything about confirmations today: a waiting Agent shows as needing attention. |
| Exit condition    | Delete this proposal once every slice ships and each rule lives in its owning package reference or standard.                                                                                                                                                                                                                                                                                                                                                                                                                                                         |

An Agent that stops on a routine confirmation, such as "run `cargo test`?", keeps a person at the
keyboard for an answer that is almost always yes. With automatic confirmation turned on, Orchbee
presses the Agent's own one-time accept key for the person. It never answers a confirmation whose
dialog it cannot read completely. It does not judge the command: one that deletes or uploads files
is answered too.

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
  "protected file, waiting for you".
- **Commands are not checked.** Decided by the product owner on 2026-11-21: a check of command text
  cannot be complete, so it gives false assurance. A recognised command confirmation with its
  one-time accept highlighted is answered whatever the command says. No setting changes that, and
  there is no list a person can edit in the first cut (Chosen).
- **File changes inside the Workspace are allowed too.** A file outside the Workspace, or a
  protected file, always waits.
- **First Agents:** Claude Code, Codex, OpenCode, Gemini CLI and Hermes Agent, shipped one at a
  time, Claude Code first.

Chosen: the first cut adds no CLI command and no option beyond these.

## When a key is sent

Orchbee sends exactly one key, the layout's verified accept key, and only when every rule below
holds for the screen the key lands on.

| Rule            | Contract                                                                                                                                                                                                                                   |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Turned on       | Automatic confirmation is on for this Agent run: for the run, or for its Agent type. Turning it off also stops a key that was already decided ([one step](#check-and-write-are-one-step))                                                  |
| Same Agent      | Two process samples around the screen capture select the same Agent process, by process id, creation time and provider, and the Terminal Host finds that process among the terminal's processes when it prepares the key                   |
| Exact layout    | A frame the Server captured for this decision matches one bundled layout of that Agent completely, from the dialog's first row to the last row of the screen. A state the Agent reports about itself never stands in for the capture       |
| One-time accept | The highlighted row is the Agent's one-time accept. Orchbee never moves the highlight and never picks a row that says always, session, or do not ask again                                                                                 |
| Readable        | The whole file path is on screen and contains only plain characters. A command may be wrapped, cut or hidden inside a dialog that is otherwise whole                                                                                       |
| Not marked      | The Agent shows no danger marker of its own                                                                                                                                                                                                |
| Passes check    | A file change passes the [path check](#file-changes); a [command is not judged](#commands)                                                                                                                                                 |
| Settled         | The same prompt has been observed, unchanged in its layout, its command or path, and its highlighted row, for at least one second across at least three captures                                                                           |
| Person is away  | No person typed or clicked in this terminal at or after the revision at which the prompt was first seen, and none in the three seconds before the key. The Terminal Host owns both facts                                                   |
| Once            | No key was written for this prompt before. A send whose outcome is unknown counts as written; a key the Host refused does not                                                                                                              |
| Still the same  | The Terminal Host writes the key only if the screen is still the one the rules read, in the same step as the write ([one step](#check-and-write-are-one-step))                                                                             |
| Not repeating   | The same prompt has not come back unchanged after three presses in a row, counting written and unknown sends. The fourth time Orchbee stops pressing for the run, says so once, and the row's menu shows it off; the person may turn it on |

Any rule that does not hold means the prompt waits for the person, exactly as today. A layout that
does not match, a dialog cut so that its options or its highlight are not on screen, an Agent
danger marker, and a prompt that is not a tool confirmation (trust, login, update, MCP, plan,
question) all wait. An Agent release that changes its wording stops matching by itself
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

The Host's and the Worker's halves are built. The
[protocol reference](../../schemas/terminal/references/protocol.md) owns the message, its rules and
its refusal codes as `ScreenGatedKeyInput`, the
[runtime model](../../apps/terminal-host/docs/runtime-model.md#screen-gated-input) owns how the
Host keeps them, and the
[Worker send transaction](../../apps/orchbee-worker/docs/worker-send-transaction.md#screen-gated-refinement)
owns how a Server send carries the key and the hold; what follows states the design and its
reasons. The Server's rules are built too: its
[confirmation reference](../../apps/orchbee-server/docs/agent-confirmation.md) owns how a dialog is read, which files wait, and what a
run remembers. No Server path calls those rules or builds such a send yet.

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
7. No person input, and no pointer motion that reached the program, was published in the last
   three seconds, by the Host's own clock. The quiet time is a constant of the Host in the
   protocol's constant registry; a sender cannot shorten it.

Rules 3 to 7 are checked when the key is prepared and again at publication, in the same locked
step as the write.

**Person input** is every client input except a focus report, pointer motion without a button,
a screen-gated key, and the cleanup the Host does when an attachment goes away, unless that
cleanup releases a key or a mouse button a person still held. Every send ends with such a
cleanup, the Server's own refused key included, so counting it would let one refusal for a
changing screen stop every later key for the same prompt. An input that encodes to no bytes counts, such as a click in a terminal
whose program does not track the pointer: it shows a person is at this terminal. The Host owns
this definition and the record of the last person input, its revision and its time. The Server
keeps no record of its own and learns that a person was there from the refusal.

**Pointer motion is not person input**, but it delays the key. OpenCode tracks every pointer move
and moves its highlight to the option under the pointer without changing any text, so a move that
reached it just before Orchbee's Enter could turn that Enter into "Always allow" on a frame the
Server had read with "Allow once" highlighted. The Host therefore refuses a key while pointer
motion that encoded to bytes reached the program within the quiet time, with the same refusal as
a recent person input; the Server reads a later frame and decides again, and the prompt stays
Orchbee's to answer. A rule on the frame alone, refusing only motion delivered at or after the
revision the key names, would miss motion delivered just before that frame and drawn just after
it, which the model's counterexample showed, and it would refuse every later key on a frame the
motion did not change. Within the quiet time an Agent draws what a pointer move changed; this is
the model's third assumption, and a layout is admitted only for an Agent that keeps it. Claude
Code, Gemini CLI and Codex on Windows, as captured, do not ask for bare pointer motion, so for
them the rule never fires.

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
gate refuses the request, and Orchbee does not press. Nothing falls back to an ungated send.
Neither carrier changes generation: `client-server` gains `agent-confirmation`, and `worker-host`
gains two names.

**Hosts of 0.1.0-beta.22.** The first Hosts that served the key list `screen-gated-input`, and
0.1.0-beta.22 shipped them before any Server sent one. They record the cleanup of every attachment
that goes away as person input, and every send ends with one, so after one refusal they refuse
every later key for the same prompt. A Host can outlive many updates, so a later Server still meets
them. The corrected rule therefore has its own name, `screen-gated-send`, and a Worker sends a
gated key only to a Host that lists it. `screen-gated-input` stays in the registry and in the
Host's answer, because `worker-host` changes only by addition and the name stays true of the Host;
no Worker offers it. In a terminal such a Host runs, a confirmation waits for the person, and the
reason says that an older Orchbee started the terminal. Nothing offers to restart it: a terminal
moves to a newer Host only by the product's own update path.

**Host handover.** The gate identifies a frame by generation and revision, and a terminal that
moves to another Host keeps both. A key decided before the move must not match the moved
terminal, so the [Host handover](../../schemas/terminal/references/protocol.md#host-handover)
starts a restored terminal with a person's input recorded on its arrival revision, at the time of
its restore, as a hold there would. Every key decided before the move names a first-seen revision
at or before that screen, and rule 6 refuses it however late it arrives; a prompt first seen after
the arrival is answered once the quiet time since the arrival has passed. A prompt already on
screen when its terminal moved is left to the person, and the Server shows the same reason it
shows after a person's input, since the refusal is the same.

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
- A prompt that stays off the screen for more than ten seconds, for example while a person leaves
  its highlight on another option, is a new prompt when it comes back, and only the quiet time
  protects the person's last key. Within ten seconds it keeps the revision it was first seen at.

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
| Accepted once     | No key Orchbee wrote confirms a highlight that a person's key or a pointer move took off the one-time accept                                                                           |
| Lands where aimed | A key is read by a prompt other than the one it was written for only when that prompt was resolved without a key while the key was unread: the first limit above, stated as a property |
| Off means off     | Once the Host has confirmed Off, no key is written until it is turned on again, whatever was decided before and however late it arrives; and nothing is decided while it is off        |
| Repeats stop      | At most three keys are written for one unchanged prompt identity in one run                                                                                                            |

The model runs as five bounded variants. Four allow one press per prompt identity under every
fault: a person typing and clicking, pointer moves that take the highlight away and back, a key
that goes astray between two prompts, and turning off with keys on their way. The fifth counts
presses up to the implementation's limit of three in a run without faults. Removing the
first-seen rule or the quiet time makes Person first fail, removing the quiet time after a
pointer move makes Accepted once fail, and removing the hold makes Off means off fail. Rule 4 of the gate is a backstop against a faulty
sender; the modeled Server never needs it.

The model treats layout matching and the path check as one pure function from a screen to
routine, hold, or not a prompt; a command is not judged. Those are deterministic transformations
and are covered by rule tests over captured screens and path tables, as
[Modeling and Correctness](../../docs/MODELING-AND-CORRECTNESS.md) assigns them. Durations are
reduced to what they guard: the settle to a count of captures, and each quiet time to a flag that
clears only under the second or third assumption below. A person's key or a pointer move may take
the highlight off the one-time accept and a later one bring it back, and a prompt that left the
screen may come back keeping the revision at which it was first seen.

The model writes three assumptions about Agents into its actions, and a layout is admitted only
for an Agent that keeps them:

- An Agent removes a prompt it drew only by resolving it or by moving its highlight. The Server
  therefore starts a new prompt only after it has seen the terminal without the pressed prompt,
  and gives a prompt whose identity equals the last pressed one three seconds to settle instead
  of one.
- Within the quiet time an Agent reads a pending key and draws what that key changed. Without
  this, a person's key that the Agent has read but not yet shown could be followed by Orchbee's.
- Within the quiet time an Agent reads a pending pointer move and draws what that move changed.
  Without this, Orchbee's key could confirm a highlight a move took off the one-time accept.

## Layouts

Chosen and built. A layout describes one confirmation dialog of one Agent as that Agent draws it,
as one anchored pattern from the dialog's top border to the last row of the screen; whatever the
pattern does not describe is a mismatch. The [Server reference](../../apps/orchbee-server/docs/agent-confirmation.md#layouts) owns the format, the
bounds and the admission rule.

Codex draws no border and indents the model's reason and command exactly as it indents its own
title, so that text can draw a title. Its layout starts at the title under a fence: the title
must be the only one on the screen, and a row that starts in the first column, which the model's
text cannot reach, must lie above it. A dialog taller than the screen, or one with a second
title anywhere, therefore waits.

Gemini CLI draws its dialog as a box that starts in the first column and puts the command, or the
new content of a file, in a second box inside it. Every row a model supplies therefore starts
behind two bars and cannot draw a border, the question or an option. In a short terminal Gemini
CLI draws the inner box empty or hides rows of it behind a row that starts with three dots; the
dialog is still whole, so it is read with the rows it shows. With several confirmations
pending, the title ends in a counter such as `1 of 2`, so a written file's path takes no space and
the counter is matched apart from it; otherwise `.env   1 of 2` would not be the protected name.

OpenCode draws the command, or a file's path, behind the same bar and two spaces as its title, so
its title must be the only one on the screen; its box is laid out from the title down, so a dialog
with any row on the screen has its title there. Its options differ only in colour, and a mouse
moving over the dialog with no button pressed selects the option under it with no change to the
text. Chosen: when a screen holds an OpenCode dialog, the Server also reads the screen's styled
rows at the frame of its text, and reads the dialog only while "Allow once" is drawn apart from the
two other options. The styled read already serves Clients, so no protocol changes. The box cuts a
long command without a mark, and the dialog is read with the rows it shows.

- Layouts are compiled into the Server and never read from a local override or a downloaded
  catalog: a rule that can press a key ships only inside a signed release.
- Layouts are matched exactly and not pinned to Agent versions (Decided as announced). Evidence
  for each layout is a set of screens captured from the real Agent, kept as test fixtures.

## Commands

Decided by the product owner on 2026-11-21, replacing the command check this page first chose:
Orchbee does not judge a command. A check of command text cannot be complete. A script the Agent
wrote, an alias, a tool no list names and a place reached through a link all pass such a check, so
it gives false assurance. A command confirmation is answered once a layout reads its dialog
completely with the one-time accept highlighted, whatever the command says, also when the command
is long, wrapped or cut inside that dialog. The copy says so: commands are not checked, and one
that deletes or uploads files runs too.

What still waits: a highlight on anything but the one-time accept, such as "always allow" or "for
this session"; a dialog the Agent itself marks as dangerous; a dialog cut so that its options or
highlight are not on screen; and every approval no layout reads, such as Codex's patch and network
approvals, OpenCode's other permissions and Gemini CLI's replace-in-file confirmation.

## File changes

A file confirmation is allowed when its path stays inside the Workspace and is not protected. The
path is placed by its spelling alone, against the Workspace's directory and the working directory
of the Agent process from the same process sample; the content of the change is not judged. A
Workspace that is the home folder, or holds it, places no file inside: it holds every file that
configures the account, so every file confirmation there waits. The
[Server reference](../../apps/orchbee-server/docs/agent-confirmation.md#path-rules) owns the path forms and the protected names.

## State and wire

The Server owns every decision. Clients offer, show, and configure.

**Per Agent run (Server memory, never persisted).** The choice for the run, the count of offers,
the current prompt (identity, first-seen revision, captures, whether a key was sent, whether the
Host reported a person there), and the count of presses of one prompt in a row. A run ends with
its Agent
process, so "this Agent only" ends there, and a Server restart forgets it, which fails closed.

**Saved (user settings).** One Boolean per supported Agent type, `agents.auto_allow.<agent>`,
default false, shown in Settings and effective at once; and `agents.auto_allow.offer`, default
true, set false by the answer "no" and kept in the settings file only.

**Rows.** A Client reads one row per Presentation with a Panel Agent action that a State stream
may follow. The row of a Presentation whose terminal runs a supported Agent carries the provider,
the effective choice (off, run, type; off once the Host answered the hold), and, when present: an
open offer, the reason the current confirmation waits, the last press (a serial number, its kind
and bounded text), and that pressing stopped for the run. A row without these means no supported
Agent runs there, so Clients show no entry. The rows are run state, never part of the Workbench
snapshot: they change without a commit and are forgotten with the run.

**Requests.** One Panel Agent action sets the choice for the Agent in a Presentation to off, run,
or type. The Server applies the run state and the saved setting together, so the row's three
states never disagree. One action answers an offer with "no". Both need the grant that navigation
changes need. Settings switches are ordinary setting edits.

**Compatibility.** The three actions are sent only to a Server whose handshake answer lists the
`client-server` capability `agent-confirmation`. A Client without it, or connected to a Server
without it, shows nothing of the feature, and such a Server never presses: nothing can turn it on.
The built contract is in the [Server reference](../../apps/orchbee-server/docs/agent-confirmation.md)
and the Workbench schemas.

**Wait reasons** are a closed list, each with one localized sentence: file outside the Workspace,
Workspace includes the home folder, protected file, path not understood, path not fully visible, marked dangerous by the Agent, not a prompt Orchbee answers, you typed or clicked in this terminal, the
screen keeps changing, the same confirmation keeps coming back, Orchbee answered but it is still
on screen, and an older Orchbee started this
terminal, said once the Worker has refused a send because the terminal's Host does not list
`screen-gated-send`.

**Failure containment.** A layout that fails to compile fails the build. At run time a screen,
rule, or Client message that cannot be handled yields "wait" and one diagnostic line of the form
`component=agent-confirmation outcome=<kind>`, without command text. The sampler turn that decides
has a fixed time budget; text sent to Clients is bounded and sanitized as every external string
is. No path in this feature ends a process or blocks a terminal's input or output.

## Interface

Both Clients use the same words. English and Simplified Chinese ship together.

| Place                | English                                                                                   | Chinese                                                     |
| -------------------- | ----------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| Offer                | Let Orchbee allow routine {agent} confirmations?                                          | 让 Orchbee 自动允许 {agent} 的常规确认？                    |
| Offer answers        | This Agent only · Always for {agent} · No                                                 | 仅此智能体 · {agent} 始终允许 · 不用                        |
| Row menu             | Allow confirmations: Off · This Agent only · Always for {agent}                           | 自动允许确认：关闭 · 仅此智能体 · {agent} 始终允许          |
| After a press        | Allowed: {command}                                                                        | 已允许：{command}                                           |
| Stopped              | Stopped allowing: the same confirmation keeps coming back.                                | 已停止自动允许：同一个确认反复出现。                        |
| Hover reason         | Needs attention · protected file, waiting for you                                         | 需要处理 · 受保护的文件，等你确认                           |
| Settings group       | Allow routine confirmations                                                               | 自动允许常规确认                                            |
| Change not applied   | Couldn't apply the change.                                                                | 没能应用更改。                                              |
| Settings description | Presses the Agent's allow-once for you.                                                   | 替你按下智能体的单次允许。                                  |
| Settings switch      | {agent}, described: Commands are not checked: one that deletes or uploads files runs too. | {agent}，说明：不检查命令内容，删除、上传等命令同样会执行。 |
| Settings last line   | Request more Agents in a GitHub issue.                                                    | 可在 GitHub issue 中申请更多智能体。                        |

- **Offer.** A status notice, never a dialog: the person may be typing, and a surface that takes
  the keyboard could turn an Enter meant for the Agent into an answer. The GUI notice carries the
  three answers as buttons, and a pointer press on one leaves the focus in the terminal. The TUI
  notice names the key that opens the same three answers as a menu (`Ctrl+b y` by default, which
  later opens the choices for the Agent in view), and the Agent row's menu holds the choices too.
  The notice stays until answered or until the Server withdraws the offer, shows only while no
  other notice does, and is about the focused Panel's Agent first, then another Agent the active
  Tab shows (Chosen).
- **Row menu.** A submenu with three single-choice items and a mark on the current one, shown only
  for a supported Agent.
- **After a press.** A timed status notice, shown by a Client only while that terminal is in its
  active Tab (Chosen). File changes read `Allowed: create {path}` and `Allowed: edit {path}`. A
  press made before a Client first read the terminal's row is not said, a newer press replaces an
  older one still showing, and no press replaces another notice (Chosen).
- **Change not applied.** The Server owns the choice, so a Client shows a new choice only once the
  Server reports it; a request the Server did not apply says so in a timed status notice.
- **Hover reason.** Added to the state's tooltip, only while automatic confirmation is on for the
  Agent and a confirmation waits.
- **Settings.** In the Agents category, one group with the description above, a switch per
  supported Agent type named by the Agent's product name, and a closing line under the switches
  that asks for more Agents. The GUI's closing line opens the project's GitHub issue page where
  the window can open a link; the TUI's is text (Chosen). The offer's own switch,
  `agents.auto_allow.offer`, is a key of the settings file that no page shows: a "no" turns it
  off, and deleting the key brings the offer back.

## Slices

Main is released often, so nothing a person can reach changes until slice 5. Slices 4 and 5 land
in one push: a saved setting that turns pressing on must never exist without both Clients showing
it.

| Slice | Content                                                                                                                   | Visible |
| ----- | ------------------------------------------------------------------------------------------------------------------------- | ------- |
| 0     | This proposal and the model                                                                                               | No      |
| 1     | Terminal Host: screen-gated key, its capability, protocol bindings and fixtures                                           | No      |
| 2     | Worker and Server-to-Worker protocol: the gated send and its outcomes                                                     | No      |
| 3     | Server: layouts, the path check, the per-run decision, rule tests over captured Claude Code screens; no way to turn it on | No      |
| 4     | Settings keys, the row read and the two requests behind their capability, with schemas and bindings; the sampler wiring   | No      |
| 5     | Both Clients together: offer, row menu, notices, hover reason, Settings, both locales. Claude Code only                   | Yes     |
| 6–9   | Codex, OpenCode, Gemini CLI, Hermes Agent: layouts, captured screens, Settings switch, one merge each                     | Yes     |

Codex is answered for its shell command confirmation only, which also carries the files it writes
through PowerShell.

Gemini CLI is answered for its shell command confirmation and for its whole-file write, which
creates a file or replaces one and is read as an edit of the path in the dialog's title. Its
replace-in-file confirmation waits: the title joins the path and two pieces of the model's text
with `: `, which a folder name may contain on macOS, so the path cannot be told from the text.

OpenCode is answered for its shell command confirmation and for its file edit, which also covers a
file written whole. Every other permission waits. OpenCode runs a session's tools in that
session's folder and writes paths from it; a session of another folder opened from inside OpenCode
is judged as working in the Agent's folder.

Hermes Agent waits for a product-owner decision. In its default configuration it asks nothing
before a routine command; the only confirmation it draws is its own approval of a command it
calls dangerous, which this design never answers.

Each slice updates its owning reference in the same change: the Host runtime model and terminal
protocol reference (1), the Worker send transaction (2), a new Server reference for confirmation
(3), the Workbench schema and settings references (4), and each Client's guide (5).

## Evidence

- The model above, registered with the other protocol models.
- Rule tests: every captured screen of every supported Agent maps to its expected outcome,
  including each delete it answers and each cut command; a path table for both path forms.
- Host tests for each refusal rule of the gate, through the real input queue.
- Server tests of the per-run decision with a controlled clock: settle, each refusal and what
  follows it, an unknown outcome, turning off with a key on its way, the repeat stop, and the
  offer count.
- An end-to-end run in a development build for each Agent, in both Clients, in an empty scratch
  folder: a command is allowed, a protected file waits with its reason, and typing stops a press.
