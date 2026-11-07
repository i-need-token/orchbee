import type {
	WorkbenchEarlierLayoutListing,
	WorkbenchEarlierLayoutOutcome,
	WorkbenchEarlierLayouts,
} from '../../workbench';
import {
	WORKBENCH_PROTOCOL_VERSION,
	type WorkbenchResponse,
} from '@orchbee/workbench-client-protocol';

import { createWorkbenchRequestId } from './createWorkbenchRequestId';
import type { ServerWorkbenchAnswer, ServerWorkbenchSession } from './ServerWorkbenchSession';

type EarlierLayoutSession = Pick<ServerWorkbenchSession, 'ask' | 'published' | 'snapshot'>;
type LayoutHistoryResult = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.layout-history.result' }
>;
type LayoutHistoryOutcome = LayoutHistoryResult['outcome'];
type LayoutHistoryAction = Extract<
	Parameters<EarlierLayoutSession['ask']>[0],
	{ readonly kind: 'workbench.layout-history' }
>['action'];

/** A reopen starts each reopened terminal before it answers, as the TUI allows for. */
const REPLY_MILLISECONDS = 30_000;

/**
 * The Server's earlier layouts, as the TUI and `orchbee layout history` read them. A reopen is sent
 * once: an answer that never arrives is reported as unknown and never repeated, and a reopen settles
 * only once this Client's snapshot holds what it reopened.
 */
export function createServerEarlierLayouts(session: EarlierLayoutSession): WorkbenchEarlierLayouts {
	return {
		list: async () => listing(outcomeOf(await ask(session, { kind: 'list' }), 'list')),
		reopen: async (savedAt) => {
			const answer = await ask(session, { kind: 'reopen', savedAt });
			const outcome = outcomeOf(answer, 'reopen');
			if (outcome === null) {
				return { kind: answer.kind === 'not-submitted' ? 'unavailable' : 'unknown' };
			}
			if (outcome.kind === 'reopened') await session.published(outcome.revision);
			return reopenOutcome(outcome);
		},
	};
}

function ask(
	session: EarlierLayoutSession,
	action: LayoutHistoryAction,
): Promise<ServerWorkbenchAnswer> {
	return session.ask(
		{
			action,
			kind: 'workbench.layout-history',
			protocolVersion: WORKBENCH_PROTOCOL_VERSION,
			requestId: createWorkbenchRequestId(),
			sessionId: session.snapshot.sessionId,
		},
		REPLY_MILLISECONDS,
	);
}

/** The Server's outcome when it answers what was asked; a listing never answers a reopen. */
function outcomeOf(
	answer: ServerWorkbenchAnswer,
	asked: LayoutHistoryAction['kind'],
): LayoutHistoryOutcome | null {
	if (answer.kind !== 'answered' || answer.response.kind !== 'workbench.layout-history.result') {
		return null;
	}
	const { outcome } = answer.response;
	return (outcome.kind === 'listed') === (asked === 'list') ? outcome : null;
}

function listing(outcome: LayoutHistoryOutcome | null): WorkbenchEarlierLayoutListing {
	if (outcome?.kind !== 'listed') return { kind: 'unavailable' };
	return {
		kind: 'listed',
		layouts: outcome.entries.map((entry) => ({
			savedAt: entry.savedAt,
			tabsNotOpen: entry.tabsNotOpen,
			workspacesNotOpen: entry.workspacesNotOpen,
		})),
	};
}

function reopenOutcome(outcome: LayoutHistoryOutcome): WorkbenchEarlierLayoutOutcome {
	switch (outcome.kind) {
		case 'reopened':
			return {
				focus: outcome.focus,
				kind: 'reopened',
				tabs: outcome.tabs,
				workspaces: outcome.workspaces,
			};
		case 'nothing-to-reopen':
		case 'not-found':
		case 'denied':
			return { kind: outcome.kind };
		case 'rejected':
			return { kind: outcome.reason === 'resource-limit' ? 'resource-limit' : 'unavailable' };
		case 'unavailable':
			return { kind: outcome.reason === 'busy' ? 'busy' : 'unavailable' };
		case 'listed':
			return { kind: 'unavailable' };
	}
}
