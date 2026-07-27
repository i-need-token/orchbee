import type { TypedActionRuntime } from './action-runtime-types';
import type { ActionRuntime } from './action-runtime';

export function createTypedActionRuntime<Catalog>(
	runtime: ActionRuntime,
): TypedActionRuntime<Catalog> {
	return runtime.createTypedRuntime<Catalog>();
}
