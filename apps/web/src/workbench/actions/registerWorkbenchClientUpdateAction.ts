import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import type { WorkbenchClientUpdate } from '../types/workbench-client-update';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from './workbench-action-ids';

interface RegisterWorkbenchClientUpdateActionOptions {
	readonly clientUpdate: WorkbenchClientUpdate;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

/**
 * Update: asks the host to switch this window to the newer version it offers. It is hidden while
 * none is offered, so no surface lists it then, and it has no default key. The request is its
 * commit; the host either leaves for the new version or says the update waits.
 */
export function registerWorkbenchClientUpdateAction({
	clientUpdate,
	scope,
	typedRuntime,
}: RegisterWorkbenchClientUpdateActionOptions): ActionDisposable {
	return typedRuntime.register(
		scope,
		defineAction({
			category: { id: 'navigation', title: workbenchMessages.actionCategoryNavigation },
			execution: {
				concurrency: 'reject-duplicate',
				concurrencyKey: () => 'client-update',
				mode: 'foreground',
			},
			getAvailability: () =>
				clientUpdate.getSnapshot() === null ? { kind: 'hidden' } : { kind: 'enabled' },
			handler: {
				execute: (context) => {
					if (context.signal.aborted) return { kind: 'cancelled' };
					const version = clientUpdate.getSnapshot();
					if (version === null) return { code: 'client-update-unavailable', kind: 'blocked' };
					context.markCommitted();
					clientUpdate.request(version);
					return { kind: 'completed' };
				},
			},
			id: WORKBENCH_ACTION_IDS.updateClient,
			input: noActionInput,
			title: workbenchMessages.update,
		}),
	);
}
