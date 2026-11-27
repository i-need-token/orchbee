# Unified search

## Authority and scope

This reference owns the maintained Workbench search interaction model, provider coordination,
result projection, and correctness claims. `@orchbee/actions` owns registration, prepared
invocations, execution, confirmation, concurrency, and cancellation after invocation. Search
providers and their semantic data remain with their feature owners; the application composition
selects providers and platform adapters.

The current implementation includes bounded synchronous and asynchronous provider coordination,
local Actions and Settings projections, deterministic ranking, nested Action input, retry, failure
isolation, accessibility behavior, and stale-completion rejection. No Agent-owned or active
Terminal-instance provider is currently composed.

File search, persistent query or action history, user-defined keybindings, action chaining, opaque
ranking, telemetry containing query or result data, and Workbench-owned Agent or Terminal runtime
state are excluded.

Every distribution offers only the `Actions` filter, without filter tabs, and no entry point to
the others; see [release-withheld Search](workbench.md#release-withheld-search).

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

The local Action catalog subscribes to its mounted runtime and reconciles again after subscribing,
so registry changes between rendering and subscription cannot leave the visible catalog stale.

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

| Event                  | Guard                                                             | Result                                                                                       |
| ---------------------- | ----------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Open                   | Any state                                                         | Create a new session, reset query and provider projection, and request applicable providers  |
| Query changed          | Query value differs                                               | Increment revision, cancel prior runs, clear their projection, and request current providers |
| Filter changed         | Browsing and filter differs                                       | Preserve query, increment revision, cancel prior runs, and request applicable providers      |
| Provider started       | Session and revision are current                                  | Record one loading run for that provider                                                     |
| Provider completed     | Current execution, session, revision, provider, and attempt match | Replace only that provider projection with bounded ready results                             |
| Provider failed        | Current execution, session, revision, provider, and attempt match | Replace only that provider projection with a stable failed state                             |
| Retry provider         | Failed run is current                                             | Increment only that provider attempt and start a replacement run                             |
| Provider model changed | Provider remains applicable                                       | Replace only that provider run; other providers and current highlight remain stable          |
| Start argument picker  | Current result carries supported unbound input                    | Preserve the browsing snapshot and enter the nested picker                                   |
| Picker read settled    | The same picker entry is still open                               | Replace the one disabled reading row with the choices, or with one disabled row saying why   |
| Back                   | Picking an argument                                               | Restore filter, query, revision, highlight, provider projection, and measured scroll         |
| Close or unmount       | Surface is open                                                   | Cancel provider work, clear the mounted projection, restore semantic focus, invoke nothing   |

A picker either carries its options or reads them. The composition supplies a reading picker for an
Action whose choices only it can see, such as the checkouts Git lists where the focused Panel stands.
It reads once per entry into the picker; going back, closing, or unmounting aborts the read and
discards its answer, so entering again reads again. Until the read settles, one disabled row says it
is reading; a read with nothing to offer, or one that rejects, leaves one disabled row in the
picker's words, which invokes nothing. A catalog projection that replaces the picker object while
it is open does not start another read.

An `AbortController` is disposal assistance, not correctness authority. Every completion is checked
against the exact active execution as well as session, revision, provider ID, and attempt even when
its provider ignores cancellation. Reconnecting with the same request fields never authorizes a prior
connection's completion.

The coordinator commits each transition and retires superseded executions before calling external
cancellation handlers or snapshot listeners. A synchronous callback may submit a newer query or
provider model change; the older invocation cannot overwrite, cancel, or start work for that newer
run. Each current loading run starts at most once, and queued work checks ownership again before
invoking a provider. Releasing a connection refuses further provider transitions during cleanup.

Each connection owns its provider subscriptions. A partial connection failure releases earlier
subscriptions; cleanup releases every owned subscription once before reporting any failures. An old
connection handle cannot close a newer connection. Acquisition and cleanup failures remain available
to the engineering caller and never become raw product copy.

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

## Quick input sessions

Where the File Bar is offered, the Workbench holds one `WorkbenchQuickInputSessions` coordinator and
renders its current session with the search surface's geometry. A feature requests a pick session
(items, separators, descriptions, title, step and busy state; it settles with one item's value) or
an input session (prompt, value, masking, a value check with severity, and surviving blur on
request; it settles with the value). The first feature that requests sessions exposes the
coordinator to its owner; no distribution requests one today.

- One session is current at a time. A new request cancels the current one before it opens, and
  every request settles exactly once, as accepted or cancelled; Escape, a blur the session does not
  survive, and unmounting the Workbench cancel it.
- An event names its session, so one that arrives after that session ended changes nothing.
- An input session shows only the current value's check: a change aborts the previous check, and an
  answer for an older value is dropped. Enter accepts only once the current check answered without
  an error.
- An open session registers the `quick-input` keyboard scope, the narrowest, so keys a picker binds
  win over wider bindings while it has focus; see
  [Keyboard scopes](keyboard-shortcuts.md#keyboard-scopes).

Multiple selection, item and inline buttons, and an initial value selection are not built yet.

## Maintenance

- Keep provider transitions, coordinator, React adapters and deterministic/generated regressions aligned when provider lifecycle semantics change.
- Reduce discovered failures to deterministic regressions before changing implementation.
- Compose providers through public provider and Action Runtime contracts, without a Workbench state writer or arbitrary execution callback.
- Admit Agent or Terminal results only after their semantic owner exposes an accepted read-model and Action contract.
