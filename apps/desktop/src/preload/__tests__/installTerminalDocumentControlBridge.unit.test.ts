import type { IpcRendererEvent } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import { TERMINAL_DOCUMENT_CONTROL_IPC_CHANNEL } from '../../shared';
import {
	installTerminalDocumentControlBridge,
	type TerminalDocumentControlIpc,
	type TerminalDocumentControlWindow,
} from '../installTerminalDocumentControlBridge';

describe('installTerminalDocumentControlBridge', () => {
	it('transfers one validated document port without exposing generic IPC', () => {
		let listener: ((event: IpcRendererEvent, value: unknown) => void) | undefined;
		const ipc: TerminalDocumentControlIpc = {
			once: vi.fn<TerminalDocumentControlIpc['once']>((channel, nextListener) => {
				expect(channel).toBe(TERMINAL_DOCUMENT_CONTROL_IPC_CHANNEL);
				listener = nextListener;
			}),
		};
		const targetWindow: TerminalDocumentControlWindow = {
			location: { origin: 'null' },
			postMessage: vi.fn<TerminalDocumentControlWindow['postMessage']>(),
		};
		const port = new MessageChannel().port1;
		installTerminalDocumentControlBridge(ipc, targetWindow, {
			contextIsolated: true,
			sandboxed: true,
		});

		listener?.({ ports: [port] } as IpcRendererEvent, {
			type: TERMINAL_DOCUMENT_CONTROL_IPC_CHANNEL,
		});

		const call = vi.mocked(targetWindow.postMessage).mock.calls[0];
		expect(call?.[0]).toEqual(
			expect.objectContaining({
				security: { contextIsolated: true, sandboxed: true },
				type: 'orchbee.desktop.terminal-document-control-ready',
			}),
		);
		expect(call?.[1]).toBe('*');
		expect(call?.[2]?.[0]).toBe(port);
	});

	it('drops malformed bootstrap values and ambiguous port sets', () => {
		let listener: ((event: IpcRendererEvent, value: unknown) => void) | undefined;
		const ipc: TerminalDocumentControlIpc = {
			once: (_channel, nextListener) => {
				listener = nextListener;
			},
		};
		const targetWindow: TerminalDocumentControlWindow = {
			location: { origin: 'https://desktop.invalid' },
			postMessage: vi.fn<TerminalDocumentControlWindow['postMessage']>(),
		};
		installTerminalDocumentControlBridge(ipc, targetWindow, {
			contextIsolated: true,
			sandboxed: true,
		});

		listener?.({ ports: [] } as unknown as IpcRendererEvent, { type: 'generic-ipc' });

		expect(targetWindow.postMessage).not.toHaveBeenCalled();
	});
});
