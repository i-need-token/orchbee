import {
	createWorkbenchTopologyDocument,
	workbenchTopologyRevision,
} from '@orchbee/workbench/topology';
import { describe, expect, it, vi } from 'vitest';

import { WorkbenchTopologyRequestIdSchema } from '../../../shared';
import { DesktopWorkbenchTopologyClient } from '../DesktopWorkbenchTopologyClient';

class TestMessagePort extends EventTarget {
	readonly close = vi.fn<() => void>();
	readonly postMessage = vi.fn<(message: unknown) => void>();
	readonly start = vi.fn<() => void>();
}

const document = createWorkbenchTopologyDocument({
	projectWorkspace: {
		activeLocation: { kind: 'standalone' },
		expandedProjectIds: [],
		nextProjectSequence: 1,
		nextTerminalContextSequence: 2,
		nextWorkspaceSequence: 1,
		projects: [],
		standaloneTerminalContextId: 'terminal-context-1',
	},
	revision: workbenchTopologyRevision('0'),
	terminalContexts: [
		{
			nextTabSequence: 1,
			panelWorkspace: {
				nextPanelSequence: 1,
				nextPanelSlotSequence: 1,
				nextSplitSequence: 1,
				nextTerminalSequence: 1,
				pendingRemovals: [],
				tabs: [],
			},
			tabCollection: {
				activeTabId: null,
				groups: [],
				mostRecentGroupId: null,
				tabs: [],
			},
			terminalContextId: 'terminal-context-1',
		},
	],
});

describe('DesktopWorkbenchTopologyClient', () => {
	it('correlates load and commit outcomes without exposing transport details', async () => {
		const port = new TestMessagePort();
		let sequence = 1;
		const client = new DesktopWorkbenchTopologyClient(port as unknown as MessagePort, () =>
			WorkbenchTopologyRequestIdSchema.parse(`request-${sequence++}`),
		);

		const load = client.load();
		expect(port.postMessage).toHaveBeenLastCalledWith({
			requestId: 'request-1',
			type: 'workbench-topology-load-requested',
		});
		port.dispatchEvent(
			new MessageEvent('message', {
				data: {
					requestId: 'request-1',
					result: { kind: 'missing' },
					type: 'workbench-topology-load-completed',
				},
			}),
		);
		expect(await load).toEqual({ kind: 'missing' });

		const commit = client.commit(null, document);
		expect(port.postMessage).toHaveBeenLastCalledWith({
			document,
			expectedRevision: null,
			requestId: 'request-2',
			type: 'workbench-topology-commit-requested',
		});
		port.dispatchEvent(
			new MessageEvent('message', {
				data: {
					requestId: 'request-2',
					result: { kind: 'committed', revision: '0' },
					type: 'workbench-topology-commit-completed',
				},
			}),
		);
		expect(await commit).toEqual({ kind: 'committed', revision: '0' });
	});

	it('retires every pending request on malformed, mismatched, or transferred responses', async () => {
		const port = new TestMessagePort();
		const client = new DesktopWorkbenchTopologyClient(port as unknown as MessagePort, () =>
			WorkbenchTopologyRequestIdSchema.parse('request-load'),
		);
		const load = client.load();

		port.dispatchEvent(
			new MessageEvent('message', {
				data: {
					requestId: 'another-request',
					result: { kind: 'missing' },
					type: 'workbench-topology-load-completed',
				},
			}),
		);

		expect(await load).toEqual({ kind: 'unavailable', reason: 'control-unavailable' });
		expect(port.close).toHaveBeenCalledOnce();
	});
});
