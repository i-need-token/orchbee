// @vitest-environment jsdom

import { act } from 'react';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';
import {
	createWorkbenchTopologyDocument,
	workbenchTopologyRevision,
} from '@orchbee/workbench/topology';
import { terminalPresentationId } from '@orchbee/workbench';

import {
	TerminalDocumentControlBootstrapSchema,
	type TerminalDocumentControlCommand,
	type WorkbenchTopologyControlCommand,
} from '../../../shared';
import { startDesktopWorkbenchRenderer } from '../startDesktopWorkbenchRenderer';

const originalResizeObserver = globalThis.ResizeObserver;

class ResizeObserverMock {
	disconnect(): void {
		return;
	}

	observe(): void {
		return;
	}

	unobserve(): void {
		return;
	}
}

beforeAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: true,
	});
	Object.defineProperty(globalThis, 'ResizeObserver', {
		configurable: true,
		value: ResizeObserverMock,
	});
});

afterAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: false,
	});
	Object.defineProperty(globalThis, 'ResizeObserver', {
		configurable: true,
		value: originalResizeObserver,
	});
});

class TestMessagePort extends EventTarget {
	peer: TestMessagePort | undefined;
	closed = false;
	starts = 0;

	close(): void {
		this.closed = true;
	}

	postMessage(message: unknown, transfer: Transferable[] = []): void {
		if (this.closed) throw new Error('MessagePort is closed.');
		const peer = this.peer;
		queueMicrotask(() => {
			if (peer?.closed === false) {
				peer.dispatchEvent(
					new MessageEvent('message', { data: message, ports: transfer as MessagePort[] }),
				);
			}
		});
	}

	start(): void {
		this.starts += 1;
	}
}

function createTestChannel(): { readonly port1: MessagePort; readonly port2: TestMessagePort } {
	const port1 = new TestMessagePort();
	const port2 = new TestMessagePort();
	port1.peer = port2;
	port2.peer = port1;
	return { port1: port1 as unknown as MessagePort, port2 };
}

describe('startDesktopWorkbenchRenderer', () => {
	it('fails closed without rendering or provisioning when durable topology cannot be loaded', async () => {
		const lifetimeChannel = createTestChannel();
		const documentChannel = createTestChannel();
		const projectionChannel = createTestChannel();
		const topologyChannel = createTestChannel();
		const documentCommands: TerminalDocumentControlCommand[] = [];
		documentChannel.port2.addEventListener('message', (event) => {
			documentCommands.push((event as MessageEvent).data as TerminalDocumentControlCommand);
		});
		topologyChannel.port2.addEventListener('message', (event) => {
			const command = (event as MessageEvent).data as WorkbenchTopologyControlCommand;
			if (command.type !== 'workbench-topology-load-requested') return;
			topologyChannel.port2.postMessage({
				requestId: command.requestId,
				result: { kind: 'unavailable', reason: 'read-failed' },
				type: 'workbench-topology-load-completed',
			});
		});
		const root = document.createElement('div');
		document.body.appendChild(root);

		await act(async () => {
			const session = startDesktopWorkbenchRenderer(
				root,
				document.documentElement,
				['en-US'],
				'apple',
				window.localStorage,
				TerminalDocumentControlBootstrapSchema.parse({
					defaultCwd: '/workspace',
					initialPresentationId: 'presentation-initial',
					presentations: [],
					type: 'orchbee.desktop.terminal-document-control-port',
				}),
				lifetimeChannel.port1,
				documentChannel.port1,
				projectionChannel.port1,
				topologyChannel.port1,
			);
			await expect(session.ready).resolves.toEqual({
				kind: 'unavailable',
				reason: 'read-failed',
			});
		});

		expect(documentCommands).toEqual([]);
		expect(root.childElementCount).toBe(0);
		expect((documentChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		expect((lifetimeChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		expect((lifetimeChannel.port1 as unknown as TestMessagePort).starts).toBe(0);
		expect((projectionChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		expect((topologyChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		root.remove();
	});

	it('requests projection only for the active visible terminal and switches demand with the Tab', async () => {
		const lifetimeChannel = createTestChannel();
		const documentChannel = createTestChannel();
		const projectionChannel = createTestChannel();
		const topologyChannel = createTestChannel();
		const documentCommands: TerminalDocumentControlCommand[] = [];
		const projectionCommands: Array<{ presentationId?: string; type?: string }> = [];
		const topologyCommands: WorkbenchTopologyControlCommand[] = [];
		documentChannel.port2.addEventListener('message', (event) => {
			documentCommands.push((event as MessageEvent).data as TerminalDocumentControlCommand);
		});
		projectionChannel.port2.addEventListener('message', (event) => {
			projectionCommands.push(
				(event as MessageEvent).data as { presentationId?: string; type?: string },
			);
		});
		topologyChannel.port2.addEventListener('message', (event) => {
			const command = (event as MessageEvent).data as WorkbenchTopologyControlCommand;
			topologyCommands.push(command);
			if (command.type === 'workbench-topology-load-requested') {
				topologyChannel.port2.postMessage({
					requestId: command.requestId,
					result: { kind: 'missing' },
					type: 'workbench-topology-load-completed',
				});
				return;
			}
			topologyChannel.port2.postMessage({
				requestId: command.requestId,
				result: { kind: 'committed', revision: command.document.revision },
				type: 'workbench-topology-commit-completed',
			});
		});
		const root = document.createElement('div');
		document.body.appendChild(root);
		let session: ReturnType<typeof startDesktopWorkbenchRenderer> | undefined;
		await act(async () => {
			const started = startDesktopWorkbenchRenderer(
				root,
				document.documentElement,
				['en-US'],
				'apple',
				window.localStorage,
				TerminalDocumentControlBootstrapSchema.parse({
					defaultCwd: '/workspace',
					initialPresentationId: 'presentation-initial',
					presentations: [
						{ launchCwd: '/one', presentationId: 'presentation-1' },
						{ launchCwd: '/two', presentationId: 'presentation-2' },
					],
					type: 'orchbee.desktop.terminal-document-control-port',
				}),
				lifetimeChannel.port1,
				documentChannel.port1,
				projectionChannel.port1,
				topologyChannel.port1,
			);
			session = started;
			await expect(started.ready).resolves.toEqual({ kind: 'ready' });
		});
		await vi.waitFor(() => expect(documentCommands).toHaveLength(2));
		expect((lifetimeChannel.port1 as unknown as TestMessagePort).starts).toBe(1);
		expect(topologyCommands.slice(0, 2).map((command) => command.type)).toEqual([
			'workbench-topology-load-requested',
			'workbench-topology-commit-requested',
		]);

		await act(async () => {
			for (const command of documentCommands) {
				documentChannel.port2.postMessage({
					operationId: command.operationId,
					presentationId: command.presentationId,
					type: 'terminal-provision-completed',
				});
			}
		});
		await vi.waitFor(() => {
			expect(projectionCommands).toEqual([
				expect.objectContaining({
					presentationId: 'presentation-1',
					type: 'terminal-projection-open-requested',
				}),
			]);
		});

		const secondTab = [...root.querySelectorAll<HTMLElement>('[role="tab"]')].find((tab) =>
			tab.getAttribute('aria-label')?.includes('Terminal 2'),
		);
		expect(secondTab).toBeDefined();
		await act(async () => {
			secondTab?.dispatchEvent(
				new MouseEvent('pointerdown', { bubbles: true, button: 0, cancelable: true }),
			);
		});
		await vi.waitFor(() => {
			expect(projectionCommands).toEqual(
				expect.arrayContaining([
					expect.objectContaining({
						presentationId: 'presentation-1',
						type: 'terminal-projection-release-requested',
					}),
					expect.objectContaining({
						presentationId: 'presentation-2',
						type: 'terminal-projection-open-requested',
					}),
				]),
			);
		});

		await act(async () => session?.dispose());
		expect((topologyChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		root.remove();
	});

	it('hydrates retain topology and uses restore-only reconciliation without rewriting intent', async () => {
		const lifetimeChannel = createTestChannel();
		const documentChannel = createTestChannel();
		const projectionChannel = createTestChannel();
		const topologyChannel = createTestChannel();
		const documentCommands: TerminalDocumentControlCommand[] = [];
		const topologyCommands: WorkbenchTopologyControlCommand[] = [];
		const topology = createRetainedTopology();
		documentChannel.port2.addEventListener('message', (event) => {
			documentCommands.push((event as MessageEvent).data as TerminalDocumentControlCommand);
		});
		topologyChannel.port2.addEventListener('message', (event) => {
			const command = (event as MessageEvent).data as WorkbenchTopologyControlCommand;
			topologyCommands.push(command);
			if (command.type !== 'workbench-topology-load-requested') return;
			topologyChannel.port2.postMessage({
				requestId: command.requestId,
				result: { document: topology, kind: 'available' },
				type: 'workbench-topology-load-completed',
			});
		});
		const root = document.createElement('div');
		document.body.appendChild(root);
		let session: ReturnType<typeof startDesktopWorkbenchRenderer> | undefined;

		await act(async () => {
			const started = startDesktopWorkbenchRenderer(
				root,
				document.documentElement,
				['en-US'],
				'apple',
				window.localStorage,
				TerminalDocumentControlBootstrapSchema.parse({
					defaultCwd: '/workspace',
					initialPresentationId: 'presentation-unused',
					presentations: [{ launchCwd: '/retained', presentationId: 'presentation-retained' }],
					type: 'orchbee.desktop.terminal-document-control-port',
				}),
				lifetimeChannel.port1,
				documentChannel.port1,
				projectionChannel.port1,
				topologyChannel.port1,
			);
			session = started;
			await expect(started.ready).resolves.toEqual({ kind: 'ready' });
		});
		await vi.waitFor(() => expect(documentCommands).toHaveLength(1));
		expect(documentCommands[0]).toMatchObject({
			launchCwd: '/retained',
			presentationId: 'presentation-retained',
			type: 'terminal-restore-requested',
		});
		expect(topologyCommands.map((command) => command.type)).toEqual([
			'workbench-topology-load-requested',
		]);

		await act(async () => {
			const restore = documentCommands[0];
			if (restore?.type !== 'terminal-restore-requested') {
				throw new Error('Expected a restore command.');
			}
			documentChannel.port2.postMessage({
				operationId: restore.operationId,
				presentationId: restore.presentationId,
				type: 'terminal-restore-completed',
			});
		});
		await vi.waitFor(() => expect(root.querySelector('[aria-label="Retained"]')).not.toBeNull());
		expect(topologyCommands.map((command) => command.type)).toEqual([
			'workbench-topology-load-requested',
		]);

		await act(async () => session?.dispose());
		root.remove();
	});
});

function createRetainedTopology() {
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
		revision: workbenchTopologyRevision('4'),
		terminalContexts: [
			{
				nextTabSequence: 2,
				panelWorkspace: {
					nextPanelSequence: 2,
					nextPanelSlotSequence: 1,
					nextSplitSequence: 1,
					nextTerminalSequence: 2,
					pendingRemovals: [],
					tabs: [
						{
							focusedPanelId: 'panel-1',
							maximizedPanelId: null,
							root: {
								activeChildPresentationId: null,
								childPresentations: [],
								id: 'panel-1',
								kind: 'panel',
								primaryPresentation: {
									intent: { kind: 'retain' },
									launchCwd: '/retained',
									presentationId: terminalPresentationId('presentation-retained'),
									title: 'Retained',
								},
							},
							tabId: 'terminal-context-1-tab-1',
						},
					],
				},
				tabCollection: {
					activeTabId: 'terminal-context-1-tab-1',
					groups: [],
					mostRecentGroupId: null,
					tabs: [
						{
							group: null,
							id: 'terminal-context-1-tab-1',
							pinned: false,
							title: 'Retained',
						},
					],
				},
				terminalContextId: 'terminal-context-1',
			},
		],
	});
}
