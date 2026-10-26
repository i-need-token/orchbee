# Client multiplexed connection

## Decision and scope

Accepted, 2026-10-24. ORC-198 replaces a TUI process's feature connections with one
OBLC admission and one OBWP v1 multiplexed carrier. One-shot CLI invocations retain
their existing single connection. Existing terminal, Workbench and Agent codecs
remain the semantic authorities; the carrier does not reinterpret their outcomes.
The change is Consequential/Modeled. It introduces no async runtime or dependency.

## Ownership and ordering

The Client connection actor owns the admitted stream, reconnect, stream identities,
cancellation and subscription replay. A request is never replayed after possible
handoff. Each subscription reopens with a fresh snapshot after connection loss or
gap; old-generation replies cannot settle a new stream. Logical lanes retain their
existing domain cancellation and result correlation without opening connections.

The Server uses one reader and one writer/dispatcher per admitted connection.
The reader admits bounded messages; the writer owns terminal connection state,
subscription state and all output. State owners publish nonblocking invalidations
after commits. Subscription evaluation and socket writes run outside state locks.
State subscriptions coalesce to the latest value. Event subscriptions retain bounded
ordered pages; overflow discards the pending page and emits a gap before another
snapshot. Persistent overrun closes only that connection with a typed reason.

## Carrier and subscriptions

OBWP remains version 1. General stream open/frame/close records carry a nonzero
connection-local stream ID, independent of terminal graphics producer identities.
An open contains a bounded existing semantic request; subscription opens distinguish
the snapshot request from the incremental request. The Server admits only read
operations as subscriptions. Cancellation ends that stream and cannot retract a
submitted mutation. Connection disposal releases every owned subscription and
terminal resource once. Existing journal epoch/sequence and explicit gap semantics
remain authoritative.

## Correctness and evidence

The small shared protocol model covers independently ordered publication, delivery,
overflow, resnapshot, cancellation and reconnect. Its claims are snapshot before
increment, no delivery after cancellation or from an old generation, bounded queues,
and isolation of a slow Client. Existing models do not cover this new carrier
boundary; ordinary request codec tests cannot explore these competing transitions.
The model abstracts payload validation and operating-system scheduling; controlled
implementation traces check the mapping and malformed/bounded frame admission.

Performance comparison uses the same Windows host and private runtime, one idle
Workspace/PowerShell, ten seconds of warm-up and three thirty-second samples of
Client/Server/Worker CPU time, thread context-switch counts, thread counts and OBLC
connections. Baseline source is d17d2b5e6; the final candidate uses the same workload
and tools. Context switches are a scheduling proxy, not hardware wakeups. Native
measurement and final verification remain explicit evidence, never inferred from
source. OS process sampling and Host screen sampling without a notification source
remain bounded and explain their missing event source beside the polling code.

## Delivery

Implement carrier and bounded Server dispatch, then the Client actor and every
feature lane, then remove old connection/reconnect paths and obsolete polling.
Update Server/TUI owning documents and the architecture data flow in both languages.
Delete this proposal when the complete behavior is described by current authorities.
