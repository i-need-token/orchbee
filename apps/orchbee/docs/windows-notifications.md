# Windows CLI notifications

`notification show <title> [--body TEXT] [--position top-left|top-right|bottom-left|bottom-right] [--sound none|done|request] [--format text|json]` submits to an existing Session. Title/body normalization collapses whitespace, removes controls and truncates to 80/240 Unicode scalar values; an empty normalized title fails before submission. Omitted position follows each Client configuration (bottom-right by default); sound defaults to none. The wire bounds each normalized string by its scalar count and UTF-8 size.

## Delivery model

The existing Server Workbench service owns an ephemeral notification registry. Each listener belongs to its authenticated connection generation and active subscription, and has one pending message with a five-second lifetime. Subscription cancellation or connection loss retires the listener; idle listening needs no timer poll. At most 32 listeners and 256 request receipts are retained; receipts expire after 60 seconds. A successful Show queues to every currently accepting live main-TUI listener and starts the one-second rate limit. Disabled, busy, rate-limited and no-foreground-client outcomes perform no delivery. Exact retained request replay returns its original result without enqueueing; conflicting request identities fail. No mutation is retried after transport uncertainty. A successful receipt describes queue admission, not physical display or audible playback. Listeners consume once; disconnection and expiry release pending state. No work is retained across Session restart.

A cancellable event subscription uses the shared Client carrier; the UI polls only its bounded inbox, with no forwarding thread or independent connection. Only the UI owner installs notifications. With card delivery enabled, one five-second transient card uses the requested corner within the content region, keeps clear of the last row where the status chip floats, captures no keys, moves no focus and changes no layout. The card stays non-modal: its painted cells join the frame's hit map, so a click on the card neither activates a covered control nor reaches the pane beneath and cannot hand off a terminal link. A custom card is a noncapturing shield; an automatic Agent card [opens its Agent](#opening-the-notification-target). Cells outside the card keep their ordinary pointer policy, and keyboard input keeps reaching the focused pane. Blocking dialogs, menus, an armed prefix, captured input and other user-owned blocking surfaces make the listener temporarily unavailable; an ordinary transient status notice does not block admission or painting. A race is resolved by discarding the incoming card rather than replacing a user-owned surface. Expiry schedules redraw while idle. Display text escapes terminal and directional formatting controls and clips using terminal display width. A successfully drawn card may issue one safe Windows sound cue: information for done, exclamation for request, no cue for none. System sound policy may suppress playback. External delivery requests sound independently of visual success; system mode requests a Windows notification-area balloon. No delivery activates a native window; only a painted Agent card navigates, and only to its own Agent.

TooSmall root frames do not paint notification cards, consume their pending sound, or acknowledge
attention. The existing card and expiry remain retained; after geometry recovers, the first
successful frame that actually paints the still-live card settles its pending sound once. A
successful root frame alone is not a successful notification-card draw.

## Boundaries and evidence

Client delivery policy is distinct from Server fallback policy. Native custom notifications honor Client delivery policy and omitted-position inheritance. Automatic Agent notifications are specified below; clipboard policy and custom audio remain separate alignment work. Matched private cohorts upgrade/roll back together. No new persisted format, dependency version or unbounded queue is introduced.

Focused evidence covers sanitizer boundaries, independent protocol validation, registry expiry/replay/fanout/backpressure, actual Windows CLI pipes, shutdown, Unicode corner rendering, idle expiry and preservation of input ownership. This does not establish authenticated provider, physical Windows visual/audio or final full-regression acceptance.

## Configured custom-notification delivery

Windows `[notifications] cli_delivery` selects `off` (default), `orchbee`, `terminal` or `system`. `[notifications] position` selects the default corner using the existing four CLI names. Omitted CLI position follows each receiving Client; explicit position wins. Startup uses the Client's settings. An explicit Session configuration publication applies the canonical bounded notification profile together with keys; malformed publications retain both previous policies. Other Agent/clipboard/sound preferences are outside this custom-notification slice.

Off suppresses visuals, while explicitly requested custom sounds remain independent. With no Client, the Server's off policy returns `disabled`; otherwise it returns `no-foreground-client`. A connected Client using off can still acknowledge transport admission as `shown`. Policy changes discard undrawn pending effects and cards; they do not replay prior messages.

Terminal delivery uses detected Ghostty/iTerm/WezTerm OSC 9 or Kitty OSC 99, with tmux escaping when applicable. Only the TUI lifecycle owner writes, after a complete frame, and never after restoration begins. Unknown backends report a nonfatal localized notice. System delivery owns one hidden, nonactivating Windows notification window/thread, a bounded inbox and a notification-area icon. The existing safe winsafe GUI wrapper supplies window creation/message dispatch; no authored unsafe code or external helper process is used. A 100 ms timer observes shutdown, releases an icon after ten seconds and removes it before window close. Drop cancels and joins. Queue admission is not OS paint acknowledgement; OS policy may suppress balloons. Physical host/visual/audio acceptance remains deferred.

## Automatic physical Agent notifications

The Windows main-TUI Client may notify for state changes from already detected terminal Agents. This
first slice uses only complete physical observations; canonical-only Agents do not enter the model,
and canonical plus physical observations are never merged heuristically. A notification carries a
normalized detected Agent label and bounded workspace, Tab and pane context. It never includes the
Agent's prompt, response, report message or terminal contents.

`notifications.agent_reminder` and `notifications.sound` govern automatic reminders. Both
default to off (`never` and `false`). The same table supplies the delay and per-Agent sound
overrides independently of manual delivery:

```toml
[notifications]
cli_delivery = "off"             # manual `notification show`; existing default
agent_reminder = "never"         # never or when-hidden
sound = false                    # global sound for automatic reminders
delay_seconds = 1                # 0 through 3600

[notifications.sound_by_agent]
codex = "off"                    # default, on, or off
```

These settings do not change manual `cli_delivery` or explicit `notification show` sound.
`notifications.sound = false` is a hard stop; with sound enabled, a per-Agent `off` mutes
that detected label and `default` or `on` permits the built-in cue.
Agent keys are lowercase detector IDs of at most 64 ASCII letters, digits, dots or hyphens, with an
alphanumeric first and last character. No Agent is muted by default. Done uses the Windows information cue and request uses the Windows
exclamation cue. Custom files, decoder dependencies and helper processes are not part of this
contract; [opening the notification target](#opening-the-notification-target) is its only navigation.

### Domain model and transitions

| Field        | Contract                                                                                                                                                                                                                                                                                                                       |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Problem      | A Client whose user has not seen a background terminal Agent transition needs one delayed local popup and, when enabled, one sound without changing Agent or Workbench state.                                                                                                                                                  |
| Inputs       | Complete physical rounds, full launch binding, semantic sequence, detected Agent label, `idle`/`working`/`blocked`/`done`/`error`/`unknown`, exact Client seen acknowledgement, monotonic time, accepted configuration revision and sink availability. Missing or unreadable rounds are absence, never an inferred transition. |
| Data         | The Client owns at most one current observation and one candidate per presentation. Candidate identity is the full presentation/generation/Host epoch/Host terminal binding plus sequence, kind, Agent label, due time, first eligible successful-draw serial and configuration revision.                                      |
| Outputs      | `done` or `request` delivery commands, or explicit pending/cancelled/suppressed/no-op states. A command independently says whether popup and built-in sound are enabled. Sink failure is nonfatal and is not retried after acceptance.                                                                                         |
| Dependencies | The pure reducer receives logical time and successful-draw facts. The attention owner supplies exact observations and seen events. The notification state supplies bounded sink admission. Only the TUI lifecycle owner writes OSC, submits a system balloon, paints a card or calls the sound API.                            |
| Ownership    | The Server remains the physical-report and configuration-publication owner. Each Client owns its observation acknowledgement and scheduler. The manual Server notification registry is not used for automatic events.                                                                                                          |

| Previous exact state                                           | Current exact state | Result                                                      |
| -------------------------------------------------------------- | ------------------- | ----------------------------------------------------------- |
| first observation, replacement binding, or changed Agent label | any                 | install a baseline; no candidate                            |
| `working`                                                      | `idle` or `done`    | schedule `done`                                             |
| `idle` or `working`                                            | `blocked`           | schedule `request`                                          |
| `unknown`                                                      | any                 | no candidate                                                |
| any other transition or repeated sequence                      | any                 | no new candidate; a state change cancels an older candidate |

The existing `working` → `idle` notification remains a quiet-transition signal; its `done` delivery kind does not prove a successful turn or change the Agent status. Explicit `done` reports can use the same delivery kind. Error reports do not create a new notification policy.

A candidate is eligible only after its delay and after at least one later successful complete draw,
so zero delay still gives the visible pane an acknowledgement opportunity. At delivery the reducer
rechecks the exact binding, sequence, state, Agent label, configuration revision and unseen state.
A successful exact paint suppresses both popup and sound for that Client; another Client remains
independent. This notification-only suppression is exact-version scoped and does not mark an
attention badge seen while outer focus is known lost. Removal, replacement, state change,
acknowledgement, valid configuration replacement or Client retirement cancels the candidate. A
manual card, dialog, blocking overlay or other user-owned blocking surface makes the sink busy; an
ordinary transient status notice does not; the bounded automatic candidate waits and never replaces
it. One accepted candidate produces effects at most once and is
not persisted across Client exit.

The Session publishes keys, sidebar settings and this complete notification profile atomically. An
unreadable, oversized, non-UTF-8 or malformed-TOML startup/reload source retains the previous
profile. An invalid notification leaf uses its safe default, emits a warning and participates in the
complete atomic candidate; a malformed private wire profile is still rejected before Client
installation. A valid changed revision cancels all old automatic candidates and undrawn notification
effects; later state transitions use the new profile. `notifications.agent_reminder` and
`notifications.sound` enable in-app automatic popups and global sound; their default is off. Disable both in Settings to stop automatic delivery.

### Opening the notification target

When a card delivery installs an automatic Agent notification, the card keeps the Agent's target:
the Workspace, Tab, Panel and Presentation that showed the Agent's exact launch binding in this
Client's canonical placements when the notification was raised. The target lives only in the Client.
The wire `Notification` that `notification show` submits stays target-less, so a custom card keeps
its shield and opens nothing.

A primary click on an Agent card, released on the card, opens its target. So does the configurable
`open_notification_target` action, default `prefix+o`, whose global action menu row "Open
notification target" ("打开通知目标") is offered only while an Agent card is current. With no
current Agent card, the key or a menu row whose card expired says "No Agent notification"
("没有智能体通知") and nothing moves. All three run through the revalidating navigation of sidebar
Agent rows ([Main Agent sidebar](machines.md#main-agent-sidebar)): the Session, Tab, Panel,
Presentation, canonical binding and expected physical host must still match, Workspace and Tab
switch as needed, and the Panel takes focus. Navigation stays inside the notification's source
Session and never switches machines.

| Outcome     | Effect                                                                                                                                                                                                                                                              |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Opened      | The target was current or its selection was admitted. The card closes and the Agent's current versions are marked seen, as [explicit Agent focus](native-client-attention.md) marks them.                                                                           |
| Held        | The user's own switch or a background read is settling. The card closes, terminal input waits as it does for a held focus move, and the opening replays once that settles, with every fact checked again. A hold past five seconds ends with the busy notice.       |
| Busy        | Other work, such as a pending change, keeps the Workbench from navigating. The busy notice answers, nothing moves, and the card stays, so a later click or key can open it.                                                                                         |
| Unavailable | The Panel was closed or removed, its terminal was rebound, or the Session, Workspace or host is unavailable. The card closes, the status line says "This Agent is no longer available" ("这个智能体已不可用"), and nothing else takes focus or replaces the target. |

The Agent card shows the shared pointer feedback: hover takes the hover surface, and a press takes
the pressed surface with a bold title. It has no tooltip, because TUI pointer tooltips belong to the
sidebar, Tab strip and pane titles. While an armed prefix, an overlay, captured input or a native
notice hides the card, it cannot be clicked; the key and the menu row still reach it until the card
expires. Terminal and system deliveries carry no target. The GUI follows the same rules for its
browser and Desktop notifications ([Workbench](../../web/docs/workbench.md)).

Clicking a Windows system balloon is follow-up work. The current notification-area path sets no
callback message. Receiving the balloon's click itself would need no installer, registry or COM
registration, but the click alone is not enough to open the target. The notification moves to
Action Center once the icon is released after ten seconds, and a later click there never reaches
this process. A console Client cannot reliably bring its hosting terminal window forward, because
the terminal, not the Client, owns that window and foreground rules apply. The click would also need
a bounded handoff from the notification thread into the UI loop. Each of these needs a design and
real-window evidence first.

### Correctness claims and evidence

The owning implementation tests cover baseline/replacement silence, exact Client/identity/sequence/kind deduplication, stale candidate rejection, client-local acknowledgement, mute policy, invalid-reload preservation, valid-reload cancellation, no effect after retirement, and the Tab, Panel and binding a raised notification targets. Opening the target reuses the sidebar Agent navigation and its evidence.

Maintain the concrete witnesses in `crates/workbench-client/src/workbench_client/agent_notifications/tests.rs`, `crates/workbench-client/src/workbench_client/agent_notifications/tests/generated.rs`, `crates/workbench-client/src/notifications/tests.rs`, `src/tui_shell/tests/notification_too_small.rs` and the adjacent state, carrier, input and rendering suites. The actual root-frame resize witness verifies hidden-card effect retention and one-time settlement without requesting OS sound. Use controlled completion/clock order and generated trace replay where the workflow has multiple outcomes.

Run `cargo test --locked -p orchbee --all-targets --all-features`. This workflow has no separate TLA model. The evidence covers named/generated implementation traces; it does not establish exhaustive interleavings, physical terminal appearance or untested platforms. Shared identity, ownership and commit protocols retain their separately registered core models.

The file's position key is `notifications.position`, and the Session's bounded notification profile spells it the same way inside its `[notifications]` table; profile publication and rollback require the same cohort.
