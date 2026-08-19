# Terminal Theme Revision

## Domain contract

One Workbench composition coordinates one complete runtime theme revision for its connected Terminal
Host epoch. The transaction changes the Host-owned terminal color state and the matching
client-owned presentation without making either side authoritative for the other.

| Field     | Contract                                                                                                                                                                                           |
| --------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem   | Client-only default-color overrides can disagree with the Host palette, cursor, projected pixels, overlays, and application surface.                                                               |
| Inputs    | A Workbench-owned operation ID, the last observed Host presentation revision, and one complete normalized presentation snapshot.                                                                   |
| Output    | One completed revision, a typed stale/failure/cancellation result, or an intentional no-op for the already-current complete snapshot.                                                              |
| Non-goals | Font, padding, opacity, cursor shape/blink, VT content revision, persisted preference layout, a new Settings surface, and browser-side VT or palette authority.                                    |
| Owner     | Workbench owns intent, supersession, and publication eligibility; Terminal Host owns revision assignment and terminal color commit; the projected client owns preparation and atomic presentation. |

`TerminalThemeSnapshot` is complete:

- `terminal` contains `colorScheme`, default foreground/background, nullable cursor RGB, and exactly
  256 palette RGB values. A null cursor has the explicit meaning “use the terminal default
  foreground”; it is not missing data.
- `overlays` contains cursor accent, selection foreground/background, search match
  foreground/background, and active search match foreground/background.
- `application` selects the complete `light` or `dark` design-token mapping for the application
  surface. Design tokens remain authoritative for the mapping's concrete interface colors.

All RGB values are unsigned 24-bit integers. The complete snapshot is immutable after request
admission. `defaultColorOverride` is not an authority or an input to this transaction.

## Revision and operation identity

Presentation revision `0` is the launch baseline for one Host epoch and has no operation ID. A
Workbench request supplies a unique operation ID and `expectedRevision`; the Host serializes
admission and assigns the next positive revision only when that expectation still names the current
Host revision. Revision is a commit barrier, so a transaction that changes only overlays or the
application surface still advances the Host revision without inventing terminal color differences.
A newer request always supersedes an active operation, even when its desired snapshot equals the
currently visible snapshot: the Host may already have committed the older, not-yet-presented
operation, so the newer intent must restore Host and presentation alignment through its own
revision.

The Host retains the current operation ID, revision, and complete terminal snapshot. An exact replay
of the current operation and terminal snapshot returns `already-applied` without another mutation.
Reusing the operation ID with different data is a non-retryable conflict. Two different operations
based on one revision race through compare-and-set: one may commit and the other becomes stale.

## Transition model

| Current state                   | Event                                   | Result and commands                                                                                                                                   |
| ------------------------------- | --------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| no active request               | request a changed complete snapshot     | Supersede nothing; prepare the client snapshot and request a Host commit against the last observed Host revision.                                     |
| request active                  | newer request                           | Revoke publication eligibility from the older operation; keep the last presented snapshot. Already-started work may settle but cannot publish.        |
| Host pending                    | exact Host commit                       | Retain the returned Host state; publish only after the same latest operation is client-ready.                                                         |
| Host pending                    | stale result                            | Retain the current presentation, adopt the returned Host state, and permit an explicit rebase of the same desired operation.                          |
| Host pending                    | known retryable pre-commit failure      | Retain both authorities and permit an explicit retry against the same observed revision.                                                              |
| Host pending                    | unknown outcome                         | Retain the current presentation and read/reconcile Host state before any retry. Blind replay is forbidden.                                            |
| client pending                  | preparation failure                     | Retain the current presentation. A retryable failure may prepare the same immutable snapshot again; a Host commit is never rolled back by the client. |
| Host committed and client ready | latest matching operation               | Atomically publish terminal pixels, cursor, overlays, and application surface at the Host-assigned revision.                                          |
| hidden or suspended             | latest matching operation becomes ready | Retain it without exposing partial presentation; resume publishes only the newest eligible complete revision.                                         |

A stale, failed, cancelled, hidden, or suspended path never writes a subset of either the Host or
client field set. Host commit may lead the visible presentation while client preparation is pending;
the prior complete presentation remains visible until the exact latest operation can publish.

## Results and retry

| Result                      | Meaning                                                                        | Retry rule                                                                                      |
| --------------------------- | ------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------- |
| `completed`                 | The Host operation and complete client presentation committed at one revision. | No retry.                                                                                       |
| `cancelled/superseded`      | A newer Workbench operation revoked publication eligibility.                   | Never retry the old intent.                                                                     |
| `stale`                     | Another operation changed the Host revision before this compare-and-set.       | Adopt current Host state, then explicitly rebase the still-current desired operation.           |
| known retryable failure     | The named owner did not commit.                                                | Retry only that failed owner; preserve already-prepared or committed peers.                     |
| known non-retryable failure | The candidate or operation identity is invalid or conflicting.                 | Start a corrected operation with a new ID.                                                      |
| unknown Host outcome        | Transport loss prevents deciding whether Host committed.                       | Reconcile the retained Host operation/revision/snapshot first; then complete locally or rebase. |

## Correctness evidence

[`../formal/tla/TerminalThemeRevision.tla`](../formal/tla/TerminalThemeRevision.tla) checks two
independently ordered operations, Host/client preparation, Host compare-and-set, exact replay,
operation conflict, known and unknown failures, reconciliation, retry, supersession, and
hidden/suspended presentation.

| SU-09 claim                                                | TLA+ property                                                                     | Deterministic refinement                                                                                                     |
| ---------------------------------------------------------- | --------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Host defaults, palette, and cursor commit together         | `HostCommitIsAtomic`, `HostChangesOnlyOnCommit`                                   | The Host transaction introduced in the next implementation slice must prepare every fallible resource before its one commit. |
| Pixels, overlays, cursor, and app surface publish together | `PresentationIsAtomic`, `PresentationChangesOnlyOnPresent`                        | `transitionTerminalThemeRevision` returns one complete committed snapshot only after exact Host and client results agree.    |
| Superseded/stale/failed paths preserve presentation        | `NonPresentationActionsPreservePresentation`, `SupersessionPreservesPresentation` | Operation-correlated events that are not the latest matching success return no publication command.                          |
| Unknown outcomes are not blindly replayed                  | `UnknownOutcomeRequiresReconciliation`                                            | Retry of an unknown Host outcome emits reconciliation rather than another apply command.                                     |
| A ready current revision makes progress while visible      | `EligibleReadyEventuallyResolves`                                                 | The composition must execute the emitted atomic publication command or report its typed failure.                             |

The TLA+ `Present` action abstracts the successful `publish-theme` command followed by its exact
`presentation-committed` result; the deterministic reducer keeps those two adapter steps explicit.

The checked bound is two operations and two Host revisions. RGB values and the 256 palette entries
are abstracted to their owning operation/revision because schema and deterministic contract tests
own their concrete validation. The model does not prove a future Host or renderer refinement until
their claim-mapped tests and system trace exist.

## Protocol staging

This model slice does not register an unexecutable wire request. The Terminal Host protocol remains
at its current exact version until the Host implementation slice. That slice must first add the
request/result schemas, limits, message registry entries, TypeScript and Zig codecs, versioned
conformance fixtures, and recovery snapshot shape; only then may it add Host mutation. The client
slice must carry the committed presentation revision in projection metadata before presenting the
new transaction.
