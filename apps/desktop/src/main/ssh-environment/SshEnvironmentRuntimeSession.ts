import { randomBytes } from 'node:crypto';
import type { Readable, Writable } from 'node:stream';

import {
	SSH_ENVIRONMENT_MAX_FRAME_BYTES,
	SSH_ENVIRONMENT_PROTOCOL_VERSION,
	SSH_ENVIRONMENT_REQUIRED_CAPABILITIES,
	SshEnvironmentChannelIdSchema,
	SshEnvironmentClientNonceSchema,
	SshEnvironmentFrameDecoder,
	encodeSshEnvironmentMessage,
	type SshEnvironmentChannelId,
	type SshEnvironmentFailure,
	type SshEnvironmentHelloAccepted,
	type SshEnvironmentPlatform,
	type SshEnvironmentReleaseComponentRole,
	type SshEnvironmentRuntimeEpoch,
	type SshEnvironmentRuntimeInstanceId,
} from '@orchbee/ssh-environment-protocol';

import {
	SshProjectRootChannelClient,
	type SshProjectRootDirectoryListingOutcome,
	type SshProjectRootInspectionOutcome,
} from './SshProjectRootChannelClient';
import {
	SshTerminalHostChannelClient,
	type SshTerminalHostConnectionOutcome,
} from './SshTerminalHostChannelClient';
import {
	SshEnvironmentBootstrapDecoder,
	type SshEnvironmentBootstrapArtifactStatus,
	type SshEnvironmentBootstrapMessage,
} from './ssh-environment-bootstrap-negotiation';
import type { SshEnvironmentArtifactLoadResult } from './ssh-environment-release-source';
import type { VerifiedSshEnvironmentReleaseCatalog } from './verify-ssh-environment-release';

const BOOTSTRAP_TIMEOUT_MILLISECONDS = 10 * 60 * 1_000;
const HANDSHAKE_TIMEOUT_MILLISECONDS = 15_000;
const INITIAL_CHANNEL_CREDIT_BYTES = 65_536;
const BOOTSTRAP_COMPONENT_ORDER = Object.freeze([
	'ssh-environment-runtime',
	'terminal-host',
] as const satisfies readonly SshEnvironmentReleaseComponentRole[]);

export type SshEnvironmentRuntimeSessionFailureReason =
	| 'compatibility-unsupported'
	| 'policy-rejected'
	| 'retryable'
	| 'runtime-release-unavailable';

export interface SshEnvironmentRuntimeIdentity {
	readonly runtimeEpoch: SshEnvironmentRuntimeEpoch;
	readonly runtimeInstanceId: SshEnvironmentRuntimeInstanceId;
}

interface SshEnvironmentRuntimeSessionOptions {
	readonly catalog: VerifiedSshEnvironmentReleaseCatalog;
	readonly loadArtifact: (
		role: SshEnvironmentReleaseComponentRole,
		platform: SshEnvironmentPlatform,
	) => Promise<SshEnvironmentArtifactLoadResult>;
	readonly onFailure: (reason: SshEnvironmentRuntimeSessionFailureReason) => void;
	readonly onLost: () => void;
	readonly onReady: (identity: SshEnvironmentRuntimeIdentity) => void;
	readonly readable: Readable;
	readonly runtimeInstanceId: SshEnvironmentRuntimeInstanceId;
	readonly writable: Writable;
}

type SessionPhase = 'bootstrapping' | 'handshaking' | 'idle' | 'ready' | 'stopped';

export class SshEnvironmentRuntimeSession {
	readonly #bootstrapDecoder = new SshEnvironmentBootstrapDecoder();
	readonly #bootstrapMessages: SshEnvironmentBootstrapMessage[] = [];
	#bootstrapPlatform: SshEnvironmentPlatform | undefined;
	#bootstrapProcessing = false;
	#bootstrapRoleIndex = 0;
	#bootstrapTimeout: NodeJS.Timeout | undefined;
	readonly #catalog: VerifiedSshEnvironmentReleaseCatalog;
	readonly #decoder = new SshEnvironmentFrameDecoder();
	#heartbeatInterval: NodeJS.Timeout | undefined;
	#heartbeatMisses = 0;
	#handshakeTimeout: NodeJS.Timeout | undefined;
	#helloAccepted: SshEnvironmentHelloAccepted | undefined;
	readonly #loadArtifact: SshEnvironmentRuntimeSessionOptions['loadArtifact'];
	#nextChannelId = 1;
	#nextHeartbeatSequence = 1;
	readonly #onFailure: SshEnvironmentRuntimeSessionOptions['onFailure'];
	readonly #onLost: SshEnvironmentRuntimeSessionOptions['onLost'];
	readonly #onReady: SshEnvironmentRuntimeSessionOptions['onReady'];
	#pendingHeartbeatSequence: number | undefined;
	#phase: SessionPhase = 'idle';
	#projectRootChannels: SshProjectRootChannelClient | undefined;
	readonly #readable: Readable;
	readonly #runtimeInstanceId: SshEnvironmentRuntimeInstanceId;
	#terminalHostChannels: SshTerminalHostChannelClient | undefined;
	readonly #writable: Writable;

	constructor(options: SshEnvironmentRuntimeSessionOptions) {
		this.#catalog = options.catalog;
		this.#loadArtifact = options.loadArtifact;
		this.#onFailure = options.onFailure;
		this.#onLost = options.onLost;
		this.#onReady = options.onReady;
		this.#readable = options.readable;
		this.#runtimeInstanceId = options.runtimeInstanceId;
		this.#writable = options.writable;
	}

	async start(): Promise<void> {
		if (this.#phase !== 'idle') throw new Error('SSH Runtime session has already started.');
		this.#phase = 'bootstrapping';
		this.#readable.on('data', this.#handleData);
		this.#readable.once('end', this.#handleTransportEnd);
		this.#readable.once('error', this.#handleTransportError);
		this.#writable.once('error', this.#handleTransportError);
		this.#bootstrapTimeout = setTimeout(
			() => this.#fail('retryable'),
			BOOTSTRAP_TIMEOUT_MILLISECONDS,
		);
	}

	stop(): void {
		if (this.#phase === 'stopped') return;
		this.#phase = 'stopped';
		this.#projectRootChannels?.retire();
		this.#projectRootChannels = undefined;
		this.#terminalHostChannels?.retire();
		this.#terminalHostChannels = undefined;
		this.#clearTimers();
		this.#readable.off('data', this.#handleData);
		this.#readable.off('end', this.#handleTransportEnd);
		this.#readable.off('error', this.#handleTransportError);
		this.#writable.off('error', this.#handleTransportError);
	}

	inspectProjectRoot(path: string, signal?: AbortSignal): Promise<SshProjectRootInspectionOutcome> {
		if (this.#phase !== 'ready' || this.#projectRootChannels === undefined) {
			return Promise.resolve({ kind: 'failed', reason: 'unavailable' });
		}
		return this.#projectRootChannels.inspect(path, signal);
	}

	listProjectDirectory(
		path: string | null,
		signal?: AbortSignal,
	): Promise<SshProjectRootDirectoryListingOutcome> {
		if (this.#phase !== 'ready' || this.#projectRootChannels === undefined) {
			return Promise.resolve({ kind: 'failed', reason: 'unavailable' });
		}
		return this.#projectRootChannels.list(path, signal);
	}

	openTerminalHost(): Promise<SshTerminalHostConnectionOutcome> {
		if (this.#phase !== 'ready' || this.#terminalHostChannels === undefined) {
			return Promise.resolve({ kind: 'failed', reason: 'unavailable' });
		}
		return this.#terminalHostChannels.open();
	}

	readonly #handleData = (chunk: Buffer): void => {
		if (this.#phase === 'stopped') return;
		if (this.#phase === 'bootstrapping') {
			const decoded = this.#bootstrapDecoder.push(chunk);
			if (decoded.kind === 'rejected') {
				this.#fail('policy-rejected');
				return;
			}
			this.#bootstrapMessages.push(...decoded.messages);
			void this.#drainBootstrapMessages();
			return;
		}
		const decoded = this.#decoder.push(chunk);
		if (!decoded.ok) {
			this.#fail('policy-rejected');
			return;
		}
		for (const message of decoded.messages) {
			if (this.#phase === 'handshaking') {
				if (message.type === 'failure') {
					this.#fail(classifyRuntimeFailure(message.payload.code));
					return;
				}
				if (message.type !== 'hello-accepted' || !this.#acceptHello(message.payload)) {
					this.#fail('policy-rejected');
					return;
				}
				continue;
			}
			if (this.#phase !== 'ready') return;
			if (message.type === 'heartbeat-acknowledged') {
				if (
					message.payload.runtimeEpoch !== this.#helloAccepted?.runtimeEpoch ||
					message.payload.sequence !== this.#pendingHeartbeatSequence
				) {
					this.#fail('policy-rejected');
					return;
				}
				this.#pendingHeartbeatSequence = undefined;
				this.#heartbeatMisses = 0;
				continue;
			}
			const terminalHostHandling = this.#terminalHostChannels?.handle(message) ?? 'unhandled';
			if (terminalHostHandling === 'attachment-invalid') {
				this.#fail('policy-rejected');
				return;
			}
			if (terminalHostHandling === 'handled') continue;
			const projectRootHandling = this.#projectRootChannels?.handle(message) ?? 'unhandled';
			if (projectRootHandling !== 'handled') {
				this.#fail('policy-rejected');
				return;
			}
		}
	};

	readonly #handleTransportEnd = (): void => {
		const wasReady = this.#phase === 'ready';
		if (this.#phase === 'stopped') return;
		if ((this.#phase === 'handshaking' || this.#phase === 'ready') && !this.#decoder.finish().ok) {
			this.#fail('policy-rejected');
			return;
		}
		this.stop();
		if (wasReady) this.#onLost();
		else this.#onFailure('retryable');
	};

	readonly #handleTransportError = (): void => {
		this.#handleTransportEnd();
	};

	async #drainBootstrapMessages(): Promise<void> {
		if (this.#bootstrapProcessing || this.#phase !== 'bootstrapping') return;
		this.#bootstrapProcessing = true;
		try {
			while (this.#phase === 'bootstrapping') {
				const message = this.#bootstrapMessages.shift();
				if (message === undefined) return;
				if (message.type === 'artifact-status') {
					if (!(await this.#acceptBootstrapArtifactStatus(message))) return;
					continue;
				}
				if (
					this.#bootstrapRoleIndex !== BOOTSTRAP_COMPONENT_ORDER.length ||
					message.platform !== this.#bootstrapPlatform ||
					this.#bootstrapMessages.length !== 0
				) {
					this.#fail('policy-rejected');
					return;
				}
				await this.#beginHandshake();
			}
		} catch {
			this.#fail('retryable');
		} finally {
			this.#bootstrapProcessing = false;
		}
	}

	async #acceptBootstrapArtifactStatus(
		status: SshEnvironmentBootstrapArtifactStatus,
	): Promise<boolean> {
		const expectedRole = BOOTSTRAP_COMPONENT_ORDER[this.#bootstrapRoleIndex];
		if (
			expectedRole === undefined ||
			status.role !== expectedRole ||
			(this.#bootstrapPlatform !== undefined && status.platform !== this.#bootstrapPlatform)
		) {
			this.#fail('policy-rejected');
			return false;
		}
		this.#bootstrapPlatform = status.platform;
		const component = this.#catalog.components[status.role];
		const descriptor = component.artifacts[status.platform];
		if (status.artifactDisposition === 'upload') {
			const loaded = await this.#loadArtifact(status.role, status.platform);
			if (this.#phase === 'stopped') return false;
			if (loaded.kind === 'unavailable') {
				this.#fail('runtime-release-unavailable');
				return false;
			}
			if (
				loaded.artifact.buildDigest !== descriptor.buildDigest ||
				loaded.artifact.bytes.byteLength !== descriptor.sizeBytes ||
				loaded.artifact.platform !== descriptor.platform ||
				loaded.artifact.role !== component.role ||
				loaded.artifact.version !== component.version
			) {
				this.#fail('policy-rejected');
				return false;
			}
			await this.#write(loaded.artifact.bytes);
		}
		this.#bootstrapRoleIndex += 1;
		return true;
	}

	async #beginHandshake(): Promise<void> {
		if (this.#bootstrapTimeout !== undefined) clearTimeout(this.#bootstrapTimeout);
		this.#bootstrapTimeout = undefined;
		this.#phase = 'handshaking';
		this.#handshakeTimeout = setTimeout(
			() => this.#fail('retryable'),
			HANDSHAKE_TIMEOUT_MILLISECONDS,
		);
		const clientNonce = SshEnvironmentClientNonceSchema.parse(randomBytes(16).toString('hex'));
		await this.#write(
			encodeSshEnvironmentMessage({
				type: 'hello',
				payload: {
					protocolVersion: SSH_ENVIRONMENT_PROTOCOL_VERSION,
					runtimeInstanceId: this.#runtimeInstanceId,
					clientNonce,
					maxFrameBytes: SSH_ENVIRONMENT_MAX_FRAME_BYTES,
					initialChannelCreditBytes: INITIAL_CHANNEL_CREDIT_BYTES,
				},
			}),
		);
	}

	#acceptHello(accepted: SshEnvironmentHelloAccepted): boolean {
		const platform = this.#bootstrapPlatform;
		if (platform === undefined) return false;
		const runtime = this.#catalog.components['ssh-environment-runtime'];
		const terminalHost = this.#catalog.components['terminal-host'];
		const runtimeArtifact = runtime.artifacts[platform];
		const terminalHostArtifact = terminalHost.artifacts[platform];
		if (
			accepted.runtimeInstanceId !== this.#runtimeInstanceId ||
			accepted.runtimeVersion !== runtime.version ||
			accepted.runtimeBuildDigest !== runtimeArtifact.buildDigest ||
			accepted.releaseCohortDigest !== this.#catalog.releaseCohortDigest ||
			accepted.terminalHostVersion !== terminalHost.version ||
			accepted.terminalHostBuildDigest !== terminalHostArtifact.buildDigest ||
			accepted.platform !== platform ||
			accepted.capabilities.length !== SSH_ENVIRONMENT_REQUIRED_CAPABILITIES.length ||
			!accepted.capabilities.every(
				(capability, index) => capability === SSH_ENVIRONMENT_REQUIRED_CAPABILITIES[index],
			) ||
			accepted.maxFrameBytes !== SSH_ENVIRONMENT_MAX_FRAME_BYTES ||
			accepted.initialChannelCreditBytes !== INITIAL_CHANNEL_CREDIT_BYTES
		) {
			return false;
		}
		this.#helloAccepted = accepted;
		this.#phase = 'ready';
		const channelOptions = {
			allocateChannelId: () => this.#allocateChannelId(),
			onAttachmentInvalid: () => this.#fail('policy-rejected'),
			runtimeEpoch: accepted.runtimeEpoch,
			write: (bytes: Uint8Array) => this.#write(bytes),
		};
		this.#projectRootChannels = new SshProjectRootChannelClient(channelOptions);
		this.#terminalHostChannels = new SshTerminalHostChannelClient({
			...channelOptions,
			expectedHostVersion: terminalHost.version,
		});
		if (this.#handshakeTimeout !== undefined) clearTimeout(this.#handshakeTimeout);
		this.#handshakeTimeout = undefined;
		this.#heartbeatInterval = setInterval(
			() => this.#heartbeatTick(),
			accepted.heartbeatIntervalMs,
		);
		this.#onReady({
			runtimeEpoch: accepted.runtimeEpoch,
			runtimeInstanceId: accepted.runtimeInstanceId,
		});
		this.#heartbeatTick();
		return true;
	}

	#allocateChannelId(): SshEnvironmentChannelId | null {
		const parsed = SshEnvironmentChannelIdSchema.safeParse(this.#nextChannelId);
		if (!parsed.success) return null;
		this.#nextChannelId += 1;
		return parsed.data;
	}

	#heartbeatTick(): void {
		if (this.#phase !== 'ready' || this.#helloAccepted === undefined) return;
		if (this.#pendingHeartbeatSequence !== undefined) {
			this.#heartbeatMisses += 1;
			if (this.#heartbeatMisses >= this.#helloAccepted.heartbeatMissLimit) {
				this.#fail('retryable');
			}
			return;
		}
		this.#pendingHeartbeatSequence = this.#nextHeartbeatSequence;
		this.#nextHeartbeatSequence =
			this.#nextHeartbeatSequence === 0xffff_ffff ? 0 : this.#nextHeartbeatSequence + 1;
		void this.#write(
			encodeSshEnvironmentMessage({
				type: 'heartbeat',
				payload: {
					runtimeEpoch: this.#helloAccepted.runtimeEpoch,
					sequence: this.#pendingHeartbeatSequence,
				},
			}),
		).catch(() => this.#fail('retryable'));
	}

	#fail(reason: SshEnvironmentRuntimeSessionFailureReason): void {
		if (this.#phase === 'stopped') return;
		const wasReady = this.#phase === 'ready';
		this.stop();
		if (wasReady) this.#onLost();
		else this.#onFailure(reason);
	}

	#clearTimers(): void {
		if (this.#bootstrapTimeout !== undefined) clearTimeout(this.#bootstrapTimeout);
		if (this.#handshakeTimeout !== undefined) clearTimeout(this.#handshakeTimeout);
		if (this.#heartbeatInterval !== undefined) clearInterval(this.#heartbeatInterval);
		this.#bootstrapTimeout = undefined;
		this.#handshakeTimeout = undefined;
		this.#heartbeatInterval = undefined;
	}

	#write(bytes: Uint8Array): Promise<void> {
		return new Promise((resolve, reject) => {
			this.#writable.write(bytes, (error: Error | null | undefined) => {
				if (error) reject(error);
				else resolve();
			});
		});
	}
}

function classifyRuntimeFailure(
	code: SshEnvironmentFailure['code'],
): SshEnvironmentRuntimeSessionFailureReason {
	if (code === 'compatibility-unsupported' || code === 'protocol-version-unsupported') {
		return 'compatibility-unsupported';
	}
	return code === 'runtime-attachment-conflict' ? 'retryable' : 'policy-rejected';
}
