import {
	SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES,
	SSH_PROJECT_ROOT_MAX_MESSAGE_BYTES,
	SSH_PROJECT_ROOT_SERVICE_ID,
	SshProjectRootDirectoryListingRequestSchema,
	SshProjectRootInspectionRequestSchema,
	decodeSshProjectRootDirectoryListingResult,
	decodeSshProjectRootInspectionResult,
	encodeSshEnvironmentMessage,
	encodeSshProjectRootDirectoryListingRequest,
	encodeSshProjectRootInspectionRequest,
	type SshEnvironmentChannelId,
	type SshEnvironmentMessage,
	type SshEnvironmentRuntimeEpoch,
	type SshProjectRootDirectoryListingResult,
	type SshProjectRootFailureReason,
	type SshProjectRootInspectionResult,
} from '@orchbee/ssh-environment-protocol';

const MAXIMUM_CONCURRENT_OPERATIONS = 4;
const OPERATION_TIMEOUT_MILLISECONDS = 30_000;

export type SshProjectRootInspectionOutcome =
	| SshProjectRootInspectionResult
	| { readonly kind: 'cancelled' };
export type SshProjectRootDirectoryListingOutcome =
	| SshProjectRootDirectoryListingResult
	| { readonly kind: 'cancelled' };

type SshProjectRootOperationOutcome =
	| SshProjectRootInspectionOutcome
	| SshProjectRootDirectoryListingOutcome;

interface SshProjectRootChannelClientOptions {
	readonly allocateChannelId: () => SshEnvironmentChannelId | null;
	readonly onAttachmentInvalid: () => void;
	readonly runtimeEpoch: SshEnvironmentRuntimeEpoch;
	readonly write: (bytes: Uint8Array) => Promise<void>;
}

type ChannelPhase = 'closing' | 'opening' | 'request-sent';
type ChannelCloseReason = 'cancelled' | 'completed' | 'protocol-failure';

interface OperationChannel {
	abortListener?: () => void;
	readonly abortSignal?: AbortSignal;
	closeReason?: ChannelCloseReason;
	readonly kind: 'inspection' | 'listing';
	phase: ChannelPhase;
	readonly requestBytes: Uint8Array;
	resolve: (outcome: SshProjectRootOperationOutcome) => void;
	resultCreditBytes: number;
	settled: boolean;
	timeout: NodeJS.Timeout;
}

export class SshProjectRootChannelClient {
	readonly #allocateChannelId: () => SshEnvironmentChannelId | null;
	readonly #channels = new Map<SshEnvironmentChannelId, OperationChannel>();
	readonly #onAttachmentInvalid: () => void;
	#retired = false;
	readonly #runtimeEpoch: SshEnvironmentRuntimeEpoch;
	readonly #write: (bytes: Uint8Array) => Promise<void>;

	constructor(options: SshProjectRootChannelClientOptions) {
		this.#allocateChannelId = options.allocateChannelId;
		this.#onAttachmentInvalid = options.onAttachmentInvalid;
		this.#runtimeEpoch = options.runtimeEpoch;
		this.#write = options.write;
	}

	inspect(path: string, signal?: AbortSignal): Promise<SshProjectRootInspectionOutcome> {
		if (this.#retired) return Promise.resolve(failed('unavailable'));
		const request = SshProjectRootInspectionRequestSchema.safeParse({
			type: 'inspect-project-root',
			path,
		});
		if (!request.success) return Promise.resolve(failed('invalid-path'));
		if (signal?.aborted === true) return Promise.resolve({ kind: 'cancelled' });
		if (this.#channels.size >= MAXIMUM_CONCURRENT_OPERATIONS) {
			return Promise.resolve(failed('busy'));
		}
		const channelId = this.#allocateChannelId();
		if (channelId === null) return Promise.resolve(failed('unavailable'));

		return new Promise((resolve) => {
			const channel: OperationChannel = {
				...(signal === undefined ? {} : { abortSignal: signal }),
				kind: 'inspection',
				phase: 'opening',
				requestBytes: encodeSshProjectRootInspectionRequest(request.data),
				resolve: (outcome) => {
					if (outcome.kind === 'listed') {
						resolve(failed('unavailable'));
						return;
					}
					resolve(outcome);
				},
				resultCreditBytes: SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES,
				settled: false,
				timeout: setTimeout(
					() => this.#cancel(channelId, failed('timed-out')),
					OPERATION_TIMEOUT_MILLISECONDS,
				),
			};
			if (signal !== undefined) {
				channel.abortListener = () => this.#cancel(channelId, { kind: 'cancelled' });
				signal.addEventListener('abort', channel.abortListener, { once: true });
			}
			this.#channels.set(channelId, channel);
			this.#send({
				type: 'channel-open',
				payload: {
					runtimeEpoch: this.#runtimeEpoch,
					channelId,
					serviceId: SSH_PROJECT_ROOT_SERVICE_ID,
					initialCreditBytes: SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES,
				},
			});
		});
	}

	list(path: string | null, signal?: AbortSignal): Promise<SshProjectRootDirectoryListingOutcome> {
		if (this.#retired) return Promise.resolve(failed('unavailable'));
		const request = SshProjectRootDirectoryListingRequestSchema.safeParse({
			type: 'list-project-directory',
			path,
		});
		if (!request.success) return Promise.resolve(failed('invalid-path'));
		if (signal?.aborted === true) return Promise.resolve({ kind: 'cancelled' });
		if (this.#channels.size >= MAXIMUM_CONCURRENT_OPERATIONS) {
			return Promise.resolve(failed('busy'));
		}
		const channelId = this.#allocateChannelId();
		if (channelId === null) return Promise.resolve(failed('unavailable'));

		return new Promise((resolve) => {
			const channel: OperationChannel = {
				...(signal === undefined ? {} : { abortSignal: signal }),
				kind: 'listing',
				phase: 'opening',
				requestBytes: encodeSshProjectRootDirectoryListingRequest(request.data),
				resolve: (outcome) => {
					if (outcome.kind === 'resolved') {
						resolve(failed('unavailable'));
						return;
					}
					resolve(outcome);
				},
				resultCreditBytes: SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES,
				settled: false,
				timeout: setTimeout(
					() => this.#cancel(channelId, failed('timed-out')),
					OPERATION_TIMEOUT_MILLISECONDS,
				),
			};
			if (signal !== undefined) {
				channel.abortListener = () => this.#cancel(channelId, { kind: 'cancelled' });
				signal.addEventListener('abort', channel.abortListener, { once: true });
			}
			this.#channels.set(channelId, channel);
			this.#send({
				type: 'channel-open',
				payload: {
					runtimeEpoch: this.#runtimeEpoch,
					channelId,
					serviceId: SSH_PROJECT_ROOT_SERVICE_ID,
					initialCreditBytes: SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES,
				},
			});
		});
	}

	handle(message: SshEnvironmentMessage): 'attachment-invalid' | 'handled' | 'unhandled' {
		const channelId = channelIdOf(message);
		if (channelId === null || !this.#channels.has(channelId)) return 'unhandled';
		switch (message.type) {
			case 'channel-opened':
				return this.#handleOpened(message.payload);
			case 'channel-data':
				return this.#handleData(message.channelId, message.data);
			case 'channel-closed':
				return this.#handleClosed(message.payload);
			default:
				return 'attachment-invalid';
		}
	}

	retire(): void {
		if (this.#retired) return;
		this.#retired = true;
		for (const [channelId, channel] of this.#channels) {
			this.#settle(channel, failed('unavailable'));
			this.#release(channelId, channel);
		}
	}

	#handleOpened(
		payload: Extract<SshEnvironmentMessage, { readonly type: 'channel-opened' }>['payload'],
	): 'attachment-invalid' | 'handled' {
		if (
			payload.runtimeEpoch !== this.#runtimeEpoch ||
			payload.initialCreditBytes !== SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES
		) {
			return 'attachment-invalid';
		}
		const channel = this.#channels.get(payload.channelId);
		if (channel === undefined || (channel.phase !== 'opening' && channel.phase !== 'closing')) {
			return 'attachment-invalid';
		}
		if (channel.phase === 'closing') return 'handled';
		channel.phase = 'request-sent';
		this.#send({ type: 'channel-data', channelId: payload.channelId, data: channel.requestBytes });
		return 'handled';
	}

	#handleData(
		channelId: SshEnvironmentChannelId,
		data: Uint8Array,
	): 'attachment-invalid' | 'handled' {
		const channel = this.#channels.get(channelId);
		if (channel === undefined || channel.phase !== 'request-sent') return 'attachment-invalid';
		if (data.byteLength > channel.resultCreditBytes) return 'attachment-invalid';
		channel.resultCreditBytes -= data.byteLength;
		const decoded =
			channel.kind === 'inspection'
				? decodeSshProjectRootInspectionResult(data)
				: decodeSshProjectRootDirectoryListingResult(data);
		if (data.byteLength > SSH_PROJECT_ROOT_MAX_MESSAGE_BYTES || !decoded.ok) {
			this.#settle(channel, failed('unavailable'));
			this.#beginClose(channelId, channel, 'protocol-failure');
			return 'handled';
		}
		this.#settle(channel, decoded.message);
		this.#beginClose(channelId, channel, 'completed');
		return 'handled';
	}

	#handleClosed(
		payload: Extract<SshEnvironmentMessage, { readonly type: 'channel-closed' }>['payload'],
	): 'attachment-invalid' | 'handled' {
		if (payload.runtimeEpoch !== this.#runtimeEpoch) return 'attachment-invalid';
		const channel = this.#channels.get(payload.channelId);
		if (
			channel === undefined ||
			channel.phase !== 'closing' ||
			channel.closeReason !== payload.reason
		) {
			return 'attachment-invalid';
		}
		this.#release(payload.channelId, channel);
		return 'handled';
	}

	#cancel(channelId: SshEnvironmentChannelId, outcome: SshProjectRootOperationOutcome): void {
		const channel = this.#channels.get(channelId);
		if (channel === undefined || channel.settled) return;
		this.#settle(channel, outcome);
		this.#beginClose(channelId, channel, 'cancelled');
	}

	#beginClose(
		channelId: SshEnvironmentChannelId,
		channel: OperationChannel,
		reason: ChannelCloseReason,
	): void {
		channel.phase = 'closing';
		channel.closeReason = reason;
		this.#send({
			type: 'channel-close',
			payload: { runtimeEpoch: this.#runtimeEpoch, channelId, reason },
		});
	}

	#settle(channel: OperationChannel, outcome: SshProjectRootOperationOutcome): void {
		if (channel.settled) return;
		channel.settled = true;
		clearTimeout(channel.timeout);
		if (channel.abortSignal !== undefined && channel.abortListener !== undefined) {
			channel.abortSignal.removeEventListener('abort', channel.abortListener);
		}
		channel.resolve(outcome);
	}

	#release(channelId: SshEnvironmentChannelId, channel: OperationChannel): void {
		clearTimeout(channel.timeout);
		if (channel.abortSignal !== undefined && channel.abortListener !== undefined) {
			channel.abortSignal.removeEventListener('abort', channel.abortListener);
		}
		this.#channels.delete(channelId);
	}

	#send(message: SshEnvironmentMessage): void {
		void this.#write(encodeSshEnvironmentMessage(message)).catch(this.#onAttachmentInvalid);
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

function failed(reason: SshProjectRootFailureReason): {
	readonly kind: 'failed';
	readonly reason: SshProjectRootFailureReason;
} {
	return { kind: 'failed', reason };
}
