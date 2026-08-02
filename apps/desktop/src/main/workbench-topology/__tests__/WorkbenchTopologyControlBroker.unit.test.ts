import { EventEmitter } from 'node:events';

import {
	createWorkbenchTopologyDocument,
	workbenchTopologyRevision,
	type WorkbenchTopologyDocument,
} from '@orchbee/workbench/topology';
import type { MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import type { WorkbenchTopologyStore } from '../decideWorkbenchTopologyCommit';
import { WorkbenchTopologyControlBroker } from '../WorkbenchTopologyControlBroker';

function topology(revision: string): WorkbenchTopologyDocument {
	return createWorkbenchTopologyDocument({
		projectWorkspace: {
			activeLocation: { kind: 'standalone' },
			expandedProjectIds: [],
			nextProjectSequence: 1,
			nextTerminalContextSequence: 2,
			nextWorkspaceSequence: 1,
			projects: [],
			standaloneTerminalContextId: 'terminal-context-1',
		},
		revision: workbenchTopologyRevision(revision),
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
}

class TestMainPort extends EventEmitter {
	readonly close = vi.fn<() => void>();
	readonly postMessage = vi.fn<(value: unknown) => void>();
	readonly start = vi.fn<() => void>();
}

function message(data: unknown, ports: readonly MessagePortMain[] = []) {
	return { data, ports };
}

describe('WorkbenchTopologyControlBroker', () => {
	it('correlates strict load and CAS commit results on one scoped port', async () => {
		const port = new TestMainPort();
		const store: WorkbenchTopologyStore = {
			commit: vi.fn<WorkbenchTopologyStore['commit']>().mockResolvedValue({
				kind: 'committed',
				revision: workbenchTopologyRevision('0'),
			}),
			load: vi.fn<WorkbenchTopologyStore['load']>().mockResolvedValue({ kind: 'missing' }),
		};
		const broker = new WorkbenchTopologyControlBroker(port as unknown as MessagePortMain, store);
		broker.start();

		port.emit(
			'message',
			message({ requestId: 'request-load', type: 'workbench-topology-load-requested' }),
		);
		port.emit(
			'message',
			message({
				document: topology('0'),
				expectedRevision: null,
				requestId: 'request-commit',
				type: 'workbench-topology-commit-requested',
			}),
		);

		await vi.waitFor(() => expect(port.postMessage).toHaveBeenCalledTimes(2));
		expect(port.postMessage).toHaveBeenCalledWith({
			requestId: 'request-load',
			result: { kind: 'missing' },
			type: 'workbench-topology-load-completed',
		});
		expect(port.postMessage).toHaveBeenCalledWith({
			requestId: 'request-commit',
			result: { kind: 'committed', revision: '0' },
			type: 'workbench-topology-commit-completed',
		});
	});

	it('retires on request-identity reuse with different content or transferred authority', () => {
		const port = new TestMainPort();
		const store: WorkbenchTopologyStore = {
			commit: vi.fn<WorkbenchTopologyStore['commit']>(),
			load: vi.fn<WorkbenchTopologyStore['load']>().mockResolvedValue({ kind: 'missing' }),
		};
		const broker = new WorkbenchTopologyControlBroker(port as unknown as MessagePortMain, store);
		broker.start();
		port.emit(
			'message',
			message({ requestId: 'same-request', type: 'workbench-topology-load-requested' }),
		);
		port.emit(
			'message',
			message({
				document: topology('0'),
				expectedRevision: null,
				requestId: 'same-request',
				type: 'workbench-topology-commit-requested',
			}),
		);

		expect(port.close).toHaveBeenCalledOnce();

		const transferredPort = new TestMainPort();
		const transferredBroker = new WorkbenchTopologyControlBroker(
			transferredPort as unknown as MessagePortMain,
			store,
		);
		transferredBroker.start();
		transferredPort.emit(
			'message',
			message({ requestId: 'transfer-request', type: 'workbench-topology-load-requested' }, [
				{} as MessagePortMain,
			]),
		);
		expect(transferredPort.close).toHaveBeenCalledOnce();
	});

	it('suppresses a completion after its renderer generation is disposed', async () => {
		let finishLoad:
			| ((value: Awaited<ReturnType<WorkbenchTopologyStore['load']>>) => void)
			| undefined;
		const store: WorkbenchTopologyStore = {
			commit: vi.fn<WorkbenchTopologyStore['commit']>(),
			load: () =>
				new Promise((resolve) => {
					finishLoad = resolve;
				}),
		};
		const port = new TestMainPort();
		const broker = new WorkbenchTopologyControlBroker(port as unknown as MessagePortMain, store);
		broker.start();
		port.emit(
			'message',
			message({ requestId: 'late-request', type: 'workbench-topology-load-requested' }),
		);
		broker.dispose();
		finishLoad?.({ kind: 'missing' });
		await Promise.resolve();

		expect(port.postMessage).not.toHaveBeenCalled();
		expect(port.close).toHaveBeenCalledOnce();
	});
});
