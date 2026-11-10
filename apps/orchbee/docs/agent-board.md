# Agent board

## Scope and ownership

The Agent board is an input-capturing overlay in the
[native Tab Session](workbench-client-runtime.md) that lists Server-owned Agent sessions and jumps to an
Agent's already-existing Panel. The Server owns every Agent fact — session identity, provider,
status, canonical freshness, attention state and presentation bindings. The board is a Client
projection, navigation and startup surface: rows never infer Agent state from terminal pixels,
and the empty-state action hands a typed request to the private startup workflow rather than
creating a Panel or synthesizing a session. Maintain this reference with the private
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
requests a refresh, and `Esc` or an ordinary backdrop press closes. A row press selects, while the
inner wheel scrolls the reading viewport without changing selection. The outer wheel, unknown keys
and paste are consumed; nothing reaches the terminal shell. Modal or confirmation ownership
refuses opening; prefix chords still resolve, so `Ctrl+B g` closes the board too. Both ownership
edges — open and close — invalidate the published pointer hit map, so a click can never land on a
vanished board row or pass through the overlay onto a stale foreground region. Notifications,
task sending, scheduling, persistence and new Panel creation are excluded.

On Windows, after a complete empty list, the board queries mapped commands on the Client's inherited
`PATH` using fixed System32 `where.exe` helpers. At most four owned workers run concurrently, with a
six-second shared deadline, a 900 ms lookup slot and bounded Job termination. Results publish one at
a time; a missing, failed or timed-out lookup never discards other results. The input thread only
consumes bounded messages and reaps exited owners. Generic `agent` is not enough evidence for Cursor,
so Cursor remains a manual choice and is not advertised as detected from that filename.
Detected commands are hints about this Client's PATH, not guarantees about the target terminal.
A remote Session performs no local PATH detection. All canonical providers remain manually selectable
while detection is loading, unavailable or empty; a bounded moving button window and Up/Down expose
the choices, with a visible selected marker even without color. The form suggests an editable
command-based name such as `claude-1`; provider changes update an untouched suggestion, typing replaces
it, and later attempts advance the local sequence. The server reservation remains the uniqueness
check, including other Clients. The user activates **Start Agent** with
`Enter` or the pointer. Both keyboard Start and Cancel require a current successful draw. Activation
injects the exact currently focused Panel id and runs the same typed reserve and single-submission
workflow as the CLI. It needs no Agent integration: it is confirmed by canonical readiness or, while
no canonical Session has reported, by the Panel's observed Agent, the Server's published run for
the requested provider with an idle or blocked published state. It never installs a provider,
inspects an integration or changes integration/default configuration; manual selection does not
bypass terminal preflight or readiness. Accepted startup closes the overlay and restores terminal input immediately. The background task keeps one five-minute deadline, reports pinned canonical attention as input needed, and continues observing after the user responds. Startup notices and the reopened board share the same typed phase. A fully sent command that times out is unconfirmed; its deadline has passed, so its Panel may start again. A lost delivery receipt remains unknown and guards its Panel until the deadline. An active or retained startup result replaces empty-session guidance and remains visible alongside any canonical rows. Closing the overlay leaves accepted startup work running; `c` or the
visible cancel control is the explicit cancellation path. Cancellation wakes cancellable reads and
the one send exchange; teardown can still wait for an already-running Windows process-creation or
Job-assignment call and the best-effort reservation release of at most one second, so cancellation
is not instantaneous. Generation and Panel identity reject stale completion, and an uncertain
mutation or input is never replayed. Empty or failed detection leaves the form available for an
explicit manual choice. An empty incomplete read is labeled as partial rather than as proof that no sessions
exist. See [Windows Agent startup](windows-agent-start.md) for readiness and target requirements.

Closing or superseding a catalog cancels its owner and moves it to a retirement queue. The input
thread reaps that owner only after it has exited. Session teardown owns the final join; application
cancellation cannot shorten the initial Windows process-creation and Job-assignment calls.

The successful surface frame grants a same-episode visibility lease for owner-local navigation and
refresh. Therefore consecutive arrow or `j`/`k` keys can advance the cursor in one input packet even
after the first step invalidates its old row proof. Starting a jump and applying a row, wheel or
control effect still require an exact current draw; they wait for the next complete frame rather than
acting on an unseen cursor.

Visible Open, Refresh, Start Agent, Cancel start and Close controls use the shared action grammar. The localized shortcut line
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

The Windows empty-state startup owner is separate from that read lane. It stores one bounded draft,
one catalog task and at most one startup task per Panel. An accepted task pins its generation and
exact Panel, continues when the overlay closes, and settles only a matching completion. The board
opens on the focused Panel and presents only that Panel's start, so another Panel's start never
locks the form. Explicit cancellation wakes the presented workflow; Session teardown also wakes and
joins every bounded task before releasing its origin. A disconnected accepted worker, a pending or
unknown reservation, accepted or partially accepted input, and an unknown input frontier all remain
recovery states across close and reopen, guarding their Panel until the workflow deadline. A result
that proves zero input and a settled reservation, or observes the shell prompt again, becomes
retryable at once. A start refused
at a prompt the Host could not confirm, or by a Session or terminal too old to confirm one, typed
nothing: the status says so with the next step and the draft stays retryable
([prompt-gated input](windows-agent-start.md#prompt-gated-input)).

| Transition     | Guard and result                                                                                                                                                                                                                                                                                                                                                  |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Open           | Modal/confirmation-free board admits a fresh epoch and one lane thread; a previous lane must have actually exited first. Last-good rows, their install age and the residual cooldown persist across the reopen; selection is repaired by opaque id against the preserved rows.                                                                                    |
| Draw settle    | The first evaluated paint either admits the episode or refuses it. A later geometry block suspends an admitted episode without destroying rows, selection, viewport or accepted work; cover revokes current-frame visibility and exact activation proof. Resume requires a complete exact-episode paint. A stale receipt cannot mutate or reopen another episode. |
| List round     | One bounded walk: at most six same-revision pages under one absolute deadline, one re-walk on a stale cursor, twelve exchanges and 12 MiB total.                                                                                                                                                                                                                  |
| Settle read    | A same-revision full walk is complete and becomes `last_good`; an exhausted budget or deadline is incomplete or failed — never a masquerading empty success, and never a rewrite of `last_good`'s install age.                                                                                                                                                    |
| Request jump   | Select by opaque session id; a lane `Get` revalidates the target after the current bounded List under its own deadline and 128 KiB bound. A newer request supersedes the logical jump immediately while the superseded `Get` drains its lane credit.                                                                                                              |
| Resolve stages | Re-resolve context, then Tab, then Panel against current canonical identity at every stage; same-context attached viewers focus locally, otherwise attach/switch. If no canonical snapshot has fed the presentation index yet, resolution waits inside the jump's own deadline and asks the session for a refresh — an unfed index is not proof of `Gone`.        |
| Land           | Install the exact requested Panel including non-zero indices; a retired or missing target reports `TargetGone` and never falls back. A target in the already-active Tab whose viewer is dead reattaches in place — a fresh-generation switch at the same position, by stable Panel identity — while a live same-Tab target stays a no-op.                         |
| Start Agent    | A complete empty read plus a current successful draw admits one valid name, one canonical provider and the exact focused Panel unless a running or unknown start guards it. The workflow reserves once, submits input once and waits for canonical or observed readiness under one absolute deadline. A generation/Panel mismatch drops the completion.           |
| Close          | Invalidate the board generation, cancel read/jump intents and reclaim that lane after actual exit. Accepted startup tasks remain Session-owned and keep running; only explicit cancel or Session teardown wakes them.                                                                                                                                             |

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

The owning implementation tests cover same-revision row assembly, stable viewport anchoring, stale or superseded jumps, exact non-zero-panel landing, viewer death, bounded lane cleanup, exact-episode input isolation, first-draw refusal, admitted suspension and resume, stale draw rejection, Client-PATH candidate discovery, keyboard and pointer startup, current-draw cancellation, lost-worker recovery, the Panel-scoped start guard, observed-readiness admission, close/cancel separation and both locales.

Maintain the concrete witnesses in `crates/workbench-client/src/agent_board/tests.rs`, `crates/workbench-client/src/tui_render/agent_board.rs` and the adjacent state, carrier, input and rendering suites. Use controlled completion/clock order and generated trace replay where the workflow has multiple outcomes.

Run `cargo test --locked -p orchbee --all-targets --all-features`. This workflow has no separate TLA model. The evidence covers named/generated implementation traces; it does not establish exhaustive interleavings, physical terminal appearance or untested platforms. Shared identity, ownership and commit protocols retain their separately registered core models.
