import type { WorkbenchAuthorityContext } from '@orchbee/workbench';

import type { ServerWorkbenchAction } from './ServerWorkbenchSession';

/** The commit point: past it, a cancellation can no longer say this did not happen. */
export function handOverServerWorkbenchAction(
	context: WorkbenchAuthorityContext,
	action: ServerWorkbenchAction | null,
): ServerWorkbenchAction | null {
	if (context.signal.aborted || action === null) return null;
	context.markCommitted();
	return action;
}
