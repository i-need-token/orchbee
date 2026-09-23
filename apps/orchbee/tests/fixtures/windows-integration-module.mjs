// Load an installed bootstrap through a real Node module boundary and observe
// its production Windows transport. Refused ACKs keep this pipe-only witness
// from writing correlation hints to the invoking developer's console.
import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';
import { createServer } from 'node:net';
import { pathToFileURL } from 'node:url';

const [provider, entry] = process.argv.slice(2);
assert.ok(['opencode', 'pi', 'omp'].includes(provider));
assert.equal(typeof entry, 'string');
const pipe = String.raw`\\.\pipe\orchbee-${randomBytes(16).toString('hex')}-observation`;
process.env.ORCHBEE_AGENT_OBSERVATION_SOCKET_V3 = pipe;
const observed = [];
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
			if (observed.length === 4) delivered.resolve();
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
const timer = setTimeout(() => delivered.reject(new Error('module events timed out')), 5000);
try {
	const loaded = await import(pathToFileURL(entry).href);
	if (provider === 'opencode') {
		const plugin = await loaded.OrchbeeAgentObservation();
		for (const event of [
			{ type: 'session.created', properties: { sessionID: 'module-session' } },
			{
				type: 'session.status',
				properties: { sessionID: 'module-session', status: { type: 'busy' } },
			},
			{
				type: 'session.status',
				properties: { sessionID: 'module-session', status: { type: 'idle' } },
			},
			{ type: 'session.deleted', properties: { sessionID: 'module-session' } },
		])
			await plugin.event({ event });
	} else {
		const handlers = new Map();
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
		observed.map((value) => value.observation.event),
		['session-started', 'turn-started', 'session-idle', 'session-ended'],
	);
	for (const envelope of observed) {
		assert.equal(envelope.adapterId, provider);
		assert.equal(envelope.version, 3);
		assert.equal(envelope.observation.sessionKey, 'module-session');
		assert.equal(envelope.correlations.length, 1);
	}
	process.stdout.write(JSON.stringify({ provider, events: observed.length }));
} finally {
	clearTimeout(timer);
	for (const socket of connections) socket.destroy();
	await new Promise((resolve) => server.close(resolve));
}
