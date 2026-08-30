import { PassThrough } from 'node:stream';

import {
	SSH_ENVIRONMENT_MAX_FRAME_BYTES,
	SSH_ENVIRONMENT_PROTOCOL_VERSION,
	SSH_ENVIRONMENT_REQUIRED_CAPABILITIES,
	SshEnvironmentBuildDigestSchema,
	SshEnvironmentFrameDecoder,
	SshEnvironmentReleaseCohortDigestSchema,
	SshEnvironmentRuntimeEpochSchema,
	SshEnvironmentRuntimeInstanceIdSchema,
	SshEnvironmentRuntimeVersionSchema,
	SshEnvironmentTerminalHostVersionSchema,
	encodeSshEnvironmentMessage,
} from '@orchbee/ssh-environment-protocol';
import { describe, expect, it, vi } from 'vitest';

import { SshEnvironmentRuntimeSession } from '../SshEnvironmentRuntimeSession';
import { SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES } from '../ssh-environment-bootstrap-negotiation';
import type { VerifiedSshEnvironmentReleaseCatalog } from '../verify-ssh-environment-release';

type RuntimeSessionOptions = ConstructorParameters<typeof SshEnvironmentRuntimeSession>[0];

const runtimeInstanceId = SshEnvironmentRuntimeInstanceIdSchema.parse('1'.repeat(32));
const runtimeEpoch = SshEnvironmentRuntimeEpochSchema.parse('2'.repeat(32));
const runtimeVersion = SshEnvironmentRuntimeVersionSchema.parse('0.2.0');
const terminalHostVersion = SshEnvironmentTerminalHostVersionSchema.parse('0.1.0');
const runtimeBuildDigest = SshEnvironmentBuildDigestSchema.parse('3'.repeat(64));
const terminalHostBuildDigest = SshEnvironmentBuildDigestSchema.parse('4'.repeat(64));
const linuxRuntimeBuildDigest = SshEnvironmentBuildDigestSchema.parse('5'.repeat(64));
const linuxTerminalHostBuildDigest = SshEnvironmentBuildDigestSchema.parse('6'.repeat(64));
const releaseCohortDigest = SshEnvironmentReleaseCohortDigestSchema.parse('7'.repeat(64));
const runtimeArtifactBytes = Uint8Array.of(9, 8, 7, 6);
const terminalHostArtifactBytes = Uint8Array.of(5, 4, 3);
const catalog: VerifiedSshEnvironmentReleaseCatalog = {
	components: {
		'ssh-environment-runtime': {
			artifacts: {
				'darwin-arm64': {
					buildDigest: runtimeBuildDigest,
					platform: 'darwin-arm64',
					sizeBytes: runtimeArtifactBytes.byteLength,
				},
				'linux-x64': {
					buildDigest: linuxRuntimeBuildDigest,
					platform: 'linux-x64',
					sizeBytes: 1,
				},
			},
			role: 'ssh-environment-runtime',
			version: runtimeVersion,
		},
		'terminal-host': {
			artifacts: {
				'darwin-arm64': {
					buildDigest: terminalHostBuildDigest,
					platform: 'darwin-arm64',
					sizeBytes: terminalHostArtifactBytes.byteLength,
				},
				'linux-x64': {
					buildDigest: linuxTerminalHostBuildDigest,
					platform: 'linux-x64',
					sizeBytes: 1,
				},
			},
			role: 'terminal-host',
			version: terminalHostVersion,
		},
	},
	releaseCohortDigest,
};

describe('SSH Environment Runtime session', () => {
	it('uploads the requested cohort in fixed order before Hello and heartbeats the exact epoch', async () => {
		const harness = createSessionHarness();
		await harness.session.start();
		expect(harness.output).toHaveLength(0);

		writeBootstrapCohort(harness, 'upload', 'upload');
		const artifactLength = runtimeArtifactBytes.byteLength + terminalHostArtifactBytes.byteLength;
		await vi.waitFor(() =>
			expect(Buffer.concat(harness.output).byteLength).toBeGreaterThan(artifactLength),
		);
		const initialOutput = Buffer.concat(harness.output);
		expect(initialOutput.subarray(0, runtimeArtifactBytes.byteLength)).toEqual(
			Buffer.from(runtimeArtifactBytes),
		);
		expect(initialOutput.subarray(runtimeArtifactBytes.byteLength, artifactLength)).toEqual(
			Buffer.from(terminalHostArtifactBytes),
		);
		expect(harness.loadArtifact.mock.calls).toEqual([
			['ssh-environment-runtime', 'darwin-arm64'],
			['terminal-host', 'darwin-arm64'],
		]);
		const initialDecoder = new SshEnvironmentFrameDecoder();
		const initialFrames = initialDecoder.push(initialOutput.subarray(artifactLength));
		expect(initialFrames.ok).toBe(true);
		if (!initialFrames.ok) throw new Error('invalid initial test frame');
		expect(initialFrames.messages).toHaveLength(1);
		expect(initialFrames.messages[0]).toMatchObject({
			payload: { runtimeInstanceId },
			type: 'hello',
		});

		harness.readable.write(helloAccepted());
		expect(harness.onReady).toHaveBeenCalledWith({
			loginShell: '/bin/zsh',
			runtimeEpoch,
			runtimeInstanceId,
		});
		const allOutput = Buffer.concat(harness.output);
		const decoder = new SshEnvironmentFrameDecoder();
		const frames = decoder.push(allOutput.subarray(artifactLength));
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

	it('uses the committed installed-cohort fast path without loading artifacts', async () => {
		const harness = createSessionHarness();
		await harness.session.start();
		writeBootstrapCohort(harness, 'present', 'present');
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

	it('fails closed on malformed, reordered, duplicate, oversized, or post-commit bootstrap input', async () => {
		for (const status of [
			'OBSI/1 darwin-arm64 unknown\n',
			`${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64']['terminal-host'].present}\n`,
			`${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64']['ssh-environment-runtime'].present}\n${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64']['ssh-environment-runtime'].present}\n`,
			`${'x'.repeat(129)}\n`,
			`${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64']['ssh-environment-runtime'].present}\n${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64']['terminal-host'].present}\n${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64'].committed}\nextra`,
		]) {
			const harness = createSessionHarness();
			await harness.session.start();
			harness.readable.write(status);
			await vi.waitFor(() => expect(harness.onFailure).toHaveBeenCalledWith('policy-rejected'));
		}
	});

	it('reports a locally unavailable release only when an artifact upload is requested', async () => {
		const harness = createSessionHarness({ artifactAvailable: false });
		await harness.session.start();
		harness.readable.write(
			`${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64']['ssh-environment-runtime'].upload}\n`,
		);
		await vi.waitFor(() =>
			expect(harness.onFailure).toHaveBeenCalledWith('runtime-release-unavailable'),
		);
		expect(harness.output).toHaveLength(0);
	});

	it('fails closed when any negotiated cohort identity does not match the catalog', async () => {
		const harness = createSessionHarness();
		await startInstalledSession(harness);
		harness.readable.write(
			helloAccepted({
				runtimeBuildDigest: SshEnvironmentBuildDigestSchema.parse('8'.repeat(64)),
			}),
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
			writeBootstrapCohort(harness, 'present', 'present');
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
	const loadArtifact = vi.fn<RuntimeSessionOptions['loadArtifact']>(async (role, platform) => {
		if (options.artifactAvailable === false) return { kind: 'unavailable' };
		const component = catalog.components[role];
		return {
			artifact: {
				buildDigest: component.artifacts[platform].buildDigest,
				bytes:
					role === 'ssh-environment-runtime' ? runtimeArtifactBytes : terminalHostArtifactBytes,
				platform,
				role,
				version: component.version,
			},
			kind: 'ready',
		};
	});
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

function writeBootstrapCohort(
	harness: ReturnType<typeof createSessionHarness>,
	runtimeDisposition: 'present' | 'upload',
	terminalHostDisposition: 'present' | 'upload',
): void {
	harness.readable.write(
		[
			SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64']['ssh-environment-runtime'][
				runtimeDisposition
			],
			SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64']['terminal-host'][
				terminalHostDisposition
			],
			SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64'].committed,
		].join('\n') + '\n',
	);
}

async function startInstalledSession(harness: ReturnType<typeof createSessionHarness>) {
	await harness.session.start();
	writeBootstrapCohort(harness, 'present', 'present');
	await vi.waitFor(() => expect(harness.output).not.toHaveLength(0));
}

function helloAccepted(
	overrides: { readonly runtimeBuildDigest?: typeof runtimeBuildDigest } = {},
): Uint8Array {
	return encodeSshEnvironmentMessage({
		type: 'hello-accepted',
		payload: {
			protocolVersion: SSH_ENVIRONMENT_PROTOCOL_VERSION,
			runtimeInstanceId,
			runtimeEpoch,
			runtimeVersion,
			runtimeBuildDigest: overrides.runtimeBuildDigest ?? runtimeBuildDigest,
			releaseCohortDigest,
			loginShell: '/bin/zsh',
			terminalHostVersion,
			terminalHostBuildDigest,
			platform: 'darwin-arm64',
			capabilities: [...SSH_ENVIRONMENT_REQUIRED_CAPABILITIES],
			heartbeatIntervalMs: 15_000,
			heartbeatMissLimit: 3,
			maxFrameBytes: SSH_ENVIRONMENT_MAX_FRAME_BYTES,
			initialChannelCreditBytes: 65_536,
		},
	});
}
