import type { IpcRendererEvent } from 'electron';
import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import { describe, expect, it, vi } from 'vitest';

import { DESKTOP_CLIENT_SESSION_IPC_CHANNEL } from '../../shared';
import {
	installDesktopClientSessionBridge,
	type DesktopClientSessionIpc,
	type DesktopClientSessionWindow,
} from '../installDesktopClientSessionBridge';

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

describe('installDesktopClientSessionBridge', () => {
	it('transfers one validated session port with its atomic bootstrap', () => {
		let listener: ((event: IpcRendererEvent, value: unknown) => void) | undefined;
		const ipc: DesktopClientSessionIpc = {
			once: vi.fn<DesktopClientSessionIpc['once']>((channel, nextListener) => {
				expect(channel).toBe(DESKTOP_CLIENT_SESSION_IPC_CHANNEL);
				listener = nextListener;
			}),
		};
		const targetWindow: DesktopClientSessionWindow = {
			location: { origin: 'https://desktop.invalid' },
			postMessage: vi.fn<DesktopClientSessionWindow['postMessage']>(),
		};
		const port = new MessageChannel().port1;
		installDesktopClientSessionBridge(ipc, targetWindow, {
			contextIsolated: true,
			sandboxed: true,
		});

		listener?.({ ports: [port] } as IpcRendererEvent, {
			clientSessionId: 'client-1',
			defaultCwd: '/workspace',
			documentState: { document, kind: 'available' },
			initialPresentationId: 'presentation-initial',
			initializationDocument: document,
			sequence: '0',
			terminalEvents: [],
			type: DESKTOP_CLIENT_SESSION_IPC_CHANNEL,
		});

		const call = vi.mocked(targetWindow.postMessage).mock.calls[0];
		expect(call?.[0]).toMatchObject({
			bootstrap: { clientSessionId: 'client-1' },
			security: { contextIsolated: true, sandboxed: true },
			type: 'orchbee.desktop.client-session-ready',
		});
		expect(call?.[1]).toBe('https://desktop.invalid');
		expect(call?.[2]?.[0]).toBe(port);
	});

	it('drops malformed bootstrap data and ambiguous port sets', () => {
		let listener: ((event: IpcRendererEvent, value: unknown) => void) | undefined;
		const targetWindow: DesktopClientSessionWindow = {
			location: { origin: 'null' },
			postMessage: vi.fn<DesktopClientSessionWindow['postMessage']>(),
		};
		installDesktopClientSessionBridge(
			{ once: (_channel, nextListener) => (listener = nextListener) },
			targetWindow,
			{ contextIsolated: true, sandboxed: true },
		);

		listener?.({ ports: [] } as unknown as IpcRendererEvent, { type: 'generic-ipc' });

		expect(targetWindow.postMessage).not.toHaveBeenCalled();
	});
});
