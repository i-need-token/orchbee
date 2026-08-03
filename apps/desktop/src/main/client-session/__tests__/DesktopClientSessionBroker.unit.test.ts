import { EventEmitter } from 'node:events';

import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench/document';
import type { MessagePortMain } from 'electron';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import type { WorkbenchDocumentStore } from '../../workbench-document';
import { DesktopClientSessionBroker } from '../DesktopClientSessionBroker';
import { DesktopClientSessionCoordinator } from '../DesktopClientSessionCoordinator';

class TestMainPort extends EventEmitter {
	readonly close = vi.fn<() => void>();
	readonly postMessage = vi.fn<(value: unknown) => void>();
	readonly start = vi.fn<() => void>();
}

describe('DesktopClientSessionBroker', () => {
	it('maps one scoped MessagePort onto bootstrap, publication, and correlated proposal result', async () => {
		const store = storeWith();
		const coordinator = await startedCoordinator(store);
		const port = new TestMainPort();
		const broker = new DesktopClientSessionBroker(port as unknown as MessagePortMain, coordinator, {
			disposeAttachments: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
		});
		broker.start();

		expect(broker.bootstrap).toMatchObject({
			documentState: { kind: 'missing' },
			sequence: '0',
			type: 'orchbee.desktop.client-session-bootstrap',
		});
		expect(port.start).toHaveBeenCalledOnce();
		port.emit('message', {
			data: {
				document: document('0', 'Committed'),
				expectedRevision: null,
				requestId: 'request-1',
				type: 'shared-document-proposed',
			},
			ports: [],
		});

		await vi.waitFor(() => expect(port.postMessage).toHaveBeenCalledTimes(2));
		expect(port.postMessage).toHaveBeenNthCalledWith(
			1,
			expect.objectContaining({ sequence: '1', type: 'shared-document-published' }),
		);
		expect(port.postMessage).toHaveBeenNthCalledWith(
			2,
			expect.objectContaining({
				requestId: 'request-1',
				result: expect.objectContaining({ kind: 'committed' }),
				type: 'shared-document-proposal-completed',
			}),
		);
	});

	it('retires only its session on malformed input or transferred authority', async () => {
		const coordinator = await startedCoordinator(storeWith());
		const port = new TestMainPort();
		const disposeAttachments = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
		const broker = new DesktopClientSessionBroker(port as unknown as MessagePortMain, coordinator, {
			disposeAttachments,
		});
		broker.start();
		const transferred = new TestMainPort();

		port.emit('message', {
			data: { type: 'generic-operation' },
			ports: [transferred as unknown as MessagePortMain],
		});
		await vi.waitFor(() => expect(disposeAttachments).toHaveBeenCalledOnce());

		expect(transferred.close).toHaveBeenCalledOnce();
		expect(port.close).toHaveBeenCalledOnce();
		expect(coordinator.sessionCount).toBe(0);
	});

	it('ignores an exact request replay and retires a conflicting reuse', async () => {
		let finishCommit:
			| ((result: Awaited<ReturnType<WorkbenchDocumentStore['commit']>>) => void)
			| undefined;
		const store = storeWith();
		store.commit.mockImplementation(
			() =>
				new Promise((resolve) => {
					finishCommit = resolve;
				}),
		);
		const coordinator = await startedCoordinator(store);
		const port = new TestMainPort();
		const broker = new DesktopClientSessionBroker(port as unknown as MessagePortMain, coordinator, {
			disposeAttachments: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
		});
		broker.start();
		const command = {
			document: document('0', 'First'),
			expectedRevision: null,
			requestId: 'same-request',
			type: 'shared-document-proposed',
		};
		port.emit('message', { data: command, ports: [] });
		port.emit('message', { data: command, ports: [] });
		await vi.waitFor(() => expect(store.commit).toHaveBeenCalledOnce());

		port.emit('message', {
			data: { ...command, document: document('0', 'Different') },
			ports: [],
		});
		await vi.waitFor(() => expect(coordinator.sessionCount).toBe(0));
		finishCommit?.({ kind: 'committed', revision: workbenchSharedDocumentRevision('0') });

		expect(store.commit).toHaveBeenCalledOnce();
	});
});

async function startedCoordinator(
	store: WorkbenchDocumentStore,
): Promise<DesktopClientSessionCoordinator> {
	let sessionSequence = 0;
	const coordinator = new DesktopClientSessionCoordinator({
		createClientSessionId: () => `client-${++sessionSequence}`,
		defaultCwd: '/workspace',
		getTerminalEvents: () => [],
		initialPresentationId: terminalPresentationId('presentation-initial'),
		initializationDocument: document('0', 'Initial'),
		reconcileDocument: async () => undefined,
		store,
		validateDocument: () => true,
	});
	const result = await coordinator.start();
	if (result.kind !== 'ready') throw new Error('Expected a ready coordinator.');
	return coordinator;
}

function storeWith(): WorkbenchDocumentStore & {
	readonly commit: ReturnType<typeof vi.fn<WorkbenchDocumentStore['commit']>>;
} {
	return {
		commit: vi.fn<WorkbenchDocumentStore['commit']>().mockImplementation(async (_, candidate) => ({
			kind: 'committed',
			revision: candidate.revision,
		})),
		load: vi.fn<WorkbenchDocumentStore['load']>().mockResolvedValue({ kind: 'missing' }),
	};
}

function document(revision: string, title: string): WorkbenchSharedDocument {
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
				nextTabSequence: title.length + 1,
				panelWorkspace: {
					nextPanelSequence: 1,
					nextPanelSlotSequence: 1,
					nextSplitSequence: 1,
					nextTerminalSequence: 1,
					pendingRemovals: [],
					tabs: [],
				},
				tabCollection: { groups: [], tabs: [] },
				terminalContextId: 'terminal-context-1',
			},
		],
	});
}
