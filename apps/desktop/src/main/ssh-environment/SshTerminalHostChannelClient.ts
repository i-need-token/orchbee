import { randomBytes } from 'node:crypto';
import { Duplex } from 'node:stream';

import {
	SSH_TERMINAL_HOST_INITIAL_CREDIT_BYTES,
	SSH_TERMINAL_HOST_MAX_CHUNK_BYTES,
	SSH_TERMINAL_HOST_SERVICE_ID,
	encodeSshEnvironmentMessage,
	type SshEnvironmentChannelClose,
	type SshEnvironmentChannelId,
	type SshEnvironmentMessage,
	type SshEnvironmentRuntimeEpoch,
	type SshEnvironmentTerminalHostVersion,
} from '@orchbee/ssh-environment-protocol';
import { ClientIdSchema } from '@orchbee/terminal-protocol';

import { HostConnection } from '../host';

const OPEN_TIMEOUT_MILLISECONDS = 15_000;

type RelayCloseReason = SshEnvironmentChannelClose['reason'];

export type SshTerminalHostConnectionOutcome =
	| { readonly connection: HostConnection; readonly kind: 'ready' }
	| {
			readonly kind: 'failed';
			readonly reason: 'busy' | 'compatibility-unsupported' | 'unavailable';
	  };

interface SshTerminalHostChannelClientOptions {
	readonly allocateChannelId: () => SshEnvironmentChannelId | null;
	readonly expectedHostVersion: SshEnvironmentTerminalHostVersion;
	readonly onAttachmentInvalid: () => void;
	readonly runtimeEpoch: SshEnvironmentRuntimeEpoch;
	readonly write: (bytes: Uint8Array) => Promise<void>;
}

interface RelayChannel {
	closeReason?: RelayCloseReason;
	connection?: HostConnection;
	desktopCreditBytes: number;
	id: SshEnvironmentChannelId;
	phase: 'closing' | 'negotiating' | 'opening' | 'ready';
	resolve: (outcome: SshTerminalHostConnectionOutcome) => void;
	settled: boolean;
	stream?: SshTerminalHostDuplex;
	timeout: NodeJS.Timeout;
}

type ChannelHandleResult = 'attachment-invalid' | 'handled' | 'unhandled';

export class SshTerminalHostChannelClient {
	readonly #allocateChannelId: () => SshEnvironmentChannelId | null;
	#channel: RelayChannel | undefined;
	readonly #expectedHostVersion: SshEnvironmentTerminalHostVersion;
	readonly #onAttachmentInvalid: () => void;
	#retired = false;
	readonly #runtimeEpoch: SshEnvironmentRuntimeEpoch;
	readonly #write: (bytes: Uint8Array) => Promise<void>;

	constructor(options: SshTerminalHostChannelClientOptions) {
		this.#allocateChannelId = options.allocateChannelId;
		this.#expectedHostVersion = options.expectedHostVersion;
		this.#onAttachmentInvalid = options.onAttachmentInvalid;
		this.#runtimeEpoch = options.runtimeEpoch;
		this.#write = options.write;
	}

	open(): Promise<SshTerminalHostConnectionOutcome> {
		if (this.#retired) return Promise.resolve(failed('unavailable'));
		if (this.#channel !== undefined) return Promise.resolve(failed('busy'));
		const channelId = this.#allocateChannelId();
		if (channelId === null) return Promise.resolve(failed('unavailable'));

		return new Promise((resolve) => {
			const channel: RelayChannel = {
				desktopCreditBytes: 0,
				id: channelId,
				phase: 'opening',
				resolve,
				settled: false,
				timeout: setTimeout(() => this.#openTimedOut(channelId), OPEN_TIMEOUT_MILLISECONDS),
			};
			this.#channel = channel;
			this.#send({
				type: 'channel-open',
				payload: {
					runtimeEpoch: this.#runtimeEpoch,
					channelId,
					serviceId: SSH_TERMINAL_HOST_SERVICE_ID,
					initialCreditBytes: SSH_TERMINAL_HOST_INITIAL_CREDIT_BYTES,
				},
			});
		});
	}

	handle(message: SshEnvironmentMessage): ChannelHandleResult {
		const channelId = channelIdOf(message);
		const channel = this.#channel;
		if (channelId === null || channel === undefined || channel.id !== channelId) {
			return 'unhandled';
		}
		switch (message.type) {
			case 'channel-opened':
				return this.#handleOpened(channel, message);
			case 'channel-data':
				return this.#handleData(channel, message.data);
			case 'channel-credit':
				return this.#handleCredit(channel, message.bytes);
			case 'channel-closed':
				return this.#handleClosed(channel, message.payload.reason);
			default:
				return 'attachment-invalid';
		}
	}

	retire(): void {
		if (this.#retired) return;
		this.#retired = true;
		const channel = this.#channel;
		if (channel === undefined) return;
		this.#settle(channel, failed('unavailable'));
		channel.stream?.retire(new Error('SSH Terminal Host relay attachment was retired.'));
		this.#release(channel);
	}

	#handleOpened(
		channel: RelayChannel,
		message: Extract<SshEnvironmentMessage, { readonly type: 'channel-opened' }>,
	): ChannelHandleResult {
		if (
			channel.phase !== 'opening' ||
			message.payload.runtimeEpoch !== this.#runtimeEpoch ||
			message.payload.initialCreditBytes !== SSH_TERMINAL_HOST_INITIAL_CREDIT_BYTES
		) {
			return 'attachment-invalid';
		}
		channel.desktopCreditBytes = message.payload.initialCreditBytes;
		channel.phase = 'negotiating';
		const stream = new SshTerminalHostDuplex({
			onDestroyed: (error) => this.#streamDestroyed(channel.id, error),
			onWrite: (bytes, callback) => this.#writeHostBytes(channel.id, bytes, callback),
		});
		const connection = new HostConnection(stream);
		channel.connection = connection;
		channel.stream = stream;
		void connection
			.negotiate(ClientIdSchema.parse(randomBytes(16).toString('hex')))
			.then((accepted) => {
				if (this.#channel !== channel || channel.phase !== 'negotiating') return;
				if (accepted.hostVersion !== this.#expectedHostVersion) {
					this.#settle(channel, failed('compatibility-unsupported'));
					this.#beginClose(channel, 'compatibility-unsupported');
					stream.retire(
						new Error('Terminal Host version is incompatible with the release cohort.'),
					);
					return;
				}
				channel.phase = 'ready';
				this.#settle(channel, { connection, kind: 'ready' });
			})
			.catch(() => {
				if (this.#channel !== channel || channel.phase === 'closing') return;
				this.#settle(channel, failed('unavailable'));
				this.#beginClose(channel, 'service-unavailable');
				stream.retire(new Error('Terminal Host negotiation failed.'));
			});
		return 'handled';
	}

	#handleData(channel: RelayChannel, data: Uint8Array): ChannelHandleResult {
		if (
			(channel.phase !== 'negotiating' && channel.phase !== 'ready') ||
			channel.stream === undefined ||
			data.byteLength === 0 ||
			data.byteLength > SSH_TERMINAL_HOST_MAX_CHUNK_BYTES
		) {
			return 'attachment-invalid';
		}
		if (!channel.stream.deliver(data)) return 'handled';
		this.#send({ type: 'channel-credit', channelId: channel.id, bytes: data.byteLength });
		return 'handled';
	}

	#handleCredit(channel: RelayChannel, bytes: number): ChannelHandleResult {
		if (
			(channel.phase !== 'negotiating' && channel.phase !== 'ready') ||
			bytes <= 0 ||
			channel.desktopCreditBytes + bytes > SSH_TERMINAL_HOST_INITIAL_CREDIT_BYTES
		) {
			return 'attachment-invalid';
		}
		channel.desktopCreditBytes += bytes;
		channel.stream?.resumeWrites();
		return 'handled';
	}

	#handleClosed(channel: RelayChannel, reason: RelayCloseReason): ChannelHandleResult {
		if (channel.phase === 'opening') {
			const failureReason = mapRemoteOpenFailure(reason);
			if (failureReason === null) return 'attachment-invalid';
			this.#settle(channel, failed(failureReason));
			this.#release(channel);
			return 'handled';
		}
		if (channel.phase === 'closing') {
			if (channel.closeReason !== reason) return 'attachment-invalid';
			channel.stream?.retire();
			this.#release(channel);
			return 'handled';
		}
		const failureReason = mapRemoteOpenFailure(reason);
		if (failureReason === null) return 'attachment-invalid';
		this.#settle(channel, failed(failureReason));
		channel.stream?.retire(new Error('Remote Terminal Host relay closed.'));
		this.#release(channel);
		return 'handled';
	}

	#writeHostBytes(
		channelId: SshEnvironmentChannelId,
		bytes: Uint8Array,
		callback: (error?: Error | null) => void,
	): void {
		const channel = this.#channel;
		if (
			channel === undefined ||
			channel.id !== channelId ||
			(channel.phase !== 'negotiating' && channel.phase !== 'ready')
		) {
			callback(new Error('SSH Terminal Host relay is unavailable.'));
			return;
		}
		if (bytes.byteLength === 0) {
			callback();
			return;
		}
		let offset = 0;
		const sendNext = (): void => {
			if (this.#channel !== channel || channel.phase === 'closing') {
				callback(new Error('SSH Terminal Host relay closed during write.'));
				return;
			}
			if (offset === bytes.byteLength) {
				callback();
				return;
			}
			if (channel.desktopCreditBytes === 0) {
				channel.stream?.waitForCredit(sendNext);
				return;
			}
			const length = Math.min(
				bytes.byteLength - offset,
				channel.desktopCreditBytes,
				SSH_TERMINAL_HOST_MAX_CHUNK_BYTES,
			);
			const data = bytes.subarray(offset, offset + length);
			channel.desktopCreditBytes -= length;
			offset += length;
			void this.#write(
				encodeSshEnvironmentMessage({ type: 'channel-data', channelId: channel.id, data }),
			).then(sendNext, (error: unknown) => {
				callback(error instanceof Error ? error : new Error('SSH relay write failed.'));
				this.#onAttachmentInvalid();
			});
		};
		sendNext();
	}

	#streamDestroyed(channelId: SshEnvironmentChannelId, error: Error | null): void {
		const channel = this.#channel;
		if (channel === undefined || channel.id !== channelId || channel.phase === 'closing') return;
		this.#beginClose(channel, error === null ? 'cancelled' : 'protocol-failure');
	}

	#openTimedOut(channelId: SshEnvironmentChannelId): void {
		const channel = this.#channel;
		if (channel === undefined || channel.id !== channelId || channel.settled) return;
		this.#settle(channel, failed('unavailable'));
		this.#beginClose(channel, 'cancelled');
		channel.stream?.retire(new Error('SSH Terminal Host relay opening timed out.'));
	}

	#beginClose(channel: RelayChannel, reason: RelayCloseReason): void {
		if (channel.phase === 'closing') return;
		channel.phase = 'closing';
		channel.closeReason = reason;
		this.#send({
			type: 'channel-close',
			payload: { runtimeEpoch: this.#runtimeEpoch, channelId: channel.id, reason },
		});
	}

	#settle(channel: RelayChannel, outcome: SshTerminalHostConnectionOutcome): void {
		if (channel.settled) return;
		channel.settled = true;
		clearTimeout(channel.timeout);
		channel.resolve(outcome);
	}

	#release(channel: RelayChannel): void {
		clearTimeout(channel.timeout);
		if (this.#channel === channel) this.#channel = undefined;
	}

	#send(message: SshEnvironmentMessage): void {
		void this.#write(encodeSshEnvironmentMessage(message)).catch(this.#onAttachmentInvalid);
	}
}

class SshTerminalHostDuplex extends Duplex {
	readonly #onDestroyed: (error: Error | null) => void;
	readonly #onWrite: (bytes: Uint8Array, callback: (error?: Error | null) => void) => void;
	#retiring = false;
	#resumeWrite: (() => void) | undefined;

	constructor(options: {
		readonly onDestroyed: (error: Error | null) => void;
		readonly onWrite: (bytes: Uint8Array, callback: (error?: Error | null) => void) => void;
	}) {
		super({ readableHighWaterMark: SSH_TERMINAL_HOST_INITIAL_CREDIT_BYTES });
		this.#onDestroyed = options.onDestroyed;
		this.#onWrite = options.onWrite;
	}

	deliver(bytes: Uint8Array): boolean {
		if (this.destroyed) return false;
		this.push(Buffer.from(bytes));
		return !this.destroyed;
	}

	resumeWrites(): void {
		const resume = this.#resumeWrite;
		this.#resumeWrite = undefined;
		resume?.();
	}

	waitForCredit(resume: () => void): void {
		if (this.#resumeWrite !== undefined) {
			this.destroy(new Error('SSH Terminal Host relay registered duplicate credit waiters.'));
			return;
		}
		this.#resumeWrite = resume;
	}

	retire(error?: Error): void {
		if (this.destroyed) return;
		this.#retiring = true;
		this.push(null);
		this.destroy(error);
	}

	override _read(): void {}

	override _write(
		chunk: Buffer,
		_encoding: BufferEncoding,
		callback: (error?: Error | null) => void,
	): void {
		this.#onWrite(chunk, callback);
	}

	override _destroy(error: Error | null, callback: (error?: Error | null) => void): void {
		this.#resumeWrite = undefined;
		if (!this.#retiring) this.#onDestroyed(error);
		callback(error);
	}
}

function channelIdOf(message: SshEnvironmentMessage): SshEnvironmentChannelId | null {
	switch (message.type) {
		case 'channel-data':
		case 'channel-credit':
			return message.channelId;
		case 'channel-opened':
		case 'channel-closed':
			return message.payload.channelId;
		default:
			return null;
	}
}

function mapRemoteOpenFailure(
	reason: RelayCloseReason,
): Extract<SshTerminalHostConnectionOutcome, { readonly kind: 'failed' }>['reason'] | null {
	switch (reason) {
		case 'service-busy':
			return 'busy';
		case 'compatibility-unsupported':
			return 'compatibility-unsupported';
		case 'service-unavailable':
		case 'attachment-lost':
			return 'unavailable';
		default:
			return null;
	}
}

function failed(
	reason: Extract<SshTerminalHostConnectionOutcome, { readonly kind: 'failed' }>['reason'],
): Extract<SshTerminalHostConnectionOutcome, { readonly kind: 'failed' }> {
	return { kind: 'failed', reason };
}
