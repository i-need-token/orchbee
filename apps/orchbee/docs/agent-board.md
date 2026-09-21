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

`Ctrl+B g` toggles the board. While open it owns input after modal and confirmation overlays and
the armed prefix: Up/Down and `j`/`k` select a row by opaque Agent session id, `Enter` requests a
jump, `r` requests a refresh, `Esc` or an ordinary backdrop press closes, click selects and the
inner wheel scrolls rows. The outer wheel, unknown keys and paste are consumed; nothing reaches the
terminal shell. Modal or confirmation ownership refuses opening; prefix chords still resolve, so
`Ctrl+B g` closes the board too. Names, notifications, task sending, scheduling, persistence and
new panel creation are excluded.

## State and transition contract

The board runs one lane thread per open epoch: a single active operation with one coalesced pending
intent. A jump request replaces a pending refresh and cancels an in-flight jump by generation
advance; a refresh request while a jump is pending is consumed without displacing it. Every
completed List attempt — success, incomplete or failed — starts a two-second cooldown that `r`
cannot bypass.

| Transition          | Guard and result                                                                                                                                                        |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Open                | Modal/confirmation-free board admits a fresh epoch and one lane thread; a previous lane must have actually exited first.                                                 |
| List round          | One bounded walk: at most six same-revision pages under one absolute deadline, one re-walk on a stale cursor, twelve exchanges and 12 MiB total.                          |
| Settle read         | A same-revision full walk is complete and becomes `last_good`; an exhausted budget or deadline is incomplete or failed — never a masquerading empty success.              |
| Request jump        | Select by opaque session id; a lane `Get` revalidates the target after the current bounded List under its own deadline and 128 KiB bound.                                 |
| Resolve stages      | Re-resolve context, then Tab, then Panel against current canonical identity at every stage; same-context attached viewers focus locally, otherwise attach/switch.         |
| Land                | Install the exact requested Panel including non-zero indices; a retired or missing target reports `TargetGone` and never falls back.                                       |
| Close               | Invalidate the generation, cancel intents and in-flight work, then the owner reclaims thread, wake handle, socket and intent; reopen only after actual exit.              |

Rows from different revisions are never spliced. An incomplete read retains `last_good` marked
stale; without `last_good` it may show only same-revision rows marked incomplete. The sidebar shows
only the last completed read's counts, timestamp and completeness mark — closed or unread never
implies live or zero. Get freshness and List freshness are separate domains and never substitute
for each other.

## Correctness evidence

`AgentBoard` covers two Agent sessions, three presentations across two contexts and tabs, a
deliberate non-zero-panel target, bounded pages with one re-walk, revision bumps, one retirement,
selection by opaque id, coalesced intents, stale Get and stale stage drops, exact staged landing,
input consumption and lane reclamation. TLC checks the full state space — 1,083,010 distinct
states — without state constraints or symmetry, with all required action witnesses and weak-fair
progress for read rounds, staged jumps and lane exit.

Run the registered model with `node scripts/tla/run-models.mjs --models agent-board`. The OBAP
framing and identity contracts refine the wire steps; `NativeContextSelection`, `NativeTabSwitch`
and the local-client models refine context switch, attach and transport admission — they do not
cover the board itself.

Deterministic tests cover cursor and same-revision row assembly, completeness honesty, wrong-lane
isolation, stale-generation jump rejection, modal input no-leak, exact cross-context landing on a
non-zero panel, `TargetGone` without mis-landing, bounded thread close and two admitted lanes
retiring independently. Board bytes must never reach the shell. Rendering tests cover both locales,
sanitized terminal text and bounded viewports.
