// Execute shipped Claude hook argv, preserving literal paths, and observe the
// production Windows ingress envelope. Refuse ACKs to avoid console APC writes.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import fs from 'node:fs';
import { createServer } from 'node:net';
import path from 'node:path';

const [provider, root, shell] = process.argv.slice(2);
assert.ok(
	[
		'claude-code',
		'github-copilot',
		'grok',
		'gemini',
		'qwen-code',
		'cursor',
		'qodercli',
		'antigravity-cli',
	].includes(provider),
);
const extensionProvider = provider === 'gemini' || provider === 'qwen-code';
const jsonProvider = ['cursor', 'qodercli', 'antigravity-cli'].includes(provider);
function hookFile() {
	if (provider === 'cursor' || provider === 'antigravity-cli') return 'hooks.json';
	if (provider === 'qodercli') return 'settings.json';
	if (provider === 'qwen-code') return 'qwen-extension.json';
	if (provider === 'grok') return 'hooks/grok.json';
	if (provider === 'github-copilot') return 'hooks/copilot.json';
	return 'hooks/hooks.json';
}
const document = JSON.parse(fs.readFileSync(path.join(root, hookFile()), 'utf8'));
const hooks =
	provider === 'antigravity-cli' ? document['orchbee-agent-observation'] : document.hooks;
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
	let command = process.execPath;
	let args;
	if (provider === 'grok' || extensionProvider || jsonProvider) {
		assert.ok(['powershell.exe', 'pwsh.exe', 'cmd.exe'].includes(shell));
		assert.ok(
			hook.command.startsWith(
				jsonProvider
					? 'powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand '
					: 'node -e "eval(Buffer.from(',
			),
		);
		command = shell;
		args =
			shell === 'cmd.exe'
				? ['/d', '/s', '/c', hook.command]
				: ['-NoProfile', '-NonInteractive', '-Command', hook.command];
	} else {
		assert.equal(provider === 'claude-code' ? hook.command : hook.exec, 'node');
		args = hook.args.map((arg) => arg.replace('${CLAUDE_PLUGIN_ROOT}', scriptRoot));
	}
	const hookEnvironment = {};
	for (const [name, value] of Object.entries(hook.env ?? {})) {
		hookEnvironment[name] = value.replaceAll('${extensionPath}', () => scriptRoot);
	}
	if (provider === 'qwen-code') assert.equal(hook.shell, 'powershell');
	if (extensionProvider) {
		assert.equal(hookEnvironment.ORCHBEE_AGENT_EXTENSION_ROOT, scriptRoot);
		assert.ok(!hook.command.includes(scriptRoot));
	}
	await new Promise((resolve, reject) => {
		const child = spawn(command, args, {
			windowsHide: true,
			windowsVerbatimArguments: shell === 'cmd.exe' && !jsonProvider,
			env: {
				...hookEnvironment,
				SystemRoot: process.env.SystemRoot,
				PATHEXT: process.env.PATHEXT,
				PATH: `${path.dirname(process.execPath)};${process.env.PATH}`,
				USERPROFILE: process.env.USERPROFILE,
				GROK_PLUGIN_ROOT: scriptRoot,
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
				assert.equal(code, 0, Buffer.concat(output).toString());
				assert.equal(signal, null);
				assert.equal(Buffer.concat(output).toString().trim(), jsonProvider ? '{}' : '');
				resolve();
			} catch (error) {
				reject(error);
			}
		});
		child.stdin.end(input);
	});
}
function hookFor(event) {
	if (provider === 'antigravity-cli') return hooks[event][0];
	if (provider === 'qodercli') {
		const entry = hooks[event]
			.flatMap((group) => group.hooks)
			.find((hook) => hook.command?.startsWith('powershell.exe -NoLogo'));
		assert.equal(entry.shell, 'powershell');
		return entry;
	}
	if (provider === 'cursor')
		return hooks[event].find((hook) => hook.command.startsWith('powershell.exe -NoLogo'));
	return provider === 'github-copilot' ? hooks[event][0] : hooks[event][0].hooks[0];
}
function eventNames() {
	if (provider === 'antigravity-cli') return ['PreInvocation'];
	if (jsonProvider) return [provider === 'cursor' ? 'sessionStart' : 'SessionStart'];
	return provider === 'gemini'
		? ['SessionStart', 'BeforeAgent', 'AfterAgent', 'SessionEnd']
		: ['SessionStart', 'UserPromptSubmit', 'Stop', 'SessionEnd'];
}
try {
	const events = eventNames();
	for (const event of events) {
		await run(
			hookFor(event),
			JSON.stringify({
				...(provider === 'grok'
					? { hookEventName: event, sessionId: 'literal-path-session', reason: 'end_turn' }
					: {}),
				...(provider === 'qwen-code' && event === 'UserPromptSubmit'
					? { submitted_prompt: 'PRIVATE_SUBMITTED_PROMPT' }
					: {}),
				...(provider === 'antigravity-cli' ? { conversationId: 'literal-path-session' } : {}),
				hook_event_name: event,
				session_id: 'literal-path-session',
				prompt: 'PRIVATE_PROMPT',
				cwd: 'PRIVATE_CWD',
				transcript_path: 'PRIVATE_TRANSCRIPT',
			}),
		);
	}
	assert.equal(observations.length, jsonProvider ? 1 : 4);
	assert.deepEqual(
		observations.map((entry) => entry.observation.event),
		jsonProvider
			? ['session-identified']
			: ['session-started', 'turn-started', 'turn-stopped', 'session-ended'],
	);
	for (const envelope of observations) {
		assert.equal(envelope.adapterId, provider);
		assert.equal(envelope.version, 3);
		assert.equal(envelope.observation.sessionKey, 'literal-path-session');
		assert.equal(envelope.correlations.length, 1);
		assert.ok(!JSON.stringify(envelope).includes('PRIVATE_'));
	}
	await run(hookFor(events[0]), '{malformed');
	await run(hookFor(events[0]), '{}', path.join(root, 'missing'));
	if (jsonProvider) {
		const original = hookFor(events[0]);
		const encoded = original.command.split(' ').at(-1);
		const script = Buffer.from(encoded, 'base64')
			.toString('utf16le')
			.replace(
				/\$env:ORCHBEE_AGENT_HOOK_ROOT='[^']*'/u,
				() =>
					`$env:ORCHBEE_AGENT_HOOK_ROOT='${Buffer.from(path.join(root, 'missing')).toString('base64')}'`,
			);
		const missing = {
			...original,
			command: original.command.replace(encoded, Buffer.from(script, 'utf16le').toString('base64')),
		};
		await run(
			missing,
			JSON.stringify({
				session_id: 'missing-runtime-session',
				conversationId: 'missing-runtime-session',
				hook_event_name: events[0],
			}),
		);
	}

	assert.equal(observations.length, jsonProvider ? 1 : 4);
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
