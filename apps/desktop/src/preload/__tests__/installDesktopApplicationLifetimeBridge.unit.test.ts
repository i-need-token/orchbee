import type { IpcRendererEvent } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	installDesktopApplicationLifetimeBridge,
	type DesktopApplicationLifetimeWindow,
} from '../installDesktopApplicationLifetimeBridge';

describe('installDesktopApplicationLifetimeBridge', () => {
	it('forwards one validated port with sandbox security metadata', () => {
		let listener: ((event: IpcRendererEvent, value: unknown) => void) | undefined;
		const postMessage = vi.fn<DesktopApplicationLifetimeWindow['postMessage']>();
		installDesktopApplicationLifetimeBridge(
			{
				once: (_channel, nextListener) => {
					listener = nextListener;
				},
			},
			{ location: { origin: 'null' }, postMessage },
			{ contextIsolated: true, sandboxed: true },
		);
		const port = {} as MessagePort;
		listener?.({ ports: [port] } as unknown as IpcRendererEvent, {
			type: 'orchbee.desktop.application-lifetime-port',
		});

		expect(postMessage).toHaveBeenCalledWith(
			{
				security: { contextIsolated: true, sandboxed: true },
				type: 'orchbee.desktop.application-lifetime-ready',
			},
			'*',
			[port],
		);
	});
});
