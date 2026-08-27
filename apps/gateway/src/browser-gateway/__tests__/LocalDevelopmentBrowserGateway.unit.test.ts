import {
	encodeGatewayControlClientMessage,
	GATEWAY_PROTOCOL_VERSION,
	GATEWAY_LIMITS,
	GatewayControlClientMessageSchema,
	GatewayDesktopCommandSchema,
	GatewayUtilityEventSchema,
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
