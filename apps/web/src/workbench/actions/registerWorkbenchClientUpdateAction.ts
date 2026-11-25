import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import { isRequestableUpdateOffer } from '../model/isRequestableUpdateOffer';
import type { WorkbenchClientUpdate } from '../types/workbench-client-update';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from './workbench-action-ids';

interface RegisterWorkbenchClientUpdateActionOptions {
	readonly clientUpdate: WorkbenchClientUpdate;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

/**
 * Update: asks the host to switch this window to the newer version it offers, or to move an older
 * Session onto this window's version. It is hidden while nothing is offered or a Session move runs,
 * so no surface lists it then, and it has no default key. The request is its commit; the host
 * either carries it out or says the update waits.
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
				isRequestableUpdateOffer(clientUpdate.getSnapshot())
					? { kind: 'enabled' }
					: { kind: 'hidden' },
			handler: {
				execute: (context) => {
					if (context.signal.aborted) return { kind: 'cancelled' };
					const offer = clientUpdate.getSnapshot();
					if (!isRequestableUpdateOffer(offer))
						return { code: 'client-update-unavailable', kind: 'blocked' };
					context.markCommitted();
					clientUpdate.request(offer.version);
					return { kind: 'completed' };
				},
			},
			id: WORKBENCH_ACTION_IDS.updateClient,
			input: noActionInput,
			title: workbenchMessages.update,
		}),
	);
}
