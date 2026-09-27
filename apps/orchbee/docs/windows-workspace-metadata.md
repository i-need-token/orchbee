# Windows Workspace metadata

## Ownership and scope

Workspace metadata is transient, externally reported display data. The Workbench authority
owns its values, source sequence high-water marks and monotonic expiry under the existing
SiteCoordinator gate. It does not change canonical topology, Agent lifecycle, terminal OSC
metadata, Git state or configuration. Restart starts with empty metadata, as in pinned
Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`. Rename and move preserve Workspace identity;
closed Workspace identities are pruned before the next metadata observation or report.

The recurring consumers are `workspace report-metadata`, Workspace list/get and their
automation clients. General metadata/expiry events, auxiliary session snapshots and TUI token
templates have separate integration obligations; this contract does not substitute a query
for an event subscription or template renderer. Panel presentation and Agent reports retain
their separate semantic owners.

## Command and normalization

```text
workspace report-metadata <workspace> --source <id> [--token <name=value>] [--clear-token <name>] [--seq <n>] [--ttl-ms <n>]
```

Resolve an exact Workspace through existing ID/label selection. Parse and normalize the entire
input before runtime admission. Source is trimmed ASCII alphanumeric or `:._-`, 1–80 characters.
Keys are ASCII alphanumeric or `_-`, 1–32 characters. Later repeated token flags replace earlier
ones. A report changes 1–16 distinct keys; a resource retains at most 32 token keys.
Values are trimmed, stripped of control characters, truncated to 80 Unicode scalar values,
then trimmed again. An empty normalized value clears the key. Source identifies sequence
freshness, not a token namespace: the last accepted writer of a key supplies its value.

Sequence is optional unsigned 64-bit decimal. TTL is optional 1–86400000 milliseconds and
applies only to values set by this patch. Omitted TTL cancels their previous expiry; untouched
keys keep their own expiry. A clear removes the value, not source sequence history.

## State and transitions

Each Workspace has one token map and at most 32 sequenced sources. Inputs include an explicit
monotonic `now`; the deterministic owner never reads a process clock itself. At a read/report
boundary, expire every deadline at or before `now` and prune missing Workspace identities.
The registry has a checked monotonic revision independent of the Workbench revision.

| Input                                            | Guard                                         | Result                                                           |
| ------------------------------------------------ | --------------------------------------------- | ---------------------------------------------------------------- |
| Fresh or unsequenced report                      | Valid target, whole patch and resource budget | Atomically replace patched values/expiries and accepted sequence |
| Equal or older sequence                          | Source high-water already covers it           | Successful stale no-op; no TTL refresh or resurrection           |
| Report without sequence                          | Other admission checks pass                   | Apply without changing the source high-water                     |
| Too many resulting keys or new sequenced sources | Capacity would be exceeded                    | Reject all writes; do not consume sequence                       |
| Expiry                                           | Deadline is at or before now                  | Remove due values, preserve source high-water                    |
| Overwrite before old expiry                      | Current value has a later/no deadline         | Old deadline cannot remove the replacement                       |
| Workspace disappears or Site stops               | Identity no longer belongs to this owner      | Release its transient state                                      |

The registry admits at most 4096 resource states and 16 MiB of accounted retained state
(UTF-8 payload plus fixed per-entry overhead). It rejects capacity exhaustion without
evicting high-water marks. Revision/deadline overflow is explicit failure, not wraparound.
Prepare the full resource replacement and byte accounting before committing it. Read output
has fresh values at its observation time; it makes no future stability promise.

## Wire and query composition

The existing authenticated Workbench carrier carries `workbench.workspace-metadata` and
`workbench.workspace-metadata.result`, OBWP kinds 31/32. Requests are bounded by 16 KiB,
responses by 128 KiB before allocation. Read requires existing read permission; Report
requires navigation-management permission. No lifetime topology-command ledger entry is
allocated for a high-frequency report. The CLI submits once: a lost reply after submission
is unknown, with request identity and exit 8, and is never silently replayed. Unsequenced
duplicate reports intentionally reapply; sequence freshness is the replay guard when supplied.

Read takes 0–8 ordered unique Workspace IDs; an empty batch reads only the revision fence.
Every requested Workspace must exist. Replies carry Site, Workbench revision and metadata
revision, with one row per requested ID. Report distinguishes applied and stale outcomes.
Unknown fields, noncanonical strings, oversized maps and invalid TTL or counters are rejected
by both bindings. Rust raw JSON decoding also rejects duplicate token properties; the
TypeScript object boundary preserves every legal key, including `__proto__` and `constructor`.
Optional sequence/TTL fields accept omission or null and serialize as omitted when absent.

Workspace list/get joins all rows under one admitted connection and original five-second
deadline. Exact IDs and canonical revision must agree; all batches and a final empty read
must have the same metadata revision. Existing final canonical/Client checks remain.
Concurrent mutation, expiry or missing targets fails the query without mixed/partial output.
Cancel and transport outcomes keep their own meaning. Values and sources are escaped in both
locales, never interpreted as terminal control sequences.

## Evidence and delivery

Deterministic cases and generated traces cover stale/equal/absent/max sequences, atomic key
and source limits, delayed expiry, overwrite races, clear without sequence reset, UTF-8
normalization, closed targets, resource bounds and revision/deadline exhaustion. Shared wire
fixtures cover both language bindings and frame limits. Admission and correlation checks cover the Server boundary. CLI integration additionally
requires process checks for changed revisions, cancellation and uncertainty without replay.
Real Windows acceptance must exercise multi-source writes, reads, TTL, clear, rename/close and unchanged
topology/Client state. Same-author checks are not independent review; TLA remains deferred.

Ship matching cohort components. Revert the matched code to roll back; no persistent migration
or user configuration change is required. Never restore transient sequence/TTL state from a
topology checkpoint.
