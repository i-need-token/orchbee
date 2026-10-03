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

| Transition     | Guard and result                                                                                                                                                                                                                                                                                                                                           |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Open           | Modal/confirmation-free board admits a fresh epoch and one lane thread; a previous lane must have actually exited first. Last-good rows, their install age and the residual cooldown persist across the reopen; selection is repaired by opaque id against the preserved rows.                                                                             |
| Draw settle    | The first paint either presents the overlay or refuses it; a refusal closes the board like `Esc` and reports `overlay_refused`, never leaving an invisible input owner. A stale presented receipt cannot reopen the board.                                                                                                                                 |
| List round     | One bounded walk: at most six same-revision pages under one absolute deadline, one re-walk on a stale cursor, twelve exchanges and 12 MiB total.                                                                                                                                                                                                           |
| Settle read    | A same-revision full walk is complete and becomes `last_good`; an exhausted budget or deadline is incomplete or failed — never a masquerading empty success, and never a rewrite of `last_good`'s install age.                                                                                                                                             |
| Request jump   | Select by opaque session id; a lane `Get` revalidates the target after the current bounded List under its own deadline and 128 KiB bound. A newer request supersedes the logical jump immediately while the superseded `Get` drains its lane credit.                                                                                                       |
| Resolve stages | Re-resolve context, then Tab, then Panel against current canonical identity at every stage; same-context attached viewers focus locally, otherwise attach/switch. If no canonical snapshot has fed the presentation index yet, resolution waits inside the jump's own deadline and asks the session for a refresh — an unfed index is not proof of `Gone`. |
| Land           | Install the exact requested Panel including non-zero indices; a retired or missing target reports `TargetGone` and never falls back. A target in the already-active Tab whose viewer is dead reattaches in place — a fresh-generation switch at the same position, by stable Panel identity — while a live same-Tab target stays a no-op.                  |
| Close          | Invalidate the generation, cancel intents and in-flight work, then the owner reclaims thread, wake handle, socket and intent; reopen only after actual exit.                                                                                                                                                                                               |

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

Results an old epoch's lane
computed stay fenced to that epoch — the lane owns its outcome receiver, so nothing it resolves is
delivered after close. The reattach stage abstracts an admitted-and-successful viewer reattach:
the real detach/candidate/attach pipeline and its failure space live in `NativeTabSwitch`, and a
failed or expired reattach surfacing as an Agent Unavailable/Expired notice is cross-layer
feedback outside this model's outcome vocabulary — the production repair and its seam evidence
own it, so the model's stage progress never claims a real reattach must succeed.

## Correctness evidence

`AgentBoard` covers two Agent sessions, three presentations across two contexts and tabs, a
deliberate non-zero-panel target, bounded pages with one re-walk, revision bumps, one retirement,
selection by opaque id, coalesced intents, stale Get and stale stage drops, exact staged landing,
input consumption and lane reclamation. The current bound — two epochs, serial budget three —
additionally exercises close/reopen including the full jump → close → lane-exit → reopen → jump
path, a per-presentation viewer-death environment action, the narrow same-target reattach
stage, and an unpaintable draw that must close the board rather than remain an invisible
modal. `LandedIsLive` records the landing-time observation; the registered edge property
`LandedOnlyOnLiveViewer` independently requires a step newly producing `landed` to find the
exact target's viewer live in the pre-state — a later ViewerDies cannot falsify a landing
that was legal when it happened.

The registered family checks three projections of the combined machine, each with the same two
epochs, two rounds and serial budget three. `ModelScope` is private verification configuration:

| Projection   | Checked observations                                                                             | Preserved fairness                             |
| ------------ | ------------------------------------------------------------------------------------------------ | ---------------------------------------------- |
| `navigation` | List/Get, cached rows, selection, exact staged landing, viewer death/reattach and lane lifecycle | All navigation-stage and lane-exit obligations |
| `input`      | Open-board input equality and the shared lifecycle                                               | Lane exit                                      |
| `paint`      | Paint refusal, eventual close and the shared lifecycle                                           | Unpaintable close and lane exit                |

Only the `Next` dispatcher selects an event family; primitive actions, their guards and the
configured invariants/properties are shared unchanged. Its bounded existential branches have
constant empty or singleton domains. They select exactly the requested family while allowing
the pinned TLC action extractor to retain the primitive action names for required coverage.
The `combined` scope retains the full
Cartesian machine as an executable reference; its direct run has not completed within the
declared resource bound. The registered checks do not claim to enumerate that product directly.

The composition argument is a forward behavior projection. Navigation actions and fair guards do
not read either input bit or paintability, except the shared Open/Close initialization. Its projection
retains the navigation record, maps the two input bits to false and paintability to true, maps
PaneByte/PaintRefused to stutter and maps UnpaintableClose to the existing Close action with the same
lifecycle result. Close acquires no new fairness assumption. All navigation and lane-exit fair
guards and non-stuttering steps remain identical; the unpaintable obligation is checked separately.
Input and paint projections retain their local bits, board, epoch and lane liveness. They map an open
lane to idle, a cancelled live lane to exiting and a reclaimed lane to idle; unrelated fields use
the typed initialization and shared-lifecycle results of that family, including the capped close
count as serial. Navigation-only actions project to stutter, every close maps to Close, and Open
admission and LaneExit keep their exact guard and non-stuttering result. Paint preserves the
UnpaintableClose guard and weak fairness; input preserves the complete truth value of
InputFrozenWhileOpen. Other fair actions are locally disabled and require no additional assumption.

Every fair combined behavior therefore projects into each checked Spec. Navigation invariants,
round/jump progress and the independent landing edge are checked by navigation, input isolation by
input, unpaintable progress by paint, and the lane/type/lifecycle facts by their respective fields.
Together those results establish the combined claims within the declared bounds; arbitrary component
behaviors need not lift back into the product. This argument is an explicit refinement trust boundary.
Any new cross-family read/write or joint property invalidates it and requires an updated mapping or
direct product checking. Required action coverage combines only after all three variants succeed.

The model deliberately does not retain which board byte, outside wheel or refresh-while-jump
event already occurred. Erasing that former coverage-only history is a bidirectional stutter
quotient: every old transition projects to the same product transition or to one of those three
named self-loops, while every quotient behavior lifts by using each old named step once and
treating any repeated self-loop as the stutter already admitted by `[Next]_vars`. No weak-fair
action reads the erased history, so action enabledness and every `WF_vars` obligation are
preserved. The three required coverage names remain explicit; the runner counts generated
successors even when the successor state was already discovered.

The quotient also canonicalizes `laneRound`, `walkRev`, `walkPages` and `retries` after a
walk settles or the board closes. These four scratch values are read by list actions only while
`lanePhase = "list"`; `StartList` initializes every value before any later read. The projection
keeps list progress and pinned revision equality during a list and maps the scratch to its typed
initial values otherwise. The opaque revision relabeling is described below. Each
old step projects to the same named product step, and a quotient step lifts by retaining the old
settled scratch until the next initialization. Their `TypeOK` range checks remain satisfied by the
typed initial values; no business guard, product invariant or temporal property reads the discarded
values outside the list phase. List actions are disabled there, and
every other weak-fair action has the same enabledness under the projection. This removes dead
history without restricting any read, reopen, cancellation, navigation or environment interleaving.

The same projection clears `landedPlacement`, `landedTarget`, `landedSerial` and `landedLive`
only when Open, Close, RequestJump, StageStaleDrop or JumpSettled explicitly clears `outcome`
to `none`. No business guard reads these diagnostics; apart from their typed ranges, invariants
read them only for a live `landed` or `target-gone` outcome. Those outcomes retain their exact
values. `StartGet` initializes all four fields before reuse, and `ResolvePanel` writes all four
before producing `landed`. A lifted history retains old diagnostics while the outcome is absent
until the original initialization replaces them. The landing-edge property retains the old
pre-state viewer facts and exact post-state target, so it remains an independent check rather
than relying on the recorded bit. Every weak-fair action retains its guard and non-stuttering
product transition; the cached rows, navigation view and live outcomes are unchanged.

Closing also canonicalizes `round`, `bumps`, `deadSeen`, `paintable` and `paneBytesAtOpen`.
Their business reads require an open board or active list, and Open initializes them before reuse.
The closed projection equates `paneBytesAtOpen` with `paneBytes`, so a closed PaneByte still
projects to the same named step. A lift retains the old closed scratch until Open; it preserves
the cache, its age, selection, cooldown and serial budget. Viewer liveness and retired bindings
remain exact wherever a future navigation can observe them, as described below.
GetStaleDrop and StageStaleDrop likewise clear `jumpSerial` only when they remove the jump;
no business action reads that serial while `jumpStage = "none"`, and StartGet replaces it.
All typed ranges, closed-board and open-board observations, and fair-action guards remain valid.

Revision labels have no numeric or ordering meaning here: only `serverRevision = walkRev`
is observed. A bijective relabeling maps the current Server label to 1, an unequal pinned label
to 2 and the unread sentinel to 0. ServerBump retains its original budget increment and flips
the nonzero pinned relationship while keeping the current label canonical. ListPage therefore
sees exactly the same first-page, same-revision, retry and incomplete branches. Two bumps still
permit the original ABA equality; no mutation interleaving is removed. A lifted history keeps
the original current label and applies its original flip on every ServerBump. Every named step,
weak-fair enabledness, invariant and temporal observation is preserved under that relabeling.

ServerRetire also canonicalizes only the permanently removed presentation's viewer facts to
`viewerLive = FALSE` and absence from `deadSeen`. Retired bindings never return; GetResolve
excludes them, and every staged resolver checks retirement before reading or reattaching the
viewer. Other viewers retain their exact liveness and death budget. An old ViewerDies for the
removed presentation projects to stutter; ViewerDies has no fairness obligation. A lift retains
that old viewer's facts and may insert its unobservable death when the original guard allows it. Every business step
and fair-action guard still matches. Previously recorded landing diagnostics remain exact, and
no new landing can target a retired binding, so the independent landing-edge check retains all
relevant pre-state viewer facts. The retirement, target-gone result and cached navigation state
are preserved; this projection does not revive a viewer or remove a live-target death.

At the navigation serial limit, the finite model retains exact viewer facts for every possible
binding of the final pending intent or verifying Get. GetResolve fixes a presentation, after which
only that exact target can still be read or reattached. RequestJump is the sole jump-intent producer
and requires remaining serial budget; serials never decrease or reset on Open. An older serial's
stage cannot land. Thus erased peers cannot become a later target, while every new landing edge
still checks the original pre-state target viewer. Open, RequestJump, StartGet, GetResolve,
stale-drop settlement, JumpSettled, CloseBoard and ServerRetire preserve this projection. Environment
deaths of erased peers project to stutter; ViewerDies has no fairness obligation. A lift retains
those peers' old world facts and inserts their deaths only when the old guard permits them.

With no pending jump intent, Get or stage after the serial limit, or after closing the final epoch,
navigation is permanently disabled. The whole viewer/death/retirement world is then unobserved and
maps to false viewer bits and empty sets. ServerRetire remains explicit but produces stutter there;
it has no fairness obligation. A lift performs any old budgeted retirement when permitted and uses
the stuttering already allowed by `[Next]_vars` for additional unobservable steps. While navigation
remains possible, the exact retirement and its original budget are retained. Refresh, list, cache,
selection, paint, lane reclamation and any admitted reopen continue unchanged. Every fair-action
guard and non-stuttering step is preserved. This suffix reduction relies only on the declared finite
model bounds, not production navigation or reopen limits.

The navigation projection returns the record unchanged before those finite suffixes: ServerRetire
already maintains each removed viewer's canonical facts. A singleton function application binds
the evaluated record, and the pinned standard `TLCEval` identity materializes small sets and viewer
functions instead of retaining nested lazy closures. This affects evaluation only; no state,
action, guard, fairness or property changes.

After the final round settles, `bumps` also becomes unread until Open reinitializes it. The projection
clears it at ListPage or ListDeadline settlement and keeps ServerBump as an explicit stutter while
`round = MaxRound` and no list is active. Every active walk retains its original mutation budget,
revision equality, ABA, retry and deadline interleavings. A lift keeps the old exhausted-round bump
count, performs original budgeted mutations when permitted and stutters thereafter; Open resets
both versions before any later list. ServerBump has no fairness obligation, and all fair guards,
cached observations and configured properties remain unchanged.

The accepted 1,083,010-distinct-state run remains the finite-bound evidence for the prior
single-epoch variant — `MaxEpoch = 1` could not reach reopen. The registered two-epoch
configuration is the current finite-bound scope, checked through the three projections above.
This document does not claim that scope has passed until the registered runner completes all three;
meanwhile its reopen, viewer-death/reattach and
unpaintable-close claims rest on the model mapping, invariants, weak-fair progress properties and
deterministic tests described here.

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

The presentation index includes each bound child under its original Panel identity. A landed jump rechecks the exact Presentation and invokes native Presentation selection if that Panel currently shows another member; sharing a Panel is not sufficient landing evidence. Per-member labels remain distinct while layout leaves and Tab positions are unchanged.
