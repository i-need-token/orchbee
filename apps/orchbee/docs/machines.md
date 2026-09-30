# Saved Windows machines

The native Windows Client owns saved SSH machine profiles. The six `machine`
commands manage labels, SSH targets, explicit remote Site names, and enabled
state. OpenSSH owns credentials, keys, host trust, and SSH configuration.
Reference behavior is Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`.

## Catalog and transitions

`machines/catalog.json` beside the selected Windows configuration file is a
private, atomic, schema-version-1 document. Its revision is a nonnegative safe
integer; profiles have distinct random 32-character lowercase hexadecimal IDs.
At most 64 profiles and 128 KiB are admitted. Labels are nonblank, control-free,
and at most 128 UTF-8 bytes. Targets are at most 1024 ASCII bytes: an SSH host
alias, `[user@]host`, or `ssh://[user@]host[:port]`; bracketed IPv6 is supported.
Passwords and shell metacharacters are rejected. Session names use the native
Windows Site grammar, defaulting to `default`. Duplicate target/session pairs
may have distinct IDs, as in the reference.

| Event          | Guard and transition                                                                                  | Effects                                    |
| -------------- | ----------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| List           | Missing catalog is empty; invalid data fails                                                          | Read only; no directory creation           |
| Add            | Validate and check capacity; prepare remote; reload latest catalog under lock; append enabled profile | Remote Site may start before local commit  |
| Rename         | Exact ID exists; valid label                                                                          | Replace label; same value is unchanged     |
| Enable/disable | Exact ID exists                                                                                       | Set flag; same value is unchanged          |
| Remove         | Exact ID exists                                                                                       | Remove profile; never stop its remote Site |

Writers use the existing Windows private-cache lock and atomic replacement.
Revision advances once per actual change. Reads reject unknown fields, invalid
identities, duplicate IDs, overflow, oversized files, and unsafe filesystem
objects. Corruption never becomes an empty catalog. A failed local commit does
not roll back a remotely started Site. Cancellation after commit preserves the
committed receipt.

## Remote preparation

Add uses the OS OpenSSH executable and a 60-second total deadline. Batch mode,
strict host-key checking, no forwarding, bounded output, and an owned process
Job apply. Password prompts and new host trust must first be handled through
OpenSSH. SSH diagnostics are not copied into machine JSON or product errors.
The Windows remote command is an encoded PowerShell program with literal data.

Preparation checks the installed `orchbee.exe` Client version, platform,
protocols, and admitted cohort before starting the selected Site. A second SSH
invocation checks that the same cohort is still running and compatible after
the first invocation has ended. Only then can the profile be committed. The
receipt distinguishes `not-contacted`, `unchanged`, `possibly-started`, and
`ready` remote state, independently of the local catalog commit. Unknown
process settlement is reported, never retried as a successful add.

Missing or incompatible remote installations fail without saving. Automated
remote installation and interactive authentication remain outside the implemented
boundary; an enabled profile is not evidence of a live connection. No remote
session is stopped by rename, enable, disable, or remove.

## Evidence and lifecycle

Deterministic validation and mutation traces cover invalid inputs, repeated
operations, capacity, and revision bounds. Private-store tests exercise the
real Windows lock and atomic writer. Remote-program tests run real PowerShell
against owned fixtures, including failed preparation and malformed replies.
These tests do not establish a real two-machine SSH/TUI witness. Fixtures and
their process Jobs are owned by the test and released after the last consumer.

## Command routing contract

`orchbee --machine <id-or-exact-label> <command>` selects an enabled saved
profile. ID takes precedence; duplicate labels are ambiguous. The selector
is a prefix, accepts `--machine=value`, cannot combine with `--site`, and never
changes the catalog. Help is local. Remote execution admits CLI commands
for workspace, tab, panel, Client queries/control, terminal queries/send, Agent operations, worktrees,
notifications, runtime configuration, manifests, and supported plugin operations.
Local management, installation, TUI, and terminal interactive streams are refused.

The owner is the native CLI relay, not the local IPC transport. It first probes
the remote Windows Client, then sends one bounded versioned request to that
exact executable. A private relay entry point revalidates the cohort, Site,
argument grammar and command allowlist before spawning its own executable with
literal arguments in an owned Windows Job. The existing remote CLI remains the
business-operation owner. No local peer credential admission is bypassed.
Terminal file and stdin sources are acquired on the caller once and sent as raw
bytes; directories and executable paths are interpreted by the remote command.
Caller terminal/worker context is cleared; explicit targets or remote Client
selection determine the destination. Output bytes, diagnostics and exit status
are returned in a bounded reply, never reconstructed from SSH diagnostics.

Preflight and local input each have a 60-second bound. Execution honors the
command's own deadline, including explicit forever waits; cancellation settles
the local SSH Job. Losing transport after dispatch cannot prove remote process
settlement or business rollback: the failure reports an unknown remote outcome
and never retries. Before dispatch it reports not-submitted. The remote helper
owns and settles its command Job; a disconnected peer can still leave an already
accepted business action, and no distributed cancellation guarantee is claimed.
Wire request/reply limits and output truncation fail closed. No real SSH or
cross-machine acceptance is inferred from fixture-only relay tests.

Private requests are at most 2 MiB and replies at most 24 MiB; raw stdout and
stderr are each at most 8 MiB. Arguments are bounded to 64 KiB and 1024 entries.
PowerShell completion supports the selector prefix and routed command scope.

## Persistent TUI connections

The Windows TUI discovers the saved catalog while open and offers a machine
picker, opened with Ctrl+B then Shift+M or the footer indicator. Selection is by
profile identity, not row index or label. Local remains
available. A disabled, removed, ambiguous, or corrupt entry cannot silently
become another machine. Enabled profiles connect automatically for read-only discovery; only an explicit
selection opens a terminal surface. Saved enabled state is never online evidence.
The current surface stays usable until the candidate reaches a drawable state;
cancellation or failure keeps the current surface. Successful switching preserves
per-machine views and invalidates pointer/input state before accepting new input.
Local is retained; at most four inactive remote surfaces are cached. Eviction
releases the Client connection, leaving the remote Site running. Cached inactive
surfaces withdraw their Client-control, notification and outer-title subscriptions;
returning reopens them and transfers the host focus observation. A hidden surface
must not advertise a Ready control destination or acknowledge visible drawing.

The persistent transport owner creates a private, same-user local pipe endpoint
and a bounded SSH byte relay for each required native Client lane. SSH authenticates
the remote account; the exact admitted remote executable verifies its installed
cohort and opens only the selected existing Site's same-user pipe. Native Client
admission and application frames pass unchanged through that stream. Neither end
fabricates an accepted generation or bypasses Server cohort admission. Remote
terminal input, layout, metadata and control keep their existing semantic owners.

A transport guard retained by Client origins owns its listener, relay task and
SSH Jobs, including secondary query/control lanes. Each lane has bounded buffers;
backpressure pauses reads instead of dropping bytes. EOF retires that lane, and
no submitted frame is replayed. Dropping the last guard cancels and joins the
relay and settles children. The remote stdio adapter likewise owns and settles
its binary reader/writer Jobs. Disconnecting a Client does not stop the remote
Site or terminal processes. No port forwarding, host-key acceptance, credentials
storage, remote installation or POSIX implementation is added.

The picker owns keyboard and mouse input while open. Escape cancels; Enter and
click choose an exact painted identity. Resizing or catalog replacement invalidates
painted rows; pasted text and the remainder of a closing input packet cannot leak
into the underlying terminal. Each asynchronous attempt has an identity and a
cancellation owner; stale completion cannot switch the active machine.

Evidence must include binary full-duplex/backpressure/EOF/cancellation, same-user
pipe admission through a real helper process, picker identity and stale-result
traces, and rendered keyboard/mouse paths. Local fixtures do not establish real
SSH network survival, remote installation, or complete two-machine acceptance.

## Automatic machine discovery

The running Windows TUI reconciles enabled profiles by ID, target and Site. Each
endpoint generation owns one observer, cancellation flag and latest-event slot.
New connections are limited to four at once; existing online observers do not
consume that startup budget. A rotating cursor gives later profiles a turn even when earlier endpoints keep failing. Disabled/removed/replaced entries cancel their old
observers, and retired threads are joined before their slots are reused. Corrupt
catalog input suspends all discovery without changing the current terminal surface.
Renaming preserves connection identity. No observer creates a terminal, claims
control, changes focus, starts a Site or installs software.

An observer uses the admitted persistent tunnel and reads native Workbench
snapshots every two seconds with a five-second read deadline. A bounded
summary retains Site/revision, project/workspace/Tab counts and up to eight
preview labels of 80 scalars each, alongside the bounded context identities and
labels used by workspace navigation below. A fresh successful native snapshot
means Online; a saved profile or SSH process alone does not. An observation older
than ten seconds is shown as stale and is not offered for connection reuse.

Transitions are Waiting -> Connecting -> Online, or Connecting/Online ->
Reconnecting/Attention. Transient failure retries after 500 milliseconds,
doubling up to 120 seconds; 60 seconds continuously online resets that history.
Authentication, host-trust, unsupported/malformed protocol, absent installation
and denied snapshots enter Attention without automatic retries. Explicit selection
can request another attempt. Catalog generations fence all results, including a
removed/re-added ID; a one-slot mailbox coalesces observations without an unbounded
history. An admitted observer retains its transport through failure delivery; the UI
clears its cached origin before acknowledging retirement. The observer clears its
mailbox and releases its last transport guard on its own thread. Profile limits bound observers; shutdown cancels every worker before joins.

The machine picker shows localized connection states and an aggregate overview;
remote labels are data and cannot supply terminal control bytes. Explicit selection
waits at most 60 seconds for a fresh observer, reuses its admitted origin on a
separate native lane, and commits only a drawable candidate. Selection failure,
expiry or Escape preserves the current surface. Escape cancels the selection,
while discovery of enabled profiles continues. Retry applies only to observations,
never submitted terminal input or Client mutations. The workspace mode below
provides combined navigation, and Agents mode adds physical Agent navigation.
Source-owned merged Agent views are described below. Remote bootstrap/authentication
and real-network longevity acceptance remain separate obligations.

Evidence must exercise retry timing/stability, permanent refusals, concurrency,
profile replacement and corrupt catalogs, coalescing and cancellation, actual
read-only snapshot frames, stale-source refusal and rendered localized overview.
The reference scope is Herdr e0507237, endpoint supervisor and saved SSH transport;
local IPC fixtures do not establish remote SSH authentication or network recovery.

## Cross-machine workspace navigation

The machine picker has Machines, Workspaces and Agents modes, switched with Tab or their
painted mode controls. Workspaces combines Local's last accepted canonical
navigation with fresh enabled remote observations; rows carry machine identity,
Site ID and the complete standalone or project/workspace/context identity. Labels
and row positions never route a selection. Native protocol limits bound the list
to 1024 contexts per machine; project/workspace labels retain at most 80 safe
scalars each. Shared immutable context arrays avoid copying the list per UI tick.
Local retained navigation is a candidate list, not proof of remote liveness.
The overview's online counts describe remote observers. Offline/stale remote
contexts are not selectable. Renaming or reordering preserves selection identity;
removal invalidates painted hits and cannot retarget a pending selection.

Explicit context selection opens a separately admitted native lane, including
when the destination machine is already active or cached. The candidate reads a
fresh snapshot and verifies the exact Site and context before any attachment.
This path never opens or creates a standalone terminal as a navigation side
effect. A vanished/replaced target fails without selecting a fallback. An empty
context bootstraps Empty membership with no viewer or terminal; normal create
actions remain available after switching. The mailbox allocates a fresh generation
for empty and populated bootstrap alike. Legacy standalone startup still opens
its normal standalone context. Every machine candidate supplies an explicit
startup deadline; remote observation deadlines never become terminal startup
authority implicitly.

Queued selection freezes endpoint and destination identity. A candidate replaces
the current surface only after the selected context is drawable; failure, timeout
and Escape keep the current surface, with no mutation replay. Local is retained
when switching away; replacing the same machine retires its previous surface
only after commit. Cached machine-only selection preserves its selected context,
including empty workspaces and unbound placeholders.
A successful switch invalidates host input and painted regions through the
existing machine handoff. The Agent mode described below shares this handoff;
the main sidebar shares the Agent projection described below; Agent cycle
bindings are described below.

Evidence includes native snapshot-first exact bootstrap, absent/mismatched
Site/context refusal before effects, empty-context membership, actual machine
candidate startup and cancellation, duplicate labels and local/remote identity
separation, reordered/stale hits, mode keyboard/mouse and narrow rendering. Local
protocol peers do not establish real SSH or real-host visual acceptance.

## Cross-machine Agent navigation

Agents mode combines on-demand physical Agent observations from Local and fresh,
enabled remote machines. Each complete read joins a native Workbench snapshot to
bounded physical state batches at one Workbench/report revision. Missing,
unavailable or stale observations are distinct from a successful empty list.
Only observations with a reported Agent label become rows; primary and child
Presentations retain their own identity. Labels are sanitized display data.
Rows carry machine, Site, context, Tab, Panel, Presentation and complete physical
binding; no name or row number grants navigation authority.

At most four observation jobs run concurrently. Each job owns a separately
admitted lane, a five-second total budget and one result slot; reads are bounded
to 4096 bound Presentations and 8 MiB of serialized physical, report and metadata rows, with a
two-second cooldown. Hiding the last Agent read consumer
cancels pending jobs. Removed/disabled/replaced sources discard their results;
freshness expires ten seconds after worker completion, including time spent
waiting for the UI to consume the result. Only explicit activation opens a terminal.
The candidate revalidates the selected snapshot and physical checkpoint, starts
at the exact member, and checks the returned Host identity before taking control.
No standalone creation or first-pane attachment precedes that selection. Old
surfaces survive failure, cancellation and target replacement; commit still
requires drawable content. There is no replay of terminal input or mutations.

Evidence must cover bounded complete reads, revision/membership/physical identity
mismatch, primary and child placement, stale job cancellation and fairness,
keyboard/mouse identity, exact bootstrap and Host mismatch before control.
Live terminal-title tokens and real SSH/OS visual acceptance remain
separate obligations. Herdr's list and aggregate sidebar enumerate physical panes,
not unattached canonical sessions. The native canonical Agent list and per-Site
Agent board already retain those sessions; cross-machine aggregation of them is
an architectural extension, not a Herdr replacement gap.

## Agent metadata, view controls and navigation

The Agent list joins physical-state and Agent-report facts with independently
owned metadata only across overlapping stable reads: every batch has the same Workbench revision,
exact member/parent/terminal binding, and one revision for each read owner. Physical
and Agent-report reads share the report revision; metadata has its separate revision. After
all initial batches, metadata, report and physical fences confirm those versions.
Failure, missing members or a changed revision refuses the whole observation.
All reads share the existing deadline, row and serialized-state byte bounds.
Agent and authority facts guard effective metadata; display titles/providers/state
labels and tokens never replace the physical navigation target.

The Client owns ephemeral status/scope/order/search controls. `f` cycles status (including Done),
`c` cycles all/current machine/current context, `s` cycles placement/priority/provider/
title/state sequence, and slash edits a query of at most 128 safe scalars. Enter
or Escape first leaves query editing; Ctrl+U clears it. Painted controls accept
mouse activation. Filtering includes safe machine/location/title/provider/token
text. Stable ordering preserves canonical placement for ties; priority orders
Blocked, Done, Working, Idle, Unknown. State sequence is a reported version, not a
cross-machine wall clock. Missing observations stay distinct from no matches.
Changing data, context or controls invalidates painted activation identities.

Ctrl+B then `{` or `}` requests previous/next Agent across machines; the configured
`previous_agent`/`next_agent` actions use the same projector. An eight-second intent
waits for bounded observations, wraps relative to the exact current member, chooses
last/first if it is absent, and excludes unavailable rows. Empty lists and a single
already-focused target cause no attachment. Further keys or pointer input cancel a
pending intent. Actual activation still uses the separately admitted, revalidated,
drawable candidate and preserves the previous surface on failure.

Evidence covers guarded metadata, overlapping revision fences and binding faults,
stable ties and machine-qualified context/identity, query bounds and control
cancellation, painted mouse/keyboard/narrow/localized rendering and global action
input isolation. Live terminal-title tokens and real-network/OS visual acceptance remain separate obligations.

## Source-owned Agent views

`agent view set --spec <json>`, `agent view clear [--source <name>]` and
`agent view get` operate on the selected Windows Site. The Server owns one
transient revisioned specification: the latest valid set replaces it; a clear
without a source removes it, and a source-qualified clear removes only an exact
match. Restart removes the override. Reads require read permission; changes
require navigation management. Invalid, denied or unavailable operations preserve
the previous specification. No mutation is automatically retried after handoff;
lost mutation replies report an unknown outcome. Read or pre-submission exchange
failures have a distinct `exchange-failed` receipt and never claim a Server refusal
or uncertain mutation. Plugin sources use `plugin:<id>` and
require an installed enabled plugin; the next read clears a view whose
plugin has been disabled or removed. Other source names are attribution, not an
authentication credential.

The specification has `source`, optional `label`, optional `filter`, and optional
`sort`. Source is trimmed, 1–120 ASCII alphanumeric or `:._-` characters; labels
are trimmed and controls removed, then require 1–32 scalars. A 64 KiB encoded
specification limit also bounds string operands. Filters use `all`, `any`, `not`,
`eq`, `in`, or `exists` in the `op` field; nonempty groups, at most eight levels,
64 nodes and 32 `in` values are allowed. Fields are `status`, `workspace_id`,
`tab_id`, `pane_id`, `agent`, `seen`, `state_change_seq`, or `{ "token": "name" }`.
Values are strings, booleans for `seen`, full unsigned 64-bit integers for sequence,
or `{ "context": "current_workspace_id" }` / `current_tab_id` for matching ID
fields. Status admits idle, working, blocked, done and unknown.

Up to eight sort entries contain `field` and optional `order` (`asc` by default,
or `desc`). Fields are workspace_order, tab_order, pane_order, attention, status,
agent, seen, state_change_seq, or a token object. Token names are 1–32 ASCII
alphanumeric, underscore or hyphen characters. Values compare without display
sanitation; missing values sort last in either direction, and complete ties
preserve canonical placement. Attention ranks Blocked, Done, Working, Idle,
Unknown. Sequence is an observation version, not a shared wall clock.

The active machine's stable source specification filters the merged list and
global Agent cycle; other machines' overrides do not stack. ID operands and
current-context values are qualified by that active machine. Workspace IDs are
native workspace identities (absent for standalone terminals), Tab IDs are native
Tab identities, and pane IDs are exact Presentation identities, including child
Presentations. Placement order comes from the snapshot's ordered workspace, Tab
and pane placement. A missing context operand compares as missing, including the
reference's missing-equals-missing behavior. UI presets/search further narrow the
source filter; explicit source sorting wins and the sort control explains its owner, otherwise the UI ordering applies.
The source specification is read before and fenced after physical checkpoints;
a changed or unavailable source never silently means an empty override.

Seen belongs to this Client. The first complete endpoint checkpoint is the seen
baseline; new/changed binding versions are unseen. Idle plus unseen projects as
Done. Existing native attention supplies exact binding/version acknowledgements
from visible terminal drawing or explicit focus. Listing, filtering and ordinary
read requests cannot acknowledge terminals. Endpoint replacement/restart and
physical rebinding cannot inherit another identity's acknowledgement.

Projection reuses unchanged source checkpoints and reconciles each attention
batch once. Private paint-version stamps are scoped to the current machine and
invalidated on Client handoff; a new Client cannot inherit an identical counter.
Hiding the last Agent read consumer releases cached rows/specifications; bounded binding-version
acknowledgements remain Client-owned, and removed/disabled sources are pruned.

Evidence covers specification bounds/type rejection, source-guarded replacement
and clear, plugin eligibility, revision and transport uncertainty, endpoint-scoped
context, missing/tied multikey sorting, complete view fences and exact attention
versions. Live terminal-title tokens and real SSH/OS
visual acceptance remain separate obligations.

## Ordinal Agent focus

Windows `[keys] focus_agent` accepts configured `1..9` digits or ranges, with
prefix/direct modifiers and explicit disabling. It is unbound by default. A
matched binding digit selects the corresponding zero-based entry in the same
active-source-filtered, stable merged list used by previous/next actions. The
first nine eligible Agent rows display those numbers; loading/unavailable/empty
placeholders never take an ordinal. Configured shortcut help lists the actual
triggers in both locales, including disabled bindings. Shifted digit symbols from
legacy terminals resolve the binding digit; an exact configured symbol binding
takes precedence over that compatibility path.

The existing eight-second intent waits for loading observations. Unknown active
sources remain explicit refusals; absent ordinals neither wrap nor clamp, and a
target already focused causes no attachment. Further input cancels the pending
intent. Once resolved, the selection freezes exact endpoint/member identity and
uses the same revalidated drawable candidate as pointer and cycle navigation.
No terminal acknowledgement follows from choosing or reading an ordinal.

Evidence covers key/profile/conflict admission, complete-modifier dispatch and
packet-tail capture, overlay priority, source-filtered numbering, loading,
absent/current targets, cancellation and exact cross-machine identity.

## Main Agent sidebar

On Windows, the main sidebar presents the same endpoint-qualified Agent projection
as the Agents picker, including the active machine's source view and Client-local
status, scope, ordering and search. The first nine eligible entries keep their
filtered-list ordinals; loading, unavailable, empty and no-match entries never
receive numbers or navigation targets. The focused physical member is marked.
The header opens the existing Agents picker for keyboard navigation, filtering,
search and complete details; the configured machine-picker binding followed by
Tab reaches that same mode. Sidebar rows are navigation entries: a left press
freezes their painted endpoint/member/physical identity, re-resolves eligibility,
and starts the existing revalidated drawable candidate. A current target is a
no-op. Changed, removed, disabled or unavailable identities never redirect to a
replacement row. Other buttons, modifiers, releases and drags cannot activate.

The sidebar retains its 30-column Full/Compact layout and disappears at Minimal
or too-small geometry. Optional business labels yield to the Agent list at short
heights. Default entries occupy two lines; configured entries use their resolved heights
and gaps. Whole entries fit when possible; an oversized entry is clipped to the
actual body, with grapheme-safe horizontal truncation;
a fixed footer reports the visible range and opens the complete list. The default
window follows the exact focused member; explicit wheel scrolling holds the
window until focus or filters change. Wheel bursts queue three entries per notch,
coalesce at the input cadence, and apply at most one painted viewport per pass;
reversal cancels the old remainder. Overlay/modal ownership and released capture
continue to precede sidebar gestures. Successful drawing publishes the same
regions used to paint; resize invalidates them.

Visible sidebar or open Agents mode is a read consumer. Their shared collector
uses the existing endpoint, freshness, deadline, row/byte and cancellation bounds;
closing only the picker leaves a visible sidebar current, while hiding the last
consumer releases source rows/specifications and cancels reads. Sidebar rendering,
scrolling and opening the list do not acknowledge terminal versions. Geometric
sidebar overlays suppress terminal-draw acknowledgements even when short height
has undocked a Wide Client's sidebar. Live terminal-title tokens and real
SSH/emulator visual acceptance remain separate obligations.

## Agent sidebar configuration model

Windows users can select sidebar content through `ui.sidebar.agents.rows`,
`rows_by_agent` and `row_gap`. The shared Windows settings owner admits the
bounded whole file and a canonical, closed JSON profile of at most 32 KiB.
The existing Site configuration owner publishes that profile alongside keys and
notification policy; only the Client UI owner installs a newer, completely
validated publication. Bad candidates retain all previously installed settings.
No renderer reads configuration files or starts background work.

Rows contain `state_icon`, `state_text`, `machine`, `workspace`, `tab`, `pane`,
`agent`, `terminal_title`, `terminal_title_stripped`, or `$name`. Missing values and tokens hidden by rules disappear; rows
with no remaining tokens disappear. An entry still retains its focus marker and
ordinal, exact target and navigation eligibility. Custom values and custom state
labels are read from the already-fenced presentation metadata. Text rules match
the original value; only painting sanitizes controls and truncates graphemes.
`terminal_title` and `terminal_title_stripped` use the live physical title
observation below; pane title and `$terminal_title` never substitute for those
builtins.

| Input or event                                 | Decision and observable result                                                                                          |
| ---------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| No rows setting or matching override           | Preserve the existing two-line default                                                                                  |
| Provider override                              | Exact physical Agent label first, then its canonical report label; displayAgent cannot select an override               |
| Empty rows, missing custom value, or hide rule | Remove configured text, preserving at least the identity line and exact hit target                                      |
| Styled text token                              | Apply base foreground/bold/dim, then the first matching rule; `hide=true` removes that token                            |
| Text rule                                      | Exactly one of equals/contains/starts_with; optional ASCII case folding                                                 |
| Numeric rule                                   | Exactly one finite gt/lt threshold; strict finite number parsing without trimming or case folding                       |
| Layout or publication changes                  | Reset follow-focus window and pending scroll; do not alter filters, seen state or physical identity                     |
| Variable-height viewport                       | Fit whole entries and inter-entry gaps; an oversized single entry is clipped to the actual body                         |
| Scroll                                         | Move by entries, at most the painted entry count per cadence; clamp against the final page computed from actual heights |
| Invalid file/profile/reload                    | Reject before publication or UI installation; retain previous keys, notification policy and sidebar                     |

At most 16 rows, 16 tokens per row, 16 rules per token and 64 provider overrides
are admitted. Custom keys use the metadata ASCII grammar; provider keys are
bounded native IDs. Foreground accepts `#rgb` or `#rrggbb`. The theme owner maps
explicit user foregrounds and modifiers; default presentation remains semantic
theme roles. Nonnegative `row_gap` fits u16 and is applied only between entries.
Empty trailing space and gaps are non-activating sidebar surfaces. Paint geometry
and input geometry share the same resolved window, including clipped entries.

Core evidence must cover strict admission and canonical profile roundtrips,
first-match/hide/case/numeric rules, raw-value matching, provider identity,
missing/empty content, final-page and focus windows, bounded/reversed wheel,
and exact hit targets. Adapter evidence must cover whole-publication retention,
real configuration IPC, and both locales/themes at short and full heights.
No new Agent authority, terminal acknowledgement, persistence migration, network
acceptance or emulator visual proof follows from configurable painting.

## Live terminal title observation model

An Agent sidebar user may select `terminal_title` or
`terminal_title_stripped` to follow the OSC title of the exact physical terminal.
The existing Host metadata event is the only title writer. The Windows Worker
keeps a connection-owned, bounded projection keyed by the complete Host identity
and metadata revision, independent of Agent report sequence, view filters and
Client seen state. Closing the Host connection drops that projection; confirmed
physical disposal removes its entry. Missing observations remain unavailable,
while an observed null title means a confirmed clear. Neither case substitutes a
pane label or a custom `$terminal_title` token.

A read workflow requests at most four exact physical bindings per batch on the
existing owned observation lane. The Site checks membership and retained Worker,
creation and Host generations before and after the Worker read. The Worker drains
at most 32 queued Host events per read, yielding to a continuously active stream,
without attaching a viewer, claiming input or
resize, reading a terminal screen, starting a Host, or acknowledging a draw.
Repeated exact physical identities across presentation aliases are read once
and projected back to each independently fenced binding.
Every reply echoes the operation and physical identities; changed bindings,
invalid revisions, malformed frames and exceeded bounds reject the candidate.
Titles are sequential observations rather than an atomic snapshot of all Host
terminals. The sidebar refreshes through the existing approximately two-second
observation cadence; raw glyphs are sampled, without promising every animation
frame. The collector retains its existing total deadline, cancellation, row,
byte and final membership/report/source-view fences.

The projection admits at most 1024 physical titles and 64 MiB of title text per
Host connection; each title follows the Host's 64 KiB metadata bound. A separate
bounded Worker reply frame carries four complete values, avoiding truncation of
raw values. Newer metadata replaces the prior value; identical replay is a no-op;
a decreasing revision, changed replay or reused terminal identity is invalid.
The Client paints raw and stripped forms from that same observation. Stripping
removes one Windows elevation prefix and at most one recognized leading activity
glyph followed by whitespace, preserving Unicode text. Rules inspect the selected
original form before display sanitization and grapheme truncation. Title-only
changes may alter row height and cancel pending old scroll geometry, but do not
change Agent attention or navigation identity.

Core evidence covers absence versus clear, monotonic revisions and replay,
identity replacement, bounded framing and exact reply correlation, stripping,
raw-value rules and custom-token separation. Adapter evidence covers actual
Windows metadata events and pipes, Site before/after physical fences, collector
cancellation and rendering. No new public CLI command, persisted title registry,
Host protocol, input authority, or independent terminal acknowledgement follows
from these presentation tokens.
