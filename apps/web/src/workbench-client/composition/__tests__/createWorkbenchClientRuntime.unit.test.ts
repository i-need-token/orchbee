import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import { terminalPresentationId } from '@orchbee/workbench';
import {
	WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
	WorkbenchClientSessionBootstrapSchema,
} from '@orchbee/workbench-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import {
	WorkbenchClientSessionClient,
	type WorkbenchClientSessionTransport,
} from '../../client-session';
import {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
	type TerminalPresentationConfigurationSource,
} from '../../projected-terminal-renderer';
import type { TerminalProjectionClient } from '../../terminal-projection';
import { createWorkbenchClientRuntime } from '../createWorkbenchClientRuntime';

const initialPresentationId = terminalPresentationId('presentation-initial');

const document = createWorkbenchSharedDocument({
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
			nextTabSequence: 2,
			panelWorkspace: {
				nextPanelSequence: 2,
				nextPanelSlotSequence: 1,
				nextSplitSequence: 1,
				nextTerminalSequence: 2,
				pendingRemovals: [],
				tabs: [
					{
						root: {
							childPresentations: [],
							id: 'panel-1',
							kind: 'panel',
							primaryPresentation: {
								intent: { kind: 'retain' },
								launchCwd: '/workspace',
								presentationId: initialPresentationId,
								title: 'Terminal 1',
							},
						},
						tabId: 'terminal-context-1-tab-1',
					},
				],
			},
			tabCollection: {
				groups: [],
				tabs: [
					{
						customTitle: null,
						group: null,
						id: 'terminal-context-1-tab-1',
						pinned: false,
						title: 'Terminal 1',
					},
				],
			},
			terminalContextId: 'terminal-context-1',
		},
	],
});

describe('createWorkbenchClientRuntime', () => {
	it('composes a transport-neutral page runtime without eagerly opening a projection', async () => {
		const closeSessionTransport = vi.fn<() => void>();
		const unsubscribeSessionTransport = vi.fn<() => void>();
		const sessionTransport: WorkbenchClientSessionTransport = {
			close: closeSessionTransport,
			send: () => {
				throw new Error('An available bootstrap must not propose an initialization document.');
			},
			subscribe: () => unsubscribeSessionTransport,
		};
		const session = new WorkbenchClientSessionClient(
			sessionTransport,
			WorkbenchClientSessionBootstrapSchema.parse({
				clientSessionId: 'client-1',
				defaultCwd: '/workspace',
				documentState: { document, kind: 'available' },
				initialPresentationId,
				initializationDocument: document,
				sequence: '0',
				terminalHostName: 'orchbee-host',
				terminalEvents: [],
				type: WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
			}),
		);
		const disposeProjectionClient = vi.fn<() => void>();
		const openProjection = vi.fn<TerminalProjectionClient['open']>(() => {
			throw new Error('Runtime composition must not project an invisible terminal.');
		});
		const projectionClient: TerminalProjectionClient = {
			dispose: disposeProjectionClient,
			open: openProjection,
		};
		const documentElement = { dir: '', lang: '' };
		const presentationConfigurationSource: TerminalPresentationConfigurationSource = {
			current: DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE.current,
			subscribe: () => () => undefined,
		};

		const runtime = createWorkbenchClientRuntime({
			clientSession: session,
			documentElement,
			preferredLocales: ['zh-CN'],
			projectionClient,
			shortcutPlatform: 'apple',
			terminalPresentationConfigurationSource: presentationConfigurationSource,
		});

		expect(await runtime.ready).toEqual({ kind: 'ready' });
		expect(runtime.locale).toBe('zh-CN');
		expect(documentElement).toEqual({ dir: 'ltr', lang: 'zh-CN' });
		expect(openProjection).not.toHaveBeenCalled();
		expect(runtime.terminalSurfaceCapability.presentationConfigurationSource).toBe(
			presentationConfigurationSource,
		);
		expect(runtime.terminalSurfaceCapability.observeTerminalMetadata).toBeTypeOf('function');
		runtime.terminalSurfaceCapability.observeTerminalMetadata(initialPresentationId, {
			currentDirectory: 'file://ORCHBEE-HOST/workspace/My%20Project',
			title: 'vim\0',
		});
		const terminalContext = runtime.projectWorkspaceRuntime.getActiveTerminalContext();
		expect(terminalContext.tabModel.getSnapshot().tabs[0]?.title).toBe('vim');
		expect(terminalContext.panelWorkspaceStore.getSnapshot().tabs[0]?.root).toMatchObject({
			primaryPresentation: {
				currentCwd: '/workspace/My Project',
				launchCwd: '/workspace',
				naturalTitle: 'vim',
				title: 'Terminal 1',
			},
		});

		runtime.dispose();
		expect(unsubscribeSessionTransport).toHaveBeenCalledOnce();
		expect(closeSessionTransport).toHaveBeenCalledOnce();
		expect(disposeProjectionClient).toHaveBeenCalledOnce();
	});
});
