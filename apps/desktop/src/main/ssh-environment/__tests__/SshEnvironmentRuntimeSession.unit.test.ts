import { PassThrough } from 'node:stream';

import {
	SSH_ENVIRONMENT_MAX_FRAME_BYTES,
	SSH_ENVIRONMENT_PROTOCOL_VERSION,
	SSH_PROJECT_ROOT_SERVICE_ID,
	SshEnvironmentBuildDigestSchema,
	SshEnvironmentFrameDecoder,
	SshEnvironmentRuntimeEpochSchema,
	SshEnvironmentRuntimeInstanceIdSchema,
	SshEnvironmentRuntimeVersionSchema,
	encodeSshEnvironmentMessage,
} from '@orchbee/ssh-environment-protocol';
import { describe, expect, it, vi } from 'vitest';

import { SshEnvironmentRuntimeSession } from '../SshEnvironmentRuntimeSession';
import { SSH_RUNTIME_BOOTSTRAP_STATUS_LINES } from '../ssh-runtime-bootstrap-negotiation';
import type { VerifiedSshRuntimeReleaseCatalog } from '../verify-ssh-runtime-artifact';

type RuntimeSessionOptions = ConstructorParameters<typeof SshEnvironmentRuntimeSession>[0];

const runtimeInstanceId = SshEnvironmentRuntimeInstanceIdSchema.parse('1'.repeat(32));
const runtimeEpoch = SshEnvironmentRuntimeEpochSchema.parse('2'.repeat(32));
const runtimeVersion = SshEnvironmentRuntimeVersionSchema.parse('0.1.0');
const buildDigest = SshEnvironmentBuildDigestSchema.parse('3'.repeat(64));
const linuxBuildDigest = SshEnvironmentBuildDigestSchema.parse('4'.repeat(64));
const artifactBytes = Uint8Array.of(9, 8, 7, 6);
const catalog: VerifiedSshRuntimeReleaseCatalog = {
	artifacts: {
		'darwin-arm64': {
			buildDigest,
			platform: 'darwin-arm64',
			runtimeVersion,
			sizeBytes: artifactBytes.byteLength,
		},
		'linux-x64': {
			buildDigest: linuxBuildDigest,
			platform: 'linux-x64',
			runtimeVersion,
			sizeBytes: 1,
		},
	},
	runtimeVersion,
};

describe('SSH Environment Runtime session', () => {
	it('uploads a requested verified artifact before Hello and heartbeats the exact epoch', async () => {
		const harness = createSessionHarness();
		await harness.session.start();
		expect(harness.output).toHaveLength(0);

		harness.readable.write(`${SSH_RUNTIME_BOOTSTRAP_STATUS_LINES['darwin-arm64'].upload}\n`);
		await vi.waitFor(() => expect(Buffer.concat(harness.output).byteLength).toBeGreaterThan(4));
		const initialOutput = Buffer.concat(harness.output);
		expect(initialOutput.subarray(0, artifactBytes.byteLength)).toEqual(Buffer.from(artifactBytes));
		const initialDecoder = new SshEnvironmentFrameDecoder();
		const initialFrames = initialDecoder.push(initialOutput.subarray(artifactBytes.byteLength));
		expect(initialFrames.ok).toBe(true);
		if (!initialFrames.ok) throw new Error('invalid initial test frame');
		expect(initialFrames.messages).toHaveLength(1);
		expect(initialFrames.messages[0]).toMatchObject({
			payload: { runtimeInstanceId },
			type: 'hello',
		});
		expect(harness.loadArtifact).toHaveBeenCalledWith('darwin-arm64');

		harness.readable.write(helloAccepted());
		expect(harness.onReady).toHaveBeenCalledWith({ runtimeEpoch, runtimeInstanceId });
		const allOutput = Buffer.concat(harness.output);
		const decoder = new SshEnvironmentFrameDecoder();
		const frames = decoder.push(allOutput.subarray(artifactBytes.byteLength));
		expect(frames.ok).toBe(true);
		if (!frames.ok) throw new Error('invalid heartbeat test frame');
		expect(frames.messages.at(-1)).toEqual({
			payload: { runtimeEpoch, sequence: 1 },
			type: 'heartbeat',
		});

		harness.readable.write(
			encodeSshEnvironmentMessage({
				type: 'heartbeat-acknowledged',
				payload: { runtimeEpoch, sequence: 1 },
			}),
		);
		harness.readable.end();
		await new Promise((resolve) => setImmediate(resolve));
		expect(harness.onLost).toHaveBeenCalledOnce();
		expect(harness.onFailure).not.toHaveBeenCalled();
	});

	it('uses the installed fast path without loading or sending artifact bytes', async () => {
		const harness = createSessionHarness();
		await harness.session.start();
		harness.readable.write(`${SSH_RUNTIME_BOOTSTRAP_STATUS_LINES['darwin-arm64'].present}\n`);
		await vi.waitFor(() => expect(harness.output).not.toHaveLength(0));

		expect(harness.loadArtifact).not.toHaveBeenCalled();
		const decoder = new SshEnvironmentFrameDecoder();
		const decoded = decoder.push(Buffer.concat(harness.output));
		expect(decoded.ok).toBe(true);
		if (!decoded.ok) throw new Error('invalid fast-path Hello frame');
		expect(decoded.messages).toHaveLength(1);
		expect(decoded.messages[0]?.type).toBe('hello');
		harness.session.stop();
	});

	it('fails closed on malformed, duplicate, oversized, or status-plus-extra input', async () => {
		for (const status of [
			'OBSI/1 darwin-arm64 unknown\n',
			`${SSH_RUNTIME_BOOTSTRAP_STATUS_LINES['darwin-arm64'].present}\nextra`,
			`${'x'.repeat(129)}\n`,
		]) {
			const harness = createSessionHarness();
			await harness.session.start();
			harness.readable.write(status);
			expect(harness.onFailure).toHaveBeenCalledWith('policy-rejected');
		}
	});

	it('reports a locally unavailable release only when upload is requested', async () => {
		const harness = createSessionHarness({ artifactAvailable: false });
		await harness.session.start();
		harness.readable.write(`${SSH_RUNTIME_BOOTSTRAP_STATUS_LINES['darwin-arm64'].upload}\n`);
		await vi.waitFor(() =>
			expect(harness.onFailure).toHaveBeenCalledWith('runtime-release-unavailable'),
		);
		expect(harness.output).toHaveLength(0);
	});

	it('fails closed when the Runtime identity does not match the selected catalog entry', async () => {
		const harness = createSessionHarness();
		await startInstalledSession(harness);
		harness.readable.write(
			helloAccepted({ buildDigest: SshEnvironmentBuildDigestSchema.parse('5'.repeat(64)) }),
		);
		expect(harness.onFailure).toHaveBeenCalledWith('policy-rejected');
	});

	it('treats a competing retained attachment as retryable', async () => {
		const harness = createSessionHarness();
		await startInstalledSession(harness);
		harness.readable.write(
			encodeSshEnvironmentMessage({
				type: 'failure',
				payload: { code: 'runtime-attachment-conflict' },
			}),
		);
		expect(harness.onFailure).toHaveBeenCalledWith('retryable');
	});

	it('classifies an EOF with a partial frame as a protocol-policy failure', async () => {
		const harness = createSessionHarness();
		await startInstalledSession(harness);
		harness.readable.end(helloAccepted().slice(0, 10));
		await new Promise((resolve) => setImmediate(resolve));
		expect(harness.onFailure).toHaveBeenCalledWith('policy-rejected');
	});

	it('does not duplicate an outstanding heartbeat before the miss bound retires it', async () => {
		vi.useFakeTimers();
		const harness = createSessionHarness();
		try {
			await harness.session.start();
			harness.readable.write(`${SSH_RUNTIME_BOOTSTRAP_STATUS_LINES['darwin-arm64'].present}\n`);
			await vi.advanceTimersByTimeAsync(0);
			harness.readable.write(helloAccepted());
			await vi.advanceTimersByTimeAsync(30_000);
			const decoder = new SshEnvironmentFrameDecoder();
			const frames = decoder.push(Buffer.concat(harness.output));
			expect(frames.ok).toBe(true);
			if (!frames.ok) throw new Error('invalid heartbeat frames');
			expect(frames.messages.filter((message) => message.type === 'heartbeat')).toHaveLength(1);

			await vi.advanceTimersByTimeAsync(15_000);
			expect(harness.onLost).toHaveBeenCalledOnce();
		} finally {
			harness.session.stop();
			vi.useRealTimers();
		}
	});
});

function createSessionHarness(options: { readonly artifactAvailable?: boolean } = {}) {
	const readable = new PassThrough();
	const writable = new PassThrough();
	const output: Buffer[] = [];
	writable.on('data', (chunk: Buffer) => output.push(chunk));
	const loadArtifact = vi.fn<RuntimeSessionOptions['loadArtifact']>().mockResolvedValue(
		options.artifactAvailable === false
			? { kind: 'unavailable' }
			: {
					artifact: {
						buildDigest,
						bytes: artifactBytes,
						platform: 'darwin-arm64',
						runtimeVersion,
					},
					kind: 'ready',
				},
	);
	const onFailure = vi.fn<RuntimeSessionOptions['onFailure']>();
	const onLost = vi.fn<RuntimeSessionOptions['onLost']>();
	const onReady = vi.fn<RuntimeSessionOptions['onReady']>();
	const session = new SshEnvironmentRuntimeSession({
		catalog,
		loadArtifact,
		onFailure,
		onLost,
		onReady,
		readable,
		runtimeInstanceId,
		writable,
	});
	return { loadArtifact, onFailure, onLost, onReady, output, readable, session };
}

async function startInstalledSession(harness: ReturnType<typeof createSessionHarness>) {
	await harness.session.start();
	harness.readable.write(`${SSH_RUNTIME_BOOTSTRAP_STATUS_LINES['darwin-arm64'].present}\n`);
	await vi.waitFor(() => expect(harness.output).not.toHaveLength(0));
}

function helloAccepted(overrides: { readonly buildDigest?: typeof buildDigest } = {}) {
	return encodeSshEnvironmentMessage({
		type: 'hello-accepted',
		payload: {
			protocolVersion: SSH_ENVIRONMENT_PROTOCOL_VERSION,
			runtimeInstanceId,
			runtimeEpoch,
			runtimeVersion,
			buildDigest: overrides.buildDigest ?? buildDigest,
			platform: 'darwin-arm64',
			capabilities: [SSH_PROJECT_ROOT_SERVICE_ID],
			heartbeatIntervalMs: 15_000,
			heartbeatMissLimit: 3,
			maxFrameBytes: SSH_ENVIRONMENT_MAX_FRAME_BYTES,
			initialChannelCreditBytes: 65_536,
		},
	});
}
