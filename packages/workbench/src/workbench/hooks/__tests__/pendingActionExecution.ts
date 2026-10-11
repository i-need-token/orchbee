import type { ActionExecution, ActionExecutionSnapshot, ActionResult } from '@orchbee/actions';
import { act } from '@testing-library/react';

export function pendingActionExecution(executionId: string): {
	readonly execution: ActionExecution;
	settle(result: ActionResult): Promise<void>;
} {
	let resolveCompletion: ((result: ActionResult) => void) | undefined;
	let snapshot: ActionExecutionSnapshot = {
		progress: null,
		result: null,
		state: 'running',
	};
	const completion = new Promise<ActionResult>((resolve) => {
		resolveCompletion = resolve;
	});
	return {
		execution: {
			cancel: () => undefined,
			completion,
			executionId,
			getSnapshot: () => snapshot,
			subscribe: () => ({ dispose: () => undefined }),
		},
		settle: async (result) => {
			snapshot = { progress: null, result, state: result.kind };
			await act(async () => {
				resolveCompletion?.(result);
				await completion;
			});
		},
	};
}
