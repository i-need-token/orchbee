import {
	resolveWorkbenchAgentDisplayState,
	resolveWorkbenchAgentProviderId,
	type WorkbenchAgentObservation,
	type WorkbenchAgentSnapshot,
	type WorkbenchAgentBoardRow,
} from '../../workbench';
import type { AgentSessionSummary } from '../server-connection';

/** One Agent as the sidebar shows it: the terminal it runs in and that terminal's observation. */
export interface ServerAgentBoardSidebarAgent {
	readonly observation: WorkbenchAgentObservation;
	readonly presentationId: string;
}

/** The 512-session runtime bound at 100 items per page, rounded up, as the TUI bounds it. */
const MAXIMUM_ROWS = 600;

/**
 * The board's rows, as the TUI builds them: every canonical session in the Server's order, then
 * every Agent the sidebar shows whose Presentation no session attaches, in the sidebar's order,
 * with rows needing attention lifted first. Sessions use the same binding-qualified completion
 * projection as terminal chrome, without replacing provider failures or approvals.
 */
export function projectServerAgentBoardRows(
	items: readonly AgentSessionSummary[],
	observation: WorkbenchAgentSnapshot | undefined,
	sidebarAgents: readonly ServerAgentBoardSidebarAgent[],
): readonly WorkbenchAgentBoardRow[] {
	const sessionRows = items.map((summary): WorkbenchAgentBoardRow => {
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
			origin: 'session',
			presentationId:
				summary.presentationAttachments.find((a) => a.navigable)?.presentationId ?? null,
			providerId: summary.providerId,
			sessionId: summary.sessionId,
			status,
		};
	});
	// A session that attaches the sidebar Agent's Presentation replaces its sidebar row.
	const attached = new Set(
		items.flatMap((summary) =>
			summary.presentationAttachments.map((attachment) => attachment.presentationId),
		),
	);
	const sidebarRows = sidebarAgents
		.filter((agent) => !attached.has(agent.presentationId))
		.map(projectSidebarRow);
	const rows = [...sessionRows, ...sidebarRows];
	// A stable lift, never a reorder: each group keeps its order.
	return [
		...rows.filter((row) => row.status.kind === 'needs-attention'),
		...rows.filter((row) => row.status.kind !== 'needs-attention'),
	].slice(0, MAXIMUM_ROWS);
}

/** The sidebar's state on the board's statuses, as the TUI maps it; the key never shows. */
function projectSidebarRow({
	observation,
	presentationId,
}: ServerAgentBoardSidebarAgent): WorkbenchAgentBoardRow {
	return {
		ended: false,
		freshness: 'fresh',
		origin: 'sidebar',
		presentationId,
		providerId: resolveWorkbenchAgentProviderId(observation.agent),
		sessionId: `terminal:${presentationId}`,
		status: projectSidebarStatus(observation),
	};
}

function projectSidebarStatus(
	observation: WorkbenchAgentObservation,
): WorkbenchAgentBoardRow['status'] {
	// The state the sidebar shows, after a seen turn's completion.
	switch (resolveWorkbenchAgentDisplayState(observation)) {
		case 'blocked':
			return { count: 1, kind: 'needs-attention' };
		case 'done':
			return { kind: 'succeeded' };
		case 'error':
			return { kind: 'failed' };
		case 'idle':
			return { kind: 'idle' };
		case 'working':
			return { kind: 'working' };
		case 'opening':
		case 'unknown':
			return { kind: 'unknown' };
	}
}
