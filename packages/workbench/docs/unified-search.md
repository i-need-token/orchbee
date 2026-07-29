# Unified search

## Authority and scope

This reference owns the maintained Workbench search interaction model, provider coordination,
result projection, and correctness claims. `@orchbee/actions` owns registration, prepared
invocations, execution, confirmation, concurrency, and cancellation after invocation. Search
providers and their semantic data remain with their feature owners; the application composition
selects providers and platform adapters.

The accepted delivery order is:

1. complete bounded asynchronous provider coordination in Workbench;
2. compose an Agent-owned read model and navigation Action without automatic submission;
3. compose active Terminal-instance Actions without transferring Terminal lifecycle ownership to
   Workbench.

File search, persistent query or action history, user-defined keybindings, action chaining, opaque
ranking, telemetry containing query or result data, and Workbench-owned Agent or Terminal runtime
state are excluded.

## Domain contract

| Field           | Contract                                                                                                                                      |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem         | A person must discover and invoke capabilities through one surface while independently owned providers load, fail, retry, and change order.   |
| Trusted input   | Reviewed open filter, mounted Action scope, registered providers, validated provider results, locale, and shortcut platform                   |
| Untrusted input | Query text and pointer or keyboard events; query text remains in mounted memory and is never executed                                         |
| Output          | Deterministically grouped visible results, loading or failure projections, semantic announcements, and at most one prepared Action invocation |
| No-op           | Stale completions, completions after close, unavailable results, repeated current selections, and cancellation before invocation              |
| Side effects    | Provider request acquisition and cancellation, Action invocation after selection, focus movement, scrolling, and accessibility announcements  |

## Ownership

| Mutable fact                                      | Sole owner                                      |
| ------------------------------------------------- | ----------------------------------------------- |
| Open session, filter, query revision, and picker  | Workbench search interaction state              |
| Provider request identity, phase, and result set  | Workbench provider-coordination state           |
| Provider data and navigation meaning              | The contributing feature                        |
| Action availability and execution                 | Mounted `ActionRuntime` and the semantic owner  |
| Agent session, composer, and submission lifecycle | Agent capability owner                          |
| Terminal instance and process lifecycle           | Application composition and Terminal capability |
| DOM focus, measured scroll, and live announcement | Narrow Workbench React adapter                  |

Providers return values and runtime-owned invocation tokens. They never receive a Workbench state
writer and never contribute an arbitrary execution callback.

## Canonical data

A provider has one stable ID, declares the filters it supports, exposes bounded default and query
operations, and optionally notifies the coordinator when its owned read model changes. Each result
has stable provider-local identity, prepared presentation, explicit grouping metadata, searchable
terms, availability, and either a prepared or unbound Action invocation.

The coordinator assigns repository-wide result identity from provider ID plus provider-local result
ID. Provider completion order is not ranking input. Query matching, recommendation, session
recency, provider priority, group order, and stable identity form the complete deterministic
comparison. Lower numeric provider priority sorts first.

One provider run records:

- provider ID;
- open-session ID;
- query revision;
- retry attempt;
- `loading`, `ready`, or `failed` phase;
- bounded result sections when ready; and
- a stable internal failure code when failed.

Raw exceptions never enter product presentation.

## Transitions

| Event                  | Guard                                          | Result                                                                                       |
| ---------------------- | ---------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Open                   | Any state                                      | Create a new session, reset query and provider projection, and request applicable providers  |
| Query changed          | Query value differs                            | Increment revision, cancel prior runs, clear their projection, and request current providers |
| Filter changed         | Browsing and filter differs                    | Preserve query, increment revision, cancel prior runs, and request applicable providers      |
| Provider started       | Session and revision are current               | Record one loading run for that provider                                                     |
| Provider completed     | Session, revision, provider, and attempt match | Replace only that provider projection with bounded ready results                             |
| Provider failed        | Session, revision, provider, and attempt match | Replace only that provider projection with a stable failed state                             |
| Retry provider         | Failed run is current                          | Increment only that provider attempt and start a replacement run                             |
| Provider model changed | Provider remains applicable                    | Replace only that provider run; other providers and current highlight remain stable          |
| Start argument picker  | Current result carries supported unbound input | Preserve the browsing snapshot and enter the nested picker                                   |
| Back                   | Picking an argument                            | Restore filter, query, revision, highlight, provider projection, and measured scroll         |
| Close or unmount       | Surface is open                                | Cancel provider work, clear the mounted projection, restore semantic focus, invoke nothing   |

An `AbortController` is disposal assistance, not correctness authority. Every completion is checked
against session, revision, provider ID, and attempt even when its provider ignores cancellation.

## Invariants

- Every visible asynchronous result belongs to the current open session and query revision.
- At most one current run exists for one provider and attempt.
- A stale, cancelled, failed, or post-close completion never changes visible results.
- Provider completion order does not change the final grouped result order.
- A highlighted result is absent or resolves in the current visible result set.
- Failure in one provider does not remove or disable another provider's results.
- Retrying one provider neither restarts nor clears unrelated ready providers.
- One accepted result reaches at most one prepared Action execution.
- Closing, provider cancellation, picker cancellation, highlight, hover, and prefetch invoke no
  product Action.
- Agent fallback may prefill an owned composer only; it never submits, starts a process, or incurs
  external cost.
- Terminal results stay bound to the exact active instance registration that produced their
  invocation.

## Correctness evidence

| Claim                                          | Owner section  | TLA+ property                          | Deterministic evidence                                          | Bounds and exclusions                                      |
| ---------------------------------------------- | -------------- | -------------------------------------- | --------------------------------------------------------------- | ---------------------------------------------------------- |
| Stale completion is excluded                   | Transitions    | `VisibleResultsAreCurrent`             | Reordered completion and reopen regressions                     | Bounded providers, sessions, revisions, and attempts       |
| Close clears provider projection               | Transitions    | `ClosedHasNoVisibleProviderResults`    | Close, unmount, ignored-cancellation, and late-completion tests | Provider correctness and network availability are excluded |
| Current highlight remains valid                | Invariants     | `HighlightIsVisibleOrNone`             | Stateful interaction property tests                             | DOM focus is checked separately                            |
| Completion order is observationally irrelevant | Canonical data | `ProviderProjectionIsCurrent`          | Permuted provider-completion property tests                     | Ranking inputs are held equal                              |
| Retry is provider-local                        | Transitions    | `RetryDoesNotInvalidateOtherProviders` | Failed-group retry timeline tests                               | Retry policy beyond explicit user retry is excluded        |
| Selection invokes at most once                 | Invariants     | N/A: Action Runtime owns execution     | Prepared-invocation integration tests                           | Action-domain result correctness is excluded               |
| Focus survives late completion                 | Ownership      | N/A: DOM integration                   | Deterministic browser trace with semantic focus checkpoints     | Supported desktop browser environment                      |

The TLA+ merge configuration explores two providers, two open sessions, bounded revisions and
attempts, cancellation, failure, retry, reordered completion, close, and reopen. Progress assumes a
current uncancelled provider run eventually settles; it makes no network-availability claim.

## Integration sequence

1. Maintain this record and its claim mapping before production behavior changes.
2. Implement and test the pure provider state transition.
3. Check the bounded TLA+ model and convert any shortest counterexample into a deterministic
   regression;
4. implement the provider lifecycle adapter against the stable transition;
5. integrate the adapter through one responsibility-focused Hook;
6. project loading, failure, retry, results, focus, and announcements in React;
7. compose real Agent and Terminal owners only after their separate public contracts are accepted;
8. run package correctness evidence and the root merge gate without retries.
