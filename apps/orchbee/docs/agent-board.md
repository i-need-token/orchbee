# Agent board

## Scope and ownership

The Agent board is an input-capturing overlay in the
[native Tab Session](native-tab-session.md) that lists Server-owned Agent sessions and jumps to an
Agent's already-existing Panel. The Server owns every Agent fact — session identity, provider,
status, canonical freshness, attention state and presentation bindings. The board is a Client
projection and navigation surface only: it never infers Agent state from terminal pixels, never
mutates Agent state, and never creates a Panel. Maintain this reference with the private
`terminal_session/agent_board/` module, its narrow access points into `client_view`, `ui`,
`tui_render`, `tui_pointer` and the dispatcher, the OBAP read lane, and their correctness evidence.

`Ctrl+B g` toggles the board. Opening claims input immediately — before the first paint — so no
keystroke falls through to the terminal shell; the first draw then settles the claim. If the
overlay cannot be presented (the viewport is too small) the draw verdict closes the board through
the same ordered cancel as `Esc` and reports the refusal separately, instead of leaving an
invisible input owner; a stale presented receipt can never reopen it. While open the board owns
input after modal and confirmation overlays and the armed prefix: Up/Down and `j`/`k` select a row
by opaque Agent session id, `Enter` requests a jump, `r` requests a refresh, `Esc` or an ordinary
backdrop press closes, click selects and the inner wheel scrolls rows. The outer wheel, unknown
keys and paste are consumed; nothing reaches the terminal shell. Modal or confirmation ownership
refuses opening; prefix chords still resolve, so `Ctrl+B g` closes the board too. Both ownership
edges — open and close — invalidate the published pointer hit map, so a click can never land on a
vanished board row or pass through the overlay onto a stale foreground region. Names,
notifications, task sending, scheduling, persistence and new panel creation are excluded.

## State and transition contract

The board runs one lane thread per open epoch: a single active operation with one coalesced pending
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

| Transition          | Guard and result                                                                                                                                                        |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Open                | Modal/confirmation-free board admits a fresh epoch and one lane thread; a previous lane must have actually exited first. Last-good rows, their install age and the residual cooldown persist across the reopen; selection is repaired by opaque id against the preserved rows. |
| Draw settle         | The first paint either presents the overlay or refuses it; a refusal closes the board like `Esc` and reports `overlay_refused`, never leaving an invisible input owner. A stale presented receipt cannot reopen the board. |
| List round          | One bounded walk: at most six same-revision pages under one absolute deadline, one re-walk on a stale cursor, twelve exchanges and 12 MiB total.                          |
| Settle read         | A same-revision full walk is complete and becomes `last_good`; an exhausted budget or deadline is incomplete or failed — never a masquerading empty success, and never a rewrite of `last_good`'s install age. |
| Request jump        | Select by opaque session id; a lane `Get` revalidates the target after the current bounded List under its own deadline and 128 KiB bound. A newer request supersedes the logical jump immediately while the superseded `Get` drains its lane credit. |
| Resolve stages      | Re-resolve context, then Tab, then Panel against current canonical identity at every stage; same-context attached viewers focus locally, otherwise attach/switch. If no canonical snapshot has fed the presentation index yet, resolution waits inside the jump's own deadline and asks the session for a refresh — an unfed index is not proof of `Gone`. |
| Land                | Install the exact requested Panel including non-zero indices; a retired or missing target reports `TargetGone` and never falls back. A target in the already-active Tab whose viewer is dead reattaches in place — a fresh-generation switch at the same position, by stable Panel identity — while a live same-Tab target stays a no-op. |
| Close               | Invalidate the generation, cancel intents and in-flight work, then the owner reclaims thread, wake handle, socket and intent; reopen only after actual exit.              |

Rows from different revisions are never spliced. An incomplete read retains `last_good` marked
stale; without `last_good` it may show only same-revision rows marked incomplete — and preserving
`last_good` never rewrites its install age, so preserved rows keep their true freshness across
reads and across close and reopen. Rows needing attention sort ahead of ordinary rows while
preserving canonical order inside each group, and every row carries the opaque Agent session id
verbatim alongside status, provider, freshness and resolved location. The sidebar shows only the
last completed read's counts, timestamp and completeness mark — closed or unread never implies
live or zero. Get freshness and List freshness are separate domains and never substitute for each
other.

The presentation index that navigation resolves through is fed only by canonical snapshots the
session actually accepted — bootstrap and every snapshot-bearing applier rebuild inside their
accepted arm, and a rejected or non-installing outcome never reaches the index. Resolution waits
on readiness instead of guessing, and a stale or invalid candidate fails conservatively rather
than landing anywhere else.

## Correctness evidence

`AgentBoard` covers two Agent sessions, three presentations across two contexts and tabs, a
deliberate non-zero-panel target, bounded pages with one re-walk, revision bumps, one retirement,
selection by opaque id, coalesced intents, stale Get and stale stage drops, exact staged landing,
input consumption and lane reclamation. TLC checks the full state space — 1,083,010 distinct
states — without state constraints or symmetry, with all required action witnesses and weak-fair
progress for read rounds, staged jumps and lane exit. That evidence predates this round's
supersession, dead-lane, draw-verdict, reopen-persistence and index-readiness contracts: the
checked model still proves the earlier boundary, and the model revision covering the new contract
is developed and verified separately.

Run the registered model with `node scripts/tla/run-models.mjs --models agent-board`. The OBAP
framing and identity contracts refine the wire steps; `NativeContextSelection`, `NativeTabSwitch`
and the local-client models refine context switch, attach and transport admission — they do not
cover the board itself.

Deterministic tests cover cursor and same-revision row assembly, completeness honesty, wrong-lane
isolation, stale-generation jump rejection, jump supersession with lane-credit retention, refused
`Enter` preserving an existing jump, dead-lane queued outcomes applying before the residual-credit
release, residual cooldown and last-good install age, draw-verdict input ownership, dispatcher
hit-map invalidation on both ownership edges, fenced index feeding and the unfed-index wait at the
real UI seam, same-Tab dead-viewer reattach to a live landing, modal input no-leak, exact
cross-context landing on a non-zero panel, `TargetGone` without mis-landing, bounded thread close
and two admitted lanes retiring independently. Board bytes must never reach the shell. Rendering
tests cover both locales, sanitized terminal text and bounded viewports.
