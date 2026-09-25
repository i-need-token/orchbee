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
fails. A selected Client must be Ready and its selected Panel must still belong to the reported
Context and Tab in the snapshot. Missing or changed membership fails with
`client-panel-unavailable`; the query never follows that Panel into another location implicitly.

The snapshot and Client record are successive observations, not an atomic focus read. The result
identifies the Panel resolved from those observations; a later focus change does not redirect the
query. `process-info` reads the resolved Panel's primary Terminal through the existing read-only
Viewer process query and does not promise that optional OS facts remain current after sampling.

## Lifetime and failures

One absolute five-second deadline covers cohort discovery, admission, snapshot, optional Client
list, and optional Host process read. Client listing also retains its existing two-second exchange
limit. An interrupt owner is armed before admission; Client and Viewer reads borrow its wake
handle. Existing admission/snapshot stages remain bounded but do not borrow that handle. Pending
cancellation is checked before additional reads and settled before output, yielding exit 130 with
the generic cancellation document. Deadline expiry yields exit 7. No request is retried.

The LocalClient exchange owner classifies timed-out I/O, including a partial Client reply, as
timeout and retires the connection. An actual EOF remains transport loss; neither result permits replay.

Session ownership follows `prepare → admit/snapshot → resolve → project/read → drop → emit`.
Process queries retain the existing Viewer detach-on-outcome discipline; dropping the session is
the fallback if detach cannot complete. Routing refusals retain the existing
`orchbee.cli.failure.client-control` v1 schema without a snapshot source; other failures retain
their owning schemas. Successful results remain `orchbee.panel.current` v3 and
`orchbee.panel.process-info` v2. The generic failure v1 schema adds the exact `current-panel` stage for cancellation, timeout,
and Client exchange faults. Catalogue version 41 contains 72 documents and 85 command paths;
result shapes and command paths remain unchanged.

## Refinement and evidence

The invocation owner enforces the target sum type and selector exclusions. The caller owner
validates environment identity; the pure Client selection owner validates availability, readiness,
and snapshot membership. Windows query composition owns the deadline, signal owner, one session,
and result emission. Existing process-query code owns Viewer cleanup and Host decoding.

Grammar/help/completion evidence covers each selector and conflicting option. Scripted Windows
peers establish explicit/caller paths without Client reads, unique/explicit Client selection,
ambiguity, invalid caller, stale membership, bounded partial replies, and absence of control or
input requests. Actual Windows PowerShell 5.1/7 and two ConPTY Clients exercise selection and
unchanged focus; a partial reply plus Ctrl+C exercises cancellation and peer teardown. These
checks do not establish atomic snapshot/Client/OS observations or other-platform support.

No shared wire protocol, dependency, durable state, or migration changes. Existing non-Windows
query behavior remains unchanged; the new `--client` mode is Windows-only. Older CLI executables
reject that flag. Rollout and rollback replace the CLI executable within the existing cohort.
