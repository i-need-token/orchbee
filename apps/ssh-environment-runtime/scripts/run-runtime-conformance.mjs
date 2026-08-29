import assert from 'node:assert/strict';
import { once } from 'node:events';
import { access, mkdir, mkdtemp, realpath, rm, stat, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

import {
	SSH_ENVIRONMENT_MAX_FRAME_BYTES,
	SSH_ENVIRONMENT_PROTOCOL_VERSION,
	SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES,
	SSH_PROJECT_ROOT_SERVICE_ID,
	SshEnvironmentFrameDecoder,
	SshEnvironmentChannelIdSchema,
	SshProjectRootInspectionRequestSchema,
	decodeSshProjectRootInspectionResult,
	encodeSshProjectRootInspectionRequest,
	encodeSshEnvironmentMessage,
} from '../../../packages/ssh-environment-protocol/dist/index.js';

const appDirectory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const runtimeExecutable = path.join(
	appDirectory,
	'zig-out',
	'bin',
	'orchbee-ssh-environment-runtime',
);
const runtimeInstanceId = '1234567890abcdef1234567890abcdef';
const runtimeBuildDigest = '0'.repeat(64);
const runtimeDirectory = await mkdtemp(path.join(tmpdir(), 'orchbee-ssh-runtime-'));
const endpointPath = path.join(runtimeDirectory, `${runtimeInstanceId}.s`);
const projectRoot = path.join(runtimeDirectory, 'project-root');
const projectRootLink = path.join(runtimeDirectory, 'project-root-link');
const projectFile = path.join(runtimeDirectory, 'project-file');
const children = new Set();

try {
	await mkdir(projectRoot);
	await symlink(projectRoot, projectRootLink);
	await writeFile(projectFile, 'not a directory');
	const canonicalProjectRoot = await realpath(projectRoot);
	const firstServer = await startServer();
	const firstAttachment = startAttachment('first');
	const firstAccepted = await hello(firstAttachment, '1'.repeat(32));
	assert.equal(firstAccepted.type, 'hello-accepted');
	assert.equal(firstAccepted.payload.runtimeInstanceId, runtimeInstanceId);
	assert.deepEqual(firstAccepted.payload.capabilities, [SSH_PROJECT_ROOT_SERVICE_ID]);
	assert.equal(firstAccepted.payload.platform, 'darwin-arm64');
	assert.equal(firstAccepted.payload.buildDigest, runtimeBuildDigest);

	firstAttachment.write(
		encodeSshEnvironmentMessage({
			type: 'heartbeat',
			payload: { runtimeEpoch: firstAccepted.payload.runtimeEpoch, sequence: 17 },
		}),
	);
	assert.deepEqual(await firstAttachment.nextMessage(), {
		type: 'heartbeat-acknowledged',
		payload: { runtimeEpoch: firstAccepted.payload.runtimeEpoch, sequence: 17 },
	});

	assert.deepEqual(await inspectProjectRoot(firstAttachment, firstAccepted, 1, projectRootLink), {
		kind: 'resolved',
		canonicalPath: canonicalProjectRoot,
	});
	assert.deepEqual(
		await inspectProjectRoot(firstAttachment, firstAccepted, 2, `${projectRoot}/missing`),
		{ kind: 'failed', reason: 'not-found' },
	);
	assert.deepEqual(await inspectProjectRoot(firstAttachment, firstAccepted, 3, projectFile), {
		kind: 'failed',
		reason: 'not-directory',
	});
	assert.deepEqual(await sendInvalidProjectRootRequest(firstAttachment, firstAccepted, 4), {
		kind: 'failed',
		reason: 'invalid-path',
	});
	await cancelProjectRootChannel(firstAttachment, firstAccepted, 5);

	const competingAttachment = startAttachment('competing');
	competingAttachment.write(helloFrame('2'.repeat(32)));
	assert.deepEqual(await competingAttachment.nextMessage(), {
		type: 'failure',
		payload: { code: 'runtime-attachment-conflict' },
	});
	await competingAttachment.stop();

	await firstAttachment.stop();
	await waitFor(() => firstServer.exitCode === null);
	await delay(100);

	const reattached = startAttachment('reattached');
	const reattachedAccepted = await hello(reattached, '3'.repeat(32));
	assert.equal(reattachedAccepted.type, 'hello-accepted');
	assert.equal(
		reattachedAccepted.payload.runtimeEpoch,
		firstAccepted.payload.runtimeEpoch,
		'an attachment loss must not restart the Runtime daemon',
	);
	await reattached.stop();
	await stopChild(firstServer);

	const restartedServer = await startServer();
	const restartedAttachment = startAttachment('restarted');
	const restartedAccepted = await hello(restartedAttachment, '4'.repeat(32));
	assert.equal(restartedAccepted.type, 'hello-accepted');
	assert.notEqual(
		restartedAccepted.payload.runtimeEpoch,
		firstAccepted.payload.runtimeEpoch,
		'a restarted Runtime daemon must report a new epoch',
	);
	await restartedAttachment.stop();
	await stopChild(restartedServer);

	const directoryStatus = await stat(runtimeDirectory);
	assert.equal(directoryStatus.mode & 0o777, 0o700);
} finally {
	for (const child of children) {
		if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
	}
	await rm(runtimeDirectory, { recursive: true, force: true });
}

function spawnRuntime(arguments_, stdio) {
	const child = spawn(runtimeExecutable, arguments_, {
		cwd: appDirectory,
		stdio,
	});
	children.add(child);
	child.once('close', () => children.delete(child));
	return child;
}

async function startServer() {
	let previousEndpointIdentity;
	try {
		const previousStatus = await stat(endpointPath);
		previousEndpointIdentity = `${previousStatus.dev}:${previousStatus.ino}`;
	} catch {
		previousEndpointIdentity = undefined;
	}
	const child = spawnRuntime(
		[
			'serve',
			'--runtime-directory',
			runtimeDirectory,
			'--instance-id',
			runtimeInstanceId,
			'--build-digest',
			runtimeBuildDigest,
		],
		['ignore', 'ignore', 'pipe'],
	);
	let diagnostics = '';
	child.stderr.setEncoding('utf8');
	child.stderr.on('data', (chunk) => {
		diagnostics = `${diagnostics}${chunk}`.slice(-8192);
		process.stderr.write(chunk);
	});
	await waitFor(async () => {
		if (child.exitCode !== null) {
			throw new Error(`Runtime server exited during startup: ${diagnostics}`);
		}
		try {
			await access(endpointPath);
			const endpointStatus = await stat(endpointPath);
			return `${endpointStatus.dev}:${endpointStatus.ino}` !== previousEndpointIdentity;
		} catch {
			return false;
		}
	});
	const endpointStatus = await stat(endpointPath);
	assert.equal(endpointStatus.mode & 0o777, 0o600);
	return child;
}

function startAttachment(label) {
	const child = spawnRuntime(
		['attach', '--runtime-directory', runtimeDirectory, '--instance-id', runtimeInstanceId],
		['pipe', 'pipe', 'pipe'],
	);
	const decoder = new SshEnvironmentFrameDecoder();
	const messages = [];
	const waiters = [];
	let diagnostics = '';
	let terminalError;
	child.stderr.setEncoding('utf8');
	child.stderr.on('data', (chunk) => {
		diagnostics = `${diagnostics}${chunk}`.slice(-8192);
	});
	child.stdout.on('data', (chunk) => {
		const result = decoder.push(chunk);
		if (!result.ok) {
			terminalError = new Error(`invalid Runtime frame: ${result.error.code}`);
			flushWaiters();
			return;
		}
		for (const message of result.messages) {
			const waiter = waiters.shift();
			if (waiter) waiter.resolve(message);
			else messages.push(message);
		}
	});
	child.once('close', (code, signal) => {
		if (!terminalError && messages.length === 0) {
			terminalError = new Error(
				`Runtime attachment ${label} closed (${code ?? signal ?? 'unknown'}): ${diagnostics}`,
			);
		}
		flushWaiters();
	});

	function flushWaiters() {
		while (terminalError && waiters.length > 0) waiters.shift().reject(terminalError);
	}

	return {
		write(bytes) {
			child.stdin.write(bytes);
		},
		nextMessage() {
			if (messages.length > 0) return Promise.resolve(messages.shift());
			if (terminalError) return Promise.reject(terminalError);
			return withTimeout(
				new Promise((resolve, reject) => waiters.push({ resolve, reject })),
				5_000,
				'waiting for Runtime frame',
			);
		},
		async stop() {
			child.stdin.destroy();
			if (child.exitCode === null && child.signalCode === null) child.kill('SIGTERM');
			if (child.exitCode === null && child.signalCode === null) await once(child, 'close');
		},
	};
}

function helloFrame(clientNonce) {
	return encodeSshEnvironmentMessage({
		type: 'hello',
		payload: {
			protocolVersion: SSH_ENVIRONMENT_PROTOCOL_VERSION,
			runtimeInstanceId,
			clientNonce,
			maxFrameBytes: SSH_ENVIRONMENT_MAX_FRAME_BYTES,
			initialChannelCreditBytes: 65_536,
		},
	});
}

async function hello(attachment, clientNonce) {
	attachment.write(helloFrame(clientNonce));
	return attachment.nextMessage();
}

async function inspectProjectRoot(attachment, accepted, channelIdValue, requestedPath) {
	const channelId = SshEnvironmentChannelIdSchema.parse(channelIdValue);
	await openProjectRootChannel(attachment, accepted, channelId);
	const request = SshProjectRootInspectionRequestSchema.parse({
		type: 'inspect-project-root',
		path: requestedPath,
	});
	attachment.write(
		encodeSshEnvironmentMessage({
			type: 'channel-data',
			channelId,
			data: encodeSshProjectRootInspectionRequest(request),
		}),
	);
	const result = await nextProjectRootResult(attachment, channelId);
	await closeProjectRootChannel(attachment, accepted, channelId, 'completed');
	return result;
}

async function sendInvalidProjectRootRequest(attachment, accepted, channelIdValue) {
	const channelId = SshEnvironmentChannelIdSchema.parse(channelIdValue);
	await openProjectRootChannel(attachment, accepted, channelId);
	attachment.write(
		encodeSshEnvironmentMessage({
			type: 'channel-data',
			channelId,
			data: new TextEncoder().encode(
				JSON.stringify({ type: 'inspect-project-root', path: 'relative/project' }),
			),
		}),
	);
	const result = await nextProjectRootResult(attachment, channelId);
	await closeProjectRootChannel(attachment, accepted, channelId, 'completed');
	return result;
}

async function cancelProjectRootChannel(attachment, accepted, channelIdValue) {
	const channelId = SshEnvironmentChannelIdSchema.parse(channelIdValue);
	await openProjectRootChannel(attachment, accepted, channelId);
	await closeProjectRootChannel(attachment, accepted, channelId, 'cancelled');
}

async function openProjectRootChannel(attachment, accepted, channelId) {
	attachment.write(
		encodeSshEnvironmentMessage({
			type: 'channel-open',
			payload: {
				runtimeEpoch: accepted.payload.runtimeEpoch,
				channelId,
				serviceId: SSH_PROJECT_ROOT_SERVICE_ID,
				initialCreditBytes: SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES,
			},
		}),
	);
	assert.deepEqual(await attachment.nextMessage(), {
		type: 'channel-opened',
		payload: {
			runtimeEpoch: accepted.payload.runtimeEpoch,
			channelId,
			initialCreditBytes: SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES,
		},
	});
}

async function nextProjectRootResult(attachment, channelId) {
	const message = await attachment.nextMessage();
	assert.equal(message.type, 'channel-data');
	assert.equal(message.channelId, channelId);
	const decoded = decodeSshProjectRootInspectionResult(message.data);
	assert.equal(decoded.ok, true);
	return decoded.message;
}

async function closeProjectRootChannel(attachment, accepted, channelId, reason) {
	attachment.write(
		encodeSshEnvironmentMessage({
			type: 'channel-close',
			payload: { runtimeEpoch: accepted.payload.runtimeEpoch, channelId, reason },
		}),
	);
	assert.deepEqual(await attachment.nextMessage(), {
		type: 'channel-closed',
		payload: { runtimeEpoch: accepted.payload.runtimeEpoch, channelId, reason },
	});
}

async function stopChild(child) {
	if (child.exitCode !== null || child.signalCode !== null) return;
	child.kill('SIGTERM');
	await withTimeout(once(child, 'close'), 5_000, 'stopping Runtime server');
}

async function waitFor(predicate) {
	const deadline = Date.now() + 5_000;
	while (Date.now() < deadline) {
		if (await predicate()) return;
		await delay(10);
	}
	throw new Error('timed out waiting for Runtime state');
}

function delay(milliseconds) {
	return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function withTimeout(promise, milliseconds, description) {
	let timeout;
	try {
		return await Promise.race([
			promise,
			new Promise((_, reject) => {
				timeout = setTimeout(() => reject(new Error(`timed out ${description}`)), milliseconds);
			}),
		]);
	} finally {
		clearTimeout(timeout);
	}
}
