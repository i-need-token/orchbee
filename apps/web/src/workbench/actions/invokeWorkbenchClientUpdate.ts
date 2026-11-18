import type { ActionExecution, ActionResult, ActionRuntime, ActionScope } from '@orchbee/actions';

import { WORKBENCH_ACTION_IDS } from './workbench-action-ids';

export interface InvokeWorkbenchClientUpdateOptions {
	/** Called when the Update action cannot run, throws, or is refused or unavailable. */
	readonly onUnavailable?: () => void;
}

function isUnavailableResult(result: ActionResult<unknown>): boolean {
	return result.kind === 'blocked' || result.kind === 'failed' || result.kind === 'unsupported';
}

/**
 * Runs Update from a button: the footer's control and About's run the same Action, from the scope
 * it was registered in or one below it. It runs through the runtime itself, since a typed runtime
 * runs only the Actions it registered and Update's registration owns another.
 */
export function invokeWorkbenchClientUpdate(
	runtime: ActionRuntime,
	scope: ActionScope,
	options?: InvokeWorkbenchClientUpdateOptions,
): void {
	let execution: ActionExecution<unknown>;
	try {
		execution = runtime.execute(WORKBENCH_ACTION_IDS.updateClient, undefined, {
			scope,
			source: 'button',
		});
	} catch {
		options?.onUnavailable?.();
		return;
	}
	const immediate = execution.getSnapshot().result;
	if (immediate !== null) {
		if (isUnavailableResult(immediate)) options?.onUnavailable?.();
		return;
	}
	void execution.completion.then(
		(result) => {
			if (isUnavailableResult(result)) options?.onUnavailable?.();
		},
		() => {
			options?.onUnavailable?.();
		},
	);
}
