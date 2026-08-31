import {
	decodeGatewayControlServerMessage,
	encodeGatewayControlClientMessage,
	GATEWAY_PROTOCOL_VERSION,
	GATEWAY_LIMITS,
	GatewayControlClientMessageSchema,
	GatewayDesktopCommandSchema,
	GatewayUtilityEventSchema,
	type GatewayControlServerMessage,
	type GatewayUtilityEvent,
} from '@orchbee/gateway-protocol';
import {
	BROWSER_SESSION_COOKIE_NAME,
	BrowserSessionIdSchema,
	BrowserSessionSigningKeySchema,
	createBrowserSessionCredential,
} from '@orchbee/remote-client-protocol';
import { createHmac } from 'node:crypto';
import { createServer, type Server } from 'node:http';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { WebSocket } from 'ws';

import {
	WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
	WorkbenchClientSessionBootstrapSchema,
} from '@orchbee/workbench-client-protocol';
import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';

import {
	LocalDevelopmentBrowserGateway,
	type GatewayMessagePort,
	type GatewayParentMessageEvent,
	type GatewayParentPort,
} from '../LocalDevelopmentBrowserGateway';

const allowedOrigin = 'http://127.0.0.1:3000';
const sessionSigningKey = BrowserSessionSigningKeySchema.parse('s'.repeat(43));
const browserSessionCredential = createBrowserSessionCredential(
	BrowserSessionIdSchema.parse('i'.repeat(43)),
	(payload) =>
		createHmac('sha256', Buffer.from(sessionSigningKey, 'base64url'))
			.update(payload, 'utf8')
			.digest('base64url'),
);
const browserSessionCookie = `${BROWSER_SESSION_COOKIE_NAME}=${browserSessionCredential}`;
const workbenchDocument = createWorkbenchSharedDocument({
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
const workbenchBootstrap = WorkbenchClientSessionBootstrapSchema.parse({
	clientSessionId: 'client-session-1',
	defaultCwd: '/workspace',
	documentState: { document: workbenchDocument, kind: 'available' },
	initialPresentationId: 'presentation-initial',
	initializationDocument: workbenchDocument,
	sequence: '0',
	terminalHostName: 'orchbee-host',
	terminalEvents: [],
	type: WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
});
const projectionBootstrap = {
	attachmentId: '11'.repeat(16),
	canInput: true,
	canResize: true,
	capabilityRevision: '1',
	creationId: '22'.repeat(16),
	hostEpoch: '33'.repeat(16),
	terminalId: '44'.repeat(16),
	terminalOutcome: null,
	theme: {
		operationId: null,
		revision: '0',
		terminal: {
			colorScheme: 'dark',
			defaultBackgroundRgb: 0x1e_1e_1e,
			defaultCursorRgb: null,
			defaultForegroundRgb: 0xd4_d4_d4,
			palette: Array.from({ length: 256 }, (_, index) => index),
		},
	},
} as const;
const mountedGeometry = { cellPixelHeight: 18, cellPixelWidth: 9, columns: 96, rows: 32 } as const;

let gateway: LocalDevelopmentBrowserGateway | undefined;

afterEach(async () => {
	await gateway?.dispose();
	gateway = undefined;
	vi.useRealTimers();
	vi.restoreAllMocks();
});

describe('LocalDevelopmentBrowserGateway', () => {
	it('binds loopback, requires an exact Origin and signed session Cookie, then performs the protocol handshake', async () => {
		const parent = new FakeParentPort();
		gateway = new LocalDevelopmentBrowserGateway(parent);
		gateway.start();
		parent.emit(
			GatewayDesktopCommandSchema.parse({
				allowedOrigins: [allowedOrigin],
				bindHost: '127.0.0.1',
				sessionSigningKey,
				type: 'gateway-start',
			}),
		);
		const listening = await parent.waitFor((event) => event.type === 'gateway-listening');
		if (listening.type !== 'gateway-listening') throw new Error('Gateway did not listen.');
		const url = `ws://127.0.0.1:${listening.port}/v1/control`;

		await expect(rejectedUpgrade(url, 'http://127.0.0.1:3001', browserSessionCookie)).resolves.toBe(
			403,
		);
		await expect(rejectedUpgrade(url, allowedOrigin)).resolves.toBe(401);
		await expect(rejectedUpgrade(url, allowedOrigin, `${browserSessionCookie}a`)).resolves.toBe(
			401,
		);
		expect(
			parent.events.filter((event) => event.type === 'gateway-client-open-requested'),
		).toHaveLength(0);

		const accepted = await openWebSocket(url, allowedOrigin, browserSessionCookie);
		accepted.send(
			encodeGatewayControlClientMessage(
				GatewayControlClientMessageSchema.parse({
					protocolVersion: GATEWAY_PROTOCOL_VERSION,
					type: 'gateway-control-authenticate',
				}),
			),
		);
		const opened = await parent.waitFor((event) => event.type === 'gateway-client-open-requested');
		if (opened.type !== 'gateway-client-open-requested') {
			throw new Error('Gateway did not request a client session.');
		}
		accepted.close();
		await parent.waitFor(
			(event) =>
				event.type === 'gateway-client-close-requested' &&
				event.connectionId === opened.connectionId,
		);

		expect(opened.connectionId).toMatch(/^gateway:client:[0-9a-f]{32}$/u);
	});

	it('binds the explicit LAN preview and accepts only configured private Origins', async () => {
		const parent = new FakeParentPort();
		gateway = new LocalDevelopmentBrowserGateway(parent);
		gateway.start();
		parent.emit(
			GatewayDesktopCommandSchema.parse({
				allowedOrigins: [allowedOrigin, 'http://192.168.1.14:3000'],
				bindHost: '0.0.0.0',
				sessionSigningKey,
				type: 'gateway-start',
			}),
		);
		const listening = await parent.waitFor((event) => event.type === 'gateway-listening');
		if (listening.type !== 'gateway-listening') throw new Error('Gateway did not listen.');
		const url = `ws://127.0.0.1:${listening.port}/v1/control`;

		await expect(rejectedUpgrade(url, 'http://192.168.1.15:3000')).resolves.toBe(403);
		const accepted = await openWebSocket(url, 'http://192.168.1.14:3000', browserSessionCookie);
		accepted.close();
		await waitForClose(accepted);
	});

	it('terminates an attachment still awaiting authentication when stopping', async () => {
		const parent = new FakeParentPort();
		gateway = new LocalDevelopmentBrowserGateway(parent);
		gateway.start();
		parent.emit(
			GatewayDesktopCommandSchema.parse({
				allowedOrigins: [allowedOrigin],
				bindHost: '127.0.0.1',
				sessionSigningKey,
				type: 'gateway-start',
			}),
		);
		const listening = await parent.waitFor((event) => event.type === 'gateway-listening');
		if (listening.type !== 'gateway-listening') throw new Error('Gateway did not listen.');
		const attachment = await openWebSocket(
			`ws://127.0.0.1:${listening.port}/v1/attachment`,
			allowedOrigin,
		);
		const closed = waitForClose(attachment);

		await gateway.dispose();
		await closed;

		expect(attachment.readyState).toBe(WebSocket.CLOSED);
		expect(parent.events.at(-1)).toEqual({ type: 'gateway-stopped' });
	});

	it('does not leave a late listener bound when the Gateway is immediately disposed', async () => {
		const parent = new FakeParentPort();
		let server: Server | undefined;
		gateway = new LocalDevelopmentBrowserGateway(parent, (requestListener) => {
			server = createServer(requestListener);
			return server;
		});
		gateway.start();
		parent.emit(
			GatewayDesktopCommandSchema.parse({
				allowedOrigins: [allowedOrigin],
				bindHost: '127.0.0.1',
				sessionSigningKey,
				type: 'gateway-start',
			}),
		);

		await gateway.dispose();
		await new Promise((resolve) => setTimeout(resolve, 20));

		try {
			expect(server?.listening).toBe(false);
			expect(parent.events).toEqual([{ type: 'gateway-stopped' }]);
		} finally {
			if (server?.listening) await new Promise<void>((resolve) => server?.close(() => resolve()));
		}
	});

	it('reports a runtime HTTP server error and stops instead of throwing it uncaught', async () => {
		const parent = new FakeParentPort();
		let server: Server | undefined;
		gateway = new LocalDevelopmentBrowserGateway(parent, (requestListener) => {
			server = createServer(requestListener);
			return server;
		});
		gateway.start();
		parent.emit(
			GatewayDesktopCommandSchema.parse({
				allowedOrigins: [allowedOrigin],
				bindHost: '127.0.0.1',
				sessionSigningKey,
				type: 'gateway-start',
			}),
		);
		await parent.waitFor((event) => event.type === 'gateway-listening');
		if (server === undefined) throw new Error('Gateway HTTP server was not created.');

		server.emit('error', new Error('runtime failure'));

		await parent.waitFor((event) => event.type === 'gateway-stopped');
		expect(parent.events).toContainEqual({ code: 'listen-failed', type: 'gateway-failed' });
	});

	it('terminates a control socket that misses the authentication deadline', async () => {
		const parent = new FakeParentPort();
		gateway = new LocalDevelopmentBrowserGateway(parent);
		gateway.start();
		parent.emit(
			GatewayDesktopCommandSchema.parse({
				allowedOrigins: [allowedOrigin],
				bindHost: '127.0.0.1',
				sessionSigningKey,
				type: 'gateway-start',
			}),
		);
		const listening = await parent.waitFor((event) => event.type === 'gateway-listening');
		if (listening.type !== 'gateway-listening') throw new Error('Gateway did not listen.');
		vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
		const socket = await openWebSocket(
			`ws://127.0.0.1:${listening.port}/v1/control`,
			allowedOrigin,
			browserSessionCookie,
		);
		const closed = waitForClose(socket);

		await vi.advanceTimersByTimeAsync(GATEWAY_LIMITS.authenticationTimeoutMilliseconds);
		await closed;

		expect(
			parent.events.filter((event) => event.type === 'gateway-client-open-requested'),
		).toHaveLength(0);
	});

	it('terminates an authenticated socket after one unanswered heartbeat interval', async () => {
		vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
		const parent = new FakeParentPort();
		gateway = new LocalDevelopmentBrowserGateway(parent);
		gateway.start();
		parent.emit(
			GatewayDesktopCommandSchema.parse({
				allowedOrigins: [allowedOrigin],
				bindHost: '127.0.0.1',
				sessionSigningKey,
				type: 'gateway-start',
			}),
		);
		const listening = await parent.waitFor((event) => event.type === 'gateway-listening');
		if (listening.type !== 'gateway-listening') throw new Error('Gateway did not listen.');
		const socket = await openWebSocket(
			`ws://127.0.0.1:${listening.port}/v1/control`,
			allowedOrigin,
			browserSessionCookie,
		);
		socket.send(
			encodeGatewayControlClientMessage(
				GatewayControlClientMessageSchema.parse({
					protocolVersion: GATEWAY_PROTOCOL_VERSION,
					type: 'gateway-control-authenticate',
				}),
			),
		);
		await parent.waitFor((event) => event.type === 'gateway-client-open-requested');
		const transport = Reflect.get(socket, '_socket') as
			| { pause(): void; resume(): void }
			| undefined;
		if (transport === undefined) throw new Error('WebSocket transport is unavailable.');
		transport.pause();
		const closed = waitForClose(socket);

		await vi.advanceTimersByTimeAsync(GATEWAY_LIMITS.heartbeatIntervalMilliseconds * 2);
		transport.resume();
		await closed;

		expect(socket.readyState).toBe(WebSocket.CLOSED);
	});

	it('discards one opened attachment that arrives after release without retiring unrelated projections', async () => {
		const parent = new FakeParentPort();
		gateway = new LocalDevelopmentBrowserGateway(parent);
		gateway.start();
		parent.emit(
			GatewayDesktopCommandSchema.parse({
				allowedOrigins: [allowedOrigin],
				bindHost: '127.0.0.1',
				sessionSigningKey,
				type: 'gateway-start',
			}),
		);
		const listening = await parent.waitFor((event) => event.type === 'gateway-listening');
		if (listening.type !== 'gateway-listening') throw new Error('Gateway did not listen.');
		const socket = await openWebSocket(
			`ws://127.0.0.1:${listening.port}/v1/control`,
			allowedOrigin,
			browserSessionCookie,
		);
		const controlMessages: GatewayControlServerMessage[] = [];
		socket.on('message', (data, isBinary) => {
			if (isBinary) return;
			const decoded = decodeGatewayControlServerMessage(data.toString());
			if (decoded.ok) controlMessages.push(decoded.value);
		});
		socket.send(
			encodeGatewayControlClientMessage(
				GatewayControlClientMessageSchema.parse({
					protocolVersion: GATEWAY_PROTOCOL_VERSION,
					type: 'gateway-control-authenticate',
				}),
			),
		);
		const opened = await parent.waitFor((event) => event.type === 'gateway-client-open-requested');
		if (opened.type !== 'gateway-client-open-requested') {
			throw new Error('Gateway did not request a client session.');
		}
		const clientSessionPort = createTestPort();
		const projectionPort = createTestPort();
		parent.emit(
			GatewayDesktopCommandSchema.parse({
				bootstrap: workbenchBootstrap,
				connectionId: opened.connectionId,
				type: 'gateway-client-opened',
			}),
			[clientSessionPort.port, projectionPort.port],
		);
		await vi.waitFor(() =>
			expect(controlMessages.some((message) => message.type === 'gateway-control-ready')).toBe(
				true,
			),
		);

		const sendProjectionCommand = (command: unknown): void => {
			socket.send(
				encodeGatewayControlClientMessage(
					GatewayControlClientMessageSchema.parse({
						command,
						type: 'terminal-projection-control-command',
					}),
				),
			);
		};
		sendProjectionCommand({
			geometry: mountedGeometry,
			presentationId: 'presentation-1',
			requestId: 'projection-1',
			type: 'terminal-projection-open-requested',
		});
		await vi.waitFor(() => expect(projectionPort.posted).toHaveLength(1));
		sendProjectionCommand({
			presentationId: 'presentation-1',
			requestId: 'projection-1',
			type: 'terminal-projection-release-requested',
		});
		await vi.waitFor(() => expect(projectionPort.posted).toHaveLength(2));
		sendProjectionCommand({
			presentationId: 'presentation-1',
			requestId: 'projection-1',
			type: 'terminal-projection-release-requested',
		});

		const lateAttachmentPort = createTestPort();
		projectionPort.emitMessage(
			{
				bootstrap: projectionBootstrap,
				presentationId: 'presentation-1',
				requestId: 'projection-1',
				type: 'terminal-projection-opened',
			},
			[lateAttachmentPort.port],
		);
		expect(lateAttachmentPort.close).toHaveBeenCalledOnce();

		sendProjectionCommand({
			geometry: mountedGeometry,
			presentationId: 'presentation-2',
			requestId: 'projection-2',
			type: 'terminal-projection-open-requested',
		});
		await vi.waitFor(() => expect(projectionPort.posted).toHaveLength(3));
		const secondAttachmentPort = createTestPort();
		projectionPort.emitMessage(
			{
				bootstrap: {
					...projectionBootstrap,
					attachmentId: '55'.repeat(16),
					creationId: '66'.repeat(16),
					terminalId: '77'.repeat(16),
				},
				presentationId: 'presentation-2',
				requestId: 'projection-2',
				type: 'terminal-projection-opened',
			},
			[secondAttachmentPort.port],
		);
		await vi.waitFor(() =>
			expect(
				controlMessages.some(
					(message) =>
						message.type === 'terminal-projection-opened' &&
						message.projection.requestId === 'projection-2',
				),
			).toBe(true),
		);
		await vi.waitFor(() =>
			expect(projectionPort.posted).toContainEqual({
				geometry: mountedGeometry,
				presentationId: 'presentation-2',
				requestId: 'projection-2',
				type: 'terminal-projection-open-requested',
			}),
		);
		expect(projectionPort.posted).toHaveLength(3);
		projectionPort.emitMessage({
			presentationId: 'presentation-1',
			requestId: 'projection-1',
			type: 'terminal-projection-released',
		});
		await vi.waitFor(() =>
			expect(controlMessages).toContainEqual({
				event: {
					presentationId: 'presentation-1',
					requestId: 'projection-1',
					type: 'terminal-projection-released',
				},
				type: 'terminal-projection-control-event',
			}),
		);

		expect(socket.readyState).toBe(WebSocket.OPEN);
		expect(secondAttachmentPort.close).not.toHaveBeenCalled();
		expect(
			controlMessages.some(
				(message) =>
					message.type === 'gateway-control-failed' && message.code === 'protocol-rejected',
			),
		).toBe(false);

		const duplicateAttachmentPort = createTestPort();
		const closed = waitForClose(socket);
		projectionPort.emitMessage(
			{
				bootstrap: {
					...projectionBootstrap,
					attachmentId: '88'.repeat(16),
					creationId: '99'.repeat(16),
					terminalId: 'aa'.repeat(16),
				},
				presentationId: 'presentation-2',
				requestId: 'projection-2',
				type: 'terminal-projection-opened',
			},
			[duplicateAttachmentPort.port],
		);
		await closed;
		expect(duplicateAttachmentPort.close).toHaveBeenCalledOnce();
	});
});

class FakeParentPort implements GatewayParentPort {
	readonly events: GatewayUtilityEvent[] = [];
	readonly #listeners = new Set<(event: GatewayParentMessageEvent) => void>();
	readonly #waiters = new Set<{
		readonly predicate: (event: GatewayUtilityEvent) => boolean;
		readonly resolve: (event: GatewayUtilityEvent) => void;
	}>();

	emit(data: unknown, ports: readonly GatewayMessagePort[] = []): void {
		for (const listener of this.#listeners) listener({ data, ports });
	}

	off(_event: 'message', listener: (event: GatewayParentMessageEvent) => void): void {
		this.#listeners.delete(listener);
	}

	on(_event: 'message', listener: (event: GatewayParentMessageEvent) => void): void {
		this.#listeners.add(listener);
	}

	postMessage(message: unknown): void {
		const event = GatewayUtilityEventSchema.parse(message);
		this.events.push(event);
		for (const waiter of this.#waiters) {
			if (!waiter.predicate(event)) continue;
			this.#waiters.delete(waiter);
			waiter.resolve(event);
		}
	}

	waitFor(predicate: (event: GatewayUtilityEvent) => boolean): Promise<GatewayUtilityEvent> {
		const existing = this.events.find(predicate);
		if (existing !== undefined) return Promise.resolve(existing);
		return new Promise((resolve) => this.#waiters.add({ predicate, resolve }));
	}
}

function createTestPort() {
	type CloseListener = () => void;
	type MessageListener = (event: {
		readonly data: unknown;
		readonly ports: readonly GatewayMessagePort[];
	}) => void;
	const closeListeners = new Set<CloseListener>();
	const messageListeners = new Set<MessageListener>();
	const close = vi.fn<() => void>();
	const posted: unknown[] = [];
	const port = {
		close,
		off: (event: 'close' | 'message', listener: CloseListener | MessageListener): void => {
			if (event === 'close') closeListeners.delete(listener as CloseListener);
			else messageListeners.delete(listener as MessageListener);
		},
		on: (event: 'close' | 'message', listener: CloseListener | MessageListener): void => {
			if (event === 'close') closeListeners.add(listener as CloseListener);
			else messageListeners.add(listener as MessageListener);
		},
		postMessage: (message: unknown): void => {
			posted.push(message);
		},
		start: vi.fn<() => void>(),
	} as unknown as GatewayMessagePort;
	return {
		close,
		emitMessage: (data: unknown, ports: readonly GatewayMessagePort[] = []): void => {
			for (const listener of messageListeners) listener({ data, ports });
		},
		port,
		posted,
	};
}

function openWebSocket(url: string, origin: string, cookie?: string): Promise<WebSocket> {
	return new Promise((resolve, reject) => {
		const socket = new WebSocket(
			url,
			cookie === undefined ? { origin } : { headers: { Cookie: cookie }, origin },
		);
		socket.once('open', () => resolve(socket));
		socket.once('error', reject);
	});
}

function rejectedUpgrade(url: string, origin: string, cookie?: string): Promise<number> {
	return new Promise((resolve, reject) => {
		const socket = new WebSocket(
			url,
			cookie === undefined ? { origin } : { headers: { Cookie: cookie }, origin },
		);
		socket.once('unexpected-response', (_request, response) => {
			resolve(response.statusCode ?? 0);
			socket.terminate();
		});
		socket.once('open', () => reject(new Error('Rejected Gateway socket opened.')));
		socket.once('error', () => undefined);
	});
}

function waitForClose(socket: WebSocket): Promise<void> {
	if (socket.readyState === WebSocket.CLOSED) return Promise.resolve();
	return new Promise((resolve) => socket.once('close', () => resolve()));
}
