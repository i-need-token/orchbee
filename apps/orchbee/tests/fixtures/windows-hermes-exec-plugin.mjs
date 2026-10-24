// Real Python import/callbacks and Node pipe transport; no Hermes Agent or authentication.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { createServer } from 'node:net';
import path from 'node:path';
const directory = process.argv[2];
const pipe = String.raw`\\.\pipe\orchbee-${randomBytes(16).toString('hex')}-observation`;
const observed = [];
const sockets = new Set();
let child;
const server = createServer({ allowHalfOpen: true }, (socket) => {
	sockets.add(socket);
	socket.on('close', () => sockets.delete(socket));
	socket.on('error', () => {});
	let bytes = Buffer.alloc(0);
	let received = false;
	socket.on('data', (part) => {
		if (received) return;
		bytes = Buffer.concat([bytes, part]);
		if (bytes.length > 65536) return socket.destroy();
		try {
			const value = JSON.parse(bytes.toString());
			received = true;
			observed.push(value);
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
const program = String.raw`
import importlib.util, pathlib, sys
root=pathlib.Path(sys.argv[1])
spec=importlib.util.spec_from_file_location('orchbee_hermes_fixture',root/'__init__.py')
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
class Context:
    def __init__(self): self.hooks={}
    def register_hook(self,name,callback):
        assert name not in self.hooks
        self.hooks[name]=callback
ctx=Context()
assert module.register(ctx) is None
assert set(ctx.hooks)=={'on_session_start','on_session_reset','pre_llm_call'}
for event,callback in ctx.hooks.items():
    for platform in ['cli','tui','desktop','acp','gateway',None,[]]:
        assert callback(platform=platform,session_id='hermes-session',messages='PRIVATE_CONTENT',prompt='PRIVATE_PROMPT') is None
    for session in ['',None,'bad/session','x'*129]:
        assert callback(platform='cli',session_id=session) is None
`;
try {
	const output = await new Promise((resolve, reject) => {
		child = spawn('python.exe', ['-c', program, directory], {
			windowsHide: true,
			env: {
				...process.env,
				PATH: path.dirname(process.execPath) + ';' + process.env.PATH,
				ORCHBEE_AGENT_OBSERVATION_SOCKET: pipe,
			},
			stdio: ['ignore', 'pipe', 'pipe'],
		});
		const data = [];
		child.stdout.on('data', (v) => data.push(v));
		child.stderr.on('data', (v) => data.push(v));
		const timer = setTimeout(() => child.kill(), 15000);
		child.once('error', (error) => {
			clearTimeout(timer);
			reject(error);
		});
		child.once('close', (code, signal) => {
			clearTimeout(timer);
			if (code !== 0 || signal !== null)
				reject(new Error(Buffer.concat(data).toString() || `Python exited ${code}/${signal}`));
			else resolve(Buffer.concat(data).toString());
		});
	});
	assert.equal(output, '');
	assert.equal(observed.length, 9);
	for (const item of observed) {
		assert.equal(item.adapterId, 'hermes');
		assert.equal(item.observation.event, 'session-identified');
		assert.equal(item.observation.sessionKey, 'hermes-session');
		assert.ok(!JSON.stringify(item).includes('PRIVATE'));
	}
	process.stdout.write(JSON.stringify({ events: observed.length, silent: true }));
} finally {
	if (child && child.exitCode === null && child.signalCode === null)
		await new Promise((resolve) => {
			child.once('close', resolve);
			child.kill();
		});
	for (const socket of sockets) socket.destroy();
	await new Promise((resolve) => server.close(resolve));
}
