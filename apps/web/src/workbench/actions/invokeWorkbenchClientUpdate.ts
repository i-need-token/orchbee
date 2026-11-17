import { createTypedActionRuntime, type ActionRuntime, type ActionScope } from '@orchbee/actions';

import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from './workbench-action-ids';

/**
 * Runs Update from a button: the footer's control and About's run the same Action, from the scope
 * it was registered in or one below it.
 */
export function invokeWorkbenchClientUpdate(runtime: ActionRuntime, scope: ActionScope): void {
	createTypedActionRuntime<WorkbenchActionCatalog>(runtime).execute(
		WORKBENCH_ACTION_IDS.updateClient,
		undefined,
		{ scope, source: 'button' },
	);
}
