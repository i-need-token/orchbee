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
	SSH_ENVIRONMENT_REQUIRED_CAPABILITIES,
	SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES,
	SSH_PROJECT_ROOT_SERVICE_ID,
	SSH_TERMINAL_HOST_INITIAL_CREDIT_BYTES,
	SSH_TERMINAL_HOST_SERVICE_ID,
	SshEnvironmentFrameDecoder,
	SshEnvironmentChannelIdSchema,
	SshProjectRootDirectoryListingRequestSchema,
	SshProjectRootInspectionRequestSchema,
	decodeSshProjectRootDirectoryListingResult,
	decodeSshProjectRootInspectionResult,
	encodeSshProjectRootDirectoryListingRequest,
	encodeSshProjectRootInspectionRequest,
	encodeSshEnvironmentMessage,
} from '@orchbee/ssh-environment-protocol';
import {
	CURRENT_PROTOCOL_VERSION,
	ClientIdSchema,
	CorrelationIdSchema,
	HelloAcceptedSchema,
	HostEpochSchema,
	MessageKind,
	PROTOCOL_LIMITS,
	PayloadEncoding,
	decodeControlJson,
	decodeProtocolEnvelope,
	encodeControlRequest,
	encodeProtocolEnvelope,
} from '@orchbee/terminal-protocol';

const appDirectory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const runtimeExecutable = path.join(
	appDirectory,
	'zig-out',
	'bin',
	'orchbee-ssh-environment-runtime',
);
const terminalHostExecutable = path.resolve(
	appDirectory,
	'../terminal-host/zig-out/bin/orchbee-terminal-host',
);
const runtimeInstanceId = '1234567890abcdef1234567890abcdef';
const runtimeBuildDigest = '0'.repeat(64);
const releaseCohortDigest = '1'.repeat(64);
const terminalHostBuildDigest = '2'.repeat(64);
const terminalHostEndpointId = '3'.repeat(32);
const terminalHostVersion = '0.1.0';
const runtimeDirectory = await mkdtemp(path.join(tmpdir(), 'orchbee-ssh-runtime-'));
const terminalHostRuntimeDirectory = path.join(runtimeDirectory, 'terminal-host-state');
const endpointPath = path.join(runtimeDirectory, `${runtimeInstanceId}.s`);
const projectRoot = path.join(runtimeDirectory, 'project-root');
const projectRootLink = path.join(runtimeDirectory, 'project-root-link');
const projectFile = path.join(runtimeDirectory, 'project-file');
const children = new Set();

try {
	await mkdir(terminalHostRuntimeDirectory, { mode: 0o700 });
	await mkdir(projectRoot);
	await mkdir(path.join(projectRoot, '.hidden'));
	await mkdir(path.join(projectRoot, 'alpha'));
	await mkdir(path.join(projectRoot, 'zeta'));
	await symlink(path.join(projectRoot, 'alpha'), path.join(projectRoot, 'linked-dir'));
	await writeFile(path.join(projectRoot, 'file.txt'), 'not a directory');
	await symlink(projectRoot, projectRootLink);
	await writeFile(projectFile, 'not a directory');
	const canonicalProjectRoot = await realpath(projectRoot);
	const firstServer = await startServer();
	const firstAttachment = startAttachment('first');
	const firstAccepted = await hello(firstAttachment, '1'.repeat(32));
	assert.equal(firstAccepted.type, 'hello-accepted');
	assert.equal(firstAccepted.payload.runtimeInstanceId, runtimeInstanceId);
	assert.deepEqual(firstAccepted.payload.capabilities, [...SSH_ENVIRONMENT_REQUIRED_CAPABILITIES]);
	assert.equal(firstAccepted.payload.platform, nativeRuntimePlatform());
	assert.equal(firstAccepted.payload.runtimeBuildDigest, runtimeBuildDigest);
	assert.equal(firstAccepted.payload.releaseCohortDigest, releaseCohortDigest);
	assert.equal(firstAccepted.payload.terminalHostVersion, terminalHostVersion);
	assert.equal(firstAccepted.payload.terminalHostBuildDigest, terminalHostBuildDigest);

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
	assert.deepEqual(await listProjectDirectory(firstAttachment, firstAccepted, 4, null), {
		kind: 'listed',
		canonicalPath: canonicalProjectRoot,
		parentPath: path.dirname(canonicalProjectRoot),
		directories: [{ name: '.hidden' }, { name: 'alpha' }, { name: 'linked-dir' }, { name: 'zeta' }],
		truncated: false,
	});
	assert.deepEqual(await sendInvalidProjectRootRequest(firstAttachment, firstAccepted, 5), {
		kind: 'failed',
		reason: 'invalid-path',
	});
	await cancelProjectRootChannel(firstAttachment, firstAccepted, 6);
	await negotiateTerminalHost(firstAttachment, firstAccepted, 7);

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
	await stopChild(firstServer, 'SIGKILL');

	const restartingAttachment = startAttachment('restarting');
	await delay(100);
	const restartedServer = await startServer();
	const restartedAccepted = await hello(restartingAttachment, '4'.repeat(32));
	assert.equal(restartedAccepted.type, 'hello-accepted');
	assert.notEqual(
		restartedAccepted.payload.runtimeEpoch,
		firstAccepted.payload.runtimeEpoch,
		'an attachment waiting through a stale endpoint must reach the restarted Runtime epoch',
	);
	await restartingAttachment.stop();
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
		env: { ...process.env, HOME: projectRoot },
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
			'--release-cohort-digest',
			releaseCohortDigest,
			'--terminal-host-version',
			terminalHostVersion,
			'--terminal-host-build-digest',
			terminalHostBuildDigest,
			'--terminal-host-executable',
			terminalHostExecutable,
			'--terminal-host-runtime-directory',
			terminalHostRuntimeDirectory,
			'--terminal-host-endpoint-id',
			terminalHostEndpointId,
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

async function listProjectDirectory(attachment, accepted, channelIdValue, requestedPath) {
	const channelId = SshEnvironmentChannelIdSchema.parse(channelIdValue);
	await openProjectRootChannel(attachment, accepted, channelId);
	const request = SshProjectRootDirectoryListingRequestSchema.parse({
		type: 'list-project-directory',
		path: requestedPath,
	});
	attachment.write(
		encodeSshEnvironmentMessage({
			type: 'channel-data',
			channelId,
			data: encodeSshProjectRootDirectoryListingRequest(request),
		}),
	);
	const message = await attachment.nextMessage();
	assert.equal(message.type, 'channel-data');
	assert.equal(message.channelId, channelId);
	const decoded = decodeSshProjectRootDirectoryListingResult(message.data);
	assert.equal(decoded.ok, true);
	await closeProjectRootChannel(attachment, accepted, channelId, 'completed');
	return decoded.message;
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

async function negotiateTerminalHost(attachment, accepted, channelIdValue) {
	const channelId = SshEnvironmentChannelIdSchema.parse(channelIdValue);
	attachment.write(
		encodeSshEnvironmentMessage({
			type: 'channel-open',
			payload: {
				runtimeEpoch: accepted.payload.runtimeEpoch,
				channelId,
				serviceId: SSH_TERMINAL_HOST_SERVICE_ID,
				initialCreditBytes: SSH_TERMINAL_HOST_INITIAL_CREDIT_BYTES,
			},
		}),
	);
	assert.deepEqual(await attachment.nextMessage(), {
		type: 'channel-opened',
		payload: {
			runtimeEpoch: accepted.payload.runtimeEpoch,
			channelId,
			initialCreditBytes: SSH_TERMINAL_HOST_INITIAL_CREDIT_BYTES,
		},
	});

	const helloPayload = encodeControlRequest(MessageKind.hello, {
		clientId: ClientIdSchema.parse('4'.repeat(32)),
		frameBytes: PROTOCOL_LIMITS.maxFrameBytes,
		inputBytes: PROTOCOL_LIMITS.maxInputBytes,
		protocol: CURRENT_PROTOCOL_VERSION,
		rowWindowBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
		supportedOptionalFeatures: [],
	});
	assert.equal(helloPayload.ok, true);
	const correlationId = CorrelationIdSchema.parse('5'.repeat(32));
	const hostHello = encodeProtocolEnvelope({
		correlationId,
		event: false,
		hostEpoch: null,
		kind: MessageKind.hello,
		optionalMessage: false,
		payload: helloPayload.value,
		payloadEncoding: PayloadEncoding.controlJson,
		protocol: CURRENT_PROTOCOL_VERSION,
		response: false,
	});
	assert.equal(hostHello.ok, true);
	attachment.write(
		encodeSshEnvironmentMessage({
			type: 'channel-data',
			channelId,
			data: hostHello.value,
		}),
	);

	let creditedInputBytes = 0;
	let responseBytes = Buffer.alloc(0);
	let decodedResponse;
	while (creditedInputBytes < hostHello.value.byteLength || decodedResponse === undefined) {
		const message = await attachment.nextMessage();
		if (message.type === 'channel-credit') {
			assert.equal(message.channelId, channelId);
			creditedInputBytes += message.bytes;
			continue;
		}
		assert.equal(message.type, 'channel-data');
		assert.equal(message.channelId, channelId);
		responseBytes = Buffer.concat([responseBytes, message.data]);
		const decoded = decodeProtocolEnvelope(responseBytes);
		if (decoded.ok) decodedResponse = decoded.value;
	}
	assert.equal(creditedInputBytes, hostHello.value.byteLength);
	assert.equal(decodedResponse.kind, MessageKind.helloAccepted);
	assert.equal(decodedResponse.correlationId, correlationId);
	assert.equal(decodedResponse.response, true);
	const helloAccepted = decodeControlJson(decodedResponse.payload, HelloAcceptedSchema);
	assert.equal(helloAccepted.ok, true);
	assert.equal(helloAccepted.value.hostVersion, terminalHostVersion);
	assert.equal(HostEpochSchema.safeParse(helloAccepted.value.hostEpoch).success, true);
	attachment.write(
		encodeSshEnvironmentMessage({
			type: 'channel-credit',
			channelId,
			bytes: responseBytes.byteLength,
		}),
	);
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

async function stopChild(child, signal = 'SIGTERM') {
	if (child.exitCode !== null || child.signalCode !== null) return;
	child.kill(signal);
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

function nativeRuntimePlatform() {
	if (process.platform === 'darwin' && process.arch === 'arm64') return 'darwin-arm64';
	if (process.platform === 'linux' && process.arch === 'x64') return 'linux-x64';
	throw new Error(`unsupported Runtime conformance platform ${process.platform}-${process.arch}`);
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
