import { DESKTOP_SERVER_CONNECTION_IPC_CHANNEL } from '@orchbee/desktop-client-protocol';
import type { IpcRendererEvent } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	type DesktopServerConnectionIpc,
	type DesktopServerConnectionWindow,
	installDesktopServerConnectionBridge,
} from '../installDesktopServerConnectionBridge';

function deliver(
	security: { readonly contextIsolated: boolean; readonly sandboxed: boolean },
	ports: MessagePort[],
): DesktopServerConnectionWindow {
	let listener: ((event: IpcRendererEvent) => void) | undefined;
	const ipc: DesktopServerConnectionIpc = {
		once: vi.fn<DesktopServerConnectionIpc['once']>((channel, nextListener) => {
			expect(channel).toBe(DESKTOP_SERVER_CONNECTION_IPC_CHANNEL);
			listener = nextListener;
		}),
	};
	const targetWindow: DesktopServerConnectionWindow = {
		location: { origin: 'orchbee://app' },
		postMessage: vi.fn<DesktopServerConnectionWindow['postMessage']>(),
	};
	installDesktopServerConnectionBridge(ipc, targetWindow, security);
	listener?.({ ports } as IpcRendererEvent);
	return targetWindow;
}

describe('installDesktopServerConnectionBridge', () => {
	it('hands the page its one connection port only from an isolated sandbox', () => {
		const { port1 } = new MessageChannel();
		const targetWindow = deliver({ contextIsolated: true, sandboxed: true }, [port1]);
		expect(targetWindow.postMessage).toHaveBeenCalledWith(
			expect.objectContaining({ security: { contextIsolated: true, sandboxed: true } }),
			'orchbee://app',
			[port1],
		);
		port1.close();
	});

	it.each([
		{ contextIsolated: false, sandboxed: true },
		{ contextIsolated: true, sandboxed: false },
	])('withholds and closes the port without the required isolation: %o', (security) => {
		const { port1 } = new MessageChannel();
		const close = vi.spyOn(port1, 'close');
		const targetWindow = deliver(security, [port1]);
		expect(targetWindow.postMessage).not.toHaveBeenCalled();
		expect(close).toHaveBeenCalledOnce();
	});

	it('closes every port when Main sends anything but exactly one', () => {
		const ports = [new MessageChannel().port1, new MessageChannel().port1];
		const closes = ports.map((port) => vi.spyOn(port, 'close'));
		const targetWindow = deliver({ contextIsolated: true, sandboxed: true }, ports);
		expect(targetWindow.postMessage).not.toHaveBeenCalled();
		for (const close of closes) expect(close).toHaveBeenCalledOnce();
	});
});
