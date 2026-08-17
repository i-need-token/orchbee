// @vitest-environment jsdom

import {
	DESKTOP_FILE_PATH_BRIDGE_KEY,
	DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL,
	DESKTOP_RENDERER_BOOTSTRAP_WINDOW_MESSAGE,
} from '@orchbee/desktop-client-protocol';
import { WORKBENCH_CLIENT_SESSION_BOOTSTRAP } from '@orchbee/workbench-client-protocol';
import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import { describe, expect, it, vi } from 'vitest';

import {
	startDesktopWebApplication,
	type DesktopWebApplicationDependencies,
} from '../startDesktopWebApplication';

describe('startDesktopWebApplication', () => {
	it('starts the managed renderer once from one trusted atomic capability bundle', () => {
		const harness = createHarness();
		Object.assign(harness.targetWindow, {
			[DESKTOP_FILE_PATH_BRIDGE_KEY]: {
				getPathForFile: () => '/tmp/user-selected.txt',
				platform: 'darwin',
			},
		});
		const ports = messagePorts(4);
		const session = startDesktopWebApplication(
			harness.targetWindow,
			harness.root,
			document.documentElement,
			['en-US'],
			'apple',
			window.localStorage,
			harness.dependencies,
		);

		harness.dispatchMessage(rendererMessage(), ports);

		expect(harness.startRenderer).toHaveBeenCalledWith(
			harness.root,
			document.documentElement,
			['en-US'],
			'apple',
			window.localStorage,
			expect.objectContaining({
				clientSessionId: 'client-1',
				terminalHostName: 'orchbee-host',
			}),
			ports[2],
			ports[0],
			ports[1],
			ports[3],
			expect.objectContaining({ prepare: expect.any(Function) }),
		);
		expect(harness.root.dataset['desktopContextIsolated']).toBe('true');
		expect(harness.root.dataset['desktopSandboxed']).toBe('true');

		harness.dispatchMessage(rendererMessage(), messagePorts(4));
		expect(harness.startRenderer).toHaveBeenCalledOnce();
		session.dispose();
		expect(harness.disposeRenderer).toHaveBeenCalledOnce();
	});

	it('fails closed without starting Preview for an untrusted or partial bundle', () => {
		const harness = createHarness();
		const ports = messagePorts(3);
		const closeSpies = ports.map((port) => vi.spyOn(port, 'close'));
		startDesktopWebApplication(
			harness.targetWindow,
			harness.root,
			document.documentElement,
			['en-US'],
			'standard',
			window.localStorage,
			harness.dependencies,
		);

		harness.dispatchMessage(rendererMessage(), ports);

		expect(harness.startRenderer).not.toHaveBeenCalled();
		expect(closeSpies.every((spy) => spy.mock.calls.length === 1)).toBe(true);
	});
});

function createHarness() {
	let messageListener: ((event: MessageEvent<unknown>) => void) | undefined;
	let unloadListener: (() => void) | undefined;
	const disposeRenderer = vi.fn<() => void>();
	const startRenderer = vi.fn<DesktopWebApplicationDependencies['startRenderer']>(() => ({
		dispose: disposeRenderer,
		ready: Promise.resolve({ kind: 'ready' }),
	}));
	const targetWindow = {
		addEventListener: vi.fn<(type: string, listener: EventListener) => void>((type, listener) => {
			if (type === 'message') {
				messageListener = listener as (event: MessageEvent<unknown>) => void;
			} else if (type === 'unload') {
				unloadListener = listener as () => void;
			}
		}),
		removeEventListener: vi.fn<(type: string, listener: EventListener) => void>(),
	} as unknown as Window;
	const root = document.createElement('div');
	return {
		dependencies: { startRenderer },
		dispatchMessage: (data: unknown, ports: readonly MessagePort[]) =>
			messageListener?.({ data, ports, source: targetWindow } as MessageEvent<unknown>),
		disposeRenderer,
		root,
		startRenderer,
		targetWindow,
		unload: () => unloadListener?.(),
	};
}

function messagePorts(count: number): MessagePort[] {
	return Array.from({ length: count }, () => new MessageChannel().port1);
}

function rendererMessage(): unknown {
	const documentState = createWorkbenchSharedDocument({
		projectWorkspace: {
			nextProjectSequence: 1,
			nextTerminalContextSequence: 2,
			nextWorkspaceSequence: 1,
			projects: [],
			standaloneTerminalContextId: 'terminal-context-1',
		},
		revision: workbenchSharedDocumentRevision('0'),
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
				tabCollection: { groups: [], tabs: [] },
				terminalContextId: 'terminal-context-1',
			},
		],
	});
	return {
		bootstrap: {
			clientSession: {
				clientSessionId: 'client-1',
				defaultCwd: '/workspace',
				documentState: { document: documentState, kind: 'available' },
				initialPresentationId: 'presentation-initial',
				initializationDocument: documentState,
				sequence: '0',
				terminalHostName: 'orchbee-host',
				terminalEvents: [],
				type: WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
			},
			type: DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL,
		},
		security: { contextIsolated: true, sandboxed: true },
		type: DESKTOP_RENDERER_BOOTSTRAP_WINDOW_MESSAGE,
	};
}
