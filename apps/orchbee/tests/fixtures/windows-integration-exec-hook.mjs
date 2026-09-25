// Execute shipped Claude hook argv, preserving literal paths, and observe the
// production Windows ingress envelope. Refuse ACKs to avoid console APC writes.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import fs from 'node:fs';
import { createServer } from 'node:net';
import path from 'node:path';

const [provider, root] = process.argv.slice(2);
assert.ok(['claude-code', 'github-copilot'].includes(provider));
const hooks = JSON.parse(
	fs.readFileSync(
		path.join(root, provider === 'claude-code' ? 'hooks/hooks.json' : 'hooks/copilot.json'),
		'utf8',
	),
).hooks;
const pipe = String.raw`\\.\pipe\orchbee-${randomBytes(16).toString('hex')}-observation`;
const observations = [];
const connections = new Set();
const children = new Set();
const server = createServer({ allowHalfOpen: true }, (socket) => {
	connections.add(socket);
	socket.once('close', () => connections.delete(socket));
	socket.on('error', () => {});
	let bytes = Buffer.alloc(0);
	let received = false;
	socket.on('data', (chunk) => {
		if (received) return;
		bytes = Buffer.concat([bytes, chunk]);
		if (bytes.length > 65536) return socket.destroy();
		try {
			const envelope = JSON.parse(bytes.toString('utf8'));
			received = true;
			observations.push(envelope);
			socket.end('rejected');
		} catch (error) {
			if (!(error instanceof SyntaxError)) socket.destroy();
		}
	});
});
await new Promise((resolve, reject) => {
	server.once('error', reject);
	server.listen(pipe, resolve);
});
async function run(hook, input, scriptRoot = root) {
	assert.equal(provider === 'claude-code' ? hook.command : hook.exec, 'node');
	const args = hook.args.map((arg) => arg.replace('${CLAUDE_PLUGIN_ROOT}', scriptRoot));
	await new Promise((resolve, reject) => {
		const child = spawn(process.execPath, args, {
			windowsHide: true,
			env: {
				USERPROFILE: process.env.USERPROFILE,
				COPILOT_PLUGIN_ROOT: scriptRoot,
				ORCHBEE_AGENT_OBSERVATION_SOCKET_V3: pipe,
			},
			stdio: ['pipe', 'pipe', 'pipe'],
		});
		children.add(child);
		const output = [];
		child.stdout.on('data', (value) => output.push(value));
		child.stderr.on('data', (value) => output.push(value));
		child.stdin.on('error', () => {});
		const timeout = setTimeout(() => child.kill(), 5000);
		child.once('error', reject);
		child.once('close', (code, signal) => {
			clearTimeout(timeout);
			children.delete(child);
			try {
				assert.equal(code, 0);
				assert.equal(signal, null);
				assert.equal(Buffer.concat(output).length, 0);
				resolve();
			} catch (error) {
				reject(error);
			}
		});
		child.stdin.end(input);
	});
}
const hookFor = (event) =>
	provider === 'claude-code' ? hooks[event][0].hooks[0] : hooks[event][0];
try {
	for (const event of ['SessionStart', 'UserPromptSubmit', 'Stop', 'SessionEnd']) {
		await run(
			hookFor(event),
			JSON.stringify({
				hook_event_name: event,
				session_id: 'literal-path-session',
				prompt: 'PRIVATE_PROMPT',
				cwd: 'PRIVATE_CWD',
				transcript_path: 'PRIVATE_TRANSCRIPT',
			}),
		);
	}
	assert.equal(observations.length, 4);
	assert.deepEqual(
		observations.map((entry) => entry.observation.event),
		['session-started', 'turn-started', 'turn-stopped', 'session-ended'],
	);
	for (const envelope of observations) {
		assert.equal(envelope.adapterId, provider);
		assert.equal(envelope.version, 3);
		assert.equal(envelope.observation.sessionKey, 'literal-path-session');
		assert.equal(envelope.correlations.length, 1);
		assert.ok(!JSON.stringify(envelope).includes('PRIVATE_'));
	}
	await run(hookFor('SessionStart'), '{malformed');
	await run(hookFor('SessionStart'), '{}', path.join(root, 'missing'));
	assert.equal(observations.length, 4);
	process.stdout.write(JSON.stringify({ events: observations.length, silent: true }));
} finally {
	await Promise.all(
		[...children].map(
			(child) =>
				new Promise((resolve) => {
					child.once('close', resolve);
					child.kill();
				}),
		),
	);
	for (const socket of connections) socket.destroy();
	await new Promise((resolve) => server.close(resolve));
}
