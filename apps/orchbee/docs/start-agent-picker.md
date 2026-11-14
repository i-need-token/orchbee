# Start Agent picker

## Scope and ownership

The Start Agent picker is an input-capturing overlay in the
[native Tab Session](workbench-client-runtime.md) that starts an Agent from the Session's Agent
catalog. The Server owns every Agent fact — the catalog, session identity, provider, status and
presentation bindings. The picker is a Client startup surface: it hands a typed request to the
private startup workflow or to the Server's Tab creation rather than synthesizing a session.
Maintain this reference with the private `agent_board/` module, its narrow access points into
`client_view`, `ui`, `tui_render`, `tui_pointer` and the dispatcher, and their correctness evidence.

One action, `agent.start`, opens the picker: `Ctrl+b Shift+G`, the action menu's
**Start Agent…**, the status line's start Agent hint, and the new-Tab button's More Agents…, which
opens it with new Tab as its place. Pressing `Ctrl+b Shift+G` again closes it. That menu's Agent
rows start directly instead: they list up to four of this picker's Recent then Installed providers,
fixed when the menu opens, and start one as the new-Tab place does, with the same notices
([action menus](native-action-menus.md)). Modal or confirmation ownership refuses opening.

Opening creates a stable episode and claims the top input slot before the first paint, so no
keystroke falls through to the terminal shell. The first evaluated draw either admits that episode
or refuses it when geometry cannot present the surface. A refused new episode closes and reports the
refusal separately. Once admitted, a temporary small viewport suspends presentation instead: the
query, cursor, place and already accepted work remain owned for the same episode. While covered or
suspended, ordinary keys, paste, wheel and activation are consumed without an effect; `Esc` remains
the local close route. After modal and confirmation owners decline a configured prefix, its binding
runs through the normal Client action owner: `Ctrl+b Shift+G` closes the picker, and other prefix
actions close or replace it through their destination. A doubled prefix is consumed by the picker
instead of reaching the covered terminal. A later complete paint resumes the episode, while a
receipt from an older episode can neither reopen it nor restore input authority. Both ownership
edges — open and close — invalidate the published pointer hit map, so a click can never land on a
vanished control or pass through the overlay onto a stale foreground region.

The picker is a Medium dialog of 56 content cells: the title with its close control, where the
Agent opens, a search field, a divider, the grouped list, a status row only while there is something
to report (the lookup running, a lookup that could not check, no match, or a host or Session that
cannot start Agents) and the hint row `Esc close │ Enter start │ ↑↓ select │ Tab place`. Typing
filters at once by product name, command or provider id. `Esc` first clears a query, then closes;
the close control acts like `Esc` with an empty query, and an outside press closes the picker. No
key reaches the Panel.

Outside the TUI, `orchbee agent start <provider>` is the scriptable entry. It takes the picker's
places as flags beside a reference Panel — `--new-tab`, its default, `--split right|down` or
`--here` — and uses the same Tab creation, split and catalog id as the picker, so an exact catalog
start in a new Panel types nothing; it adds the named start the picker does not offer. See
[Windows Agent startup](windows-agent-start.md#invocation-and-prerequisites).

The list comes from the Session's [Agent catalog](../../orchbee-server/README.md#agent-catalog),
which the Server keeps for the machine the Session runs on; the TUI looks at no `PATH` of its own.
While connected the Client holds one State stream of that catalog, which the Server pushes on every
change, so nothing is polled. Each opening of the picker asks the Server to look again, and the
list updates when the new lookup's results arrive; opening the new-Tab button's menu never asks.
Before the first catalog arrives the picker lists nothing and says the lookup is running. A stream
that fails keeps the last catalog and opens again five seconds later. When the stream fails or is
refused before any catalog arrived, or the Session cannot start Agents, the picker says starting is
not supported and the new-Tab menu has no Agent rows.

Groups follow the list order, by the display rule both Clients share
([rows corpus](../../../schemas/workbench/README.md)): **Recent** holds the first three of the
catalog's recent providers that were found; **Installed** holds every other found provider, A–Z by
product name; the rest fold into one row that expands on `Enter` or a click and scrolls the N
providers into view with the cursor on the first. The row says what it holds: "N more not found on
this computer" when every folded provider is not found, "N more couldn't be checked" when none is,
and "N more not found or not checked" for some of each. Opened, or listed by a query, the group's
heading follows the rows it lists the same way: "Not found", "Not checked", or "Not found or not
checked". A provider in Recent never repeats in another group. Any query also searches the folded
providers. Each row shows the product name and, in secondary text, its command, "not found", or
"can't check" for a command the Server cannot check, such as a generic one (`agent` for Cursor,
`snow` for Snow CLI), or one a completed lookup could not settle. The cursor starts on the most
recent provider, else the first installed one, and a later catalog never moves it off the provider
it rests on. While a lookup runs, a provider it is still checking is listed nowhere: there is no
folded row, a query or an opened fold lists the providers already known, and the status row says
the lookup is running. A later lookup keeps the last rows while it runs or if it fails. When no
lookup has ever completed on the Session's machine and the last one failed or cannot run there, the
status row says "Couldn't check installed Agents" and every provider not known to be missing is
listed under "Not checked on this machine" A–Z, after Recent.

Installed is a hint, not a promise. The lookup searches the `PATH` a new shell on that machine
starts with, before its profile runs: a version manager's shim directory stored in the system or
user `PATH` is seen, but a directory that only a shell profile adds is not. An Agent listed as not
found can still be started from its row, and once it runs, the Server corrects its row to found
([Worker probe](../../orchbee-worker/README.md#agent-probe-job)).

There is no name field. `Enter` or a row click, against the current successful draw, closes the
picker on the same frame and starts the provider; only new Worktree keeps the picker under the form
it opens. A terminal's Enter arrives as a bare CR, the canonical `Ctrl+M`, and starts the same way.
A start the picker cannot make leaves it open and says why: the status row on a host or Session that
cannot start Agents, otherwise "Agent did not start" in the status line.

The place row names where the Agent opens before `Enter`. The place is automatic and nothing is
asked: the focused Panel when its shell is idle — no Agent the sidebar shows holds it and none of
this Client's starts guards it — else a new Tab. `Tab` moves to the next offered place and
`Shift+Tab` to the previous one, wrapping, in the order this Panel, new Tab, split right, split
down, new Worktree; a click on the place row acts like `Tab`. This Panel is offered only when it is
the automatic place, the two splits only while a Panel is focused, and new Worktree only while the
[New Worktree form](native-worktrees.md) could open a fresh create for the focused Workspace, under
the refusals `Ctrl+b Shift+W` meets; the offer is fixed when the picker opens. The chosen place holds
while the picker stays open, also across filtering, and is forgotten when it closes. The place row
is never removed under height pressure; a row too narrow for the label and the place drops the
label "Opens in" first, then truncates the place name, and the hint row drops `Tab place` before
its other hints.

In the focused Panel the provider is typed through the same single-submission workflow as the CLI,
which keeps its named start. A new Tab comes from the ordinary Tab creation in the focused
Workspace with the provider's catalog id: the Server launches the Tab's shell with the Agent as its
first command, so nothing is typed, the Panel shows the Agent's own first screen, and the shell
stays when the Agent exits. A split goes through the split keys' own action on the focused Panel
with the same catalog id, so their admission, refusals and notices apply unchanged; a refusal
leaves the picker closed. New Worktree opens the New Worktree form over the picker with the branch
`agent/<provider id>` prefilled; nothing starts before the form's own `Enter`, and `Esc` returns to
the picker with its query, cursor and place. The submitted create carries the same catalog id and
closes the picker; the Server launches the new Workspace's first shell with the Agent as its first
command. A failed create shows the form's own failure, and a reused Workspace starts nothing. The
Client asks once, never polls and runs no workflow for a new Panel: the Server
[delivers the start](../../orchbee-server/docs/agent-start-delivery.md), watches it, and publishes a
one-shot `agentStartFailure` on the new Presentation when it did not happen. The Client reads that
fact from the snapshots it already receives, as it reads a resume fact, and shows it whichever Panel
has focus: once per Presentation and time, only within 60 seconds of it by the Client's clock, and
never for a fact found before this Client attached, so attaching or reconnecting repeats nothing.
`not-found` and `exited` show the notices below, `unsupported-shell` — a shell such as a custom
launch profile that takes no start command — shows "Codex not started · this shell takes no start
command", and `prompt-unconfirmed` and `terminal-too-old` show the not-at-prompt and older-terminal
"Nothing was typed" notices. A Host refusal because the focused Panel is no longer at an idle prompt
starts in a new Tab instead and says "Panel busy · opened a new Tab". The workflow never installs a
provider, inspects an integration or changes configuration. Success shows no notice; the Panel's
Agent row shows the Agent opening. A failure is a short status notice, such as
"Codex not found · Ctrl+b Shift+G choose another" or "Codex exited · see its Tab"; the key it names
is the effective Start Agent binding's label, and its shorter form drops the key. A provider not
found is listed under not found afterwards: the Server marks its catalog row for a new Panel, and
the Client marks it for the focused Panel until the connection ends. Recent follows the catalog,
which moves a provider to the front when one of its runs starts, typed or not. See
[Windows Agent startup](windows-agent-start.md) for readiness and target requirements.

The catalog stream belongs to the UI loop, like the terminal title stream, and runs no thread of
its own. A new origin or Session drops the held catalog and the stream follows the new one;
connection release closes the stream and drops the catalog.

The successful surface frame grants a same-episode visibility lease for owner-local keys.
Therefore consecutive arrow keys can advance the cursor in one input packet even after the first
step invalidates its old draw proof. Starting a provider and applying a row, wheel or control effect
still require an exact current draw; they wait for the next complete frame rather than acting on an
unseen cursor.

The row, place and close controls use the shared action grammar. The localized hint line uses atomic
key and verb items, ordered with `Esc` first, so narrow layouts drop only complete trailing hints. A
control publishes the stable episode, the list's revision, the cursor and the viewport offset from
the successful frame, and invokes only after a matching release inside the still-painted target.
Keyboard input, focus loss, changed geometry or another input owner cancels an armed control. List
wheel uses three rows per notch, coalesces at one viewport per pass and keeps the first new notch on
reversal.

## State and transition contract

The startup owner keeps at most one startup task per Panel. An accepted task pins its generation and
exact Panel, continues when the picker closes, and settles only a matching completion; another
Panel's start never blocks this one. Session teardown wakes and joins every bounded task before
releasing its origin. A disconnected accepted worker, a pending or unknown reservation, accepted or
partially accepted input, and an unknown input frontier remain unknown and guard their Panel until
the workflow deadline. A result that proves zero input and a settled reservation, or observes the
shell prompt again, ends the guard at once. A start refused at a prompt the Host could not confirm,
or by a Session or terminal too old to confirm one, typed nothing
([prompt-gated input](windows-agent-start.md#prompt-gated-input)); the next choice in the picker
starts afresh, since there is no retry state.

| Transition  | Guard and result                                                                                                                                                                                                                                                                                                                                                          |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Open        | A modal- and confirmation-free Client admits a fresh episode, computes the offered places and asks the Server for a new catalog lookup.                                                                                                                                                                                                                                   |
| Draw settle | The first evaluated paint either admits the episode or refuses it. A later geometry block suspends an admitted episode without destroying the query, cursor, place or accepted work; cover revokes current-frame visibility and exact activation proof. Resume requires a complete exact-episode paint. A stale receipt cannot mutate or reopen another episode.          |
| Start Agent | A current successful picker draw admits one canonical provider and the shown place: the focused idle Panel unless a running or unknown start guards it (then a new Tab), or a new Tab or split whose shell runs the Agent as its first command. The workflow submits input at most once and ends after its early check; a generation/Panel mismatch drops the completion. |
| Close       | Invalidate the episode. Accepted startup tasks remain Session-owned and keep running; only Session teardown wakes them.                                                                                                                                                                                                                                                   |

## Correctness evidence

The owning implementation tests cover exact-episode input isolation, first-draw refusal, admitted
suspension and resume, stale draw rejection, every case of the shared Agent start rows corpus, the
cursor held across catalogs, the new-Tab menu's rows, `Esc` order, the offered places and their
`Tab` cycle, new Worktree's form opening over the picker and `Esc` restoring it through the raw-byte
route, keyboard and pointer startup against the current draw, lost-worker recovery, the Panel-scoped
start guard, observed-readiness admission and both locales.

Maintain the concrete witnesses in `crates/workbench-client/src/agent_board/tests.rs`,
`crates/workbench-client/src/agent_board/picker/tests.rs`,
`crates/workbench-client/src/workbench_client/ui/tests/agent_board/` and the adjacent state,
carrier, input and rendering suites. Use controlled completion/clock order and generated trace
replay where the workflow has multiple outcomes.

Run `cargo test --locked -p orchbee --all-targets --all-features`. This workflow has no separate TLA
model. The evidence covers named/generated implementation traces; it does not establish exhaustive
interleavings, physical terminal appearance or untested platforms. Shared identity, ownership and
commit protocols retain their separately registered core models.
