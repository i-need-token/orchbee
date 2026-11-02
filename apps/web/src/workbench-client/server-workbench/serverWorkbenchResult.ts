import type { ActionResult } from '@orchbee/actions';
import type { WorkbenchAuthorityContext } from '@orchbee/workbench';

import type { ServerWorkbenchOutcome, ServerWorkbenchSubmission } from './ServerWorkbenchSession';

/** What the Server's answer means to the Action that asked. `committed` reads what it reports. */
export function serverWorkbenchResult<Output>(
	context: WorkbenchAuthorityContext,
	submission: ServerWorkbenchSubmission,
	committed: (outcome: ServerWorkbenchOutcome) => ActionResult<Output>,
): ActionResult<Output> {
	switch (submission.kind) {
		case 'not-submitted':
			return context.signal.aborted
				? { kind: 'cancelled' }
				: { code: 'not-submitted', kind: 'blocked' };
		case 'outcome-unknown':
			return { failure: { code: 'outcome-unknown' }, kind: 'failed' };
		case 'settled':
			break;
	}
	const { outcome } = submission;
	switch (outcome.kind) {
		case 'rejected':
			return { code: outcome.reason, kind: 'blocked' };
		case 'denied':
		case 'launch-profile-ambiguous':
		case 'launch-profile-unavailable':
			return { code: outcome.kind, kind: 'blocked' };
		case 'unavailable':
			return { failure: { code: outcome.reason }, kind: 'failed' };
		case 'unchanged':
			return { kind: 'unchanged' };
		default:
			return committed(outcome);
	}
}
