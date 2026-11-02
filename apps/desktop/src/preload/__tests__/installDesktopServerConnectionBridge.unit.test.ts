import {
	DESKTOP_SERVER_CONNECTION_IPC_CHANNEL,
	DESKTOP_SERVER_CONNECTION_REQUEST_IPC_CHANNEL,
	DESKTOP_SERVER_CONNECTION_WINDOW_REQUEST,
} from '@orchbee/desktop-client-protocol';
import type { IpcRendererEvent } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	type DesktopServerConnectionIpc,
	type DesktopServerConnectionWindow,
	installDesktopServerConnectionBridge,
} from '../installDesktopServerConnectionBridge';

const BOOTSTRAP = {
	automaticWorkspaceAllowed: true,
	defaultDirectory: 'C:\\Users\\someone',
	loginShellAvailable: false,
	terminalHostName: 'host',
};
const ISOLATED = { contextIsolated: true, sandboxed: true };
const DOCUMENT = '00000000-0000-4000-8000-000000000001';
const OTHER_DOCUMENT = '00000000-0000-4000-8000-000000000002';

function install(security: { readonly contextIsolated: boolean; readonly sandboxed: boolean }) {
	let receiveReply: ((event: IpcRendererEvent, reply: unknown) => void) | undefined;
	let receiveWindow: ((event: MessageEvent<unknown>) => void) | undefined;
	const ipc = {
		on: vi.fn<DesktopServerConnectionIpc['on']>((channel, listener) => {
			expect(channel).toBe(DESKTOP_SERVER_CONNECTION_IPC_CHANNEL);
			receiveReply = listener;
		}),
		send: vi.fn<DesktopServerConnectionIpc['send']>(),
	};
	const targetWindow = {
		addEventListener: vi.fn<DesktopServerConnectionWindow['addEventListener']>(
			(_type, listener) => {
				receiveWindow = listener;
			},
		),
		location: { origin: 'orchbee://app' },
		postMessage: vi.fn<DesktopServerConnectionWindow['postMessage']>(),
	};
	installDesktopServerConnectionBridge(ipc, targetWindow, security, () => DOCUMENT);
	const pageAsks = (source: unknown = targetWindow): void => {
		receiveWindow?.({
			data: { type: DESKTOP_SERVER_CONNECTION_WINDOW_REQUEST },
			source,
		} as MessageEvent<unknown>);
	};
	const mainAnswers = (ports: MessagePort[], reply: unknown): void => {
		receiveReply?.({ ports } as unknown as IpcRendererEvent, reply);
	};
	return { ipc, mainAnswers, pageAsks, targetWindow };
}

describe('installDesktopServerConnectionBridge', () => {
	it('asks Main once for the page, however often the page asks', () => {
		const { ipc, pageAsks } = install(ISOLATED);
		expect(ipc.send).not.toHaveBeenCalled();
		pageAsks({ another: 'window' });
		expect(ipc.send).not.toHaveBeenCalled();
		pageAsks();
		pageAsks();
		expect(ipc.send.mock.calls).toEqual([
			[DESKTOP_SERVER_CONNECTION_REQUEST_IPC_CHANNEL, DOCUMENT],
		]);
	});

	it('hands the page its one connection port only from an isolated sandbox', () => {
		const { mainAnswers, pageAsks, targetWindow } = install(ISOLATED);
		pageAsks();
		const { port1 } = new MessageChannel();
		mainAnswers([port1], { bootstrap: BOOTSTRAP, document: DOCUMENT });
		expect(targetWindow.postMessage).toHaveBeenCalledWith(
			expect.objectContaining({ bootstrap: BOOTSTRAP, security: ISOLATED }),
			'orchbee://app',
			[port1],
		);
		port1.close();
	});

	it('closes what Main meant for the document this one replaced', () => {
		const { mainAnswers, pageAsks, targetWindow } = install(ISOLATED);
		const early = new MessageChannel().port1;
		const closeEarly = vi.spyOn(early, 'close');
		mainAnswers([early], { bootstrap: BOOTSTRAP, document: OTHER_DOCUMENT });
		expect(closeEarly).toHaveBeenCalledOnce();

		pageAsks();
		const replaced = new MessageChannel().port1;
		const closeReplaced = vi.spyOn(replaced, 'close');
		mainAnswers([replaced], { bootstrap: BOOTSTRAP, document: OTHER_DOCUMENT });
		expect(closeReplaced).toHaveBeenCalledOnce();
		expect(targetWindow.postMessage).not.toHaveBeenCalled();

		const own = new MessageChannel().port1;
		mainAnswers([own], { bootstrap: BOOTSTRAP, document: DOCUMENT });
		expect(targetWindow.postMessage).toHaveBeenCalledOnce();
		own.close();
	});

	it('hands over only one port for a document', () => {
		const { mainAnswers, pageAsks, targetWindow } = install(ISOLATED);
		pageAsks();
		const first = new MessageChannel().port1;
		mainAnswers([first], { bootstrap: BOOTSTRAP, document: DOCUMENT });
		const second = new MessageChannel().port1;
		const closeSecond = vi.spyOn(second, 'close');
		mainAnswers([second], { bootstrap: BOOTSTRAP, document: DOCUMENT });
		expect(targetWindow.postMessage).toHaveBeenCalledOnce();
		expect(closeSecond).toHaveBeenCalledOnce();
		first.close();
	});

	it.each([
		{ contextIsolated: false, sandboxed: true },
		{ contextIsolated: true, sandboxed: false },
	])('withholds and closes the port without the required isolation: %o', (security) => {
		const { mainAnswers, pageAsks, targetWindow } = install(security);
		pageAsks();
		const { port1 } = new MessageChannel();
		const close = vi.spyOn(port1, 'close');
		mainAnswers([port1], { bootstrap: BOOTSTRAP, document: DOCUMENT });
		expect(targetWindow.postMessage).not.toHaveBeenCalled();
		expect(close).toHaveBeenCalledOnce();
	});

	it('withholds and closes the port when Main sends an unreadable reply', () => {
		const { mainAnswers, pageAsks, targetWindow } = install(ISOLATED);
		pageAsks();
		const { port1 } = new MessageChannel();
		const close = vi.spyOn(port1, 'close');
		mainAnswers([port1], null);
		expect(targetWindow.postMessage).not.toHaveBeenCalled();
		expect(close).toHaveBeenCalledOnce();
	});

	it('closes every port when Main sends anything but exactly one', () => {
		const { mainAnswers, pageAsks, targetWindow } = install(ISOLATED);
		pageAsks();
		const ports = [new MessageChannel().port1, new MessageChannel().port1];
		const closes = ports.map((port) => vi.spyOn(port, 'close'));
		mainAnswers(ports, { bootstrap: BOOTSTRAP, document: DOCUMENT });
		expect(targetWindow.postMessage).not.toHaveBeenCalled();
		for (const close of closes) expect(close).toHaveBeenCalledOnce();
	});
});
