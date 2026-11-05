import {
	parseWorkbenchRequest,
	parseWorkbenchResponse,
	type WorkbenchRequest,
} from '@orchbee/workbench-client-protocol';
import { expect, it } from 'vitest';

import {
	fixtureContextId,
	fixtureId,
	fixtureTabId,
	oneWorkspaceSnapshot,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';
import { duplicateServerTabs } from '../duplicateServerTabs';
import type { ServerWorkbenchAnswer } from '../ServerWorkbenchSession';

type DuplicationSession = Parameters<typeof duplicateServerTabs>[0];
type DuplicateRequest = Extract<WorkbenchRequest, { readonly kind: 'workbench.tab-duplicate' }>;

const CONTEXT = fixtureContextId(1);

function member(sequence: number, copy: number | null, started: boolean): unknown {
	return {
		copy:
			copy === null
				? null
				: {
						revision: `${copy}`,
						tabId: fixtureId(0xa00 + sequence),
						terminal: started
							? {
									geometry: { cellPixelHeight: 16, cellPixelWidth: 8, columns: 80, rows: 24 },
									kind: 'started',
								}
							: { kind: 'outcome-unknown' },
					},
		tabId: fixtureTabId(sequence),
	};
}

it('sends a Start whose reply was lost again as the same operation and reports where the Server stopped it', async () => {
	const snapshot = oneWorkspaceSnapshot(1, [snapshotTab(1), snapshotTab(2), snapshotTab(3)]);
	const asked: DuplicateRequest[] = [];
	const answers: ((request: DuplicateRequest) => ServerWorkbenchAnswer)[] = [
		() => ({ kind: 'outcome-unknown' }),
		(request) =>
			available(request, { kind: 'pending' }, [member(1, 2, true), member(3, null, false)]),
		(request) =>
			available(request, { kind: 'failed', reason: 'outcome-unknown' }, [
				member(1, 2, true),
				member(3, 3, false),
			]),
	];
	const published: string[] = [];
	const session: DuplicationSession = {
		ask: (request) => {
			const parsed = parseWorkbenchRequest(request);
			if (parsed.kind !== 'workbench.tab-duplicate') throw new Error(`unexpected ${parsed.kind}`);
			asked.push(parsed);
			const answer = answers.shift();
			if (answer === undefined) throw new Error('asked once too often');
			return Promise.resolve(answer(parsed));
		},
		published: (revision) => {
			published.push(revision);
			return Promise.resolve();
		},
		snapshot,
	};

	const result = await duplicateServerTabs(
		session,
		{ markCommitted: () => undefined, signal: new AbortController().signal },
		{ tabIds: [fixtureTabId(3), fixtureTabId(1)], terminalContextId: CONTEXT },
	);

	expect(result).toEqual({
		kind: 'completed',
		value: { created: 2, kind: 'stopped', stop: 'terminal-not-started', total: 2 },
	});
	const [lost, resent, read] = asked.map((request) => request.action);
	expect(lost).toMatchObject({
		expectedRevision: '1',
		kind: 'start',
		tabIds: [fixtureTabId(1), fixtureTabId(3)],
		terminalContextId: CONTEXT,
	});
	expect(resent).toEqual(lost);
	expect(read).toEqual({ kind: 'read', operationId: lost?.operationId });
	// The copies show before the stop is told.
	expect(published).toEqual(['3']);
});

function available(
	request: DuplicateRequest,
	status: unknown,
	members: readonly unknown[],
): ServerWorkbenchAnswer {
	return {
		kind: 'answered',
		response: parseWorkbenchResponse({
			kind: 'workbench.tab-duplicate.result',
			operationId: request.action.operationId,
			outcome: { kind: 'available', members, status, terminalContextId: CONTEXT },
			protocolVersion: 1,
			requestId: request.requestId,
		}),
	};
}
