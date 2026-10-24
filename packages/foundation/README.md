# @orchbee/foundation

Orchbee's private package for domain-neutral, dependency-light programming primitives shared across workspace boundaries.

## Admission

An API belongs here only when it has a concrete workspace consumer and remains meaningful without product, feature, platform, transport, persistence, or presentation semantics. Reuse, file shape, or implementation similarity alone does not qualify. Owner-specific helpers stay with their semantic owner.

The root entry is a reviewed allowlist rather than an automatic barrel. Framework and environment integration also has an explicit subpath so consumers can name that dependency boundary directly.

## React

`@orchbee/foundation/react` owns domain-neutral primitives whose semantics depend on the React render lifecycle. React is a peer dependency. `useMemoizedFn` is exported only through the explicit React subpath; the root has no compatibility alias.

```ts
import { useMemoizedFn } from '@orchbee/foundation/react';

const handleCommit = useMemoizedFn(() => {
	// Reads the latest render values without changing function identity.
});
```

`useMemoizedFn` preserves one function identity across renders while invoking the latest committed implementation. A render that suspends or is otherwise discarded cannot change the callable behavior exposed by the prior commit. The Hook preserves the original parameters, return type, and caller-provided `this`.

The repository-wide admission and package rules live in [`docs/PACKAGES.md`](../../docs/PACKAGES.md#foundation-boundary).
