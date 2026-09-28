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
selected Client record, including its view revision, before joining the same Agent, terminal
and viewport observations as `panel get`. It never reselects a Client or redirects to a new
Panel. Before publication, the canonical revision and selected Client view/revision must still
match. A move away and back also fails because the view revision advanced. These fences do not
make Host/process/scroll observations atomic; each retains its own provenance.

Explicit/caller `panel current` reads remain headless: they receive primary Agent membership, terminal
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
uses `orchbee.panel.current` v6, with the enriched Panel Get v9 shape and its own schema ID.
Non-Windows current retains v3; the embedded schema strictly separates these shapes.
`orchbee.panel.process-info` remains v2. Catalog v95 contains 91 documents and 108 command paths.

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
must accept the v6 enrichment contract. Rollout and rollback replace the CLI executable within
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

The passive DTO is bounded to 4 KiB and has a required nullable `presentation` field. A present
member contains its exact installed viewport observation; an absent member does not manufacture
viewport facts. Native projection validates canonical owner and binding before exporting the
existing local generation token. Server retains only a bounded operation receipt. The private
matched cohort rolls out together and rolls back with source; no durable migration is required.
Client control receipt v6 admits this new intent while v1–v5 remain closed.

Evidence covers strict contract parsing and reply identity, native child projection without
primary borrowing, caller child resolution, and controlled Windows CLI request ordering and
stale-member rejection. Full Windows/provider/multi-Client acceptance remains a final gate.
Full `panel current` enrichment and report writes still require their own exact-member adapters.
