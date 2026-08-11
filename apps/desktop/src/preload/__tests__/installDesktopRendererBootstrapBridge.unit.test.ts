import { DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL } from '@orchbee/desktop-client-protocol';
import { WORKBENCH_CLIENT_SESSION_BOOTSTRAP } from '@orchbee/workbench-client-protocol';
import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import type { IpcRendererEvent } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	installDesktopRendererBootstrapBridge,
	type DesktopRendererBootstrapIpc,
	type DesktopRendererBootstrapWindow,
} from '../installDesktopRendererBootstrapBridge';

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

describe('installDesktopRendererBootstrapBridge', () => {
	it('transfers one validated bootstrap with four ports in their fixed role order', () => {
		let listener: ((event: IpcRendererEvent, value: unknown) => void) | undefined;
		const ipc: DesktopRendererBootstrapIpc = {
			once: vi.fn<DesktopRendererBootstrapIpc['once']>((channel, nextListener) => {
				expect(channel).toBe(DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL);
				listener = nextListener;
			}),
		};
		const targetWindow: DesktopRendererBootstrapWindow = {
			location: { origin: 'https://desktop.invalid' },
			postMessage: vi.fn<DesktopRendererBootstrapWindow['postMessage']>(),
		};
		const ports = [
			new MessageChannel().port1,
			new MessageChannel().port1,
			new MessageChannel().port1,
			new MessageChannel().port1,
		];
		installDesktopRendererBootstrapBridge(ipc, targetWindow, {
			contextIsolated: true,
			sandboxed: true,
		});

		listener?.({ ports } as IpcRendererEvent, rendererBootstrap());

		const call = vi.mocked(targetWindow.postMessage).mock.calls[0];
		expect(call?.[0]).toMatchObject({
			bootstrap: { clientSession: { clientSessionId: 'client-1' } },
			security: { contextIsolated: true, sandboxed: true },
			type: 'orchbee.desktop.renderer-bootstrap-ready',
		});
		expect(call?.[1]).toBe('https://desktop.invalid');
		expect(call?.[2]).toEqual(ports);
	});

	it('closes every transferred port for malformed or partial bootstrap data', () => {
		let listener: ((event: IpcRendererEvent, value: unknown) => void) | undefined;
		const targetWindow: DesktopRendererBootstrapWindow = {
			location: { origin: 'null' },
			postMessage: vi.fn<DesktopRendererBootstrapWindow['postMessage']>(),
		};
		const ports = [new MessageChannel().port1, new MessageChannel().port1];
		const closeSpies = ports.map((port) => vi.spyOn(port, 'close'));
		installDesktopRendererBootstrapBridge(
			{ once: (_channel, nextListener) => (listener = nextListener) },
			targetWindow,
			{ contextIsolated: true, sandboxed: true },
		);

		listener?.({ ports } as IpcRendererEvent, { type: 'generic-ipc' });

		expect(targetWindow.postMessage).not.toHaveBeenCalled();
		expect(closeSpies.every((spy) => spy.mock.calls.length === 1)).toBe(true);
	});
});

function rendererBootstrap(): unknown {
	return {
		clientSession: {
			clientSessionId: 'client-1',
			defaultCwd: '/workspace',
			documentState: { document, kind: 'available' },
			initialPresentationId: 'presentation-initial',
			initializationDocument: document,
			sequence: '0',
			terminalEvents: [],
			type: WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
		},
		type: DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL,
	};
}
