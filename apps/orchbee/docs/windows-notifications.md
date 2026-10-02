# Windows CLI notifications

`notification show <title> [--body TEXT] [--position top-left|top-right|bottom-left|bottom-right] [--sound none|done|request] [--format text|json]` submits to an existing Site. Title/body normalization collapses whitespace, removes controls and truncates to 80/240 Unicode scalar values; an empty normalized title fails before submission. Omitted position follows each Client configuration (bottom-right by default); sound defaults to none. The wire bounds each normalized string by its scalar count and UTF-8 size.

## Delivery model

The existing Server Workbench service owns an ephemeral notification registry. Each listener belongs to its authenticated connection generation, expires after five seconds without a poll and has one pending message with a five-second lifetime. At most 32 listeners and 256 request receipts are retained; receipts expire after 60 seconds. A successful Show queues to every currently accepting live main-TUI listener and starts the one-second rate limit. Disabled, busy, rate-limited and no-foreground-client outcomes perform no delivery. Exact retained request replay returns its original result without enqueueing; conflicting request identities fail. No mutation is retried after transport uncertainty. A successful receipt describes queue admission, not physical display or audible playback. Listeners consume once; disconnection and expiry release pending state. No work is retained across Site restart.

A cancellable Client-owned connection polls independently of input/rendering. Only the UI owner installs notifications. With card delivery enabled, one five-second transient card uses the requested corner within the content region, never covers the status row, captures no keys or mouse, moves no focus and changes no layout. For cards, existing notices, dialogs and overlays make the listener temporarily unavailable; a race is resolved by discarding the incoming card rather than replacing a user-owned surface. Expiry schedules redraw while idle. Display text escapes terminal and directional formatting controls and clips using terminal display width. A successfully drawn card may issue one safe Windows sound cue: information for done, exclamation for request, no cue for none. System sound policy may suppress playback. External delivery requests sound independently of visual success; system mode requests a Windows notification-area balloon. No delivery activates a native window or navigates to an Agent target.

## Boundaries and evidence

Herdr reference is `e0507237ad8d41f60aece10d00ef29ce2339ca34`, especially `server/headless/notifications.rs`: Client delivery policy is distinct from Server fallback policy. Native custom notifications honor Client delivery policy and omitted-position inheritance. Automatic Agent notifications are specified below; clipboard policy, custom audio and target navigation remain separate alignment work. Matched private cohorts upgrade/roll back together. No new persisted format, dependency version or unbounded queue is introduced.

Focused evidence covers sanitizer boundaries, independent protocol validation, registry expiry/replay/fanout/backpressure, actual Windows CLI pipes, shutdown, Unicode corner rendering, idle expiry and preservation of input ownership. This does not establish authenticated provider, physical Windows visual/audio or final full-regression acceptance.

## Configured custom-notification delivery

Windows `[ui.toast] delivery` selects `off` (default), `orchbee` (`herdr` is an import alias), `terminal` or `system`. `[ui.toast.herdr] position` selects the default corner using the existing four CLI names. Omitted CLI position follows each receiving Client; explicit position wins. Startup uses the Client's settings. An explicit Site configuration publication applies the canonical bounded notification profile together with keys; malformed publications retain both previous policies. Other Agent/clipboard/sound preferences are outside this custom-notification slice.

Off suppresses visuals, while explicitly requested custom sounds remain independent. With no Client, the Server's off policy returns `disabled`; otherwise it returns `no-foreground-client`. A connected Client using off can still acknowledge transport admission as `shown`. Policy changes discard undrawn pending effects and cards; they do not replay prior messages.

Terminal delivery uses detected Ghostty/iTerm/WezTerm OSC 9 or Kitty OSC 99, with tmux escaping when applicable. Only the TUI lifecycle owner writes, after a complete frame, and never after restoration begins. Unknown backends report a nonfatal localized notice. System delivery owns one hidden, nonactivating Windows notification window/thread, a bounded inbox and a notification-area icon. The existing safe winsafe GUI wrapper supplies window creation/message dispatch; no authored unsafe code or external helper process is used. A 100 ms timer observes shutdown, releases an icon after ten seconds and removes it before window close. Drop cancels and joins. Queue admission is not OS paint acknowledgement; OS policy may suppress balloons. Physical host/visual/audio acceptance remains deferred.

## Automatic physical Agent notifications

The Windows main-TUI Client may notify for state changes from already detected terminal Agents. This
first slice uses only complete physical observations; canonical-only Agents do not enter the model,
and canonical plus physical observations are never merged heuristically. A notification carries a
normalized detected Agent label and bounded workspace, Tab and pane context. It never includes the
Agent's prompt, response, report message or terminal contents.

The automatic policy shares the bounded notification configuration profile, while retaining a
separate setting from manual CLI delivery:

```toml
[ui.toast]
delivery = "off"                 # manual `notification show`; existing default
agent_delivery = "orchbee"       # automatic Agent popup; default
delay_seconds = 1                # 0 through 3600

[ui.sound]
enabled = false                  # automatic Agent sound; default

[ui.sound.agents]
codex = "off"                    # default, on, or off
```

`agent_delivery` accepts `off`, `orchbee` (`herdr` as an import alias), `terminal` or `system`.
It does not change the existing `delivery` default or the explicit sound on `notification show`.
`[ui.sound]` controls automatic sounds only. Global `enabled = false` is a hard stop; with sound
enabled, a per-Agent `off` mutes that detected label and `default` or `on` permits the built-in cue.
Agent keys are lowercase detector IDs of at most 64 ASCII letters, digits, dots or hyphens, with an
alphanumeric first and last character. No Agent is muted by default. Done uses the Windows information cue and request uses the Windows
exclamation cue. Custom files, decoder dependencies, helper processes and click navigation are not
part of this contract.

### Domain model and transitions

| Field | Contract |
| --- | --- |
| Problem | A Client whose user has not seen a background terminal Agent transition needs one delayed local popup and, when enabled, one sound without changing Agent or Workbench state. |
| Inputs | Complete physical rounds, full launch binding, semantic sequence, detected Agent label, `idle`/`working`/`blocked`/`unknown`, exact Client seen acknowledgement, monotonic time, accepted configuration revision and sink availability. Missing or unreadable rounds are absence, never an inferred transition. |
| Data | The Client owns at most one current observation and one candidate per presentation. Candidate identity is the full presentation/generation/Host epoch/Host terminal binding plus sequence, kind, Agent label, due time, first eligible successful-draw serial and configuration revision. |
| Outputs | `done` or `request` delivery commands, or explicit pending/cancelled/suppressed/no-op states. A command independently says whether popup and built-in sound are enabled. Sink failure is nonfatal and is not retried after acceptance. |
| Dependencies | The pure reducer receives logical time and successful-draw facts. The attention owner supplies exact observations and seen events. The notification state supplies bounded sink admission. Only the TUI lifecycle owner writes OSC, submits a system balloon, paints a card or calls the sound API. |
| Ownership | The Server remains the physical-report and configuration-publication owner. Each Client owns its observation acknowledgement and scheduler. The manual Server notification registry is not used for automatic events. |

| Previous exact state | Current exact state | Result |
| --- | --- | --- |
| first observation, replacement binding, or changed Agent label | any | install a baseline; no candidate |
| `working` | `idle` | schedule `done` |
| `idle` or `working` | `blocked` | schedule `request` |
| `unknown` | any | no candidate |
| any other transition or repeated sequence | any | no new candidate; a state change cancels an older candidate |

A candidate is eligible only after its delay and after at least one later successful complete draw,
so zero delay still gives the visible pane an acknowledgement opportunity. At delivery the reducer
rechecks the exact binding, sequence, state, Agent label, configuration revision and unseen state.
A successful exact paint suppresses both popup and sound for that Client; another Client remains
independent. This notification-only suppression is exact-version scoped and does not mark an
attention badge seen while outer focus is known lost. Removal, replacement, state change,
acknowledgement, valid configuration replacement or Client retirement cancels the candidate. A
manual card, dialog, notice or other user-owned surface makes the sink busy; the bounded automatic
candidate waits and never replaces it. One accepted candidate produces effects at most once and is
not persisted across Client exit.

The Site publishes keys, sidebar settings and this complete notification profile atomically. An
invalid startup or reload candidate retains the previous profile. A valid changed revision cancels
all old automatic candidates and undrawn notification effects; later state transitions use the new
profile. The default automatic popup is in-app and the default automatic sound is off. Disabling
both at runtime and reloading is the immediate rollback. Because settings parsing is strict, rolling
back to an older binary also requires removing `agent_delivery`, `delay_seconds` and `[ui.sound]`.

### Correctness claims and evidence

The reducer claims: baselines, replacements and unknown transitions never alert; only the two rows
above create candidates; stale or seen candidates never deliver; acknowledgement is Client-local;
each exact Client/identity/sequence/kind delivers at most once; muted Agents have no automatic
sound; invalid reload preserves policy; valid reload cancels old work; a busy manual surface is
never overwritten; pending work is bounded; and retirement produces no late effect.
`NativeAgentNotifications` is a global transition machine parameterized by any nonempty finite
`Clients` set. The inventory TLC configuration enumerates one symmetric Client coordinate with the
full time, version, generation, draw and configuration bounds. The two-Client conclusion is supplied
by the projection argument below and by deterministic two-Client Rust traces; it is not reported as
a direct TLC enumeration of the Cartesian product.

| Action frame | Actions |
| --- | --- |
| Shared logical clock only | `AdvanceTime` |
| One Client coordinate only | baseline installation; all four changed observations; replacement; unavailable; exact presentation; successful draw; valid reload; sink busy/available; emit; seen suppression; retirement |
| Pure stutter | repeated observation; stale presentation; invalid reload; all-retired stutter |

For a Client `c`, project a global behavior to `c`'s status, observation, identity generation,
version, seen state, candidate, configuration, sink, draw, last-emitted position and violation
witnesses, plus shared `now`. Every non-stutter local action guard reads only that coordinate and
`now`, and its frame writes only that coordinate. An action for another Client is therefore a
stutter in this projection; `AdvanceTime` is identical in every projection. There is no global
candidate capacity, sink, seen or delivery-witness writer. Local action enabledness is consequently
the same before and after projection, so the `WF` assumptions for draw, sink availability and seen
suppression, and the `SF` assumption for emit, are preserved. The shared clock's `WF` assumption is
preserved directly. Each projected behavior satisfies the enumerated archetype specification, so
its pointwise safety claims and `Ready(c)` progress property lift to every Client in the finite
composition. `PendingIsBounded` follows from `TypeOK`: each coordinate contains zero or one
candidate, hence the number pending cannot exceed `Cardinality(Clients)`.

The bounded model projects delivery history to each Client's last emitted `(generation, version)`
position plus sticky policy-violation witnesses. This is conservative because versions increase
within one generation, replacement increases generation before resetting version, and observation
gaps preserve both. Every legal emitted position is therefore strictly newer; any duplicate or
reused position is detected without storing an ever-growing delivery set. The existing at-most-once
claim is strengthened to at most one effect per physical semantic position, which permits only one
notification kind by the transition matrix. Progress assumes logical time advances and a retained
candidate's sink eventually becomes available; no progress is claimed after replacement,
cancellation or retirement. This composition proof does not cover shared operating-system sound or
external notification side effects; those remain part of native cohort acceptance.

Deterministic Rust evidence covers the transition matrix, delay boundaries, zero-delay draw gate,
seen cancellation, two independent Clients, binding replacement, configuration cancellation,
manual-sink backpressure, per-Agent mute, atomic malformed rounds, at-most-once settlement and
retirement. The stateful generator in
`terminal_session/agent_notifications/tests/generated.rs` records an append-only oracle ledger for
two Clients and two physical Agents, supports seed/JSON replay and semantic shrinking, and requires
coverage of every admitted action class. Adapter tests must additionally prove exact physical
binding/version conversion and after-draw acknowledgement order. Configuration tests cover strict
keys, bounds, profile round-trip, atomic valid reload and invalid retention. Final acceptance
requires a real Windows cohort with a visible and a background pane, completion, blocked request,
live reload and audible cues. Unit and model evidence do not claim physical OS paint or sound.
