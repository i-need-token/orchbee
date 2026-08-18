# @orchbee/actions

Framework-independent runtime for Orchbee product-semantic Actions.

The package owns scoped registration and resolution, discovery, availability rechecks, observable sync/async execution, confirmation, cooperative cancellation, progress, and explicit concurrency policy. Feature packages retain their authoritative state and register handlers that call their existing domain operations. Applications create one `ActionRuntime` per mounted composition and inject the runtime or a narrower invocation capability.

## Use

```ts
import {
	ActionRuntime,
	createTypedActionRuntime,
	defineAction,
	noActionInput,
	type ActionSignature,
} from '@orchbee/actions';

interface ApplicationActionCatalog {
	readonly 'workbench.sidebar.toggle': ActionSignature<void>;
}

const runtime = new ActionRuntime();
const applicationScope = runtime.createScope('application');
const actions = createTypedActionRuntime<ApplicationActionCatalog>(runtime);

const registration = actions.register(
	applicationScope,
	defineAction({
		id: 'workbench.sidebar.toggle',
		title: { id: 'workbench.sidebar.toggle' },
		category: { id: 'workbench', title: { id: 'action.category.workbench' } },
		input: noActionInput,
		execution: {
			mode: 'foreground',
			concurrency: 'parallel',
			concurrencyKey: () => 'workbench-sidebar',
		},
		getAvailability: () => ({ kind: 'enabled' }),
		handler: {
			execute: () => ({ kind: 'completed' }),
		},
	}),
);

const execution = actions.execute('workbench.sidebar.toggle', undefined, {
	scope: applicationScope,
	source: 'button',
});
await execution.completion;

registration.dispose();
runtime.dispose();
```

Action definitions own stable IDs, descriptors, dynamic-input validation, availability, confirmation, execution policy, and result mapping. The runtime is not a product-state Store and does not grant automation authority merely because an Action can be enumerated.

`discover(...)` resolves the active scope outward by default. A global command surface can request descendant registrations from its application scope and receives scope-bound `ready` or `requires-input` results. An unresolved action is enumerable only when its definition supplies `getDiscoveryAvailability`; pass its runtime-owned token and collected input to `prepareInvocation(...)`, then execute the resulting opaque `PreparedActionInvocation` with `executePrepared(...)`. The token retains the validator's canonical input and exact registration identity; disposal or replacement blocks execution instead of falling back to another registration. Validators must return owner-owned canonical values and copy any caller-owned mutable collection or nested value retained for later execution.

An owner-defined Action catalog is effective only when the same `createTypedActionRuntime(...)` adapter registers definitions and invokes them. Its `register(...)` method checks each definition's ID, input, and handler output against the catalog, while `execute(...)` checks invocation input and execution output.

`execute(...)` captures one shallow read-only invocation snapshot before resolution and always returns an `ActionExecution` immediately. Availability, confirmation, concurrency, and handler phases observe that same snapshot. A synchronous handler may already have a terminal snapshot; an asynchronous handler exposes the same handle, `completion`, progress, cooperative cancellation signal, and explicit commit point. Completion resolves to a structured result and never rejects with an unclassified handler, confirmation, validation, availability, or concurrency-key exception. Each definition declares `parallel`, `reject-duplicate`, `replace-latest`, or `serialize` behavior for its semantic concurrency key.

`formal/tla/ActionExecution.tla` is the bounded correctness model for this runtime contract. One variant checks an exact registration generation, prepared-token invalidation, invocation snapshotting, confirmation, progress, commit, cancellation, result mapping, and runtime disposal for one execution. Four separate variants check `parallel`, `reject-duplicate`, and `replace-latest` with two executions, plus `serialize` with three executions so a cancelled middle queue entry cannot block its successor. Splitting the registration and handler lifecycle dimensions from concurrency is an explicit state-space bound, not a claim that production events are serialized. TLC checks safety invariants, serialized queue progress under weak fairness of the ready-queue drain, and required nonzero action coverage across all variants. It does not prove arbitrary execution counts, handler termination, caller-owned mutable values below the documented shallow snapshot, or correctness of product handlers.

The repository-wide semantic and lifecycle contract is [`docs/frontend/actions.md`](../../docs/frontend/actions.md).

```sh
pnpm --filter @orchbee/actions verify:correctness
pnpm --filter @orchbee/actions test:tla
pnpm --filter @orchbee/actions test:exports
```
