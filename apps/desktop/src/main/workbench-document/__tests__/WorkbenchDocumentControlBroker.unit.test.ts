import { EventEmitter } from 'node:events';

import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench/document';
import type { MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import type { WorkbenchDocumentStore } from '../decideWorkbenchDocumentCommit';
import { WorkbenchDocumentControlBroker } from '../WorkbenchDocumentControlBroker';

function document(revision: string): WorkbenchSharedDocument {
	return createWorkbenchSharedDocument({
		projectWorkspace: {
			nextProjectSequence: 1,
			nextTerminalContextSequence: 2,
			nextWorkspaceSequence: 1,
			projects: [],
			standaloneTerminalContextId: 'terminal-context-1',
		},
		revision: workbenchSharedDocumentRevision(revision),
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
					groups: [],
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

describe('WorkbenchDocumentControlBroker', () => {
	it('correlates strict load and CAS commit results on one scoped port', async () => {
		const port = new TestMainPort();
		const store: WorkbenchDocumentStore = {
			commit: vi.fn<WorkbenchDocumentStore['commit']>().mockResolvedValue({
				kind: 'committed',
				revision: workbenchSharedDocumentRevision('0'),
			}),
			load: vi.fn<WorkbenchDocumentStore['load']>().mockResolvedValue({ kind: 'missing' }),
		};
		const broker = new WorkbenchDocumentControlBroker(port as unknown as MessagePortMain, store);
		broker.start();

		port.emit(
			'message',
			message({ requestId: 'request-load', type: 'workbench-document-load-requested' }),
		);
		port.emit(
			'message',
			message({
				document: document('0'),
				expectedRevision: null,
				requestId: 'request-commit',
				type: 'workbench-document-commit-requested',
			}),
		);

		await vi.waitFor(() => expect(port.postMessage).toHaveBeenCalledTimes(2));
		expect(port.postMessage).toHaveBeenCalledWith({
			requestId: 'request-load',
			result: { kind: 'missing' },
			type: 'workbench-document-load-completed',
		});
		expect(port.postMessage).toHaveBeenCalledWith({
			requestId: 'request-commit',
			result: { kind: 'committed', revision: '0' },
			type: 'workbench-document-commit-completed',
		});
	});

	it('retires on request-identity reuse with different content or transferred authority', () => {
		const port = new TestMainPort();
		const store: WorkbenchDocumentStore = {
			commit: vi.fn<WorkbenchDocumentStore['commit']>(),
			load: vi.fn<WorkbenchDocumentStore['load']>().mockResolvedValue({ kind: 'missing' }),
		};
		const broker = new WorkbenchDocumentControlBroker(port as unknown as MessagePortMain, store);
		broker.start();
		port.emit(
			'message',
			message({ requestId: 'same-request', type: 'workbench-document-load-requested' }),
		);
		port.emit(
			'message',
			message({
				document: document('0'),
				expectedRevision: null,
				requestId: 'same-request',
				type: 'workbench-document-commit-requested',
			}),
		);

		expect(port.close).toHaveBeenCalledOnce();

		const transferredPort = new TestMainPort();
		const transferredBroker = new WorkbenchDocumentControlBroker(
			transferredPort as unknown as MessagePortMain,
			store,
		);
		transferredBroker.start();
		transferredPort.emit(
			'message',
			message({ requestId: 'transfer-request', type: 'workbench-document-load-requested' }, [
				{} as MessagePortMain,
			]),
		);
		expect(transferredPort.close).toHaveBeenCalledOnce();
	});

	it('suppresses a completion after its renderer generation is disposed', async () => {
		let finishLoad:
			| ((value: Awaited<ReturnType<WorkbenchDocumentStore['load']>>) => void)
			| undefined;
		const store: WorkbenchDocumentStore = {
			commit: vi.fn<WorkbenchDocumentStore['commit']>(),
			load: () =>
				new Promise((resolve) => {
					finishLoad = resolve;
				}),
		};
		const port = new TestMainPort();
		const broker = new WorkbenchDocumentControlBroker(port as unknown as MessagePortMain, store);
		broker.start();
		port.emit(
			'message',
			message({ requestId: 'late-request', type: 'workbench-document-load-requested' }),
		);
		broker.dispose();
		finishLoad?.({ kind: 'missing' });
		await Promise.resolve();

		expect(port.postMessage).not.toHaveBeenCalled();
		expect(port.close).toHaveBeenCalledOnce();
	});
});
