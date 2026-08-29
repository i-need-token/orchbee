import { randomBytes } from 'node:crypto';
import type { Readable, Writable } from 'node:stream';

import {
	SSH_ENVIRONMENT_MAX_FRAME_BYTES,
	SSH_ENVIRONMENT_PROTOCOL_VERSION,
	SSH_PROJECT_ROOT_SERVICE_ID,
	SshEnvironmentClientNonceSchema,
	SshEnvironmentFrameDecoder,
	encodeSshEnvironmentMessage,
	type SshEnvironmentFailure,
	type SshEnvironmentHelloAccepted,
	type SshEnvironmentRuntimeEpoch,
	type SshEnvironmentRuntimeInstanceId,
} from '@orchbee/ssh-environment-protocol';

import {
	SshProjectRootChannelClient,
	type SshProjectRootInspectionOutcome,
} from './SshProjectRootChannelClient';
import type { SshRuntimeArtifactLoadResult } from './ssh-runtime-release-source';
import {
	SshRuntimeBootstrapStatusDecoder,
	type SshRuntimeBootstrapStatus,
} from './ssh-runtime-bootstrap-negotiation';
import type {
	VerifiedSshRuntimeArtifactDescriptor,
	VerifiedSshRuntimeReleaseCatalog,
} from './verify-ssh-runtime-artifact';

const BOOTSTRAP_TIMEOUT_MILLISECONDS = 10 * 60 * 1_000;
const HANDSHAKE_TIMEOUT_MILLISECONDS = 15_000;
const INITIAL_CHANNEL_CREDIT_BYTES = 65_536;

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
	readonly catalog: VerifiedSshRuntimeReleaseCatalog;
	readonly loadArtifact: (
		platform: SshRuntimeBootstrapStatus['platform'],
	) => Promise<SshRuntimeArtifactLoadResult>;
	readonly onFailure: (reason: SshEnvironmentRuntimeSessionFailureReason) => void;
	readonly onLost: () => void;
	readonly onReady: (identity: SshEnvironmentRuntimeIdentity) => void;
	readonly readable: Readable;
	readonly runtimeInstanceId: SshEnvironmentRuntimeInstanceId;
	readonly writable: Writable;
}

type SessionPhase =
	| 'handshaking'
	| 'idle'
	| 'preparing-artifact'
	| 'ready'
	| 'stopped'
	| 'waiting-bootstrap-status';

export class SshEnvironmentRuntimeSession {
	readonly #bootstrapDecoder = new SshRuntimeBootstrapStatusDecoder();
	#bootstrapTimeout: NodeJS.Timeout | undefined;
	readonly #catalog: VerifiedSshRuntimeReleaseCatalog;
	readonly #decoder = new SshEnvironmentFrameDecoder();
	#heartbeatInterval: NodeJS.Timeout | undefined;
	#heartbeatMisses = 0;
	#handshakeTimeout: NodeJS.Timeout | undefined;
	#helloAccepted: SshEnvironmentHelloAccepted | undefined;
	readonly #loadArtifact: SshEnvironmentRuntimeSessionOptions['loadArtifact'];
	#nextHeartbeatSequence = 1;
	readonly #onFailure: SshEnvironmentRuntimeSessionOptions['onFailure'];
	readonly #onLost: SshEnvironmentRuntimeSessionOptions['onLost'];
	readonly #onReady: SshEnvironmentRuntimeSessionOptions['onReady'];
	#pendingHeartbeatSequence: number | undefined;
	#phase: SessionPhase = 'idle';
	#projectRootChannels: SshProjectRootChannelClient | undefined;
	readonly #readable: Readable;
	readonly #runtimeInstanceId: SshEnvironmentRuntimeInstanceId;
	#selectedArtifact: VerifiedSshRuntimeArtifactDescriptor | undefined;
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
		this.#phase = 'waiting-bootstrap-status';
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

	readonly #handleData = (chunk: Buffer): void => {
		if (this.#phase === 'stopped') return;
		if (this.#phase === 'waiting-bootstrap-status') {
			const decoded = this.#bootstrapDecoder.push(chunk);
			if (decoded.kind === 'pending') return;
			if (decoded.kind === 'rejected') {
				this.#fail('policy-rejected');
				return;
			}
			this.#phase = 'preparing-artifact';
			void this.#completeBootstrap(decoded.status);
			return;
		}
		if (this.#phase === 'preparing-artifact') {
			this.#fail('policy-rejected');
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
			if (this.#projectRootChannels?.handle(message) !== 'handled') {
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

	async #completeBootstrap(status: SshRuntimeBootstrapStatus): Promise<void> {
		const selectedArtifact = this.#catalog.artifacts[status.platform];
		this.#selectedArtifact = selectedArtifact;
		try {
			if (status.artifactDisposition === 'upload') {
				const loaded = await this.#loadArtifact(status.platform);
				if (this.#phase === 'stopped') return;
				if (loaded.kind === 'unavailable') {
					this.#fail('runtime-release-unavailable');
					return;
				}
				if (
					loaded.artifact.buildDigest !== selectedArtifact.buildDigest ||
					loaded.artifact.bytes.byteLength !== selectedArtifact.sizeBytes ||
					loaded.artifact.platform !== selectedArtifact.platform ||
					loaded.artifact.runtimeVersion !== selectedArtifact.runtimeVersion
				) {
					this.#fail('policy-rejected');
					return;
				}
				await this.#write(loaded.artifact.bytes);
			}
			if (this.#phase !== 'preparing-artifact') return;
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
		} catch {
			this.#fail('retryable');
		}
	}

	#acceptHello(accepted: SshEnvironmentHelloAccepted): boolean {
		const selectedArtifact = this.#selectedArtifact;
		if (
			selectedArtifact === undefined ||
			accepted.runtimeInstanceId !== this.#runtimeInstanceId ||
			accepted.runtimeVersion !== selectedArtifact.runtimeVersion ||
			accepted.buildDigest !== selectedArtifact.buildDigest ||
			accepted.platform !== selectedArtifact.platform ||
			accepted.capabilities.length !== 1 ||
			accepted.capabilities[0] !== SSH_PROJECT_ROOT_SERVICE_ID ||
			accepted.maxFrameBytes !== SSH_ENVIRONMENT_MAX_FRAME_BYTES ||
			accepted.initialChannelCreditBytes !== INITIAL_CHANNEL_CREDIT_BYTES
		) {
			return false;
		}
		this.#helloAccepted = accepted;
		this.#phase = 'ready';
		this.#projectRootChannels = new SshProjectRootChannelClient({
			onAttachmentInvalid: () => this.#fail('policy-rejected'),
			runtimeEpoch: accepted.runtimeEpoch,
			write: (bytes) => this.#write(bytes),
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
