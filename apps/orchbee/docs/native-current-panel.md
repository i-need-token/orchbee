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

Explicit/caller Panel reads remain headless: they receive primary Agent membership, terminal
title, directory and process observations, but no inferred Client focus. `scroll` is null with
`scrollObservation.state: no-client`. Unique/explicit Client reads include focus, its observed
live/history scroll or explicit unavailable/pending state, and read-operation provenance.
`process-info` continues to read only the resolved primary Terminal and does not promise that
optional OS facts remain current after sampling.

## Lifetime and failures

One absolute five-second deadline covers discovery, cancellable admission/snapshot, optional
Client list, enrichment, and final revision checks. Client listing retains its two-second
exchange limit. One interrupt owner supplies the wake handle to every cancellable read; there
is no second admission, renewed budget or replay. Pending cancellation is settled before output
(exit 130); deadline expiry yields exit 7. Terminal/process unavailability remains explicit data
in enriched queries; protocol, permission or transport failures produce no partial success.

Session ownership follows `prepare → admit/snapshot → resolve → enrich/read → fence → drop → emit`.
Read-only Agent-attention and viewport operations may create bounded Client operation receipts;
they do not acknowledge attention, focus, scroll, resize or send terminal input. Each Host viewer
detaches on outcome; retiring the admitted connection is the cleanup fallback. A partial reply
that times out retires the connection, while actual EOF remains transport loss.

Routing refusals retain `orchbee.cli.failure.client-control` v1. Enrichment uses the existing
Workbench-query failures, including `client-changed` and `workbench-changed`, without partial
results. Generic current-stage failures retain the exact `current-panel` stage. Windows success
uses `orchbee.panel.current` v4, with the enriched Panel Get v7 shape and its own schema ID.
Non-Windows current retains v3; the embedded schema strictly separates these shapes.
`orchbee.panel.process-info` remains v2. Catalog v84 contains 88 documents and 99 command paths.

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
v3/v4 schemas, and absence of mutating controls or input requests. Actual Windows PowerShell 5.1/7 and two ConPTY Clients exercise selection and
unchanged focus; a partial reply plus Ctrl+C exercises cancellation and peer teardown. These
checks do not establish atomic snapshot/Client/OS observations or other-platform support.

No shared wire protocol, dependency, durable state, or migration changes. Existing non-Windows
query behavior remains unchanged; `--client` is Windows-only. Consumers of Windows current v3
must accept the v4 enrichment contract. Rollout and rollback replace the CLI executable within
the existing cohort; they do not migrate saved data.
