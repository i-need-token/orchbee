// @vitest-environment jsdom

import {
	DESKTOP_FILE_PATH_BRIDGE_KEY,
	DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL,
	DESKTOP_RENDERER_BOOTSTRAP_WINDOW_MESSAGE,
	DESKTOP_RENDERER_PORT_ROLES,
} from '@orchbee/desktop-client-protocol';
import { WORKBENCH_CLIENT_SESSION_BOOTSTRAP } from '@orchbee/workbench-client-protocol';
import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	startDesktopWebApplication,
	type DesktopWebApplicationDependencies,
} from '../startDesktopWebApplication';

const ownedPorts = new Set<MessagePort>();

afterEach(() => {
	for (const port of ownedPorts) port.close();
	ownedPorts.clear();
	vi.restoreAllMocks();
});

describe('startDesktopWebApplication', () => {
	it('starts the managed renderer once from one trusted atomic capability bundle', () => {
		const harness = createHarness();
		const storage = desktopStorage();
		Object.assign(harness.targetWindow, {
			[DESKTOP_FILE_PATH_BRIDGE_KEY]: {
				getPathForFile: () => '/tmp/user-selected.txt',
				platform: 'darwin',
			},
		});
		const ports = messagePorts(DESKTOP_RENDERER_PORT_ROLES.length);
		const session = startDesktopWebApplication(
			harness.targetWindow,
			harness.root,
			document.documentElement,
			['en-US'],
			'apple',
			storage,
			harness.dependencies,
		);

		harness.dispatchMessage(rendererMessage(), ports);

		expect(harness.startRenderer).toHaveBeenCalledWith(
			harness.root,
			document.documentElement,
			['en-US'],
			'apple',
			storage,
			expect.objectContaining({
				clientSessionId: 'client-1',
				terminalHostName: 'orchbee-host',
			}),
			ports[4],
			ports[0],
			ports[1],
			ports[2],
			ports[3],
			ports[5],
			ports[6],
			ports[7],
			ports[8],
			ports[9],
			ports[10],
			expect.objectContaining({ prepare: expect.any(Function) }),
		);
		expect(harness.root.dataset['desktopContextIsolated']).toBe('true');
		expect(harness.root.dataset['desktopSandboxed']).toBe('true');

		harness.dispatchMessage(rendererMessage(), messagePorts(DESKTOP_RENDERER_PORT_ROLES.length));
		expect(harness.startRenderer).toHaveBeenCalledOnce();
		session.dispose();
		expect(harness.disposeRenderer).toHaveBeenCalledOnce();
	});

	it('fails closed without starting Preview for an untrusted or partial bundle', () => {
		const harness = createHarness();
		const ports = messagePorts(DESKTOP_RENDERER_PORT_ROLES.length - 1);
		const closeSpies = ports.map((port) => vi.spyOn(port, 'close'));
		const session = startDesktopWebApplication(
			harness.targetWindow,
			harness.root,
			document.documentElement,
			['en-US'],
			'standard',
			desktopStorage(),
			harness.dependencies,
		);

		harness.dispatchMessage(rendererMessage(), ports);

		expect(harness.startRenderer).not.toHaveBeenCalled();
		expect(closeSpies.every((spy) => spy.mock.calls.length === 1)).toBe(true);
		session.dispose();
	});

	it('releases a renderer returned after unload without publishing bootstrap security markers', () => {
		const harness = createHarness();
		harness.startRenderer.mockImplementationOnce(() => {
			harness.unload();
			return { dispose: harness.disposeRenderer, ready: Promise.resolve({ kind: 'ready' }) };
		});
		const session = startHarness(harness);

		harness.dispatchMessage(rendererMessage(), messagePorts(DESKTOP_RENDERER_PORT_ROLES.length));

		expect(harness.disposeRenderer).toHaveBeenCalledOnce();
		expect(harness.root.dataset['desktopContextIsolated']).toBeUndefined();
		expect(harness.root.dataset['desktopSandboxed']).toBeUndefined();
		session.dispose();
		expect(harness.disposeRenderer).toHaveBeenCalledOnce();
	});

	it('reserves bootstrap admission before invoking the renderer factory', () => {
		const harness = createHarness();
		const extraPorts = messagePorts(DESKTOP_RENDERER_PORT_ROLES.length);
		const closeSpies = extraPorts.map((port) => vi.spyOn(port, 'close'));
		harness.startRenderer.mockImplementationOnce(() => {
			harness.dispatchMessage(rendererMessage(), extraPorts);
			return { dispose: harness.disposeRenderer, ready: Promise.resolve({ kind: 'ready' }) };
		});
		const session = startHarness(harness);

		harness.dispatchMessage(rendererMessage(), messagePorts(DESKTOP_RENDERER_PORT_ROLES.length));

		expect(harness.startRenderer).toHaveBeenCalledOnce();
		expect(closeSpies.every((spy) => spy.mock.calls.length === 1)).toBe(true);
		session.dispose();
		expect(harness.disposeRenderer).toHaveBeenCalledOnce();
	});

	it('closes the bundle when capability discovery unloads the application before startup', () => {
		const harness = createHarness();
		const discoverCapability = vi.fn<() => undefined>(() => {
			harness.unload();
			return undefined;
		});
		Object.defineProperty(harness.targetWindow, DESKTOP_FILE_PATH_BRIDGE_KEY, {
			get: discoverCapability,
		});
		const ports = messagePorts(DESKTOP_RENDERER_PORT_ROLES.length);
		const closeSpies = ports.map((port) => vi.spyOn(port, 'close'));
		const session = startHarness(harness);

		harness.dispatchMessage(rendererMessage(), ports);

		expect(discoverCapability).toHaveBeenCalledOnce();
		expect(harness.startRenderer).not.toHaveBeenCalled();
		expect(closeSpies.every((spy) => spy.mock.calls.length === 1)).toBe(true);
		expect(harness.root.dataset['desktopContextIsolated']).toBeUndefined();
		session.dispose();
	});

	it('closes a failed bootstrap bundle and retires its listeners', () => {
		const harness = createHarness();
		const ports = messagePorts(DESKTOP_RENDERER_PORT_ROLES.length);
		const closeSpies = ports.map((port) => vi.spyOn(port, 'close'));
		harness.startRenderer.mockImplementationOnce(() => {
			throw new Error('Renderer acquisition failed.');
		});
		const session = startHarness(harness);

		harness.dispatchMessage(rendererMessage(), ports);
		harness.dispatchMessage(rendererMessage(), messagePorts(DESKTOP_RENDERER_PORT_ROLES.length));

		expect(closeSpies.every((spy) => spy.mock.calls.length === 1)).toBe(true);
		expect(harness.startRenderer).toHaveBeenCalledOnce();
		expect(harness.targetWindow.removeEventListener).toHaveBeenCalledTimes(2);
		expect(harness.root.dataset['desktopContextIsolated']).toBeUndefined();
		session.dispose();
	});
});

function startHarness(harness: ReturnType<typeof createHarness>) {
	return startDesktopWebApplication(
		harness.targetWindow,
		harness.root,
		document.documentElement,
		['en-US'],
		'standard',
		desktopStorage(),
		harness.dependencies,
	);
}

function desktopStorage() {
	return {
		focusRecovery: window.sessionStorage,
		preferences: window.localStorage,
	};
}

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
		removeEventListener: vi.fn<(type: string, listener: EventListener) => void>(
			(type, listener) => {
				if (type === 'message' && messageListener === listener) messageListener = undefined;
				if (type === 'unload' && unloadListener === listener) unloadListener = undefined;
			},
		),
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
	return Array.from({ length: count }, () => {
		const channel = new MessageChannel();
		ownedPorts.add(channel.port1);
		ownedPorts.add(channel.port2);
		return channel.port1;
	});
}

function rendererMessage(): unknown {
	const documentState = createWorkbenchSharedDocument({
		projectWorkspace: {
			nextResourceScopeSequence: 1,
			resourceScopes: [],
			workspaces: [],
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
