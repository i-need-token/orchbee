import type { IpcRendererEvent } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import { TERMINAL_PROJECTION_CONTROL_IPC_CHANNEL } from '../../shared';
import {
	installTerminalProjectionControlBridge,
	type TerminalProjectionControlIpc,
	type TerminalProjectionControlWindow,
} from '../installTerminalProjectionControlBridge';

describe('installTerminalProjectionControlBridge', () => {
	it('transfers one validated projection-control port without generic IPC authority', () => {
		let listener: ((event: IpcRendererEvent, value: unknown) => void) | undefined;
		const ipc: TerminalProjectionControlIpc = {
			once: vi.fn<TerminalProjectionControlIpc['once']>((channel, nextListener) => {
				expect(channel).toBe(TERMINAL_PROJECTION_CONTROL_IPC_CHANNEL);
				listener = nextListener;
			}),
		};
		const targetWindow: TerminalProjectionControlWindow = {
			location: { origin: 'null' },
			postMessage: vi.fn<TerminalProjectionControlWindow['postMessage']>(),
		};
		const port = new MessageChannel().port1;
		installTerminalProjectionControlBridge(ipc, targetWindow, {
			contextIsolated: true,
			sandboxed: true,
		});

		listener?.({ ports: [port] } as IpcRendererEvent, {
			type: TERMINAL_PROJECTION_CONTROL_IPC_CHANNEL,
		});

		const call = vi.mocked(targetWindow.postMessage).mock.calls[0];
		expect(call?.[0]).toEqual({
			security: { contextIsolated: true, sandboxed: true },
			type: 'orchbee.desktop.terminal-projection-control-ready',
		});
		expect(call?.[1]).toBe('*');
		expect(call?.[2]?.[0]).toBe(port);
	});

	it('drops malformed bootstrap values and ambiguous port sets', () => {
		let listener: ((event: IpcRendererEvent, value: unknown) => void) | undefined;
		const targetWindow: TerminalProjectionControlWindow = {
			location: { origin: 'https://desktop.invalid' },
			postMessage: vi.fn<TerminalProjectionControlWindow['postMessage']>(),
		};
		installTerminalProjectionControlBridge(
			{ once: (_channel, nextListener) => (listener = nextListener) },
			targetWindow,
			{ contextIsolated: true, sandboxed: true },
		);

		listener?.({ ports: [] } as unknown as IpcRendererEvent, { type: 'generic-ipc' });

		expect(targetWindow.postMessage).not.toHaveBeenCalled();
	});
});
