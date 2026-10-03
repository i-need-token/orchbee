# Native context selection

## Scope and ownership

The native Client selects the standalone Terminal Context or a Workspace in the connected local
Site. Server owns navigation, membership, revision and terminal lifetime; the Client owns its active
location and selection. This reference governs the context-selection extension to the
[native Tab Session](native-tab-session.md). Maintain it with the Session reducer, transport,
navigation rendering and their correctness evidence.

`Ctrl+B [` and `Ctrl+B ]` select the previous and next location in Server order: standalone,
then the canonical global Workspace display order. An absent/empty explicit order retains
Project-major traversal; sorting never changes Project ownership. Selection wraps at either boundary. Labels are presentation;
Project, Workspace and context IDs establish identity. Entering another location restores this
Client's last selected Tab and Panel when those identities still belong there; otherwise it selects
the first surviving member, or Empty when there are no Tabs. Empty can be
entered and left without creating a terminal. Existing Tab controls act on the active context;
explicit `c` uses Server's existing directory admission and never falls back to the home directory.
Bare Windows startup follows the preference below. Without a saved preference,
[directory startup](native-directory-startup.md) creates the first Workspace only in an eligible
empty Site; other Sites retain standalone Open. Both use the existing first-frame readiness contract.

Interactive Project/Workspace creation and first-directory configuration belong to
[Workspace setup](native-workspace-setup.md); directory startup owns automatic empty-Site creation.
Both reuse this context installation owner.
Panel layouts, remote access and production recovery are excluded.
This unpublished cohort rolls out and rolls back together.
Changing the Client location does not change canonical navigation or dispose retained execution.
[Location label editing](native-location-labels.md) changes canonical Project or Workspace labels
while preserving these typed location identities.

## State and transition contract

The inputs are a validated Workbench snapshot, an explicit local direction, current Session state,
exact attachment identity and the existing monotonic operation budget. Navigation and the active
context projection must come from one snapshot revision. A nonactive context with an unsupported
layout does not invalidate the active context. Selecting an unsupported target is a definite local
failure and preserves the previous location.

| Transition               | Guard and result                                                                                                                                                                              |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Admit location selection | A stable Session with no unresolved mutation, Close confirmation/reconciliation or membership operation retains an exact target identity and operation generation; otherwise report busy.     |
| Read target snapshot     | Revalidate the target's canonical owner, Site and revision before changing attachments. Definite unavailable or unsupported target preserves the previous location.                           |
| Clean old attachment     | Fence old input, frames and grants, then detach the exact previous Viewer before installing a different selection. No terminal Dispose or Start is authorized.                                |
| Install target           | Atomically publish the new context, membership, provenance and optional desired attachment through the existing membership owner and mailbox transaction. Empty has no invented Tab identity. |
| Attach selected Tab      | Use the exact retained binding and the existing setup, first-frame and controller geometry rules. Late old-generation results cannot become current.                                          |
| Fail after handoff       | Preserve known facts and apply the existing deadline/retirement rules; never retry an ambiguous effect or synthesize successful cleanup.                                                      |

Known Create provenance must survive leaving and returning to a context. A snapshot binding cannot
turn a known Pending, failed, unavailable or outcome-unknown Create into permission to attach.
Retained provenance is bounded by the protocol's context and Tab limits and pruned only with
validated membership evidence. Pending Close work keeps its original exact target and blocks
location changes until the existing settlement rules permit them. Current-context refresh and
Create/Move/Rename/Close result handling must not silently return to standalone.

## Client selection memory

Read-only layout queries resolve an inactive Tab's `focusedPanelId` from this Client's remembered
Panel and current canonical membership, falling back to the first member. `selected` remains false;
the query neither visits the target nor changes navigation, attachments or terminal geometry.

`state/navigation_memory.rs` owns volatile, Client-private selection history. Each exact location
holds its last Tab ID and each visited Tab's last Panel ID. Labels, ordinals and terminal bindings
are not keys. The Site is fixed by the owning Session; this history is neither Server state nor
persistent storage. Herdr's Workspace active Tab and Tab layout focus are the reference behavior.

| Input                           | Guard and result                                                                                                                                                                                                     |
| ------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Admit Context departure         | Remember the current stable selection after existing busy and target guards pass. Empty adds no invented selection.                                                                                                  |
| Admit Tab departure             | Remember the logically stable departure, including a Ready frame-proof gap; superseding switches keep the original memory and never record an unvisited switching target.                                            |
| Resolve ordinary destination    | Validate canonical membership and provenance first; use the remembered Tab and Panel only while each belongs to its exact location and parent Tab. Missing history or removed members fall back to the first member. |
| Resolve explicit Panel or Agent | The explicit stable target overrides history and keeps existing refusal when absent; memory cannot substitute another Panel.                                                                                         |
| Install canonical membership    | Prune absent navigation locations, Tabs and Panels. Current-context membership takes precedence over any older retained entry; departed-context history uses retained membership until revalidation.                 |
| Refuse, supersede or cancel     | Only the previously stable departure may have been remembered; no target is marked visited and no additional command is emitted. Existing cleanup and generation settlement remain authoritative.                    |

Memory is bounded by the protocol's known Context and Tab membership, with at most one Panel ID
per visited Tab. Local Tab creation and Workspace setup admission also remember the stable
departure before their existing creation policy selects a new member. Retaining uncertain Create provenance does not retain navigation memory for a
canonically removed location. Restoration never changes Create permission, chooses another Client,
resizes a terminal or activates an OS window. Ordinary keyboard, pointer and CLI navigation share
this Session policy. After Tab switch admission the exact Panel ID is fixed: a reordered candidate
re-resolves its ordinal. A missing Panel in a surviving Tab fails explicitly; removal of the whole
Tab retains existing membership fallback to another Tab or Empty. The prepared candidate owns
the same Panel identity for state installation and mailbox publication. Explicit creation, membership repair and Close keep their existing selection
rules. Restart discards this per-location history; Windows preserves only the last displayed
position as described below. Cohort rollback requires no checkpoint migration.

Deterministic traces cover remembered nonfirst members, reorder by identity, removal to first/Empty,
explicit override, superseded/refused switches, stale retained membership, independent Clients and
known nonattachable Create provenance. Windows process evidence exercises two live ConPTY Clients
through CLI Tab/Workspace/Panel focus and checks unchanged peer selection and OS foreground.
The existing context model has one Tab per nonempty context and abstracts all memory choices to
that sole member. Its historical TLC result below does not prove multi-Tab/Panel memory or pruning;
the volatile-history increment uses deterministic state and process evidence, with TLA execution deferred.

## Windows startup preference

Bare local Windows startup remembers the last successfully displayed location, Tab, Panel and
presentation for the current Windows user and local Site runtime. This is Client-owned navigation
advice, separate from Server checkpoints and terminal lifetime. Its versioned, bounded JSON record
lives under the private `LOCALAPPDATA/Orchbee/tui-position/<runtime-key>` cache; the key includes
the user-scoped runtime directory and runtime base. Stable structural IDs survive checkpoint
restart. The new Server's Site ID and bindings always come from its validated snapshot.

The renderer returns the stable position and focused pane incarnation only when the workbench is
uncovered and drawable. Successful shell draw/flush precedes revalidation and publication.
Covered, failed, stale and switching draws do not publish. Empty locations invent no member.
An app-private worker coalesces pending positions to one record, performs private locked atomic
replacement off the input/draw path, and joins at shutdown. Windows keep independent live focus
and read the preference only at startup. The latest atomic publication supplies the next startup
preference; unchanged redraws do not write again. Storage failure displays a localized notice
without retiring a live terminal.

Restoration validates current canonical ownership before ordinary bootstrap/attachment. A removed
member selects the first surviving member in its location. A removed location selects the first
nonempty canonical location, or standalone Empty when all locations are empty. Both produce a
localized notice. Without a saved record, the directory-startup policy chooses eligible empty-Site
creation or existing standalone Open. Invalid or unsupported records are preserved,
disable preference writes for that invocation and produce a localized notice. Explicit context or
Agent startup retains its exact target. Preferences never grant input, change Create provenance,
launch, dispose, change checkpoint data or move another live Client.

This accepted modeling record separates durable advice, Client-local selection, pending draw proof
and asynchronous storage publication. Exact membership and pane incarnation guard publication;
restart resolves current membership before the existing bootstrap owner runs. Failure preserves
terminal state. `NativeStartupPreference` models two Clients, two members, removal, stale/covered/
failed draws, coalescing and atomic publication. Filesystem permissions and transport remain covered
by private-cache and native carrier checks. Required evidence is codec/resolution tests, stale-draw
and independent-Client tests, actual carrier ordering, selected TLC coverage and Windows ConPTY
detach/reopen and removed-member scenes. This does not claim Windows Terminal manual acceptance
or completion of the repository's pending model checks.

Run `node scripts/tla/run-models.mjs --models native-startup-preference` for this bounded model.
The complete graph has 37,560 distinct states, depth 17 and all ten required action witnesses;
it has no state constraints or symmetry and makes no arbitrary filesystem-progress claim.

## Correctness evidence

`NativeContextSelection` covers one Site, standalone and two Workspaces, two retained bindings,
an Empty Workspace, four operation epochs, two navigation revisions and five known Create outcomes.
Its 38 required action witnesses include both directions through Empty, wraparound, busy refusal,
validated snapshot before cleanup, atomic installation, stale epochs and provenance on reentry.
TLC checks 188,409 distinct states without state constraints or symmetry. Weak fairness of expiry
supports eventual stable/retired settlement within this abstraction. Existing Switch,
membership-removal and refresh models retain detailed transport, first-frame, geometry and cleanup
reserve obligations; the new model does not prove history pruning or arbitrary runtime progress.

Run the registered model with `node scripts/tla/run-models.mjs --models native-context-selection`.
Snapshot validation refines the model's read step, the membership owner and mailbox transaction
refine cleanup/install, and the carrier trace refines snapshot/detach/attach commands. Local location
selection has no Create, Start, Dispose or canonical navigation-write command.

Deterministic tests must cover same-label distinct identities, navigation revision changes,
unsupported nonactive members, target refusal, Empty in both directions, provenance retention,
busy mutation/Close admission and stale results. Carrier tests must observe exact Detach before
Attach and no navigation mutation or implicit Create. Linux native process evidence must exercise
Workspace selection, retained standalone reentry, explicit creation in an empty configured
Workspace, independent peer selection, and Workspace Close-to-Empty with the exact shell's exit.
Rendering tests cover both locales, terminal-text sanitization and bounded viewports.
These checks establish local development behavior, not packaged or cross-platform readiness.
