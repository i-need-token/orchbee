import { createHash, generateKeyPairSync, sign, type KeyObject } from 'node:crypto';
import { PassThrough } from 'node:stream';

import {
	SSH_ENVIRONMENT_MAX_FRAME_BYTES,
	SSH_ENVIRONMENT_PROTOCOL_VERSION,
	SSH_ENVIRONMENT_RELEASE_SCHEMA_VERSION,
	SSH_ENVIRONMENT_RELEASE_SIGNING_KEY_ID_PREFIX,
	SSH_ENVIRONMENT_RELEASE_TRUST_SCHEMA_VERSION,
	SSH_ENVIRONMENT_REQUIRED_CAPABILITIES,
	SshEnvironmentBuildDigestSchema,
	SshEnvironmentFrameDecoder,
	SshEnvironmentReleaseManifestSchema,
	SshEnvironmentReleaseTrustDescriptorSchema,
	SshEnvironmentReleaseUnsignedManifestSchema,
	SshEnvironmentRuntimeEpochSchema,
	SshEnvironmentRuntimeInstanceIdSchema,
	SshEnvironmentRuntimeVersionSchema,
	SshEnvironmentTerminalHostVersionSchema,
	createSshEnvironmentReleaseManifestSigningPayload,
	encodeSshEnvironmentMessage,
	encodeSshEnvironmentReleaseManifest,
} from '@orchbee/ssh-environment-protocol';
import { describe, expect, it, vi } from 'vitest';

import { OpenSshTargetResolver } from '../OpenSshTargetResolver';
import type { OpenSshTransportAdapter } from '../OpenSshTransportAdapter';
import type { SshEnvironmentReleaseSource } from '../ssh-environment-release-source';
import {
	SshEnvironmentSupervisor,
	type SshEnvironmentSupervisorDependencies,
} from '../SshEnvironmentSupervisor';
import { SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES } from '../ssh-environment-bootstrap-negotiation';
import { verifySshEnvironmentReleaseCatalog } from '../verify-ssh-environment-release';

const runtimeInstanceId = SshEnvironmentRuntimeInstanceIdSchema.parse('1'.repeat(32));
const runtimeEpoch = SshEnvironmentRuntimeEpochSchema.parse('2'.repeat(32));
const runtimeVersion = SshEnvironmentRuntimeVersionSchema.parse('0.2.0');
const terminalHostVersion = SshEnvironmentTerminalHostVersionSchema.parse('0.1.0');
const artifactBytes = Buffer.from('signed Runtime artifact');
const terminalHostArtifactBytes = Buffer.from('signed Terminal Host artifact');
const buildDigest = SshEnvironmentBuildDigestSchema.parse(
	createHash('sha256').update(artifactBytes).digest('hex'),
);
const terminalHostBuildDigest = SshEnvironmentBuildDigestSchema.parse(
	createHash('sha256').update(terminalHostArtifactBytes).digest('hex'),
);
type ResolverDependencies = NonNullable<ConstructorParameters<typeof OpenSshTargetResolver>[1]>;

describe('SSH Environment supervisor', () => {
	it('drives resolve, transient AskPass, signed install stream, Hello, and reconnect fencing', async () => {
		const signer = generateKeyPairSync('ed25519');
		const release = signedRelease(signer);
		const releaseSource = createReleaseSource(release);
		const readable = new PassThrough();
		const writable = new PassThrough();
		const output: Buffer[] = [];
		writable.on('data', (chunk: Buffer) => output.push(chunk));
		const retire = vi.fn<() => void>();
		let exitListener:
			| ((exit: { readonly code: null | number; readonly signal: null | NodeJS.Signals }) => void)
			| undefined;
		let openedOptions: Parameters<OpenSshTransportAdapter['open']>[0] | undefined;
		const open = vi.fn<OpenSshTransportAdapter['open']>((options) => {
			openedOptions = options;
			return {
				readable,
				writable,
				diagnostics: () => '',
				onExit: (listener: typeof exitListener) => {
					exitListener = listener;
				},
				retire,
			};
		});
		const interaction = vi
			.fn<SshEnvironmentSupervisorDependencies['interaction']>()
			.mockResolvedValue({
				kind: 'submitted' as const,
				response: 'one-use password',
			});
		const askPassDispose = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
		let askPassResult: unknown;
		const supervisor = new SshEnvironmentSupervisor(
			{
				askPassHelperEntryPath: '/tmp/ssh-askpass-helper.cjs',
				askPassRuntimeExecutablePath: '/usr/bin/node',
				connectionId: 'environment-1',
				runtimeInstanceId,
				target: { configurationFilePath: null, destination: 'build-host' },
			},
			{
				askPassBridgeFactory: (options) => ({
					dispose: askPassDispose,
					start: async () => {
						askPassResult = await options.interaction({
							handle: 'ssh-prompt-test',
							kind: 'none',
							prompt: '(builder@host) Password:',
						});
						return { environment: {} };
					},
				}),
				interaction,
				randomDelay: (minimum) => minimum,
				releaseSource,
				targetResolver: resolvedTargetResolver(),
				transportAdapter: { open },
			},
		);

		supervisor.start();
		await eventually(() => open.mock.calls.length === 1);
		expect(askPassResult).toEqual({ kind: 'submitted', response: 'one-use password' });
		expect(interaction).toHaveBeenCalledWith({
			generation: 1,
			handle: 'ssh-prompt-test',
			kind: 'none',
			prompt: '(builder@host) Password:',
		});
		expect(output).toHaveLength(0);
		readable.write(
			`${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64']['ssh-environment-runtime'].upload}\n`,
		);
		await eventually(() => Buffer.concat(output).byteLength >= artifactBytes.byteLength);
		readable.write(
			`${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64']['terminal-host'].upload}\n`,
		);
		const cohortArtifactLength = artifactBytes.byteLength + terminalHostArtifactBytes.byteLength;
		await eventually(() => Buffer.concat(output).byteLength >= cohortArtifactLength);
		readable.write(`${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64'].committed}\n`);
		await eventually(() => Buffer.concat(output).byteLength > cohortArtifactLength);
		const initialOutput = Buffer.concat(output);
		expect(initialOutput.subarray(0, artifactBytes.byteLength)).toEqual(artifactBytes);
		expect(initialOutput.subarray(artifactBytes.byteLength, cohortArtifactLength)).toEqual(
			terminalHostArtifactBytes,
		);
		const decoder = new SshEnvironmentFrameDecoder();
		const initialFrames = decoder.push(initialOutput.subarray(cohortArtifactLength));
		expect(initialFrames.ok).toBe(true);
		if (!initialFrames.ok) throw new Error('invalid supervisor Hello frame');
		expect(initialFrames.messages).toHaveLength(1);
		expect(initialFrames.messages[0]).toMatchObject({
			type: 'hello',
			payload: { runtimeInstanceId },
		});
		expect(openedOptions?.remoteCommand).not.toMatch(/\bcurl\b|\bwget\b/u);

		readable.write(
			encodeSshEnvironmentMessage({
				type: 'hello-accepted',
				payload: {
					protocolVersion: SSH_ENVIRONMENT_PROTOCOL_VERSION,
					runtimeInstanceId,
					runtimeEpoch,
					runtimeVersion,
					runtimeBuildDigest: buildDigest,
					releaseCohortDigest: releaseSource.releaseCohortDigest,
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
			}),
		);
		expect(supervisor.observation()).toMatchObject({
			connected: true,
			generation: 1,
			phase: 'connected',
			runtimeEpoch,
		});

		readable.end();
		await eventually(() => supervisor.observation().phase === 'retry-wait');
		expect(supervisor.observation()).toMatchObject({
			connected: false,
			generation: 2,
			phase: 'retry-wait',
			runtimeEpoch,
		});
		expect(retire).toHaveBeenCalledOnce();
		expect(askPassDispose).toHaveBeenCalledOnce();
		expect(exitListener).toBeDefined();
		await supervisor.stop();
		expect(supervisor.observation()).toMatchObject({ phase: 'stopped' });
	});

	it.each([
		{
			blockedReason: 'compatibility-unsupported' as const,
			code: 71,
			diagnostics: '',
			name: 'bootstrap compatibility',
		},
		{
			blockedReason: 'authentication-required' as const,
			code: 255,
			diagnostics: 'Permission denied (publickey).',
			name: 'OpenSSH authentication',
		},
	])('maps $name failure before the Runtime handshake', async (failure) => {
		const signer = generateKeyPairSync('ed25519');
		const release = signedRelease(signer);
		const releaseSource = createReleaseSource(release);
		const readable = new PassThrough();
		const writable = new PassThrough();
		writable.resume();
		let exitListener:
			| ((exit: { readonly code: null | number; readonly signal: null | NodeJS.Signals }) => void)
			| undefined;
		const supervisor = new SshEnvironmentSupervisor(
			{
				askPassHelperEntryPath: '/tmp/ssh-askpass-helper.cjs',
				askPassRuntimeExecutablePath: '/usr/bin/node',
				connectionId: 'environment-compatibility',
				runtimeInstanceId,
				target: { configurationFilePath: null, destination: 'build-host' },
			},
			{
				askPassBridgeFactory: () => ({
					dispose: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
					start: vi
						.fn<() => Promise<{ readonly environment: Readonly<NodeJS.ProcessEnv> }>>()
						.mockResolvedValue({ environment: {} }),
				}),
				interaction: vi.fn<SshEnvironmentSupervisorDependencies['interaction']>(),
				releaseSource,
				targetResolver: resolvedTargetResolver(),
				transportAdapter: {
					open: () => ({
						readable,
						writable,
						diagnostics: () => failure.diagnostics,
						onExit: (listener: typeof exitListener) => {
							exitListener = listener;
						},
						retire: vi.fn<() => void>(),
					}),
				},
			},
		);

		supervisor.start();
		await eventually(() => exitListener !== undefined);
		exitListener?.({ code: failure.code, signal: null });
		await eventually(() => supervisor.observation().phase === 'blocked');
		expect(supervisor.observation()).toMatchObject({
			blockedReason: failure.blockedReason,
			phase: 'blocked',
		});
		await supervisor.stop();
	});

	it('blocks before OpenSSH when packaged Runtime release resources are unavailable', async () => {
		const open = vi.fn<OpenSshTransportAdapter['open']>();
		const supervisor = new SshEnvironmentSupervisor(
			{
				askPassHelperEntryPath: '/tmp/ssh-askpass-helper.cjs',
				askPassRuntimeExecutablePath: '/usr/bin/node',
				connectionId: 'environment-release-unavailable',
				runtimeInstanceId,
				target: { configurationFilePath: null, destination: 'build-host' },
			},
			{
				interaction: vi.fn<SshEnvironmentSupervisorDependencies['interaction']>(),
				releaseSource: {
					loadArtifact: vi.fn<SshEnvironmentReleaseSource['loadArtifact']>(),
					loadCatalog: vi
						.fn<SshEnvironmentReleaseSource['loadCatalog']>()
						.mockResolvedValue({ kind: 'unavailable' }),
				},
				targetResolver: resolvedTargetResolver(),
				transportAdapter: { open },
			},
		);

		supervisor.start();
		await eventually(() => supervisor.observation().phase === 'blocked');
		expect(supervisor.observation()).toMatchObject({
			blockedReason: 'runtime-release-unavailable',
			phase: 'blocked',
		});
		expect(open).not.toHaveBeenCalled();
		await supervisor.stop();
	});
});

function resolvedTargetResolver(): OpenSshTargetResolver {
	return new OpenSshTargetResolver('/usr/bin/ssh', {
		isAbsolute: () => true,
		query: vi.fn<ResolverDependencies['query']>().mockResolvedValue({
			kind: 'completed',
			stdout: [
				'hostname 10.0.0.8',
				'user builder',
				'port 22',
				'stricthostkeychecking ask',
				'',
			].join('\n'),
		}),
	});
}

function signedRelease(signer: { readonly privateKey: KeyObject; readonly publicKey: KeyObject }) {
	const unsigned = SshEnvironmentReleaseUnsignedManifestSchema.parse({
		schemaVersion: SSH_ENVIRONMENT_RELEASE_SCHEMA_VERSION,
		components: [
			{
				role: 'ssh-environment-runtime',
				version: runtimeVersion,
				artifacts: [
					{
						platform: 'darwin-arm64' as const,
						sizeBytes: artifactBytes.byteLength,
						sha256: buildDigest,
					},
					{
						platform: 'linux-x64' as const,
						sizeBytes: 1,
						sha256: SshEnvironmentBuildDigestSchema.parse('0'.repeat(64)),
					},
				],
			},
			{
				role: 'terminal-host',
				version: terminalHostVersion,
				artifacts: [
					{
						platform: 'darwin-arm64' as const,
						sizeBytes: terminalHostArtifactBytes.byteLength,
						sha256: terminalHostBuildDigest,
					},
					{
						platform: 'linux-x64' as const,
						sizeBytes: 1,
						sha256: SshEnvironmentBuildDigestSchema.parse('2'.repeat(64)),
					},
				],
			},
		],
	});
	const manifest = SshEnvironmentReleaseManifestSchema.parse({
		...unsigned,
		signature: sign(
			null,
			createSshEnvironmentReleaseManifestSigningPayload(unsigned),
			signer.privateKey,
		).toString('base64url'),
	});
	const manifestBytes = encodeSshEnvironmentReleaseManifest(manifest);
	const publicKeySpki = signer.publicKey.export({ format: 'der', type: 'spki' });
	return {
		manifestBytes,
		trustDescriptor: SshEnvironmentReleaseTrustDescriptorSchema.parse({
			schemaVersion: SSH_ENVIRONMENT_RELEASE_TRUST_SCHEMA_VERSION,
			signingKeyId: `${SSH_ENVIRONMENT_RELEASE_SIGNING_KEY_ID_PREFIX}${createHash('sha256')
				.update(publicKeySpki)
				.digest('hex')}`,
			publicKeySpki: publicKeySpki.toString('base64url'),
			manifestSha256: SshEnvironmentBuildDigestSchema.parse(
				createHash('sha256').update(manifestBytes).digest('hex'),
			),
		}),
	};
}

function createReleaseSource(release: ReturnType<typeof signedRelease>) {
	const verification = verifySshEnvironmentReleaseCatalog({
		manifestBytes: release.manifestBytes,
		trustDescriptor: release.trustDescriptor,
	});
	if (verification.kind === 'failed') throw new Error('Expected a verified test release.');
	return {
		loadArtifact: vi.fn<SshEnvironmentReleaseSource['loadArtifact']>(async (role, platform) => {
			const component = verification.catalog.components[role];
			return {
				artifact: {
					buildDigest: component.artifacts[platform].buildDigest,
					bytes: role === 'ssh-environment-runtime' ? artifactBytes : terminalHostArtifactBytes,
					platform,
					role,
					version: component.version,
				},
				kind: 'ready',
			};
		}),
		loadCatalog: vi.fn<SshEnvironmentReleaseSource['loadCatalog']>().mockResolvedValue({
			catalog: verification.catalog,
			kind: 'ready',
		}),
		releaseCohortDigest: verification.catalog.releaseCohortDigest,
	};
}

async function eventually(predicate: () => boolean): Promise<void> {
	const deadline = Date.now() + 5_000;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((resolve) => setTimeout(resolve, 5));
	}
	throw new Error('timed out waiting for supervisor state');
}
