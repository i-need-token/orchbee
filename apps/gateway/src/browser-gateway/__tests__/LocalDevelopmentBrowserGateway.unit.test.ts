import {
	encodeGatewayControlClientMessage,
	GatewayControlClientMessageSchema,
	GatewayDesktopCommandSchema,
	GatewayUtilityEventSchema,
	type GatewayUtilityEvent,
} from '@orchbee/gateway-protocol';
import { afterEach, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';

import {
	LocalDevelopmentBrowserGateway,
	type GatewayMessagePort,
	type GatewayParentMessageEvent,
	type GatewayParentPort,
} from '../LocalDevelopmentBrowserGateway';

const allowedOrigin = 'http://127.0.0.1:3000';
const capability = 'a'.repeat(43);

let gateway: LocalDevelopmentBrowserGateway | undefined;

afterEach(async () => {
	await gateway?.dispose();
	gateway = undefined;
});

describe('LocalDevelopmentBrowserGateway', () => {
	it('binds loopback, rejects the wrong Origin, and authenticates before requesting Main ports', async () => {
		const parent = new FakeParentPort();
		gateway = new LocalDevelopmentBrowserGateway(parent);
		gateway.start();
		parent.emit(
			GatewayDesktopCommandSchema.parse({
				allowedOrigins: [allowedOrigin],
				bindHost: '127.0.0.1',
				capability,
				type: 'gateway-start',
			}),
		);
		const listening = await parent.waitFor((event) => event.type === 'gateway-listening');
		if (listening.type !== 'gateway-listening') throw new Error('Gateway did not listen.');
		const url = `ws://127.0.0.1:${listening.port}/v1/control`;

		await expect(rejectedUpgrade(url, 'http://127.0.0.1:3001')).resolves.toBe(403);
		const rejected = await openWebSocket(url, allowedOrigin);
		rejected.send(
			encodeGatewayControlClientMessage(
				GatewayControlClientMessageSchema.parse({
					capability: 'b'.repeat(43),
					protocolVersion: '1.2',
					type: 'gateway-control-authenticate',
				}),
			),
		);
		await waitForClose(rejected);
		expect(
			parent.events.filter((event) => event.type === 'gateway-client-open-requested'),
		).toHaveLength(0);

		const accepted = await openWebSocket(url, allowedOrigin);
		accepted.send(
			encodeGatewayControlClientMessage(
				GatewayControlClientMessageSchema.parse({
					capability,
					protocolVersion: '1.2',
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
				capability,
				type: 'gateway-start',
			}),
		);
		const listening = await parent.waitFor((event) => event.type === 'gateway-listening');
		if (listening.type !== 'gateway-listening') throw new Error('Gateway did not listen.');
		const url = `ws://127.0.0.1:${listening.port}/v1/control`;

		await expect(rejectedUpgrade(url, 'http://192.168.1.15:3000')).resolves.toBe(403);
		const accepted = await openWebSocket(url, 'http://192.168.1.14:3000');
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
				capability,
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

function openWebSocket(url: string, origin: string): Promise<WebSocket> {
	return new Promise((resolve, reject) => {
		const socket = new WebSocket(url, { origin });
		socket.once('open', () => resolve(socket));
		socket.once('error', reject);
	});
}

function rejectedUpgrade(url: string, origin: string): Promise<number> {
	return new Promise((resolve, reject) => {
		const socket = new WebSocket(url, { origin });
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
