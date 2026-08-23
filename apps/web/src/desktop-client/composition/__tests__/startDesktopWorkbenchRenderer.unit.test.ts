// @vitest-environment jsdom

import { act } from 'react';
import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench/document';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	type TerminalPresentationEvent,
} from '@orchbee/workbench/terminal-presentation';

import {
	WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
	WorkbenchClientSessionBootstrapSchema,
	WorkbenchClientSessionCommandSchema,
	WorkbenchClientSessionEventSchema,
	type WorkbenchClientSessionBootstrap,
} from '@orchbee/workbench-client-protocol';
import {
	DesktopProjectFolderAcquisitionMainEventSchema,
	DesktopProjectFolderAcquisitionRendererCommandSchema,
	DesktopTerminalLaunchProfileMainEventSchema,
	DesktopTerminalLaunchProfileRendererCommandSchema,
} from '@orchbee/desktop-client-protocol';
import { startDesktopWorkbenchRenderer } from '../startDesktopWorkbenchRenderer';

const mountedGeometry = { cellPixelHeight: 18, cellPixelWidth: 9, columns: 96, rows: 32 } as const;

vi.mock(import('@orchbee/terminal'), async (importOriginal) => ({
	...(await importOriginal()),
	measureMountedTerminalGeometry: async () => mountedGeometry,
}));

const originalResizeObserver = globalThis.ResizeObserver;
const originalScrollTo = window.scrollTo;
type WorkbenchClientSessionCommand = ReturnType<typeof WorkbenchClientSessionCommandSchema.parse>;

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
	Object.defineProperty(window, 'scrollTo', {
		configurable: true,
		value: () => undefined,
	});
});

beforeEach(() => {
	window.localStorage.clear();
	window.sessionStorage.clear();
	window.history.replaceState({}, '', '/');
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
	Object.defineProperty(window, 'scrollTo', {
		configurable: true,
		value: originalScrollTo,
	});
});

class TestMessagePort extends EventTarget {
	closed = false;
	peer: TestMessagePort | undefined;
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

function serveSuccessfulClientSession(port: TestMessagePort): WorkbenchClientSessionCommand[] {
	const commands: WorkbenchClientSessionCommand[] = [];
	let sequence = 0n;
	port.addEventListener('message', (event) => {
		const command = WorkbenchClientSessionCommandSchema.parse((event as MessageEvent).data);
		commands.push(command);
		sequence += 1n;
		port.postMessage(
			WorkbenchClientSessionEventSchema.parse({
				document: command.document,
				sequence: sequence.toString(),
				type: 'shared-document-published',
			}),
		);
		port.postMessage(
			WorkbenchClientSessionEventSchema.parse({
				requestId: command.requestId,
				result: { document: command.document, kind: 'committed' },
				type: 'shared-document-proposal-completed',
			}),
		);
	});
	return commands;
}

function serveTerminalLaunchProfiles(port: TestMessagePort): void {
	port.addEventListener('message', (event) => {
		const command = DesktopTerminalLaunchProfileRendererCommandSchema.parse(
			(event as MessageEvent).data,
		);
		port.postMessage(
			DesktopTerminalLaunchProfileMainEventSchema.parse({
				requestId: command.requestId,
				result: {
					kind: 'completed',
					snapshot: {
						capabilities: { loginShell: true },
						catalog: {
							defaultProfileId: 'system-default',
							profiles: [
								{
									command: { kind: 'system-shell', login: false },
									id: 'system-default',
									name: null,
								},
							],
							version: 1,
						},
						persistenceIssue: null,
						source: 'built-in',
					},
				},
				type: 'desktop-terminal-launch-profile-responded',
			}),
		);
	});
}

describe('startDesktopWorkbenchRenderer', () => {
	it('fails closed without rendering when the initial shared-document commit is unavailable', async () => {
		const lifetimeChannel = createTestChannel();
		const clientSessionChannel = createTestChannel();
		const projectionChannel = createTestChannel();
		const browserAccessChannel = createTestChannel();
		const projectFolderAcquisitionChannel = createTestChannel();
		const terminalLaunchProfileChannel = createTestChannel();
		serveTerminalLaunchProfiles(terminalLaunchProfileChannel.port2);
		const initializationDocument = provisionDocument('0', ['Terminal 1']);
		const commands: WorkbenchClientSessionCommand[] = [];
		clientSessionChannel.port2.addEventListener('message', (event) => {
			const command = WorkbenchClientSessionCommandSchema.parse((event as MessageEvent).data);
			commands.push(command);
			clientSessionChannel.port2.postMessage(
				WorkbenchClientSessionEventSchema.parse({
					requestId: command.requestId,
					result: { document: null, kind: 'unavailable', reason: 'read-failed' },
					type: 'shared-document-proposal-completed',
				}),
			);
		});
		const root = document.createElement('div');
		document.body.appendChild(root);

		await act(async () => {
			const session = startDesktopWorkbenchRenderer(
				root,
				document.documentElement,
				['en-US'],
				'apple',
				desktopStorage(),
				bootstrap({ kind: 'missing' }, initializationDocument),
				lifetimeChannel.port1,
				clientSessionChannel.port1,
				projectionChannel.port1,
				browserAccessChannel.port1,
				projectFolderAcquisitionChannel.port1,
				terminalLaunchProfileChannel.port1,
			);
			await expect(session.ready).resolves.toEqual({ kind: 'unavailable', reason: 'read-failed' });
		});

		expect(commands).toHaveLength(1);
		expect(root.childElementCount).toBe(0);
		expect((clientSessionChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		expect((lifetimeChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		expect((projectionChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		expect((browserAccessChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		expect((projectFolderAcquisitionChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		expect((terminalLaunchProfileChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		root.remove();
	});

	it('projects only the visible Tab while client-only selection stays out of the shared document', async () => {
		const lifetimeChannel = createTestChannel();
		const clientSessionChannel = createTestChannel();
		const projectionChannel = createTestChannel();
		const browserAccessChannel = createTestChannel();
		const projectFolderAcquisitionChannel = createTestChannel();
		const terminalLaunchProfileChannel = createTestChannel();
		serveTerminalLaunchProfiles(terminalLaunchProfileChannel.port2);
		const sharedDocument = provisionDocument('4', ['Terminal 1', 'Terminal 2']);
		const sessionCommands = serveSuccessfulClientSession(clientSessionChannel.port2);
		const projectionCommands: Array<{ presentationId?: string; type?: string }> = [];
		projectionChannel.port2.addEventListener('message', (event) => {
			projectionCommands.push(
				(event as MessageEvent).data as { presentationId?: string; type?: string },
			);
		});
		projectFolderAcquisitionChannel.port2.addEventListener('message', (event) => {
			const command = DesktopProjectFolderAcquisitionRendererCommandSchema.parse(
				(event as MessageEvent).data,
			);
			projectFolderAcquisitionChannel.port2.postMessage(
				DesktopProjectFolderAcquisitionMainEventSchema.parse({
					requestId: command.requestId,
					result: { kind: 'selected', path: '/workspace/native-project' },
					type: 'desktop-project-folder-acquisition-completed',
				}),
			);
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
				desktopStorage(),
				bootstrap(
					{ document: sharedDocument, kind: 'available' },
					sharedDocument,
					provisionCompletedEvents(2),
				),
				lifetimeChannel.port1,
				clientSessionChannel.port1,
				projectionChannel.port1,
				browserAccessChannel.port1,
				projectFolderAcquisitionChannel.port1,
				terminalLaunchProfileChannel.port1,
			);
			session = started;
			await expect(started.ready).resolves.toEqual({ kind: 'ready' });
		});
		await vi.waitFor(() => {
			expect(projectionCommands).toEqual([
				expect.objectContaining({
					presentationId: 'presentation-1',
					type: 'terminal-projection-open-requested',
				}),
			]);
		});
		const proposalCountAfterStartup = sessionCommands.length;
		expect(root.querySelector('[data-slot="desktop-workbench"]')).not.toBeNull();

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
		expect(sessionCommands).toHaveLength(proposalCountAfterStartup);
		const settings = root.querySelector<HTMLElement>('[data-slot="workbench-sidebar-settings"]');
		expect(settings).not.toBeNull();
		await act(async () => {
			settings?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
		});
		await vi.waitFor(() => {
			expect(root.querySelector('[data-slot="terminal-settings-surface"]')).not.toBeNull();
		});
		expect(document.querySelector('[data-slot="desktop-browser-access-dialog"]')).toBeNull();
		const browserAccessCategory = [...root.querySelectorAll<HTMLButtonElement>('button')].find(
			(button) => button.textContent?.trim() === 'Browser access',
		);
		expect(browserAccessCategory).toBeDefined();
		await act(async () => browserAccessCategory?.click());
		const openBrowserAccess = [...root.querySelectorAll<HTMLButtonElement>('button')].find(
			(button) => button.textContent?.trim() === 'Open Browser access',
		);
		expect(openBrowserAccess).toBeDefined();
		await act(async () => openBrowserAccess?.click());
		await vi.waitFor(() => {
			expect(document.querySelector('[data-slot="desktop-browser-access-dialog"]')).not.toBeNull();
		});
		expect(document.body.textContent).toContain('Full terminal control on trusted networks only');
		await act(async () => {
			document.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }));
		});
		await vi.waitFor(() => {
			expect(document.querySelector('[data-slot="desktop-browser-access-dialog"]')).toBeNull();
		});
		expect(root.querySelector('[data-slot="terminal-settings-surface"]')).not.toBeNull();
		await vi.waitFor(() => expect(document.activeElement).toBe(openBrowserAccess));
		await act(async () => {
			root
				.querySelector<HTMLElement>('[data-slot="terminal-settings-surface"]')
				?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }));
		});
		await vi.waitFor(() => {
			expect(root.querySelector('[data-slot="terminal-settings-surface"]')).toBeNull();
		});

		const createProject = root.querySelector<HTMLButtonElement>(
			'[data-slot="workbench-sidebar-project-create"] button',
		);
		expect(createProject).not.toBeNull();
		await act(async () => createProject?.click());
		let selectFolder: HTMLDivElement | undefined;
		await vi.waitFor(() => {
			selectFolder = [...document.querySelectorAll<HTMLDivElement>('[role="menuitem"]')].find(
				(item) => item.textContent?.trim() === 'Select Folder…',
			);
			expect(selectFolder).toBeDefined();
		});
		await act(async () => selectFolder?.click());
		await vi.waitFor(() =>
			expect(sessionCommands.at(-1)?.document.projectWorkspace.projects).toEqual(
				expect.arrayContaining([
					expect.objectContaining({
						directories: [expect.objectContaining({ path: '/workspace/native-project' })],
					}),
				]),
			),
		);

		await act(async () => session?.dispose());
		root.remove();
	});

	it('hydrates retained terminals from Main observations without issuing renderer effects', async () => {
		const lifetimeChannel = createTestChannel();
		const clientSessionChannel = createTestChannel();
		const projectionChannel = createTestChannel();
		const browserAccessChannel = createTestChannel();
		const projectFolderAcquisitionChannel = createTestChannel();
		const terminalLaunchProfileChannel = createTestChannel();
		serveTerminalLaunchProfiles(terminalLaunchProfileChannel.port2);
		const sharedDocument = retainedDocument('4');
		const sessionCommands = serveSuccessfulClientSession(clientSessionChannel.port2);
		const root = document.createElement('div');
		document.body.appendChild(root);
		let session: ReturnType<typeof startDesktopWorkbenchRenderer> | undefined;
		const operationId = terminalPresentationOperationId('restore-1');
		const presentationId = terminalPresentationId('presentation-retained');
		const terminalEvents: readonly TerminalPresentationEvent[] = [
			{ operationId, presentationId, type: 'terminal-restore-started' },
			{ operationId, presentationId, type: 'terminal-restore-completed' },
		];

		await act(async () => {
			const started = startDesktopWorkbenchRenderer(
				root,
				document.documentElement,
				['en-US'],
				'apple',
				desktopStorage(),
				bootstrap({ document: sharedDocument, kind: 'available' }, sharedDocument, terminalEvents),
				lifetimeChannel.port1,
				clientSessionChannel.port1,
				projectionChannel.port1,
				browserAccessChannel.port1,
				projectFolderAcquisitionChannel.port1,
				terminalLaunchProfileChannel.port1,
			);
			session = started;
			await expect(started.ready).resolves.toEqual({ kind: 'ready' });
		});

		expect(
			[...root.querySelectorAll<HTMLElement>('[role="tab"]')].map((tab) =>
				tab.getAttribute('aria-label'),
			),
		).toEqual(expect.arrayContaining([expect.stringContaining('Retained')]));
		expect(sessionCommands).toEqual([]);

		await act(async () => session?.dispose());
		root.remove();
	});
});

function desktopStorage() {
	return {
		focusRecovery: window.sessionStorage,
		preferences: window.localStorage,
	};
}

function bootstrap(
	documentState:
		| { readonly document: WorkbenchSharedDocument; readonly kind: 'available' }
		| { readonly kind: 'missing' },
	initializationDocument: WorkbenchSharedDocument,
	terminalEvents: readonly TerminalPresentationEvent[] = [],
): WorkbenchClientSessionBootstrap {
	return WorkbenchClientSessionBootstrapSchema.parse({
		clientSessionId: 'client-1',
		defaultCwd: '/workspace',
		documentState,
		initialPresentationId: 'presentation-initial',
		initializationDocument,
		sequence: '0',
		terminalHostName: 'orchbee-host',
		terminalEvents,
		type: WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
	});
}

function provisionCompletedEvents(count: number): readonly TerminalPresentationEvent[] {
	return Array.from({ length: count }, (_, index) => {
		const sequence = index + 1;
		return {
			operationId: terminalPresentationOperationId(`operation-${sequence}`),
			presentationId: terminalPresentationId(`presentation-${sequence}`),
			type: 'terminal-provision-completed',
		};
	});
}

function provisionDocument(revision: string, titles: readonly string[]): WorkbenchSharedDocument {
	return documentWithPresentations(
		revision,
		titles.map((title, index) => {
			const sequence = index + 1;
			return {
				intent: {
					kind: 'provision' as const,
					operationId: terminalPresentationOperationId(`operation-${sequence}`),
				},
				launchCwd: `/workspace/${sequence}`,
				presentationId: terminalPresentationId(`presentation-${sequence}`),
				title,
			};
		}),
	);
}

function retainedDocument(revision: string): WorkbenchSharedDocument {
	return documentWithPresentations(revision, [
		{
			intent: { kind: 'retain' as const },
			launchCwd: '/retained',
			presentationId: terminalPresentationId('presentation-retained'),
			title: 'Retained',
		},
	]);
}

function documentWithPresentations(
	revision: string,
	presentations: ReadonlyArray<{
		readonly intent:
			| { readonly kind: 'retain' }
			| {
					readonly kind: 'provision';
					readonly operationId: ReturnType<typeof terminalPresentationOperationId>;
			  };
		readonly launchCwd: string;
		readonly presentationId: ReturnType<typeof terminalPresentationId>;
		readonly title: string;
	}>,
): WorkbenchSharedDocument {
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
				nextTabSequence: presentations.length + 1,
				panelWorkspace: {
					nextPanelSequence: presentations.length + 1,
					nextPanelSlotSequence: 1,
					nextSplitSequence: 1,
					nextTerminalSequence: presentations.length + 1,
					pendingRemovals: [],
					tabs: presentations.map((presentation, index) => ({
						root: {
							childPresentations: [],
							id: `panel-${index + 1}`,
							kind: 'panel' as const,
							primaryPresentation: presentation,
						},
						tabId: `tab-${index + 1}`,
					})),
				},
				tabCollection: {
					groups: [],
					tabs: presentations.map((presentation, index) => ({
						group: null,
						customTitle: null,
						id: `tab-${index + 1}`,
						pinned: false,
						title: presentation.title,
					})),
				},
				terminalContextId: 'terminal-context-1',
			},
		],
	});
}
