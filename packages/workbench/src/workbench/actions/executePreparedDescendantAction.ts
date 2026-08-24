import {
	type ActionExecutionContext,
	type ActionResult,
	type ActionRuntime,
	type PreparedActionInvocation,
} from '@orchbee/actions';

export async function executePreparedDescendantAction(
	context: ActionExecutionContext,
	runtime: ActionRuntime,
	invocation: PreparedActionInvocation,
): Promise<ActionResult<void>> {
	const execution = runtime.executePrepared(invocation);
	const cancelLowerExecution = () => execution.cancel();
	context.signal.addEventListener('abort', cancelLowerExecution, { once: true });
	if (context.signal.aborted) execution.cancel();
	try {
		const result = await execution.completion;
		if (result.kind === 'completed') context.markCommitted();
		return withoutValue(result);
	} finally {
		context.signal.removeEventListener('abort', cancelLowerExecution);
	}
}

function withoutValue(result: ActionResult): ActionResult<void> {
	switch (result.kind) {
		case 'blocked':
			return {
				code: result.code,
				...(result.details === undefined ? {} : { details: result.details }),
				kind: 'blocked',
			};
		case 'cancelled':
			return { kind: 'cancelled' };
		case 'completed':
			return { kind: 'completed' };
		case 'failed':
			return { failure: result.failure, kind: 'failed' };
		case 'unchanged':
			return { kind: 'unchanged' };
		case 'unsupported':
			return { kind: 'unsupported' };
	}
}
