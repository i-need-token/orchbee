# Agent board

## Scope and ownership

The Agent board is an input-capturing overlay in the
[native Tab Session](workbench-client-runtime.md) that lists Server-owned Agent sessions and jumps to an
Agent's already-existing Panel. The Server owns every Agent fact — session identity, provider,
status, canonical freshness, attention state and presentation bindings. The board is a Client
projection, navigation and startup surface: rows never infer Agent state from terminal pixels,
and the Start Agent picker hands a typed request to the private startup workflow rather than
synthesizing a session. Maintain this reference with the private
`agent_board/` module, its narrow access points into `client_view`, `ui`,
`tui_render`, `tui_pointer` and the dispatcher, the OBAP read lane, and their correctness evidence.

Each row uses the shared six-state presentation: a stationary white-to-gray breathing `⬢` for working (two seconds), a pulsing yellow `⬢` for attention, green `⬢` for succeeded, red `⬢` for failed/canceled/interrupted, gray `⬡` for idle and gray `•` for unknown. The Board shares the Sidebar animation phase; it also requests animation when only its own rows need it. ASCII and monochrome keep distinct outcome glyphs. A separate trailing blue dot reflects this Client’s unread observation. Reading clears only that dot; selected-row backgrounds preserve semantic foregrounds. Freshness remains a separate label.

`Ctrl+B g` toggles the board. Opening creates a stable episode and claims the top input slot before
the first paint, so no keystroke falls through to the terminal shell. The first evaluated draw
either admits that episode or refuses it when geometry cannot present the surface. A refused new
episode closes and reports the refusal separately. Once admitted, a temporary small viewport
suspends presentation instead: rows, selection, manual reading position and already accepted work
remain owned for the same episode. While covered or suspended, ordinary keys, paste, wheel and
activation are consumed without an effect; `Esc` remains the board's local close route. After modal
and confirmation owners decline a configured prefix, its binding runs through the normal Client
action owner: `Ctrl+B g` closes the board, and other prefix actions close or replace it through their
destination. A doubled prefix is consumed by the board instead of reaching the covered terminal. A
later complete paint resumes the episode, while a receipt from an older episode can neither reopen
it nor restore input authority. Accepted lane work may settle into the same hidden episode, but staged
jump navigation advances only while the compositor still names the board as the top native owner and
its same-episode visibility lease is live. It never moves Workbench focus behind a covering surface,
including the interval after a child opens and before the next cover receipt.

While interactive, Up/Down and `j`/`k` move the logical selection by opaque Agent session id and
reveal it, `Enter` requests a jump only when that selection is in the exact painted viewport, `r`
requests a refresh, `s` opens the Start Agent picker, and `Esc` or an ordinary backdrop press
closes. Up from the first row selects the **+ Start Agent** row, where `Enter` opens the picker;
with no rows that row is the selection. A row press selects, while the
inner wheel scrolls the reading viewport without changing selection. The outer wheel, unknown keys
and paste are consumed; nothing reaches the terminal shell. Modal or confirmation ownership
refuses opening; prefix chords still resolve, so `Ctrl+B g` closes the board too. Both ownership
edges — open and close — invalidate the published pointer hit map, so a click can never land on a
vanished board row or pass through the overlay onto a stale foreground region. Notifications,
task sending, scheduling and persistence are excluded.

One action, `agent.start`, opens the Start Agent picker: `Ctrl+B G`, the action menu's
**Start Agent…**, the Board's always-present **+ Start Agent** row above its sessions, `s` anywhere
in the Board, the empty Workspace hint beside new Tab, and the new-Tab button's More Agents…, which
opens it with new Tab as its place. That menu's Agent rows start directly instead: they list up to
four of this picker's Recent then Installed providers, fixed when the menu opens, and start one as
the new-Tab place does, with the same watch, notices and first-prompt fallback
([action menus](native-action-menus.md)). From the Board the picker replaces the
Board's content in the same owner, episode and hit map; `Ctrl+B G` opens it alone without a Board
read. It is a Medium dialog of 56 content cells: the title with its close control, where the Agent
opens, a search field, a divider, the grouped list, a status row only while there is something to
report (the lookup running, no match, or a host that cannot start Agents) and the hint row
`Esc close │ Enter start │ ↑↓ select │ Tab place`. Typing filters at once by product name, command
or provider id. `Esc` first clears a query, then returns to the Board when the picker came from it,
else closes; the close control acts like `Esc` with an empty query, and an outside press closes the
whole owner. No key reaches the Panel.

Groups follow the list order: **Recent** holds at most three installed providers, most recent
first; **Installed** follows A–Z; the rest fold into one row, "N more not found on this computer",
that expands on `Enter` or a click and scrolls the N providers into view with the cursor on the
first. Any query also searches the folded providers. Each row shows the
product name and, in secondary text, its command, "not found", or "can't check" for a command the
process-identity registry classes as generic (`agent` for Cursor, `snow` for Snow CLI); generic
commands are never advertised as found and sit with the not-found providers. The cursor starts on
the most recent provider, else the first installed one. Recent is computed in the Client from this
Client's own starts, newest first, then the Session's Agents by their latest state sequence, one
entry per provider; nothing is persisted. A remote Session performs no local PATH detection and
lists Recent, then every provider under "Not checked on this machine" A–Z.

On Windows each opening repeats the lookup of mapped commands on the Client's inherited `PATH`
using fixed System32 `where.exe` helpers, keeping the previous list until it completes. Opening the
new-Tab button's menu begins the first lookup when none has run, and never repeats one. At most
four owned workers run concurrently, with a six-second shared deadline, a 900 ms lookup slot and
bounded Job termination. Results publish one at a time; a missing, failed or timed-out lookup never
discards other results, and an incomplete first lookup claims nothing installed. Until the first
lookup completes, a provider it has not found yet is still being checked: it is listed nowhere and
never counted as not found, the folded row waits for every check, and the status row says the
lookup is running. A late result never moves the cursor off the provider it rests on. Found commands are
hints about this Client's PATH, not guarantees about the target terminal.

There is no name field. `Enter` or a row click, against the current successful draw, closes the
picker on the same frame and starts the provider; only new Worktree keeps the picker under the form
it opens. A terminal's Enter arrives as a bare CR, the
canonical `Ctrl+M`, and starts the same way. A start the picker cannot make leaves it open and says
why: the status row on a host that cannot start Agents, otherwise "Agent did not start" in the status
line.

The place row names where the Agent opens before `Enter`. The place is automatic and nothing is
asked: the focused Panel when its shell is idle — no known Agent holds it and none of this Client's
starts guards it — else a new Tab. `Tab` moves to the next offered place and `Shift+Tab` to the
previous one, wrapping, in the order this Panel, new Tab, split right, split down, new Worktree; a
click on the place row acts like `Tab`. This Panel is offered only when it is the automatic place,
the two splits only while a Panel is focused, and new Worktree only while the
[New Worktree form](native-worktrees.md) could open a fresh create for the focused Workspace, under
the refusals `Ctrl+B W` meets; the offer is fixed when the picker opens. The chosen place holds
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
Client asks once and never polls; when the creation or split commits, or the create's Opened result
arrives, it watches the new Panel the way a typed start is watched. A shell that cannot take a first
command, such as cmd or a custom launch profile, gets the typed start at its first prompt instead. A
Host refusal because the focused Panel is no longer at an idle prompt starts in a new Tab instead
and says "Panel busy · opened a new Tab". The workflow
never installs a provider, inspects an integration or changes configuration. Success shows no
notice; the Panel's Agent row shows the Agent opening. A failure is a short status notice, such as
"Codex not found · Ctrl+B G choose another" or "Codex exited · see its Tab", and a provider not
found is listed under not found afterwards. See [Windows Agent startup](windows-agent-start.md)
for readiness and target requirements.

An empty incomplete read is labeled as partial rather than as proof that no sessions exist.

Closing or superseding a catalog cancels its owner and moves it to a retirement queue. The input
thread reaps that owner only after it has exited. Session teardown owns the final join; application
cancellation cannot shorten the initial Windows process-creation and Job-assignment calls.

The successful surface frame grants a same-episode visibility lease for owner-local navigation and
refresh. Therefore consecutive arrow or `j`/`k` keys can advance the cursor in one input packet even
after the first step invalidates its old row proof. Starting a jump and applying a row, wheel or
control effect still require an exact current draw; they wait for the next complete frame rather than
acting on an unseen cursor.

Visible Open, Refresh, Start Agent and Close controls use the shared action grammar. The localized shortcut line
uses atomic key and verb items, ordered with `Esc` first, so narrow layouts drop only complete
trailing hints. The header also has a close control. A row or control publishes the stable episode,
row-source revision, logical selection, jump state, viewport offset and visible-row proof from the
successful frame. A row press validates that proof and moves the provisional selection immediately;
a control invokes only after a matching release inside the still-painted target. Keyboard input,
focus loss, changed geometry or another input owner cancels an armed control. List wheel uses three
rows per notch, coalesces at one viewport per pass and keeps the first new notch on reversal. Wheel
never changes the logical selection; the next keyboard navigation reveals that selection.

## State and transition contract

The board runs one read-lane thread per open epoch: a single active operation with one coalesced pending
intent. A jump request replaces a pending refresh and supersedes an in-flight jump immediately —
the logical jump is dead the moment the serial advances, whatever stage it reached. The superseded
`Get` is different: its outcome may still be on the wire, so it keeps the physical lane credit
until that outcome drains or the lane exits, and the newer jump's `Get` waits behind it. A stale
outcome releases only the credit it held — it can never clear or displace the newer jump. A
refresh request while a jump is pending is consumed without displacing it. Every completed List
attempt — success, incomplete or failed — starts a two-second cooldown that `r` cannot bypass;
the cooldown survives close and reopen, so a refresh requested at reopen still waits out the
residual. If the lane thread dies while a request is outstanding, an outcome that was already
queued and still matches applies first; a request whose outcome can never arrive releases its
credit honestly — a jump fails `Unavailable`, a List settles as a failed round — and the board
never hangs in flight.

The startup owner is separate from that read lane. It keeps one catalog task and at most one
startup task per Panel. An accepted task pins its generation and exact Panel, continues when the
overlay closes, and settles only a matching completion; another Panel's start never blocks this
one. Session teardown wakes and joins every bounded task before releasing its origin. A
disconnected accepted worker, a pending or unknown reservation, accepted or partially accepted
input, and an unknown input frontier remain unknown and guard their Panel until the workflow
deadline. A result that proves zero input and a settled reservation, or observes the shell prompt
again, ends the guard at once. A start refused at a prompt the Host could not confirm, or by a
Session or terminal too old to confirm one, typed nothing
([prompt-gated input](windows-agent-start.md#prompt-gated-input)); the next choice in the picker
starts afresh, since there is no retry state.

| Transition     | Guard and result                                                                                                                                                                                                                                                                                                                                                          |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Open           | Modal/confirmation-free board admits a fresh epoch and one lane thread; a previous lane must have actually exited first. Last-good rows, their install age and the residual cooldown persist across the reopen; selection is repaired by opaque id against the preserved rows.                                                                                            |
| Draw settle    | The first evaluated paint either admits the episode or refuses it. A later geometry block suspends an admitted episode without destroying rows, selection, viewport or accepted work; cover revokes current-frame visibility and exact activation proof. Resume requires a complete exact-episode paint. A stale receipt cannot mutate or reopen another episode.         |
| List round     | One bounded walk: at most six same-revision pages under one absolute deadline, one re-walk on a stale cursor, twelve exchanges and 12 MiB total.                                                                                                                                                                                                                          |
| Settle read    | A same-revision full walk is complete and becomes `last_good`; an exhausted budget or deadline is incomplete or failed — never a masquerading empty success, and never a rewrite of `last_good`'s install age.                                                                                                                                                            |
| Request jump   | Select by opaque session id; a lane `Get` revalidates the target after the current bounded List under its own deadline and 128 KiB bound. A newer request supersedes the logical jump immediately while the superseded `Get` drains its lane credit.                                                                                                                      |
| Resolve stages | Re-resolve context, then Tab, then Panel against current canonical identity at every stage; same-context attached viewers focus locally, otherwise attach/switch. If no canonical snapshot has fed the presentation index yet, resolution waits inside the jump's own deadline and asks the session for a refresh — an unfed index is not proof of `Gone`.                |
| Land           | Install the exact requested Panel including non-zero indices; a retired or missing target reports `TargetGone` and never falls back. A target in the already-active Tab whose viewer is dead reattaches in place — a fresh-generation switch at the same position, by stable Panel identity — while a live same-Tab target stays a no-op.                                 |
| Start Agent    | A current successful picker draw admits one canonical provider and the shown place: the focused idle Panel unless a running or unknown start guards it (then a new Tab), or a new Tab or split whose shell runs the Agent as its first command. The workflow submits input at most once and ends after its early check; a generation/Panel mismatch drops the completion. |
| Close          | Invalidate the board generation, cancel read/jump intents and reclaim that lane after actual exit. Accepted startup tasks remain Session-owned and keep running; only Session teardown wakes them.                                                                                                                                                                        |

Every Agent the sidebar shows for the Session is a row, whatever started it. An Agent without a
canonical session — one started without a hook integration, by the picker, the CLI or by hand,
with or without a name — is listed from the sidebar's report for its Panel: the sidebar's state
mapped onto the six-state presentation, its Presentation as the only attachment, and an internal
key derived from that Presentation. Such a row has no session to `Get`, so `Enter` navigates to
its Panel at once through the same stages. It follows the sidebar while the board is open; a
canonical session that attaches the same Presentation replaces it at the next read.

Rows from different revisions are never spliced. An incomplete read retains `last_good` marked
stale; without `last_good` it may show only same-revision rows marked incomplete — and preserving
`last_good` never rewrites its install age, so preserved rows keep their true freshness across
reads and across close and reopen. Rows needing attention sort ahead of ordinary rows while
preserving canonical order inside each group. Every row keys selection and jumps by the opaque Agent
session id but never displays it: it shows status, the attached terminal's name under the
[Tab label rule](native-tab-labels.md#canonical-state-and-display), provider, freshness and resolved
Workspace location (omitting a Tab name equal to the title), and a session without a
navigable panel shows a localized placeholder in place of the location. The sidebar shows only the
last completed read's counts, timestamp and completeness mark — closed or unread never implies
live or zero. Get freshness and List freshness are separate domains and never substitute for each
other.

A refresh repairs selection by its stable id and independently preserves the current top-row
id when it still exists; background data never snaps a manually scrolled reader back to selection.

The presentation index that navigation resolves through is fed only by canonical snapshots the
session actually accepted — bootstrap and every snapshot-bearing applier rebuild inside their
accepted arm, and a rejected or non-installing outcome never reaches the index. Resolution waits
on readiness instead of guessing, and a stale or invalid candidate fails conservatively rather
than landing anywhere else.

Each epoch’s lane owns its outcome receiver; closing the lane prevents its late results from reaching a replacement. Viewer reattach uses the existing detach/candidate/attach implementation and its failure paths. Failed or expired reattach reports Agent Unavailable/Expired through the production repair path. Board, native executor and UI seam tests cover those outcomes; no abstract board progress claim promises reattach success.

## Correctness evidence

The owning implementation tests cover same-revision row assembly, stable viewport anchoring, stale or superseded jumps, exact non-zero-panel landing, viewer death, bounded lane cleanup, exact-episode input isolation, first-draw refusal, admitted suspension and resume, stale draw rejection, Client-PATH candidate discovery, picker grouping, order, filtering and `Esc` order, the offered places and their `Tab` cycle, new Worktree's form opening over the picker and `Esc` restoring it through the raw-byte route, keyboard and pointer startup against the current draw, lost-worker recovery, the Panel-scoped start guard, observed-readiness admission and both locales.

Maintain the concrete witnesses in `crates/workbench-client/src/agent_board/tests.rs`, `crates/workbench-client/src/agent_board/picker/tests.rs`, `crates/workbench-client/src/tui_render/agent_board.rs` and the adjacent state, carrier, input and rendering suites. Use controlled completion/clock order and generated trace replay where the workflow has multiple outcomes.

Run `cargo test --locked -p orchbee --all-targets --all-features`. This workflow has no separate TLA model. The evidence covers named/generated implementation traces; it does not establish exhaustive interleavings, physical terminal appearance or untested platforms. Shared identity, ownership and commit protocols retain their separately registered core models.
