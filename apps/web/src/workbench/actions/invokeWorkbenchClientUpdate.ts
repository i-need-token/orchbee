import type { ActionRuntime, ActionScope } from '@orchbee/actions';

import { WORKBENCH_ACTION_IDS } from './workbench-action-ids';

/**
 * Runs Update from a button: the footer's control and About's run the same Action, from the scope
 * it was registered in or one below it. It runs through the runtime itself, since a typed runtime
 * runs only the Actions it registered and Update's registration owns another.
 */
export function invokeWorkbenchClientUpdate(runtime: ActionRuntime, scope: ActionScope): void {
	runtime.execute(WORKBENCH_ACTION_IDS.updateClient, undefined, { scope, source: 'button' });
}
