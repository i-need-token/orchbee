# Native current Panel queries

## Authority and scope

This reference owns Windows target selection and process lifetime for `panel current` and
`panel process-info`. [Native CLI queries](native-cli-query.md) owns their result projections;
[Windows Client control](windows-client-control.md) owns Client records and routing failures.
The existing caller resolver owns Terminal/Worker identity validation. These commands observe an
existing Site; they never activate it or change selection, input, geometry, or control authority.

## Target model

The grammar produces exactly one of `Current`, `Panel(target)`, or `Client(id)`:

| Input                                                                | Resolution                                                                                                    |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `--panel <id-or-suffix>`; also a positional Panel for `process-info` | Resolve against the snapshot, ignoring caller environment and without reading Clients.                        |
| `--client <id>` with omitted target or `--current`                   | Select that current Client, ignoring caller environment.                                                      |
| Omitted target or `--current`, with both caller variables absent     | Require exactly one current Client.                                                                           |
| Omitted target or `--current`, with caller metadata present          | Validate both `ORCHBEE_TERMINAL_ID` and `ORCHBEE_WORKER_ID`, then resolve the caller's Panel in the snapshot. |

Explicit Panel and Client selectors conflict. Partial, malformed, or stale caller metadata never
falls back to Client selection. Multiple Clients are ambiguous; a missing or stale explicit Client
fails. `panel current` accepts Ready or Busy observations (history reading is Busy);
`process-info` retains Ready-only selection. The selected Panel must still belong to the reported
Context and Tab in the snapshot. Missing or changed membership fails with
`client-panel-unavailable`; the query never follows that Panel into another location implicitly.

The snapshot and Client record are successive observations. `panel current` pins the
selected Client record, including its view revision, before joining Agent, terminal, report,
metadata and viewport observations for its exact current member. It never reselects a Client or redirects to a new
Panel. Before publication, the canonical revision and selected Client view/revision must still
match. A move away and back also fails because the view revision advanced. These fences do not
make Host/process/scroll observations atomic; each retains its own provenance.

Explicit/caller `panel current` reads remain headless: they receive resolved-member Agent membership, terminal
title, directory and process observations, but no inferred Client focus. `scroll` is null with
`scrollObservation.state: no-client`. Unique/explicit Client reads include focus, its observed
live/history scroll or explicit unavailable/pending state, and read-operation provenance.
`process-info` reads the exact caller/Client member as defined below; explicit Panel targets use
the primary. Optional OS facts are not guaranteed to remain current after sampling.

## Lifetime and failures

One absolute five-second deadline covers discovery, cancellable admission/snapshot, optional
Client list, enrichment, and final revision checks. Client listing retains its two-second
exchange limit. One interrupt owner supplies the wake handle to every cancellable read; there
is no second admission, renewed budget or replay. Pending cancellation is settled before output
(exit 130); deadline expiry yields exit 7. Terminal/process unavailability remains explicit data
in enriched queries; protocol, permission or transport failures produce no partial success.

Session ownership follows `prepare → admit/snapshot → resolve → enrich/read → fence → drop → emit`.
Read-only Agent-attention, Presentation and viewport operations may create bounded Client operation receipts;
they do not acknowledge attention, focus, scroll, resize or send terminal input. Each Host viewer
detaches on outcome; retiring the admitted connection is the cleanup fallback. A partial reply
that times out retires the connection, while actual EOF remains transport loss.

Routing refusals retain `orchbee.cli.failure.client-control` v1. Enrichment uses the existing
Workbench-query failures, including `client-changed` and `workbench-changed`, without partial
results. Generic current-stage failures retain the exact `current-panel` stage. Windows success
uses `orchbee.panel.current` v8, with canonical Panel structure and an explicit `currentPresentation`.
Non-Windows current retains v3; the embedded schema strictly separates these shapes.
`orchbee.panel.process-info` remains v2. Catalog v100 contains 92 documents and 109 command paths.

## Refinement and evidence

The invocation owner enforces the target sum type and selector exclusions. The caller owner
validates environment identity; the pure Client selection owner validates availability, readiness,
and snapshot membership. Windows query composition owns the deadline, signal owner, one session,
and result emission. The Workbench-query collector owns shared enrichment and final consistency
checks. Existing process-query code owns Viewer cleanup and Host decoding.

Grammar/help/completion evidence covers each selector and conflicting option. Scripted Windows
peers establish explicit/caller paths without Client reads, unique/explicit Client selection,
ambiguity, invalid caller, stale membership, bounded partial replies, enriched/unavailable
observations, stable Busy/history reads, unchanged process-query readiness guards, changed Client views (including move-away-and-back), changed snapshots, strict
legacy/current schemas, and absence of mutating controls or input requests. Actual Windows PowerShell 5.1/7 and two ConPTY Clients exercise selection and
unchanged focus; a partial reply plus Ctrl+C exercises cancellation and peer teardown. These
checks do not establish atomic snapshot/Client/OS observations or other-platform support.

The passive Presentation read extends the matched private wire cohort; no dependency, durable state,
or migration changes. Existing non-Windows
query behavior remains unchanged; `--client` is Windows-only. Consumers of Windows current v3
must accept the v8 enrichment contract. Rollout and rollback replace the CLI executable within
the existing cohort; they do not migrate saved data.

## Exact current Presentation for process information

Windows `panel process-info` resolves an explicit Panel to its primary terminal. With caller
metadata it retains the exact validated terminal, including a child Presentation. With a Client
it passively reads `Presentation { panelId }` before querying the Host: the native Session owns
the selected Presentation, binding, attachment generation and pane incarnation. Missing local
membership is unavailable, never a fallback to the primary. Existing process-info v2 already
identifies the exact terminal and Presentation and remains closed.

The workflow is snapshot → exact member read → Host process read/viewer cleanup → canonical
revision fence → exact member read. Both Client reads require the original view revision and
matching observed view; their Presentation ID, binding, attachment generation and incarnation
must agree. Viewport updates alone do not invalidate process information. Canonical change,
Client selection change, detach/reattach or away-and-back selection fails before publication.
Explicit/caller reads also fence canonical membership. All phases share the existing session,
five-second absolute deadline and cancellation owner; reads send no input or focus request.

The passive DTO is bounded to 4 KiB and has a required nullable `presentation` field. A bound
member contains its exact installed viewport observation; an absent member does not manufacture
viewport facts. Native projection validates canonical owner and binding before exporting the
existing local generation token. Server retains only a bounded operation receipt. The private
matched cohort rolls out together and rolls back with source; no durable migration is required.
Client control receipt v6 admits this new intent while v1–v5 remain closed.

Evidence covers strict contract parsing and reply identity, native child projection without
primary borrowing, caller child resolution, and controlled Windows CLI request ordering and
stale-member rejection. Full Windows/provider/multi-Client acceptance remains a final gate.
The following enrichment uses this same member identity; [Report writes](windows-panel-agent-reports.md#exact-member-targets) select an explicit member separately.

## Current member enrichment

Windows `panel current` v8 resolves one canonical Presentation before joining observations.
Explicit Panel targets choose their primary (including an unbound primary), caller metadata
chooses that exact bound terminal, and Client targets use the passive exact-member read.
`currentPresentation` identifies this member, its canonical label/override/binding and origin
(`primary`, `caller`, `client` with local attachment generation/incarnation, or
`client-unbound` with selection generation). Canonical
`primaryPresentation` and `childPresentations` retain their actual structure. Existing v3–v7
remain closed; Panel Get/List retain their existing primary semantics.

Current Agent membership, terminal title/directory/runtime, reports, metadata and Client scroll
all use the same resolved member. Exact report reads validate Presentation ID, owning Panel and
binding against the initial snapshot; missing or mismatched rows fail. Metadata/report revisions
are independently fenced with the existing empty global reads, whose empty rows name no primary
and whose revision owners are shared with exact-member reads. Aggregate limits and the overlapping
read/fence order remain unchanged. Headless reads do not infer Client focus or scroll.

A Client member read brackets the joins and final canonical-revision check. Both receipts require
the original Client view revision and observation; local member tokens must agree, while viewport
updates alone are allowed. Both operation IDs are retained in query provenance. Missing member,
canonical change, reattachment or selection change yields no partial result. This uses one admitted
session, the original five-second deadline, and existing signal/viewer cleanup owners. It does not
claim atomic Host/process/Agent observations, and creates no input or focus effects.

Pure target tests and actual Windows CLI peer traces cover primary, child caller, selected child,
unbound explicit primary, exact report/metadata association, stale tokens/revisions, failure
envelopes and legacy schema rejection. The final provider/multi-Client/full gate remains deferred.
No new dependency or durable migration is required; the unbound wire alternative is described below. Roll out the updated native CLI
with its existing matched cohort, and roll back by source. [Report writes](windows-panel-agent-reports.md#exact-member-targets) select an explicit member separately.

## Unbound current member

A stable selected empty Presentation is a real Client-local member. Its passive wire selection is
`{presentationId, selectionGeneration}`; the nonzero decimal generation is the existing selected
anchor, not a fabricated attachment. Bound selections keep their existing binding, attachment,
incarnation and viewport shape. Null still means no local member. Only Ready or stable
SelectedWithoutAttachment may export empty identities; a pending Switch target is not an observation.
The Client checks exact canonical Panel/member membership and absence of binding before answering.

Windows current v8 uses `source: {kind: "client-unbound", selectionGeneration}` with `binding: null`.
Both member reads must retain the same kind, ID and generation, alongside existing canonical and
Client view fences. Exact report/metadata/Agent joins retain the empty member, scroll is not-attached,
and no physical terminal is queried. Client process-info fails with the existing missing-terminal
result; it never borrows a bound sibling. Legacy current v7 and control v6 reject this new shape.
Strict cross-binding vectors, state/UI reads and actual Windows CLI pipe traces cover these rules;
provider and multi-Client visual acceptance remain final validation obligations. This extends the
matched private cohort and public current v8/control v7; source rollback needs no data migration.
