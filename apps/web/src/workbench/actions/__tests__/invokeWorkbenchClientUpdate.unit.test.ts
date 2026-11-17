import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { expect, it, vi } from 'vitest';

import type { WorkbenchClientUpdate } from '../../types/workbench-client-update';
import { invokeWorkbenchClientUpdate } from '../invokeWorkbenchClientUpdate';
import { registerWorkbenchClientUpdateAction } from '../registerWorkbenchClientUpdateAction';
import type { WorkbenchActionCatalog } from '../workbench-action-ids';

// Regression: a button ran Update through a typed runtime of its own, which the runtime refuses
// for an Action another typed runtime registered, so neither the footer nor About switched.
it('runs Update from the scope it was registered in and from one below it', async () => {
	const runtime = new ActionRuntime();
	const host = runtime.createScope('web-application');
	const workbench = runtime.createScope('workbench', host);
	const request = vi.fn<(version: string) => void>();
	const clientUpdate: WorkbenchClientUpdate = {
		getSnapshot: () => '0.3.0',
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
