// @vitest-environment jsdom

import { GatewayBrowserConnectionSchema } from '@orchbee/gateway-protocol';
import {
	WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
	WorkbenchClientSessionBootstrapSchema,
} from '@orchbee/workbench-client-protocol';
import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import type { Root } from 'react-dom/client';
import { describe, expect, it, vi } from 'vitest';

import type { WebApplicationRouting } from '../../../application-routing';
import type { TerminalProjectionClient, WorkbenchClientRuntime } from '../../../workbench-client';
import type { GatewayControlConnection } from '../../connection';
import type { GatewayTerminalProjectionClient } from '../../terminal-projection';
import {
	startGatewayWebApplication,
	type GatewayWebApplicationDependencies,
} from '../startGatewayWebApplication';

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
const bootstrap = WorkbenchClientSessionBootstrapSchema.parse({
	clientSessionId: 'client-session-1',
	defaultCwd: '/workspace',
	documentState: { document: documentState, kind: 'available' },
	initialPresentationId: 'presentation-initial',
	initializationDocument: documentState,
	sequence: '0',
	terminalEvents: [],
	type: WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
});

describe('startGatewayWebApplication', () => {
	it('composes the common Workbench runtime with Gateway transports and Browser effects', async () => {
		const close = vi.fn<() => void>();
		const control = {
			bootstrap,
			close,
			sendClientSession: vi.fn<GatewayControlConnection['sendClientSession']>(),
			sendProjection: vi.fn<GatewayControlConnection['sendProjection']>(),
			subscribeClientSession: vi.fn<GatewayControlConnection['subscribeClientSession']>(
				() => () => undefined,
			),
			subscribeProjection: vi.fn<GatewayControlConnection['subscribeProjection']>(
				() => () => undefined,
			),
		} satisfies GatewayControlConnection;
		const runtimeDispose = vi.fn<() => void>();
		const runtime = {
			actionRuntime: {},
			dispose: runtimeDispose,
			documentCoordinator: {},
			locale: 'en-US',
			projectWorkspaceRuntime: {},
			ready: Promise.resolve({ kind: 'ready' as const }),
			terminalSurfaceCapability: {},
		} as unknown as WorkbenchClientRuntime;
		const createRuntime = vi.fn<GatewayWebApplicationDependencies['createRuntime']>(() => runtime);
		const routingDispose = vi.fn<() => void>();
		const createRouting = vi.fn<GatewayWebApplicationDependencies['createRouting']>(
			() =>
				({
					dispose: routingDispose,
					router: {},
				}) as unknown as WebApplicationRouting,
		);
		const render = vi.fn<Root['render']>();
		const unmount = vi.fn<Root['unmount']>();
		const connection = GatewayBrowserConnectionSchema.parse({
			capability: 'a'.repeat(43),
			port: 42_000,
			protocolVersion: '1.0',
		});
		const rootElement = document.createElement('div');
		const session = startGatewayWebApplication(
			window,
			rootElement,
			document.documentElement,
			['en-US'],
			'apple',
			window.localStorage,
			connection,
			{
				connectControl: async () => control,
				createProjectionClient: () =>
					({
						dispose: () => undefined,
						open: vi.fn<TerminalProjectionClient['open']>(),
					}) as unknown as GatewayTerminalProjectionClient,
				createReactRoot: () => ({ render, unmount }) as unknown as Root,
				createRouting,
				createRuntime,
			},
		);

		expect(await session.ready).toEqual({ kind: 'ready' });
		expect(createRuntime).toHaveBeenCalledWith(
			expect.objectContaining({ terminalPlatformEffects: expect.any(Object) }),
		);
		expect(createRouting).toHaveBeenCalledOnce();
		expect(render).toHaveBeenCalledOnce();
		session.dispose();
		expect(routingDispose).toHaveBeenCalledOnce();
		expect(runtimeDispose).toHaveBeenCalledOnce();
		expect(close).toHaveBeenCalled();
		expect(unmount).toHaveBeenCalledOnce();
	});
});
