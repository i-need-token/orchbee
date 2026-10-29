// @vitest-environment jsdom

import { act } from 'react';
import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import {
	WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
	DEFAULT_USER_PREFERENCES,
	WorkbenchClientSessionBootstrapSchema,
	WorkbenchClientSessionCommandSchema,
	WorkbenchClientSessionEventSchema,
	type WorkbenchClientSessionBootstrap,
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	terminalPresentationId,
	terminalPresentationOperationId,
	type WorkbenchSharedDocument,
	type TerminalPresentationEvent,
} from '@orchbee/workbench-client-protocol';
import {
	DESKTOP_AGENT_INTEGRATION_IDS,
	DesktopUserPreferencesCommandSchema,
	DesktopUserPreferencesEventSchema,
	DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
	DesktopAgentIntegrationManagementMainEventSchema,
	DesktopAgentIntegrationManagementRendererCommandSchema,
	DesktopAgentObservationMainEventSchema,
	DesktopAgentObservationRendererCommandSchema,
	DesktopProjectFolderAcquisitionMainEventSchema,
	DesktopProjectFolderAcquisitionRendererCommandSchema,
	DesktopTerminalDefaultSettingsMainEventSchema,
	DesktopTerminalDefaultSettingsRendererCommandSchema,
	DesktopTerminalLaunchProfileMainEventSchema,
	DesktopTerminalLaunchProfileRendererCommandSchema,
} from '@orchbee/desktop-client-protocol';
import { createAgentRuntimeSnapshot } from '@orchbee/agent';
import { startDesktopWorkbenchRenderer } from '../startDesktopWorkbenchRenderer';

const mountedGeometry = { cellPixelHeight: 18, cellPixelWidth: 9, columns: 96, rows: 32 } as const;

function createPreferencesChannel(): ReturnType<typeof createTestChannel> {
	const channel = createTestChannel();
	channel.port2.addEventListener('message', (event) => {
		const command = DesktopUserPreferencesCommandSchema.parse(
			(event as MessageEvent<unknown>).data,
		);
		channel.port2.postMessage(
			DesktopUserPreferencesEventSchema.parse({
				type: 'desktop-user-preferences-observed',
				requestId: command.requestId,
				applicationIssue: null,
				response: {
					kind: 'snapshot',
					changed: false,
					snapshot: {
						source: 'missing',
						values: DEFAULT_USER_PREFERENCES,
						claimed: [],
					},
				},
			}),
		);
	});
	return channel;
}

const reactCleanup = vi.hoisted(() => ({ afterUnmount: vi.fn<() => void>() }));

vi.mock(import('react-dom/client'), async (importOriginal) => {
	const reactDom = await importOriginal();
	return {
		...reactDom,
		createRoot: (...parameters: Parameters<typeof reactDom.createRoot>) => {
			const root = reactDom.createRoot(...parameters);
			return {
				render: root.render.bind(root),
				unmount: () => {
					root.unmount();
					reactCleanup.afterUnmount();
				},
			};
		},
	};
});

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
	reactCleanup.afterUnmount.mockReset();
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

function serveTerminalDefaultSettings(port: TestMessagePort): void {
	port.addEventListener('message', (event) => {
		const command = DesktopTerminalDefaultSettingsRendererCommandSchema.parse(
			(event as MessageEvent).data,
		);
		const configuration =
			command.type === 'desktop-terminal-default-settings-update-requested'
				? command.configuration
				: { scrollbackBytes: 10_000_000 };
		port.postMessage(
			DesktopTerminalDefaultSettingsMainEventSchema.parse({
				requestId: command.requestId,
				result: {
					kind: 'completed',
					snapshot: { configuration, persistenceIssue: null, source: 'stored' },
				},
				type: 'desktop-terminal-default-settings-responded',
			}),
		);
	});
}

function serveAgentObservation(port: TestMessagePort): void {
	port.addEventListener('message', (event) => {
		const command = DesktopAgentObservationRendererCommandSchema.parse(
			(event as MessageEvent).data,
		);
		port.postMessage(
			DesktopAgentObservationMainEventSchema.parse({
				requestId: command.requestId,
				snapshot: createAgentRuntimeSnapshot(),
				type: 'desktop-agent-observation-snapshot-published',
				version: DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
			}),
		);
	});
}

function serveAgentIntegrationManagement(port: TestMessagePort): void {
	port.addEventListener('message', (event) => {
		const command = DesktopAgentIntegrationManagementRendererCommandSchema.parse(
			(event as MessageEvent).data,
		);
		port.postMessage(
			DesktopAgentIntegrationManagementMainEventSchema.parse({
				requestId: command.requestId,
				result: {
					kind: 'completed',
					snapshot: {
						integrations: DESKTOP_AGENT_INTEGRATION_IDS.map((integrationId) => ({
							availability: 'not-found',
							integrationId,
							issue: null,
							managementMode: 'managed-local',
							state: 'unknown',
						})),
						launchedIntegrationIds: [],
						revision: 0,
					},
				},
				type: 'desktop-agent-integration-responded',
			}),
		);
	});
}

describe('startDesktopWorkbenchRenderer', () => {
	it('fails closed without rendering when the initial shared-document commit is unavailable', async () => {
		const lifetimeChannel = createTestChannel();
		const clientSessionChannel = createTestChannel();
		const projectionChannel = createTestChannel();
		const agentObservationChannel = createTestChannel();
		const agentIntegrationManagementChannel = createTestChannel();
		const projectFolderAcquisitionChannel = createTestChannel();
		const projectWorktreeChannel = createTestChannel();
		const terminalLaunchProfileChannel = createTestChannel();
		const terminalDefaultSettingsChannel = createTestChannel();
		const clipboardReadChannel = createTestChannel();
		const userPreferencesChannel = createPreferencesChannel();
		serveTerminalLaunchProfiles(terminalLaunchProfileChannel.port2);
		serveTerminalDefaultSettings(terminalDefaultSettingsChannel.port2);
		serveAgentObservation(agentObservationChannel.port2);
		serveAgentIntegrationManagement(agentIntegrationManagementChannel.port2);
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
				agentObservationChannel.port1,
				agentIntegrationManagementChannel.port1,
				projectFolderAcquisitionChannel.port1,
				projectWorktreeChannel.port1,
				terminalLaunchProfileChannel.port1,
				terminalDefaultSettingsChannel.port1,
				clipboardReadChannel.port1,
				userPreferencesChannel.port1,
			);
			await expect(session.ready).resolves.toEqual({ kind: 'unavailable', reason: 'read-failed' });
		});

		expect(commands).toHaveLength(1);
		expect(root.childElementCount).toBe(0);
		expect((clientSessionChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		expect((lifetimeChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		expect((projectionChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		expect((agentObservationChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		expect((agentIntegrationManagementChannel.port1 as unknown as TestMessagePort).closed).toBe(
			true,
		);
		expect((projectFolderAcquisitionChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		expect((projectWorktreeChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		expect((terminalLaunchProfileChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		expect((terminalDefaultSettingsChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		expect((clipboardReadChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		expect((userPreferencesChannel.port1 as unknown as TestMessagePort).closed).toBe(true);
		root.remove();
	});

	it('projects visible Tabs and coordinates the application lifetime', async () => {
		const lifetimeChannel = createTestChannel();
		const clientSessionChannel = createTestChannel();
		const projectionChannel = createTestChannel();
		const agentObservationChannel = createTestChannel();
		const agentIntegrationManagementChannel = createTestChannel();
		const projectFolderAcquisitionChannel = createTestChannel();
		const projectWorktreeChannel = createTestChannel();
		const terminalLaunchProfileChannel = createTestChannel();
		const terminalDefaultSettingsChannel = createTestChannel();
		const clipboardReadChannel = createTestChannel();
		const userPreferencesChannel = createPreferencesChannel();
		serveTerminalLaunchProfiles(terminalLaunchProfileChannel.port2);
		serveTerminalDefaultSettings(terminalDefaultSettingsChannel.port2);
		serveAgentObservation(agentObservationChannel.port2);
		serveAgentIntegrationManagement(agentIntegrationManagementChannel.port2);
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
				agentObservationChannel.port1,
				agentIntegrationManagementChannel.port1,
				projectFolderAcquisitionChannel.port1,
				projectWorktreeChannel.port1,
				terminalLaunchProfileChannel.port1,
				terminalDefaultSettingsChannel.port1,
				clipboardReadChannel.port1,
				userPreferencesChannel.port1,
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

		const secondTab = root.querySelector<HTMLElement>('[role="tab"][data-tab-id="tab-2"]');
		expect(secondTab?.getAttribute('aria-label')).toBe('2');
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
		expect(
			[...root.querySelectorAll<HTMLButtonElement>('button')].find(
				(button) => button.textContent?.trim() === 'SSH Connections',
			),
		).toBeUndefined();
		await act(async () => {
			root
				.querySelector<HTMLElement>('[data-slot="terminal-settings-surface"]')
				?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }));
		});
		await vi.waitFor(() => {
			expect(root.querySelector('[data-slot="terminal-settings-surface"]')).toBeNull();
		});

		const createProject = root.querySelector<HTMLButtonElement>(
			'[data-slot="workbench-sidebar-project-create"] button[aria-label="Create Project"]',
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
		expect(
			[...document.querySelectorAll<HTMLDivElement>('[role="menuitem"]')].find(
				(item) => item.textContent?.trim() === 'Connect via SSH…',
			),
		).toBeUndefined();
		await act(async () => selectFolder?.click());
		await vi.waitFor(() =>
			expect(sessionCommands.at(-1)?.document.projectWorkspace.resourceScopes).toEqual(
				expect.arrayContaining([
					expect.objectContaining({
						directories: [expect.objectContaining({ path: '/workspace/native-project' })],
					}),
				]),
			),
		);

		await act(async () => session?.dispose());
		root.remove();
	}, 15_000);

	it.each([false, true])(
		'hydrates retained terminals and retires capabilities with React cleanup failure: %s',
		async (failCleanup) => {
			const lifetimeChannel = createTestChannel();
			const clientSessionChannel = createTestChannel();
			const projectionChannel = createTestChannel();
			const agentObservationChannel = createTestChannel();
			const agentIntegrationManagementChannel = createTestChannel();
			const projectFolderAcquisitionChannel = createTestChannel();
			const projectWorktreeChannel = createTestChannel();
			const terminalLaunchProfileChannel = createTestChannel();
			const terminalDefaultSettingsChannel = createTestChannel();
			const clipboardReadChannel = createTestChannel();
			const userPreferencesChannel = createPreferencesChannel();
			serveTerminalLaunchProfiles(terminalLaunchProfileChannel.port2);
			serveTerminalDefaultSettings(terminalDefaultSettingsChannel.port2);
			serveAgentObservation(agentObservationChannel.port2);
			serveAgentIntegrationManagement(agentIntegrationManagementChannel.port2);
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
					bootstrap(
						{ document: sharedDocument, kind: 'available' },
						sharedDocument,
						terminalEvents,
					),
					lifetimeChannel.port1,
					clientSessionChannel.port1,
					projectionChannel.port1,
					agentObservationChannel.port1,
					agentIntegrationManagementChannel.port1,
					projectFolderAcquisitionChannel.port1,
					projectWorktreeChannel.port1,
					terminalLaunchProfileChannel.port1,
					terminalDefaultSettingsChannel.port1,
					clipboardReadChannel.port1,
					userPreferencesChannel.port1,
				);
				session = started;
				await expect(started.ready).resolves.toEqual({ kind: 'ready' });
			});

			expect(
				[...root.querySelectorAll<HTMLElement>('[role="tab"]')].map((tab) =>
					tab.getAttribute('aria-label'),
				),
			).toEqual(expect.arrayContaining(['retained']));
			expect(sessionCommands).toEqual([]);

			try {
				const failure = new Error('Renderer cleanup failed.');
				if (failCleanup) {
					reactCleanup.afterUnmount.mockImplementationOnce(() => {
						throw failure;
					});
				}
				const failures: unknown[] = [];
				await act(async () => {
					try {
						session?.dispose();
					} catch (error) {
						failures.push(error);
					}
				});
				expect(failures).toEqual(failCleanup ? [failure] : []);
				for (const channel of [
					lifetimeChannel,
					clientSessionChannel,
					projectionChannel,
					agentObservationChannel,
					agentIntegrationManagementChannel,
					projectFolderAcquisitionChannel,
					projectWorktreeChannel,
					terminalLaunchProfileChannel,
					terminalDefaultSettingsChannel,
					clipboardReadChannel,
					userPreferencesChannel,
				]) {
					expect((channel.port1 as unknown as TestMessagePort).closed).toBe(true);
				}
				await act(async () => session?.dispose());
				expect(reactCleanup.afterUnmount).toHaveBeenCalledOnce();
			} finally {
				await act(async () => session?.dispose());
				root.remove();
			}
		},
	);
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
			nextResourceScopeSequence: 1,
			resourceScopes: [],
			workspaces: [],
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
