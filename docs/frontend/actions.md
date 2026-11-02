# Frontend Actions

## Semantic boundary

An action is a named, user-invocable product intent whose meaning survives beyond one control or
input path. Buttons, menus, context menus, keyboard shortcuts, command surfaces, and approved
automation invoke the same action instead of implementing parallel mutations.

GUI and TUI operation meaning, target scope, and outcomes follow
[`INTERFACE-DESIGN.md`](../INTERFACE-DESIGN.md). This document owns the frontend action runtime;
native invocation maps to the same domain contract through its existing owner.

| Behavior                                                      | Representation                                                                                     |
| ------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Discrete product intent with a semantic result                | Action                                                                                             |
| Operation needed by more than one invocation path             | Action                                                                                             |
| Operation requiring stable availability or observability      | Action                                                                                             |
| Local input, hover, disclosure, or list-highlight state       | Component or feature interaction state                                                             |
| Pointer movement, drag preview, or continuous resize input    | Gesture event and reversible feature projection                                                    |
| Committed result of a gesture used by another invocation path | Action                                                                                             |
| Process, network, storage, or resource update                 | Event owned by that resource lifecycle; an action may initiate it                                  |
| Effect command returned by a domain transition                | Package-local domain command under [`MODELING-AND-CORRECTNESS.md`](../MODELING-AND-CORRECTNESS.md) |

Do not wrap every callback in an action. A generic UI primitive continues to expose typed callbacks;
its product-semantic consumer decides whether a callback invokes an action.

An object row that exposes an ellipsis action menu uses one ordered menu model for both the
ellipsis control and the row's `contextmenu` gesture. Both projections preserve entries, ordering,
grouping, icons, availability, destructive treatment, and invocation targets. Right-click or long
press does not silently activate, select, or expand the object unless that state change is itself
an accepted action. Suppress the native platform context menu only inside the owned trigger and
only while a product Context Menu is available. Presentation and interaction follow
[`menus.md`](menus.md).

## Ownership

| Responsibility                                                | Owner                                     |
| ------------------------------------------------------------- | ----------------------------------------- |
| Stable action identity, descriptor, and input contract        | The feature that owns the semantic result |
| Availability, confirmation, handler, and result mapping       | The feature that owns the semantic result |
| Authoritative state and domain invariants                     | The owning domain capability              |
| Registry, scope resolution, and execution lifecycle           | The mounted product runtime               |
| Platform, process, storage, transport, and navigation effects | The application or existing runtime owner |
| Product messages and translations                             | `@orchbee/i18n`                           |
| Local visual and interaction state                            | The narrowest mounted component owner     |

The runtime dispatches and applies cross-cutting policy; it never becomes a global implementation of
feature logic or an authoritative product-state Store. A handler calls the public operation owned by
its feature. Cross-feature behavior belongs in an explicit composition workflow.

Construct one runtime per mounted application composition and inject it, or a narrower invocation
capability, into product packages. Packages never import a process-wide action singleton or select a
platform adapter through host detection.

## Identity and registration

- Name an action ID `{owner}.{resource}.{verb}` with stable dot-delimited segments.
- Never derive an ID from localized copy, a component name, a menu location, or a shortcut.
- One ID identifies one semantic result; entity, target, and option values are typed input.
- Renaming an ID migrates every binding, menu contribution, command result, stored reference,
  diagnostic, test, and automation contract.
- Compatibility aliases are explicit, temporary, non-enumerable, and removed with their last
  consumer.
- Return an exact disposable from registration and remove the registration with its owner.

A registration binds an action to an application, product, feature-surface, or temporary-interaction
scope. Multiple mounted instances may bind the same action ID in different instance scopes; reject a
duplicate ID inside one scope. Resolve from the narrowest active scope outward. Registration order is
never precedence.

Registries are enumerable and notify consumers when definitions, bindings, or availability
dependencies change. Enumeration exposes only actions visible to the requesting context and does not
grant automation or remote authority.

### Discovery and prepared invocation

Discovery returns an invocation-shaped value rather than only presentation metadata:

- `ready` contains a runtime-owned `PreparedActionInvocation` token bound to the action ID, the
  validator's canonical input, and the exact registration identity that resolved it.
- `requires-input` contains an `UnboundActionInvocation` with the action ID, named input contract,
  and exact registration identity. The invoking surface passes collected input to
  `prepareInvocation(...)`; only the runtime can validate that input and create a prepared token.

Narrow discovery starts at the active scope and resolves outward, with the same shadowing semantics
as execution. A global command surface may explicitly request subtree discovery from its application
scope to compose registrations owned by sibling features. Subtree results remain bound to their
registration scopes; a consumer never re-resolves one through an arbitrary current sibling. If the
same action ID is contributed by multiple descendant instances, result identity includes scope and
the surface either disambiguates those instances or asks the owner to expose one broader action with
typed target input.

An action whose required input is not yet available opts into pre-input enumeration with a pure
synchronous discovery-availability projection. Without that projection, failed input parsing omits
the action; the runtime never assumes that an unresolved action is visible. Hidden and
automation-denied registrations remain absent. Selecting a `ready` result executes the stored
prepared invocation so the action ID, input, scope, source, and target cannot drift between
presentation and invocation. Prepared and unbound tokens are runtime capabilities, not serializable
requests. Copying their public fields does not create a valid token. Disposal or replacement of the
bound registration invalidates them; preparation returns no result and prepared execution returns a
typed stale-invocation block instead of resolving an ancestor or replacement registration.

## Definition contract

An action definition supplies or references:

| Field                    | Contract                                                                                    |
| ------------------------ | ------------------------------------------------------------------------------------------- |
| `id`                     | Stable semantic identity                                                                    |
| `title`                  | Product-message descriptor                                                                  |
| `description`            | Optional product-message descriptor that adds decision-relevant context                     |
| `category`               | Stable semantic grouping metadata, never an absolute position across unrelated owners       |
| `icon`                   | Optional approved product-icon name                                                         |
| `keywords`               | Optional localized descriptors or stable non-visible aliases                                |
| `input`                  | Named type plus boundary validator that returns an owner-owned canonical snapshot           |
| `scope`                  | Registration and resolution scope                                                           |
| `availability`           | Pure synchronous projection over one read-only context snapshot                             |
| `discovery availability` | Optional pure pre-input visibility projection for unresolved dynamic input                  |
| `confirmation`           | Optional feature-owned confirmation model for the resolved input and target                 |
| `handler`                | Feature-owned synchronous or asynchronous execution                                         |
| `bindings`               | Default bindings owned once in the feature's responsibility-focused `*-shortcuts.ts` module |
| `execution policy`       | Surface lifetime, concurrency, progress, and cancellation decisions when applicable         |

Use one owner-defined action catalog plus one catalog-bound adapter for both registration and
invocation so trusted feature code retains the compile-time relationship between each action ID,
definition input, handler result, invocation input, and execution result. Registering through the
untyped runtime and merely wrapping later invocation does not satisfy this contract. Dynamic
command, automation, IPC, network, or persisted input remains `unknown` until the owning validator
parses it at the first trusted boundary. The validator must copy every caller-owned mutable
collection or nested value retained by execution; a canonical snapshot never aliases mutable input
owned by the invoking surface.

Action descriptors carry message descriptors, not formatted strings. A menu, button Tooltip,
command result, shortcut reference, progress presentation, and confirmation format the same owned
descriptors through the effective locale under [`localization.md`](localization.md).

## Context and targets

Invocation context contains execution-wide facts, not unrestricted services or feature-private
state:

- invocation source;
- one read-only scope and focus snapshot;
- an `AbortSignal`;
- the semantic target handle when applicable; and
- narrow capabilities explicitly admitted by the owning workflow.

Use semantic target handles such as stable resource IDs; never retain a DOM node as an action
target. The owner declares whether an invocation uses the target captured at invocation or resolves
the current target immediately before execution.

Re-resolve the target after argument collection, asynchronous preparation, and confirmation. A
missing or stale target produces a typed blocked result and never falls through to an unrelated
active item. The handler continues to protect domain preconditions because state may change after
the runtime's last check.

## Availability and confirmation

| Availability | Meaning                                                                    | Presentation                                                      |
| ------------ | -------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| `enabled`    | The action may run for the current input, scope, and target                | Visible and invocable                                             |
| `disabled`   | The action is relevant but temporarily cannot run                          | Visible, non-invocable, with a localized reason                   |
| `hidden`     | The action is irrelevant, unauthorized, unsupported, or unsafe to disclose | Omitted from menus, command surfaces, references, and enumeration |

- Evaluate availability synchronously and purely over one context snapshot.
- Re-evaluate it for presentation, shortcut resolution, and immediately before handler invocation.
- Keep shortcut `when` applicability separate from action availability under
  [`keyboard-shortcuts.md`](keyboard-shortcuts.md#context-and-availability).
- Represent checked state as a semantic toggle value, not visual selection inside one surface.
- Put asynchronous preconditions in an explicit preparation workflow; do not make shortcut routing
  wait on network or process work.

The feature decides whether resolved input and target require confirmation and supplies localized
title, explanation, confirmation label, and risk. The runtime presents the model consistently,
revalidates after acceptance, and does not invoke the handler before acceptance. Confirmation never
substitutes for authorization, target validation, or domain guards. When another party can change
what an open question describes, the feature supplies `watch`, and the runtime withdraws the question
as a cancellation once it reports a change.

## Results

Every invocation completes exactly once with one result variant:

| Result        | Meaning                                                                                |
| ------------- | -------------------------------------------------------------------------------------- |
| `completed`   | The semantic operation committed and may include a typed value                         |
| `unchanged`   | The request was valid and intentionally produced no change                             |
| `blocked`     | A known guard, stale target, unavailable capability, or permission prevented execution |
| `cancelled`   | The caller or owning lifecycle cancelled before commit                                 |
| `failed`      | An expected typed failure or mapped unexpected failure occurred                        |
| `unsupported` | No active registration can serve the invocation                                        |

Expected failures carry stable codes and structured values. Product surfaces map them to localized
messages. Raw exceptions may enter owned diagnostics but never product copy. The runtime maps a
handler exception to `failed` and resolves the completion; callers do not receive an unclassified
rejected Promise.

Record the commit point for every state-changing asynchronous action. Cancellation before commit
produces no authoritative change unless the owner explicitly models reversible optimistic state.
Cancellation after commit cannot relabel a completed operation as cancelled.

## Execution contract

The runtime returns an execution handle immediately. A handler may return an immediate result or a
Promise; callers always observe one execution model.

```ts
interface ActionHandler<Input, Output> {
	execute(
		context: ActionExecutionContext,
		input: Input,
	): ActionResult<Output> | Promise<ActionResult<Output>>;
}

interface ActionExecution<Output> {
	readonly executionId: ActionExecutionId;
	readonly completion: Promise<ActionResult<Output>>;

	getSnapshot(): ActionExecutionSnapshot<Output>;
	subscribe(listener: () => void): ActionDisposable;
	cancel(): void;
}
```

An immediately completed synchronous handler may produce a terminal snapshot before
`ActionRuntime.execute(...)` returns. Consumers read `getSnapshot()` before subscribing and use
`completion` for the exactly-once terminal result; event delivery alone is never the source of
truth.

### States

| State                   | Meaning                                                                      |
| ----------------------- | ---------------------------------------------------------------------------- |
| `resolving`             | Resolving registration, validating input, target, and availability           |
| `awaiting-confirmation` | Waiting for feature-owned confirmation                                       |
| `running`               | Handler work is active                                                       |
| `cancelling`            | Cancellation was requested but the handler has not reached a terminal state  |
| Terminal result         | `completed`, `unchanged`, `blocked`, `cancelled`, `failed`, or `unsupported` |

- State transitions are monotonic and one execution reaches one terminal result.
- One accepted invocation reaches at most one handler.
- `cancel()` is a cooperative request, not proof that work stopped.
- A handler that has crossed its commit point may complete after a cancellation request.
- The runtime never retries, times out, or converts a failure to success implicitly.
- Owner workflows define retry, recovery, timeout, rollback, and idempotency.
- Unmanaged fire-and-forget work is forbidden; every execution has a lifecycle owner and observable
  terminal outcome or returns an externally owned stable job identity.

## Asynchronous lifecycle

Classify asynchronous work by ownership and presentation:

| Mode         | Contract                                                                                                   |
| ------------ | ---------------------------------------------------------------------------------------------------------- |
| Foreground   | The invoking product surface owns pending presentation and remains available until the required result     |
| Background   | The invoking surface may close; an application-owned activity or notification surface observes execution   |
| External job | A process or service owns durable work; the action returns a stable job ID and the application observes it |

Closing or unmounting the invoking control unsubscribes its presentation; it does not cancel accepted
business work unless the action explicitly assigns execution ownership to that surface. Disposing
the action runtime requests cancellation of runtime-owned executions. External jobs follow their
process or service lifecycle and must not be represented as durable only because an in-memory
Promise remains pending.

Progress is structured state, not product strings emitted by arbitrary handlers:

```ts
interface ActionProgress {
	readonly phase: string;
	readonly completed?: number;
	readonly total?: number;
	readonly message?: MessageDescriptor;
}
```

- Omit `completed` and `total` for indeterminate progress.
- Keep totals stable within one phase or publish a new phase.
- Format message descriptors at presentation; never place raw exceptions or user-sensitive input in
  progress.
- Bound and coalesce high-frequency progress so rendering and telemetry do not become the workload.
- A Button, menu follow-up, command surface, and activity view observe the same `executionId`; they
  do not own competing pending state.

Late completion never steals focus or navigation from later user work. Restore focus or navigate
only while the execution still owns the relevant interaction token; otherwise report through the
application's nonintrusive result channel.

## Concurrency

Every asynchronous action defines how another invocation with the same semantic concurrency key is
handled:

| Policy             | Behavior                                                        |
| ------------------ | --------------------------------------------------------------- |
| `parallel`         | Start another independent execution                             |
| `reject-duplicate` | Return a blocked result while the matching execution is active  |
| `replace-latest`   | Request cancellation of the previous execution and start latest |
| `serialize`        | Queue matching executions in a deterministic order              |

The owner derives the key from the minimum relevant action ID, target, and validated input. Do not
use a global default, object identity, localized text, registration order, or incidental component
instance as the key.

Define the behavior when cancellation is delayed, a replaced execution crosses commit, a queued
target becomes stale, or completion arrives out of order. A caller deadline does not cancel shared
work unless it explicitly requests cancellation and owns that authority.

## Invocation surfaces and automation

- Product controls prepare a typed invocation and call the runtime; they never implement a second
  mutation path.
- Presentational components do not inspect the registry, resolve targets, or own domain transitions.
- Command and search surfaces project discovery results; result selection executes the same prepared
  invocation, while unresolved results enter their declared input-collection flow.
- Different surfaces may present one execution differently, but they observe the same execution
  handle and terminal result.
- A hidden action is absent from user and automation enumeration.
- Local enumeration does not make an action remotely invocable.

Automation requires an explicit opt-in with a stable serializable input schema, authorization
policy, noninteractive confirmation policy, compatibility lifecycle, and safe output contract.
Unsafe, interactive-only, or context-ambiguous actions remain unavailable even when registered
locally.

## Privacy and observability

Execution observation may include action ID, invocation source, result kind, stable failure code,
duration bucket, and non-user target type. Do not record input, result labels, target names, raw
errors, terminal content, transcripts, secrets, or confirmation copy by default.

Give every execution a correlation ID for diagnostics and external-job handoff. Diagnostics redact
input unless the feature owns a reviewed safe projection. Progress and retries do not imply
telemetry events.

## Verification

| Boundary                 | Required evidence                                                                                          |
| ------------------------ | ---------------------------------------------------------------------------------------------------------- |
| Definition and registry  | Identity, duplicate rejection, scoped resolution, enumeration, disposal, and change notification           |
| Availability             | Enabled, disabled reason, hidden, checked, stale target, recheck, and shortcut fallthrough                 |
| Execution                | Immediate and promised handlers, exactly-once result, exception mapping, confirmation, and unsupported     |
| Cancellation             | Before prepare, during confirmation, before commit, after commit, delayed cancellation, and owner disposal |
| Concurrency              | Every declared policy, semantic key, stale queued target, replacement, and out-of-order completion         |
| Progress                 | Initial snapshot, determinate and indeterminate phases, coalescing, late subscription, and terminal state  |
| Invocation paths         | Visible control, applicable menu, shortcut, command result, and approved automation reach one operation    |
| Focus and accessibility  | Pending, cancellation, failure, surface close, late completion, and no late focus theft                    |
| Localization and privacy | Both shipped locales, formatted descriptors, structured failures, and redacted observation                 |

Use controlled completion order, cancellation, stale-result, failure and cleanup tests for Action workflows. Independently ordered actors justify a shared TLA protocol only when a specific ownership/commit/progress risk is inadequately covered by implementation traces, under [`MODELING-AND-CORRECTNESS.md`](../MODELING-AND-CORRECTNESS.md). Async work or a cancellation branch alone does not require a feature model.

Browser verification invokes every visible path for a representative action and compares the owned
semantic result, pending behavior, focus, and error presentation rather than only rendered pixels.
