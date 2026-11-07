// Load an installed bootstrap through a real Node module boundary, admitting it
// as each supported loader generation does, and observe its production Windows
// transport. Refused ACKs keep this pipe-only witness from writing correlation
// hints to the invoking developer's console. `opencode-client` names the
// OpenCode 2.x client directory rather than a module file.
import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';
import { createServer } from 'node:net';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const [provider, entry] = process.argv.slice(2);
assert.ok(['opencode', 'opencode-client', 'kilo', 'pi', 'omp'].includes(provider));
assert.equal(typeof entry, 'string');
const adapter = provider === 'opencode-client' ? 'opencode' : provider;
// The client reports the displayed root, its busy status, then a switch to a
// new root; the module entries report one session's lifecycle.
const expected =
	provider === 'opencode-client'
		? [
				['session-started', 'module-session'],
				['turn-started', 'module-session'],
				['session-ended', 'module-session'],
				['session-started', 'module-other'],
			]
		: [
				['session-started', 'module-session'],
				['turn-started', 'module-session'],
				['session-idle', 'module-session'],
				['session-ended', 'module-session'],
			];
const pipe = String.raw`\\.\pipe\orchbee-${randomBytes(16).toString('hex')}-observation`;
process.env.ORCHBEE_AGENT_OBSERVATION_SOCKET = pipe;
const observed = [];
// The client polls its TUI state each second and moves on as each report arrives.
let advance = () => {};
const connections = new Set();
const delivered = Promise.withResolvers();
const server = createServer({ allowHalfOpen: true }, (socket) => {
	connections.add(socket);
	socket.once('close', () => connections.delete(socket));
	socket.on('error', delivered.reject);
	const chunks = [];
	let bytes = 0;
	let received = false;
	socket.on('data', (chunk) => {
		if (received) return;
		bytes += chunk.length;
		if (bytes > 65536) {
			delivered.reject(new Error('unbounded provider envelope'));
			socket.destroy();
			return;
		}
		chunks.push(chunk);
		// A Windows pipe does not promise a TCP-style half-close. Admit a
		// complete bounded document before EOF, just like the native endpoint.
		try {
			const envelope = JSON.parse(Buffer.concat(chunks).toString('utf8'));
			received = true;
			observed.push(envelope);
			socket.end('rejected');
			advance();
			if (observed.length === expected.length) delivered.resolve();
		} catch (error) {
			if (!(error instanceof SyntaxError)) delivered.reject(error);
		}
	});
	socket.once('end', () => {
		if (!received) delivered.reject(new Error('incomplete provider envelope'));
	});
});
await new Promise((resolve, reject) => {
	server.once('error', reject);
	server.listen(pipe, resolve);
});
const timer = setTimeout(() => delivered.reject(new Error('module events timed out')), 10000);
try {
	if (provider === 'opencode-client') {
		// OpenCode 2.x resolves the directory's `server` and `tui` entries and
		// decodes each default as a plain object with a string id and a setup
		// function. The server's setup observes nothing; the TUI's reports the
		// root of the session its route displays, from the TUI plugin context.
		const definitions = [];
		for (const name of ['server.js', 'tui.js']) {
			const definition = (await import(pathToFileURL(path.join(entry, name)).href)).default;
			assert.ok(
				typeof definition === 'object' && definition !== null && !Array.isArray(definition),
			);
			assert.equal(typeof definition.id, 'string');
			assert.equal(typeof definition.setup, 'function');
			definitions.push(definition);
		}
		const empty = { async *[Symbol.asyncIterator]() {} };
		const [serverPlugin, tuiPlugin] = definitions;
		assert.equal(
			await serverPlugin.setup({
				id: serverPlugin.id,
				options: {},
				event: { subscribe: () => empty },
			}),
			undefined,
		);
		const sessions = new Map([
			['module-session', { id: 'module-session' }],
			['module-child', { id: 'module-child', parentID: 'module-session' }],
			['module-other', { id: 'module-other' }],
		]);
		const statuses = new Map();
		let route = { type: 'session', sessionID: 'module-child' };
		advance = () => {
			if (observed.length === 1) statuses.set('module-session', 'running');
			if (observed.length === 2) route = { type: 'session', sessionID: 'module-other' };
		};
		const cleanup = await tuiPlugin.setup({
			options: {},
			ui: { router: { current: () => route } },
			data: {
				session: {
					get: (id) => sessions.get(id),
					root: (id) => sessions.get(id)?.parentID ?? id,
					status: (id) => statuses.get(id) ?? 'idle',
				},
			},
		});
		assert.equal(typeof cleanup, 'function');
		await delivered.promise;
		cleanup();
	} else if (provider === 'opencode' || provider === 'kilo') {
		// OpenCode 2.x decodes `default` as a plain object with a string id and
		// a setup function, then awaits setup(context) and keeps its cleanup.
		const definition = (await import(pathToFileURL(entry).href)).default;
		assert.ok(typeof definition === 'object' && definition !== null && !Array.isArray(definition));
		assert.equal(typeof definition.id, 'string');
		assert.equal(typeof definition.setup, 'function');
		const empty = { async *[Symbol.asyncIterator]() {} };
		const cleanup = await definition.setup({
			id: definition.id,
			options: {},
			event: { subscribe: () => empty },
		});
		assert.ok(cleanup === undefined || typeof cleanup === 'function');
		// OpenCode 1.3.4+ and Kilo admit the same default as a server plugin:
		// a server function and no tui, and an id for a file plugin.
		assert.equal(typeof definition.server, 'function');
		assert.equal(definition.tui, undefined);
		const plugin = await definition.server({}, undefined);
		for (const event of [
			{ type: 'session.created', properties: { info: { id: 'module-session' } } },
			{
				type: 'session.status',
				properties: { sessionID: 'module-session', status: { type: 'busy' } },
			},
			{
				type: 'session.status',
				properties: { sessionID: 'module-session', status: { type: 'idle' } },
			},
			{
				type: 'session.deleted',
				properties: { sessionID: 'module-session', info: { id: 'module-session' } },
			},
		])
			await plugin.event({ event });
	} else {
		const handlers = new Map();
		const loaded = await import(pathToFileURL(entry).href);
		await loaded.default({ on: (name, handler) => handlers.set(name, handler) });
		const context = { sessionManager: { getSessionId: () => 'module-session' } };
		for (const name of [
			'session_start',
			'agent_start',
			provider === 'pi' ? 'agent_settled' : 'agent_end',
			'session_shutdown',
		]) {
			assert.equal(typeof handlers.get(name), 'function', `missing ${name}`);
			await handlers.get(name)({}, context);
		}
	}
	await delivered.promise;
	assert.deepEqual(
		observed.map((value) => [value.observation.event, value.observation.sessionKey]),
		expected,
	);
	for (const envelope of observed) {
		assert.equal(envelope.adapterId, adapter);
		assert.equal(envelope.version, 1);
		assert.equal(envelope.correlations.length, 1);
	}
	process.stdout.write(JSON.stringify({ provider, events: observed.length }));
} finally {
	clearTimeout(timer);
	for (const socket of connections) socket.destroy();
	await new Promise((resolve) => server.close(resolve));
}
