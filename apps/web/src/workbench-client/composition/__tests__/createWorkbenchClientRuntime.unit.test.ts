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

import { ApplicationLocaleController } from '../../../application-locale';
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
		let publishSession: ((value: unknown) => void) | undefined;
		const sessionTransport: WorkbenchClientSessionTransport = {
			close: closeSessionTransport,
			send: () => {
				throw new Error('An available bootstrap must not propose an initialization document.');
			},
			subscribe: (listener) => {
				publishSession = listener;
				return unsubscribeSessionTransport;
			},
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
		const applicationLocaleSource = new ApplicationLocaleController({
			documentElement,
			preferredLocales: ['zh-CN'],
			storage: { getItem: () => null, setItem: () => undefined },
		});

		const runtime = createWorkbenchClientRuntime({
			applicationLocaleSource,
			clientSession: session,
			documentElement,
			preferredLocales: ['zh-CN'],
			projectionClient,
			shortcutPlatform: 'apple',
			terminalPresentationConfigurationSource: presentationConfigurationSource,
		});

		expect(await runtime.ready).toEqual({ kind: 'ready' });
		expect(runtime.applicationLocaleSource.current()).toBe('zh-CN');
		expect(documentElement).toEqual({ dir: 'ltr', lang: 'zh-CN' });
		expect(openProjection).not.toHaveBeenCalled();
		expect(runtime.terminalSurfaceCapability.presentationConfigurationSource).toBe(
			presentationConfigurationSource,
		);
		expect(runtime.terminalSurfaceCapability.platformEffects).toEqual({});
		expect(runtime.terminalSurfaceCapability.observeTerminalMetadata).toBeTypeOf('function');
		const applyTerminalNaturalMetadataObservation = vi.spyOn(
			runtime.projectWorkspaceRuntime,
			'applyTerminalNaturalMetadataObservation',
		);
		const firstTerminalIdentity = {
			creationId: '11'.repeat(16),
			hostEpoch: '22'.repeat(16),
			terminalId: '33'.repeat(16),
		};
		runtime.terminalSurfaceCapability.observeTerminalMetadata(initialPresentationId, {
			closeActivity: 'idle',
			currentDirectory: 'file://ORCHBEE-HOST/workspace/My%20Project',
			revision: 1n,
			title: 'vim\0',
		});
		const terminalContext = runtime.projectWorkspaceRuntime.getActiveTerminalContext();
		expect(terminalContext.tabModel.getSnapshot().tabs[0]?.title).toBe('vim');
		expect(terminalContext.panelWorkspaceStore.getSnapshot().tabs[0]?.root).toMatchObject({
			primaryPresentation: {
				closeActivity: 'idle',
				currentCwd: '/workspace/My Project',
				launchCwd: '/workspace',
				naturalTitle: 'vim',
				title: 'Terminal 1',
			},
		});
		publishSession?.({
			metadata: {
				currentDirectory: 'file://orchbee-host/workspace/conflict',
				identity: firstTerminalIdentity,
				presentationId: initialPresentationId,
				revision: '1',
				title: 'conflicting surface-first revision',
			},
			sequence: '1',
			type: 'terminal-metadata-published',
		});
		expect(applyTerminalNaturalMetadataObservation).not.toHaveBeenCalled();
		expect(terminalContext.tabModel.getSnapshot().tabs[0]?.title).toBe('vim');
		expect(terminalContext.panelWorkspaceStore.getSnapshot().tabs[0]?.root).toMatchObject({
			primaryPresentation: {
				closeActivity: 'idle',
				currentCwd: '/workspace/My Project',
				naturalTitle: 'vim',
			},
		});
		publishSession?.({
			metadata: {
				currentDirectory: 'file://ORCHBEE-HOST/workspace/My%20Project',
				identity: firstTerminalIdentity,
				presentationId: initialPresentationId,
				revision: '1',
				title: 'vim\0',
			},
			sequence: '2',
			type: 'terminal-metadata-published',
		});
		expect(applyTerminalNaturalMetadataObservation).not.toHaveBeenCalled();
		publishSession?.({
			metadata: {
				currentDirectory: 'file://orchbee-host/workspace/background-agent',
				identity: firstTerminalIdentity,
				presentationId: initialPresentationId,
				revision: '2',
				title: 'background agent',
			},
			sequence: '3',
			type: 'terminal-metadata-published',
		});
		expect(applyTerminalNaturalMetadataObservation).toHaveBeenCalledOnce();
		expect(terminalContext.tabModel.getSnapshot().tabs[0]?.title).toBe('background agent');
		expect(terminalContext.panelWorkspaceStore.getSnapshot().tabs[0]?.root).toMatchObject({
			primaryPresentation: {
				closeActivity: 'idle',
				currentCwd: '/workspace/background-agent',
				naturalTitle: 'background agent',
			},
		});
		publishSession?.({
			metadata: {
				currentDirectory: 'file://orchbee-host/workspace/background-agent',
				identity: firstTerminalIdentity,
				presentationId: initialPresentationId,
				revision: '2',
				title: 'background agent',
			},
			sequence: '4',
			type: 'terminal-metadata-published',
		});
		expect(applyTerminalNaturalMetadataObservation).toHaveBeenCalledOnce();
		publishSession?.({
			metadata: {
				currentDirectory: 'file://orchbee-host/workspace/conflict',
				identity: firstTerminalIdentity,
				presentationId: initialPresentationId,
				revision: '2',
				title: 'conflicting equal revision',
			},
			sequence: '5',
			type: 'terminal-metadata-published',
		});
		expect(applyTerminalNaturalMetadataObservation).toHaveBeenCalledOnce();
		expect(terminalContext.tabModel.getSnapshot().tabs[0]?.title).toBe('background agent');
		expect(terminalContext.panelWorkspaceStore.getSnapshot().tabs[0]?.root).toMatchObject({
			primaryPresentation: {
				closeActivity: 'idle',
				currentCwd: '/workspace/background-agent',
				naturalTitle: 'background agent',
			},
		});

		// Surface-owned close activity can change without advancing the Host
		// metadata revision, so identityless equal-revision observations stay live.
		runtime.terminalSurfaceCapability.observeTerminalMetadata(initialPresentationId, {
			closeActivity: 'unknown',
			currentDirectory: 'file://orchbee-host/workspace/background-agent',
			revision: 2n,
			title: 'background agent',
		});
		expect(terminalContext.panelWorkspaceStore.getSnapshot().tabs[0]?.root).toMatchObject({
			primaryPresentation: { closeActivity: 'unknown' },
		});
		runtime.terminalSurfaceCapability.observeTerminalMetadata(initialPresentationId, {
			closeActivity: 'active',
			currentDirectory: 'file://orchbee-host/workspace/surface-conflict',
			revision: 2n,
			title: 'surface conflicting equal revision',
		});
		expect(terminalContext.tabModel.getSnapshot().tabs[0]?.title).toBe('background agent');
		expect(terminalContext.panelWorkspaceStore.getSnapshot().tabs[0]?.root).toMatchObject({
			primaryPresentation: {
				closeActivity: 'unknown',
				currentCwd: '/workspace/background-agent',
				naturalTitle: 'background agent',
			},
		});

		publishSession?.({
			metadata: {
				currentDirectory: 'file://orchbee-host/workspace/replacement',
				identity: {
					creationId: '44'.repeat(16),
					hostEpoch: '55'.repeat(16),
					terminalId: '66'.repeat(16),
				},
				presentationId: initialPresentationId,
				revision: '1',
				title: 'replacement terminal',
			},
			sequence: '6',
			type: 'terminal-metadata-published',
		});
		expect(applyTerminalNaturalMetadataObservation).toHaveBeenCalledTimes(2);
		expect(terminalContext.tabModel.getSnapshot().tabs[0]?.title).toBe('replacement terminal');
		expect(terminalContext.panelWorkspaceStore.getSnapshot().tabs[0]?.root).toMatchObject({
			primaryPresentation: {
				currentCwd: '/workspace/replacement',
				naturalTitle: 'replacement terminal',
			},
		});
		const startRejected = {
			kind: 'rejected' as const,
			reason: 'terminal-creation-sealed',
		};
		vi.spyOn(runtime.projectWorkspaceRuntime, 'startStoppedTerminalSession').mockReturnValue(
			startRejected,
		);
		expect(
			runtime.terminalSurfaceCapability.startStoppedTerminalSession(initialPresentationId),
		).toEqual(startRejected);

		runtime.dispose();
		applicationLocaleSource.dispose();
		expect(unsubscribeSessionTransport).toHaveBeenCalledOnce();
		expect(closeSessionTransport).toHaveBeenCalledOnce();
		expect(disposeProjectionClient).toHaveBeenCalledOnce();
	});
});
