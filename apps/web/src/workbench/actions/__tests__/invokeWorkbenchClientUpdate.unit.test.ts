import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { expect, it, vi } from 'vitest';

import type {
	WorkbenchClientUpdate,
	WorkbenchUpdateOffer,
} from '../../types/workbench-client-update';
import { invokeWorkbenchClientUpdate } from '../invokeWorkbenchClientUpdate';
import { registerWorkbenchClientUpdateAction } from '../registerWorkbenchClientUpdateAction';
import type { WorkbenchActionCatalog } from '../workbench-action-ids';

const OFFER: WorkbenchUpdateOffer = { kind: 'client', version: '0.3.0' };

// Regression: a button ran Update through a typed runtime of its own, which the runtime refuses
// for an Action another typed runtime registered, so neither the footer nor About switched.
it('runs Update from the scope it was registered in and from one below it', async () => {
	const runtime = new ActionRuntime();
	const host = runtime.createScope('web-application');
	const workbench = runtime.createScope('workbench', host);
	const request = vi.fn<(version: string) => void>();
	const clientUpdate: WorkbenchClientUpdate = {
		getSnapshot: () => OFFER,
		request,
		subscribe: () => () => undefined,
		subscribeWaiting: () => () => undefined,
	};
	registerWorkbenchClientUpdateAction({
		clientUpdate,
		scope: host,
		typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(runtime),
	});

	invokeWorkbenchClientUpdate(runtime, workbench);
	invokeWorkbenchClientUpdate(runtime, host);
	await Promise.resolve();

	expect(request.mock.calls).toEqual([['0.3.0'], ['0.3.0']]);
});

it('calls onUnavailable when Update is unregistered, refused, throws, or invoked from a foreign scope', async () => {
	const runtime = new ActionRuntime();
	const host = runtime.createScope('web-application');
	const foreign = runtime.createScope('foreign');
	const onUnavailable = vi.fn<() => void>();

	// Unregistered -> unsupported
	invokeWorkbenchClientUpdate(runtime, host, { onUnavailable });
	expect(onUnavailable).toHaveBeenCalledTimes(1);

	// Registered, but offer disappeared before click -> blocked ('client-update-unavailable')
	let offered: WorkbenchUpdateOffer | null = null;
	registerWorkbenchClientUpdateAction({
		clientUpdate: {
			getSnapshot: () => offered,
			request: () => {
				throw new Error('boom');
			},
			subscribe: () => () => undefined,
			subscribeWaiting: () => () => undefined,
		},
		scope: host,
		typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(runtime),
	});
	invokeWorkbenchClientUpdate(runtime, host, { onUnavailable });
	await Promise.resolve();
	expect(onUnavailable).toHaveBeenCalledTimes(2);

	// Handler throws -> failed
	offered = OFFER;
	invokeWorkbenchClientUpdate(runtime, host, { onUnavailable });
	await Promise.resolve();
	expect(onUnavailable).toHaveBeenCalledTimes(3);

	// Foreign scope inside same runtime without registration -> unsupported
	invokeWorkbenchClientUpdate(runtime, foreign, { onUnavailable });
	expect(onUnavailable).toHaveBeenCalledTimes(4);

	// Scope owned by another runtime -> sync throw caught
	const otherRuntime = new ActionRuntime();
	invokeWorkbenchClientUpdate(runtime, otherRuntime.createScope('other'), { onUnavailable });
	expect(onUnavailable).toHaveBeenCalledTimes(5);
	otherRuntime.dispose();
	runtime.dispose();
});
