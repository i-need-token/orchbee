import { expect, it, vi } from 'vitest';

import {
	activeTerminalContext,
	createSnapshotRuntime,
	fixturePresentationId,
	fixtureTabId,
	oneWorkspaceSnapshot,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';
import type { WorkbenchAgentSource } from '../../../workbench';
import type { getServerAgentSession, listServerAgentSessions } from '../../server-workbench';
import { bindServerAgentBoard } from '../bindServerAgentBoard';

const reads = vi.hoisted(() => ({
	get: vi.fn<typeof getServerAgentSession>(),
	list: vi.fn<typeof listServerAgentSessions>(),
}));

vi.mock(import('../../server-workbench'), async (importOriginal) => ({
	...(await importOriginal()),
	getServerAgentSession: reads.get,
	listServerAgentSessions: reads.list,
}));

it('opens an Agent the sidebar shows without a session at its terminal, reading no session first', async () => {
	reads.list.mockResolvedValue({
		complete: true,
		items: [],
		kind: 'listed',
	});
	const runtime = createSnapshotRuntime(oneWorkspaceSnapshot(1, [snapshotTab(1), snapshotTab(2)]));
	const presentationId = fixturePresentationId(2);
	const agentSource: WorkbenchAgentSource = {
		getSnapshot: () => ({
			complete: true,
			presentations: new Map([
				[
					presentationId,
					{ agent: 'codex', binding: 'generation:epoch:terminal', sequence: 1n, state: 'working' },
				],
			]),
		}),
		subscribe: () => () => {},
	};
	const { board, dispose } = bindServerAgentBoard({
		agentObservationSource: agentSource,
		connection: { exchange: () => Promise.reject(new Error('No exchange is expected.')) },
		runtime,
	});
	const tabs = activeTerminalContext(runtime).tabModel;
	expect(tabs.getSnapshot().activeTabId).toBe(fixtureTabId(1));

	board.open();
	await vi.waitFor(() => {
		expect(board.getSnapshot().read.kind).toBe('listed');
	});
	const [row] = board.getSnapshot().rows;
	expect(row).toMatchObject({ origin: 'sidebar', presentationId });

	await expect(board.jump(row?.sessionId ?? '')).resolves.toBe('shown');
	expect(reads.get).not.toHaveBeenCalled();
	expect(tabs.getSnapshot().activeTabId).toBe(fixtureTabId(2));
	dispose();
});
