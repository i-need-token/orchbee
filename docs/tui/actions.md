# TUI Actions

## Authority and scope

This standard owns the native TUI adapter contract for user-invocable semantic actions: projection,
availability, invocation parity, execution observation, and result presentation. Operation meaning,
target eligibility, validation, commit, and durable effects remain with the feature or domain that
owns the result under [`INTERFACE-DESIGN.md`](../INTERFACE-DESIGN.md).

This is the required contract for new and materially changed TUI behavior. It does not assert that
every current surface already conforms. Record a staged gap with its owner and completion condition
instead of weakening the contract or representing unsupported behavior as available.

Key decoding, bindings, ownership, and effective shortcut display follow
[`keyboard-input.md`](keyboard-input.md). Pointer admission and gesture validity follow
[`mouse-input.md`](mouse-input.md). Control appearance follows [`components.md`](components.md), and
confirmation, pending feedback, notices, and focus return follow
[`overlays-and-status.md`](overlays-and-status.md). The frontend runtime remains governed by
[`frontend/actions.md`](../frontend/actions.md); this standard does not require the two Clients to
share a runtime or implementation.

## Admission and ownership

Use an action when a named product intent is reachable from more than one invocation path, needs
stable availability or discovery, starts observable work, or has a semantic result. Keep hover,
focus movement, disclosure, provisional selection, editor mutations, and continuous gesture input
with their component owner. A committed gesture result may invoke an action.

| Responsibility                                                              | Owner                                                                               |
| --------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| Identity, typed input and target, product name, and semantic outcome        | Feature that owns the result                                                        |
| Domain invariants, authorization, validation, commit, and durable state     | Owning domain capability                                                            |
| TUI context projection, availability, invocation, and execution observation | Native application composition                                                      |
| Key binding and input precedence                                            | [`keyboard-input.md`](keyboard-input.md) and the owning application binding catalog |
| Hit geometry, pointer gesture, control state, and visual treatment          | Pointer and component owners                                                        |
| Process, storage, transport, navigation, and host effects                   | Existing application or runtime boundary                                            |

Do not create a second business writer in a renderer, a universal UI Store, or a process-wide action
singleton. Action definitions, component-local keymaps, and command entries may have different
structured owners. A TUI composition may normalize their projections for discovery and dispatch;
this contract does not require every interaction to live in one registry or framework.

An action may commit an immediate local view effect when its semantic owner declares activation as
the commit point. A presenter never invents persistence, an external effect, or a later commit merely
because the local projection changed.

## Action projection

Every recurring action projection supplies or references the following facts. One semantic identity
names one result; instance, entity, and option values are typed input rather than new IDs.

| Field            | Contract                                                                                             |
| ---------------- | ---------------------------------------------------------------------------------------------------- |
| Identity         | Stable nonlocalized action ID owned by the feature                                                   |
| Input and target | Validated input plus a typed semantic handle; never a row number, cell, label, or pointer address    |
| Scope            | Input owner, feature instance, and target scope in which the action resolves                         |
| Metadata         | Localized title, optional decision-relevant description, category, and reviewed search aliases       |
| Availability     | `enabled`, `disabled` with a localized reason, or `hidden` for the current context snapshot          |
| Semantic state   | Optional checked or current value owned by the feature; never inferred from focus or hover           |
| Entry            | Effective binding references and the pointer, menu, or command entries currently offered             |
| Execution        | Commit point, surface lifetime, cancellation, concurrency, and result policy when work is observable |

IDs never derive from translated copy, control placement, shortcuts, or menu order. Dynamic or
external input is untrusted until the feature-owned boundary validates and copies the canonical
snapshot it needs. Presentation metadata contains locale descriptors rather than preformatted
strings so every surface formats the same meaning for the effective locale.

## Resolution, availability, and refusal

- Resolve an action from the active input owner and exact feature instance. Registration or render
  order is never precedence, and a stale target never falls through to an unrelated current item.
- Evaluate availability purely against one read-only context snapshot for presentation. Revalidate
  input, target, authorization, and domain guards immediately before the effect boundary.
- `enabled` is visible and invocable. `disabled` is relevant but unavailable and remains visible
  with a reason. `hidden` is irrelevant, unsupported, unauthorized, or unsafe to disclose and is
  absent from menus, help, search, and enumeration.
- If state changes between presentation and invocation, refuse the invocation with a typed reason.
  Never execute merely because an old row, hit rectangle, prepared menu entry, or shortcut hint was
  once valid.
- Availability answers whether an action may start now. Pending, checked, selected, focused, and
  current-context state remain independent and must not be encoded as availability.
- Unrelated background work never makes an action unavailable. A background read that refreshes a
  projection the action does not own, such as a publication read prompted by another terminal's
  output, leaves availability to the facts the action targets. An open confirmation stays open
  while those facts are unchanged and rebases onto the newer projection; it closes with the
  localized changed-target notice when they change.

## Invocation parity

Keyboard, pointer, menu, command, and other approved local entries for one operation resolve the
same action identity, validated input, target, availability, confirmation, commit boundary, and
result mapping. Each entry may adapt its layout and hint, but it does not implement another mutation
path.

- A presented entry carries enough identity to bind activation to its surface instance, action,
  target, and presented-state generation. Pointer activation also satisfies the current hit-map and
  press/release contract in [`mouse-input.md`](mouse-input.md).
- Moving focus, hovering, selecting a row, opening a menu, or typing a search query never activates
  the action unless that transition is itself the control's documented semantic intent.
- A menu or command result invokes the prepared target it described. If that target or definition
  became stale, it is refused rather than re-resolved through another scope.
- Confirmation is feature-owned structured data. Every entry reaches the same confirmation and
  revalidation path; no presenter bypasses it or treats confirmation as authorization.
- Activation consumes one invocation authority. Key repeat, buffered input, double-click, and a
  second presenter cannot start another execution unless the action's concurrency policy admits it.

## Execution and effect boundary

An observable invocation receives a unique execution identity before work crosses the effect
boundary. Its snapshot progresses monotonically through applicable preparation, confirmation,
running, cancellation-requested, and one terminal outcome. Immediate work may reach a terminal
snapshot before the invoker subscribes, so the current snapshot and terminal result are authoritative;
edge notifications alone are not.

Present the semantic owner's terminal result through the classifications below. They define TUI
meaning and mapping, not a new Rust enum, common runtime, or replacement for owner-specific variants.

| Presentation classification | Meaning                                                                                                   |
| --------------------------- | --------------------------------------------------------------------------------------------------------- |
| `completed`                 | The semantic owner reports that the operation committed                                                   |
| `unchanged`                 | The request was valid and intentionally made no change                                                    |
| `blocked`                   | A known guard, refusal, stale target, conflict, or unavailable capability prevented commit                |
| `cancelled`                 | The owning lifecycle ended the operation before commit                                                    |
| `failed`                    | The owner reports a definite failure according to its existing result contract                            |
| `unsupported`               | No current capability can serve the action                                                                |
| `unknown`                   | The owner explicitly reports that commit is indeterminate and requires observation before another attempt |

Existing owner-specific `refused` variants map to the `blocked` presentation meaning while retaining
their stable reason. The adapter never invents `unknown` from latency, a deadline, or a missing reply,
and never coerces an owner-reported `unknown` into definite `failed`. This mapping does not change or
claim conformance of the current frontend action runtime.

The TUI reducer may produce a narrow effect request, but the existing application or domain boundary
performs storage, process, transport, navigation, and other external work. A completion carries the
execution identity, semantic target, and request generation that produced it. Only the matching
current presenter may update pending state, preview, selection, focus, or navigation. The effect
owner still settles the completion into authoritative domain state after a presenter closes; without
a matching presenter it may update only an application-owned activity or notice channel, never a
replacement surface's interaction state.

Raw exceptions and external strings stay in diagnostics. Product results use stable codes and
localized messages. The adapter never retries, times out, rolls back, or converts failure to success
unless the owning workflow explicitly defines that behavior.

## Cancellation, concurrency, and lifetime

- Dismissing or replacing a presenter releases its subscription and interaction authority; it does
  not cancel accepted work. Cancellation of work is an explicit action or execution request owned by
  the workflow.
- Cancellation is cooperative. A request before commit may end as `cancelled`; work that committed
  before cancellation completes with its committed outcome.
- Every asynchronous action defines a semantic concurrency key and one policy: independent parallel
  execution, reject a duplicate, replace the latest cancellable execution, or serialize in a
  deterministic order. The key uses stable action, target, and validated-input facts rather than
  component identity or translated text.
- The owner defines delayed cancellation, queued-target revalidation, out-of-order completion, and a
  replaced execution that crossed commit. A caller deadline does not silently cancel shared work.
- When an invocation needs a serialized resource that background work currently holds, the owner
  holds the invocation and replays it through its ordinary entry point as soon as the background
  work settles, before admitting further background work. The hold is bounded; expiry ends with a
  visible busy refusal, and the replay revalidates every fact. An invocation is never dropped
  because unrelated background work was in flight.
- A confirmed decision waits the same way when another of the user's own operations holds the
  resource its submission needs, such as an earlier close's status read or exchange. Its
  confirmation stays open and armed while the decision waits; cancelling it withdraws the decision
  with nothing sent, and a target that drifts meanwhile ends it with the localized changed-target
  notice. One confirmation either commits its decision or leaves a visible reason.
- The user's own switch that is still landing settles the same way: an invocation pressed while
  the destination's read or terminal attach is in flight, or while a requested navigation still
  waits behind an earlier switch or read, waits for the switch and then runs against the
  destination. An invocation that targets "current" binds to the destination of the newest
  request when it is pressed and runs only once the switch lands there; a switch that lands
  elsewhere or not at all refuses it with the localized changed-target notice instead of
  resolving it against whatever is shown. A pointer click dropped while the switch lands may have
  chosen another destination, so it makes the destination uncertain: such an invocation is
  refused the same way while the switch lands and, until a bounded interval after the click has
  passed, once more after it landed, so that the user's retry acts on what is shown. The switch
  is no reason to report the invocation as busy or its target as not ready.
- A commit refused only because the projection advanced past the invocation's baseline is retried
  once against the newer projection when the targeted facts are unchanged; otherwise, or when the
  retry is refused the same way, the action ends with the localized changed-target notice.
- Closing a foreground surface is refused while that surface must own a required decision, or the
  workflow explicitly parks the task and exposes a return route. Background completion never steals
  focus, changes context, or replaces a newer draft.
- Fire-and-forget work is not an action result. Return an observable execution or a stable externally
  owned job identity with its own lifecycle.

## Discovery and presentation

Discovery derives from the same effective projection used for dispatch. A button, row action, menu,
shortcut footer, full help surface, and command search do not maintain independent labels, bindings,
availability, or target rules.

- Show the effective localized title and effective binding for the active context, including admitted
  user overrides and host limitations. Never advertise a shadowed or unsupported chord as usable.
- Prioritize the current owner's few useful actions in the footer. Full help may include disabled
  actions for learning only when it shows their reason; hidden actions remain absent.
- Search localized titles, descriptions, categories, and reviewed aliases. Search changes only the
  projection and never invokes an action.
- Preserve destructive meaning, checked state, grouping, and relative order across equivalent entry
  surfaces. Short labels may be documented variants of the same product term, not new action names.
- Sanitize external target text before painting. Do not expose raw errors, secrets, terminal content,
  or user input through action metadata, progress, or diagnostics by default.

## Verification

| Changed claim               | Required evidence                                                                                                                                         |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Identity or resolution      | Tests cover exact scope, typed target, duplicate or stale definition, hidden capability, and no unrelated fallback                                        |
| Availability                | Tests cover enabled, disabled reason, hidden, revalidation, checked-state independence, and refusal after state change                                    |
| Invocation entry            | Keyboard, pointer, menu, and command paths reach the same action, input, target, confirmation, and commit point                                           |
| Execution                   | Tests cover immediate and delayed completion, unique identity, exactly one terminal outcome, exception mapping, and current snapshot before subscription  |
| Cancellation or concurrency | Controlled-order tests cover the declared policy, duplicate input, cancellation before and after commit, queued stale target, and out-of-order completion |
| Presentation lifetime       | Tests reject stale surface and hit generations, preserve newer focus and drafts, and route late completion through nonintrusive feedback                  |
| Discovery                   | Both shipped locales show effective labels, bindings, order, disabled reasons, compact truncation, and no hidden action                                   |

Use cell-grid, input, resize, locale, and host evidence under
[`verification.md`](verification.md). Add a formal model only when independently ordered actors or an
effect ownership risk meet the triggers in
[`MODELING-AND-CORRECTNESS.md`](../MODELING-AND-CORRECTNESS.md); asynchronous syntax alone does not.
