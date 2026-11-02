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

/** The connection port and the Project folder picker port Main sends with a reply. */
function documentPorts(count = 2): MessagePort[] {
	return Array.from({ length: count }, () => new MessageChannel().port1);
}

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

function closeAll(ports: readonly MessagePort[]): void {
	for (const port of ports) port.close();
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

	it('hands the page its ports, in Main order, only from an isolated sandbox', () => {
		const { mainAnswers, pageAsks, targetWindow } = install(ISOLATED);
		pageAsks();
		const ports = documentPorts();
		mainAnswers(ports, { bootstrap: BOOTSTRAP, document: DOCUMENT });
		expect(targetWindow.postMessage).toHaveBeenCalledWith(
			expect.objectContaining({ bootstrap: BOOTSTRAP, security: ISOLATED }),
			'orchbee://app',
			ports,
		);
		closeAll(ports);
	});

	it('closes what Main meant for the document this one replaced', () => {
		const { mainAnswers, pageAsks, targetWindow } = install(ISOLATED);
		const early = documentPorts();
		const closeEarly = early.map((port) => vi.spyOn(port, 'close'));
		mainAnswers(early, { bootstrap: BOOTSTRAP, document: OTHER_DOCUMENT });
		for (const close of closeEarly) expect(close).toHaveBeenCalledOnce();

		pageAsks();
		const replaced = documentPorts();
		const closeReplaced = replaced.map((port) => vi.spyOn(port, 'close'));
		mainAnswers(replaced, { bootstrap: BOOTSTRAP, document: OTHER_DOCUMENT });
		for (const close of closeReplaced) expect(close).toHaveBeenCalledOnce();
		expect(targetWindow.postMessage).not.toHaveBeenCalled();

		const own = documentPorts();
		mainAnswers(own, { bootstrap: BOOTSTRAP, document: DOCUMENT });
		expect(targetWindow.postMessage).toHaveBeenCalledOnce();
		closeAll(own);
	});

	it('hands over one set of ports for a document', () => {
		const { mainAnswers, pageAsks, targetWindow } = install(ISOLATED);
		pageAsks();
		const first = documentPorts();
		mainAnswers(first, { bootstrap: BOOTSTRAP, document: DOCUMENT });
		const second = documentPorts();
		const closeSecond = second.map((port) => vi.spyOn(port, 'close'));
		mainAnswers(second, { bootstrap: BOOTSTRAP, document: DOCUMENT });
		expect(targetWindow.postMessage).toHaveBeenCalledOnce();
		for (const close of closeSecond) expect(close).toHaveBeenCalledOnce();
		closeAll(first);
	});

	it.each([
		{ contextIsolated: false, sandboxed: true },
		{ contextIsolated: true, sandboxed: false },
	])('withholds and closes the ports without the required isolation: %o', (security) => {
		const { mainAnswers, pageAsks, targetWindow } = install(security);
		pageAsks();
		const ports = documentPorts();
		const closes = ports.map((port) => vi.spyOn(port, 'close'));
		mainAnswers(ports, { bootstrap: BOOTSTRAP, document: DOCUMENT });
		expect(targetWindow.postMessage).not.toHaveBeenCalled();
		for (const close of closes) expect(close).toHaveBeenCalledOnce();
	});

	it('withholds and closes the ports when Main sends an unreadable reply', () => {
		const { mainAnswers, pageAsks, targetWindow } = install(ISOLATED);
		pageAsks();
		const ports = documentPorts();
		const closes = ports.map((port) => vi.spyOn(port, 'close'));
		mainAnswers(ports, null);
		expect(targetWindow.postMessage).not.toHaveBeenCalled();
		for (const close of closes) expect(close).toHaveBeenCalledOnce();
	});

	it.each([1, 3])('closes every port when Main sends %i instead of one per role', (count) => {
		const { mainAnswers, pageAsks, targetWindow } = install(ISOLATED);
		pageAsks();
		const ports = documentPorts(count);
		const closes = ports.map((port) => vi.spyOn(port, 'close'));
		mainAnswers(ports, { bootstrap: BOOTSTRAP, document: DOCUMENT });
		expect(targetWindow.postMessage).not.toHaveBeenCalled();
		for (const close of closes) expect(close).toHaveBeenCalledOnce();
	});
});
