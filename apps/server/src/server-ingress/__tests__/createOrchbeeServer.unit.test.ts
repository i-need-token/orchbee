import type { FastifyInstance } from 'fastify';
import {
	BrowserSessionSigningKeySchema,
	PersonalAccessTokenSchema,
	PersonalAccessTokenVerifierSchema,
} from '@orchbee/remote-client-protocol';
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';

import { createOrchbeeServer } from '../createOrchbeeServer';

const rendererMarker = '<main>orchbee-renderer</main>';
const personalAccessToken = PersonalAccessTokenSchema.parse('p'.repeat(43));
const authentication = {
	personalAccessTokenVerifier: PersonalAccessTokenVerifierSchema.parse(
		createHash('sha256').update(personalAccessToken, 'utf8').digest('base64url'),
	),
	sessionSigningKey: BrowserSessionSigningKeySchema.parse('s'.repeat(43)),
};
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
		server = createOrchbeeServer({ authentication, rendererRoot });

		const asset = await server.inject({ method: 'GET', url: '/static/application.123.js' });
		const login = await server.inject({
			headers: { accept: 'text/html' },
			method: 'GET',
			url: '/login',
		});
		const redirectedDocument = await server.inject({
			headers: { accept: 'text/html' },
			method: 'GET',
			url: '/projects/alpha/workspaces/primary/tabs/terminal?panel=1',
		});
		const cookie = await authenticate(server);
		const document = await server.inject({
			headers: { accept: 'TEXT/HTML,application/xhtml+xml', cookie },
			method: 'GET',
			url: '/projects/alpha/workspaces/primary/tabs/terminal',
		});
		const head = await server.inject({
			headers: { accept: 'text/html', cookie },
			method: 'HEAD',
			url: '/settings',
		});
		const apiMiss = await server.inject({
			headers: { accept: 'text/html', cookie },
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
			headers: { accept: 'text/html', cookie },
			method: 'GET',
			url: '/apiary',
		});

		expect(asset.body).toContain('globalThis.orchbee');
		expect(asset.headers['cache-control']).toContain('immutable');
		expect(asset.headers['x-content-type-options']).toBe('nosniff');
		expect(login.body).toBe(rendererMarker);
		expect(redirectedDocument.statusCode).toBe(302);
		expect(redirectedDocument.headers.location).toBe(
			'/login?returnTo=%2Fprojects%2Falpha%2Fworkspaces%2Fprimary%2Ftabs%2Fterminal%3Fpanel%3D1',
		);
		expect(redirectedDocument.body).not.toContain(rendererMarker);
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
		server = createOrchbeeServer({ authentication, rendererRoot });

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

	it('issues a signed HttpOnly session Cookie without storing the personal access token in it', async () => {
		server = createOrchbeeServer({ authentication, rendererRoot });

		const response = await server.inject({
			headers: { host: 'localhost', origin: 'http://localhost' },
			method: 'POST',
			payload: { personalAccessToken },
			url: '/api/v1/auth/session',
		});

		expect(response.statusCode).toBe(204);
		expect(response.headers['cache-control']).toBe('no-store');
		const setCookie = String(response.headers['set-cookie']);
		expect(setCookie).toContain('orchbee-browser-session=v1.');
		expect(setCookie).toContain('; HttpOnly; Path=/; SameSite=Strict');
		expect(setCookie).not.toContain('Secure');
		expect(setCookie).not.toContain(personalAccessToken);
	});

	it('rejects cross-origin, invalid, rate-limited, and tampered authentication attempts', async () => {
		server = createOrchbeeServer({ authentication, rendererRoot });

		const crossOrigin = await server.inject({
			headers: { host: 'localhost', origin: 'http://attacker.example' },
			method: 'POST',
			payload: { personalAccessToken },
			url: '/api/v1/auth/session',
		});
		expect(crossOrigin.statusCode).toBe(403);
		expect(crossOrigin.json()).toEqual({ error: { code: 'invalid-origin' } });

		for (let attempt = 0; attempt < 5; attempt += 1) {
			const invalid = await server.inject({
				headers: { host: 'localhost', origin: 'http://localhost' },
				method: 'POST',
				payload: { personalAccessToken: 'x'.repeat(43) },
				remoteAddress: '192.0.2.10',
				url: '/api/v1/auth/session',
			});
			expect(invalid.statusCode).toBe(401);
		}
		const limited = await server.inject({
			headers: { host: 'localhost', origin: 'http://localhost' },
			method: 'POST',
			payload: { personalAccessToken },
			remoteAddress: '192.0.2.10',
			url: '/api/v1/auth/session',
		});
		expect(limited.statusCode).toBe(429);
		expect(limited.json()).toEqual({ error: { code: 'rate-limited' } });

		const cookie = await authenticate(server);
		const finalCharacter = cookie.at(-1);
		const tamperedCookie = `${cookie.slice(0, -1)}${finalCharacter === 'a' ? 'b' : 'a'}`;
		const protectedDocument = await server.inject({
			headers: { accept: 'text/html', cookie: tamperedCookie },
			method: 'GET',
			url: '/settings',
		});
		expect(protectedDocument.statusCode).toBe(302);
		expect(protectedDocument.headers.location).toBe('/login?returnTo=%2Fsettings');
	});

	it('uses one listener for ordinary HTTP and registered WebSocket routes', async () => {
		server = createOrchbeeServer({
			applicationPlugin: async (application) => {
				application.get('/ws/v1/echo', { websocket: true }, (socket) => {
					socket.on('message', (message) => socket.send(message));
				});
			},
			authentication,
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

		expect(() => createOrchbeeServer({ authentication, rendererRoot })).toThrow(/ENOENT/u);
	});
});

async function authenticate(instance: FastifyInstance): Promise<string> {
	const response = await instance.inject({
		headers: { host: 'localhost', origin: 'http://localhost' },
		method: 'POST',
		payload: { personalAccessToken },
		url: '/api/v1/auth/session',
	});
	expect(response.statusCode).toBe(204);
	const setCookie = response.headers['set-cookie'];
	if (typeof setCookie !== 'string') throw new Error('Authentication Cookie was not issued.');
	const cookie = setCookie.split(';', 1)[0];
	if (cookie === undefined) throw new Error('Authentication Cookie was malformed.');
	return cookie;
}

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
