import type { IpcRendererEvent } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import { WORKBENCH_TOPOLOGY_CONTROL_IPC_CHANNEL } from '../../shared';
import {
	installWorkbenchTopologyControlBridge,
	type WorkbenchTopologyControlIpc,
	type WorkbenchTopologyControlWindow,
} from '../installWorkbenchTopologyControlBridge';

describe('installWorkbenchTopologyControlBridge', () => {
	it('transfers one validated topology port without generic IPC authority', () => {
		let listener: ((event: IpcRendererEvent, value: unknown) => void) | undefined;
		const ipc: WorkbenchTopologyControlIpc = {
			once: vi.fn<WorkbenchTopologyControlIpc['once']>((channel, nextListener) => {
				expect(channel).toBe(WORKBENCH_TOPOLOGY_CONTROL_IPC_CHANNEL);
				listener = nextListener;
			}),
		};
		const targetWindow: WorkbenchTopologyControlWindow = {
			location: { origin: 'null' },
			postMessage: vi.fn<WorkbenchTopologyControlWindow['postMessage']>(),
		};
		const port = new MessageChannel().port1;
		installWorkbenchTopologyControlBridge(ipc, targetWindow, {
			contextIsolated: true,
			sandboxed: true,
		});

		listener?.({ ports: [port] } as IpcRendererEvent, {
			type: WORKBENCH_TOPOLOGY_CONTROL_IPC_CHANNEL,
		});

		const call = vi.mocked(targetWindow.postMessage).mock.calls[0];
		expect(call?.[0]).toEqual({
			security: { contextIsolated: true, sandboxed: true },
			type: 'orchbee.desktop.workbench-topology-control-ready',
		});
		expect(call?.[1]).toBe('*');
		expect(call?.[2]?.[0]).toBe(port);
	});

	it('drops malformed bootstrap values and ambiguous port sets', () => {
		let listener: ((event: IpcRendererEvent, value: unknown) => void) | undefined;
		const targetWindow: WorkbenchTopologyControlWindow = {
			location: { origin: 'https://desktop.invalid' },
			postMessage: vi.fn<WorkbenchTopologyControlWindow['postMessage']>(),
		};
		installWorkbenchTopologyControlBridge(
			{ once: (_channel, nextListener) => (listener = nextListener) },
			targetWindow,
			{ contextIsolated: true, sandboxed: true },
		);

		listener?.({ ports: [] } as unknown as IpcRendererEvent, { type: 'generic-ipc' });

		expect(targetWindow.postMessage).not.toHaveBeenCalled();
	});
});
