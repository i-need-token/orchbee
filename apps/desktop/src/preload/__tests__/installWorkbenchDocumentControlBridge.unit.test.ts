import type { IpcRendererEvent } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import { WORKBENCH_DOCUMENT_CONTROL_IPC_CHANNEL } from '../../shared';
import {
	installWorkbenchDocumentControlBridge,
	type WorkbenchDocumentControlIpc,
	type WorkbenchDocumentControlWindow,
} from '../installWorkbenchDocumentControlBridge';

describe('installWorkbenchDocumentControlBridge', () => {
	it('transfers one validated document port without generic IPC authority', () => {
		let listener: ((event: IpcRendererEvent, value: unknown) => void) | undefined;
		const ipc: WorkbenchDocumentControlIpc = {
			once: vi.fn<WorkbenchDocumentControlIpc['once']>((channel, nextListener) => {
				expect(channel).toBe(WORKBENCH_DOCUMENT_CONTROL_IPC_CHANNEL);
				listener = nextListener;
			}),
		};
		const targetWindow: WorkbenchDocumentControlWindow = {
			location: { origin: 'null' },
			postMessage: vi.fn<WorkbenchDocumentControlWindow['postMessage']>(),
		};
		const port = new MessageChannel().port1;
		installWorkbenchDocumentControlBridge(ipc, targetWindow, {
			contextIsolated: true,
			sandboxed: true,
		});

		listener?.({ ports: [port] } as IpcRendererEvent, {
			type: WORKBENCH_DOCUMENT_CONTROL_IPC_CHANNEL,
		});

		const call = vi.mocked(targetWindow.postMessage).mock.calls[0];
		expect(call?.[0]).toEqual({
			security: { contextIsolated: true, sandboxed: true },
			type: 'orchbee.desktop.workbench-document-control-ready',
		});
		expect(call?.[1]).toBe('*');
		expect(call?.[2]?.[0]).toBe(port);
	});

	it('drops malformed bootstrap values and ambiguous port sets', () => {
		let listener: ((event: IpcRendererEvent, value: unknown) => void) | undefined;
		const targetWindow: WorkbenchDocumentControlWindow = {
			location: { origin: 'https://desktop.invalid' },
			postMessage: vi.fn<WorkbenchDocumentControlWindow['postMessage']>(),
		};
		installWorkbenchDocumentControlBridge(
			{ once: (_channel, nextListener) => (listener = nextListener) },
			targetWindow,
			{ contextIsolated: true, sandboxed: true },
		);

		listener?.({ ports: [] } as unknown as IpcRendererEvent, { type: 'generic-ipc' });

		expect(targetWindow.postMessage).not.toHaveBeenCalled();
	});
});
