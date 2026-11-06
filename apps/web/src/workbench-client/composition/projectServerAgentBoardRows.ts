import {
	resolveWorkbenchAgentDisplayState,
	type WorkbenchAgentSnapshot,
	type WorkbenchAgentBoardRow,
} from '../../workbench';
import type { AgentSessionSummary } from '../server-connection';

/** Same binding-qualified completion projection as terminal chrome, without replacing provider failures/approvals. */
export function projectServerAgentBoardRows(
	items: readonly AgentSessionSummary[],
	observation: WorkbenchAgentSnapshot | undefined,
): readonly WorkbenchAgentBoardRow[] {
	const rows = items.map((summary) => {
		let status: WorkbenchAgentBoardRow['status'] =
			summary.status.kind === 'needs-attention'
				? { kind: 'needs-attention', count: summary.status.count }
				: { kind: summary.status.kind };
		if (summary.status.kind === 'idle' || summary.status.kind === 'succeeded') {
			const fact = summary.presentationAttachments
				.map((attachment) => {
					const current = observation?.presentations.get(attachment.presentationId);
					return current?.binding.split(':')[0] === attachment.generation ? current : undefined;
				})
				.find((current) => current !== undefined);
			if (fact !== undefined) {
				const state = resolveWorkbenchAgentDisplayState({
					state: summary.status.kind === 'idle' ? 'idle' : 'done',
					...(fact.completion === undefined ? {} : { completion: fact.completion }),
				});
				status = { kind: state === 'done' ? 'succeeded' : 'idle' };
			}
		}
		return {
			ended: summary.lifecycle === 'ended',
			freshness: summary.observation.freshness,
			presentationId:
				summary.presentationAttachments.find((a) => a.navigable)?.presentationId ?? null,
			providerId: summary.providerId,
			sessionId: summary.sessionId,
			status,
		};
	});
	return [
		...rows.filter((row) => row.status.kind === 'needs-attention'),
		...rows.filter((row) => row.status.kind !== 'needs-attention'),
	].slice(0, 600);
}
