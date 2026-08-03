import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import {
	WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
	WorkbenchClientSessionBootstrapSchema,
} from '@orchbee/workbench-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import {
	WorkbenchClientSessionClient,
	type WorkbenchClientSessionTransport,
} from '../../client-session';
import type { TerminalProjectionClient } from '../../terminal-projection';
import { createWorkbenchClientRuntime } from '../createWorkbenchClientRuntime';

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
				initialPresentationId: 'presentation-initial',
				initializationDocument: document,
				sequence: '0',
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

		const runtime = createWorkbenchClientRuntime({
			clientSession: session,
			documentElement,
			preferredLocales: ['zh-CN'],
			projectionClient,
			shortcutPlatform: 'apple',
		});

		expect(await runtime.ready).toEqual({ kind: 'ready' });
		expect(runtime.locale).toBe('zh-CN');
		expect(documentElement).toEqual({ dir: 'ltr', lang: 'zh-CN' });
		expect(openProjection).not.toHaveBeenCalled();

		runtime.dispose();
		expect(unsubscribeSessionTransport).toHaveBeenCalledOnce();
		expect(closeSessionTransport).toHaveBeenCalledOnce();
		expect(disposeProjectionClient).toHaveBeenCalledOnce();
	});
});
