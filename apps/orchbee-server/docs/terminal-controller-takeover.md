# Terminal controller takeover

## Authority and model

An explicitly requested takeover replaces the input-and-resize controller of one existing terminal.
It leaves both read-only Viewers attached and never starts, stops, or selects a terminal. Ordinary
Acquire still refuses an occupied terminal. OBCS `takeover` carries only the submitting Session's
`attachmentId`; Server selects the incumbent and allocates fresh operation/controller identities.
Clients cannot name another Session's controller or supply their own grant.

Server owns an eight-entry reservation map keyed by Worker identity/generation and Host
epoch/terminal. A reservation contains its Viewer, controller and Pending or Active phase.
Worker owns the corresponding Host attachment and serializes whole control operations; Host owns
the actual input capabilities and resize lease. Registry, Site and reservation locks are acquired
in that order and released before any transport wait.

| State      | Event and guard                                                  | Next state and effect                                                              |
| ---------- | ---------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Vacant     | Acquire or Takeover, current unfenced target                     | Pending new grant; submit Worker Acquire                                           |
| Active old | Ordinary Acquire                                                 | Refuse controller-busy; preserve old grant                                         |
| Active old | Explicit Takeover                                                | Pending new grant; fence old Server effects; candidate owns old and new cleanup    |
| Pending    | Another Acquire or Takeover                                      | Refuse controller-busy; never steal incomplete work                                |
| Pending    | Worker proves new Host grant, target still current and unfenced  | Active new; publish Acquired                                                       |
| Pending    | Refusal, cancellation, delivery loss, or failed revalidation     | Clean exact old/new grants; remove only after proof, disposal or Worker retirement |
| Active     | Exact controller Input, Resize or Paste with increasing sequence | Submit effect under that grant                                                     |
| Active     | Stale controller effect or cleanup                               | Refuse effect; never release replacement grant                                     |

## Transfer and failure semantics

OBCN Takeover carries the expected previous controller. Worker first validates the new Viewer and
exact terminal, controller uniqueness, and the expected incumbent. An already-released incumbent
is allowed; a different live incumbent is refused. Within one serialized operation, Worker detaches
the old Host controller and waits for its exact acknowledgement before attaching the new one.
There is a possible unavailable interval, but never two valid controllers. Previously queued old
input can finish before this transfer; old input, paste or resize dispatched after the transfer is
rejected by Worker even if it passed Server admission earlier. No command is replayed.

Server transfers cleanup responsibility when reserving takeover, before transport submission.
Failed takeover can therefore revoke the incumbent even when no replacement is granted. It never
silently restores old authority. Until Worker confirms acquisition, the candidate owns both exact
cleanup requests, including pre-delivery rejection/backpressure. Lost cleanup proof retains the
Pending fence; it is not a vacant terminal. Late cleanup for the old grant cannot remove the new
reservation or its Host attachment. Known target disposal and Worker retirement settle ownership.
Existing typed Rejected, Unavailable and OutcomeUnknown outcomes remain distinct.

## Compatibility and evidence

This additive unshipped OBCS v2 action and private OBCN action tag 5 require one development cohort.
Older peers reject the action; there is no fallback that disconnects another Client or retries an
uncertain effect. No new Host protocol or dependency is needed. Native stream control and attach
consume this authority in later slices; this contract alone does not deliver those CLI commands.

Protocol corpus checks cover strict action decoding and correlation. Reservation tests cover
pending exclusion, cleanup ownership and stale removal. Windows-capable Worker local-pipe checks cover acknowledged
detach-before-attach, refusal/failure and old-token input/resize fences. Same-author targeted tests
do not establish independent review, formal verification, crash recovery or packaged Windows
interactive acceptance. Final cohort acceptance remains required; TLA execution is deferred.
