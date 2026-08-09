import type { FastifyInstance } from 'fastify';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';

import { createOrchbeeServer } from '../createOrchbeeServer';

const rendererMarker = '<main>orchbee-renderer</main>';
let rendererRoot = '';
let server: FastifyInstance | undefined;

beforeEach(async () => {
	rendererRoot = await mkdtemp(join(tmpdir(), 'orchbee-server-'));
	await mkdir(join(rendererRoot, 'static'));
	await writeFile(join(rendererRoot, 'index.html'), rendererMarker);
	await writeFile(join(rendererRoot, 'static', 'application.123.js'), 'globalThis.orchbee = true;');
});

afterEach(async () => {
	await server?.close();
	server = undefined;
	await rm(rendererRoot, { force: true, recursive: true });
});

describe('createOrchbeeServer', () => {
	it('serves renderer assets and document routes without namespace fallthrough', async () => {
		server = createOrchbeeServer({ rendererRoot });

		const asset = await server.inject({ method: 'GET', url: '/static/application.123.js' });
		const document = await server.inject({
			headers: { accept: 'TEXT/HTML,application/xhtml+xml' },
			method: 'GET',
			url: '/projects/alpha/workspaces/primary/tabs/terminal',
		});
		const head = await server.inject({
			headers: { accept: 'text/html' },
			method: 'HEAD',
			url: '/settings',
		});
		const apiMiss = await server.inject({
			headers: { accept: 'text/html' },
			method: 'GET',
			url: '/api/v1/missing',
		});
		const websocketOverHttp = await server.inject({
			headers: { accept: 'text/html' },
			method: 'GET',
			url: '/ws/v1/events',
		});
		const staticMiss = await server.inject({
			headers: { accept: 'text/html' },
			method: 'GET',
			url: '/static/missing.js',
		});
		const textualPrefix = await server.inject({
			headers: { accept: 'text/html' },
			method: 'GET',
			url: '/apiary',
		});

		expect(asset.body).toContain('globalThis.orchbee');
		expect(asset.headers['cache-control']).toContain('immutable');
		expect(asset.headers['x-content-type-options']).toBe('nosniff');
		expect(document.body).toBe(rendererMarker);
		expect(document.headers['cache-control']).toBe('no-cache');
		expect(head.statusCode).toBe(200);
		expect(head.body).toBe('');
		expect(apiMiss.json()).toEqual({ error: { code: 'api-not-found' } });
		expect(websocketOverHttp.statusCode).toBe(426);
		expect(websocketOverHttp.headers.upgrade).toBe('websocket');
		expect(staticMiss.json()).toEqual({ error: { code: 'static-not-found' } });
		expect(staticMiss.body).not.toContain(rendererMarker);
		expect(textualPrefix.body).toBe(rendererMarker);
	});

	it('reserves encoded namespace separators and rejects malformed paths', async () => {
		server = createOrchbeeServer({ rendererRoot });

		const encodedApiMiss = await server.inject({
			headers: { accept: 'text/html' },
			method: 'GET',
			url: '/api%2fv1/missing',
		});
		const malformedPath = await server.inject({
			headers: { accept: 'text/html' },
			method: 'GET',
			url: '/projects/%zz',
		});

		expect(encodedApiMiss.json()).toEqual({ error: { code: 'api-not-found' } });
		expect(malformedPath.statusCode).toBe(400);
		expect(malformedPath.body).not.toContain(rendererMarker);
	});

	it('uses one listener for ordinary HTTP and registered WebSocket routes', async () => {
		server = createOrchbeeServer({
			applicationPlugin: async (application) => {
				application.get('/ws/v1/echo', { websocket: true }, (socket) => {
					socket.on('message', (message) => socket.send(message));
				});
			},
			rendererRoot,
		});
		const origin = await server.listen({ host: '127.0.0.1', port: 0 });
		const websocketOrigin = origin.replace(/^http/u, 'ws');
		await expect(rejectedWebSocketUpgrade(`${websocketOrigin}/ws/v1/missing`)).resolves.toBe(426);
		const socket = await openWebSocket(`${websocketOrigin}/ws/v1/echo`);
		const echoed = waitForMessage(socket);
		socket.send('one-port');
		const asset = await fetch(`${origin}/static/application.123.js`);

		expect(await echoed).toBe('one-port');
		expect(await asset.text()).toContain('globalThis.orchbee');

		const closed = waitForClose(socket);
		await server.close();
		server = undefined;
		await closed;
		expect(socket.readyState).toBe(WebSocket.CLOSED);
	});

	it('requires a complete trusted renderer distribution before readiness', async () => {
		await rm(join(rendererRoot, 'index.html'));

		expect(() => createOrchbeeServer({ rendererRoot })).toThrow(/ENOENT/u);
	});
});

function openWebSocket(url: string): Promise<WebSocket> {
	return new Promise((resolve, reject) => {
		const socket = new WebSocket(url);
		socket.once('open', () => resolve(socket));
		socket.once('error', reject);
	});
}

function rejectedWebSocketUpgrade(url: string): Promise<number> {
	return new Promise((resolve, reject) => {
		const socket = new WebSocket(url);
		socket.once('unexpected-response', (_request, response) => {
			resolve(response.statusCode ?? 0);
			socket.terminate();
		});
		socket.once('open', () => reject(new Error('Rejected WebSocket route opened.')));
		socket.once('error', () => undefined);
	});
}

function waitForMessage(socket: WebSocket): Promise<string> {
	return new Promise((resolve, reject) => {
		socket.once('message', (message) => resolve(message.toString()));
		socket.once('error', reject);
	});
}

function waitForClose(socket: WebSocket): Promise<void> {
	if (socket.readyState === WebSocket.CLOSED) return Promise.resolve();
	return new Promise((resolve) => socket.once('close', () => resolve()));
}
