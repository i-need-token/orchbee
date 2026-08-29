import {
	SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES,
	SSH_PROJECT_ROOT_MAX_MESSAGE_BYTES,
	SSH_PROJECT_ROOT_SERVICE_ID,
	SshEnvironmentChannelIdSchema,
	SshProjectRootInspectionRequestSchema,
	decodeSshProjectRootInspectionResult,
	encodeSshEnvironmentMessage,
	encodeSshProjectRootInspectionRequest,
	type SshEnvironmentChannelId,
	type SshEnvironmentMessage,
	type SshEnvironmentRuntimeEpoch,
	type SshProjectRootInspectionResult,
} from '@orchbee/ssh-environment-protocol';

const MAXIMUM_CONCURRENT_INSPECTIONS = 4;
const INSPECTION_TIMEOUT_MILLISECONDS = 30_000;

export type SshProjectRootInspectionOutcome =
	| SshProjectRootInspectionResult
	| { readonly kind: 'cancelled' };

interface SshProjectRootChannelClientOptions {
	readonly onAttachmentInvalid: () => void;
	readonly runtimeEpoch: SshEnvironmentRuntimeEpoch;
	readonly write: (bytes: Uint8Array) => Promise<void>;
}

type ChannelPhase = 'closing' | 'opening' | 'request-sent';
type ChannelCloseReason = 'cancelled' | 'completed' | 'protocol-failure';

interface InspectionChannel {
	abortListener?: () => void;
	readonly abortSignal?: AbortSignal;
	closeReason?: ChannelCloseReason;
	phase: ChannelPhase;
	readonly requestBytes: Uint8Array;
	resolve: (outcome: SshProjectRootInspectionOutcome) => void;
	resultCreditBytes: number;
	settled: boolean;
	timeout: NodeJS.Timeout;
}

export class SshProjectRootChannelClient {
	readonly #channels = new Map<SshEnvironmentChannelId, InspectionChannel>();
	#nextChannelId = 1;
	readonly #onAttachmentInvalid: () => void;
	#retired = false;
	readonly #runtimeEpoch: SshEnvironmentRuntimeEpoch;
	readonly #write: (bytes: Uint8Array) => Promise<void>;

	constructor(options: SshProjectRootChannelClientOptions) {
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
		if (this.#channels.size >= MAXIMUM_CONCURRENT_INSPECTIONS) {
			return Promise.resolve(failed('busy'));
		}
		const channelId = SshEnvironmentChannelIdSchema.safeParse(this.#nextChannelId);
		if (!channelId.success) return Promise.resolve(failed('unavailable'));
		this.#nextChannelId += 1;

		return new Promise((resolve) => {
			const channel: InspectionChannel = {
				...(signal === undefined ? {} : { abortSignal: signal }),
				phase: 'opening',
				requestBytes: encodeSshProjectRootInspectionRequest(request.data),
				resolve,
				resultCreditBytes: SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES,
				settled: false,
				timeout: setTimeout(
					() => this.#cancel(channelId.data, failed('timed-out')),
					INSPECTION_TIMEOUT_MILLISECONDS,
				),
			};
			if (signal !== undefined) {
				channel.abortListener = () => this.#cancel(channelId.data, { kind: 'cancelled' });
				signal.addEventListener('abort', channel.abortListener, { once: true });
			}
			this.#channels.set(channelId.data, channel);
			this.#send({
				type: 'channel-open',
				payload: {
					runtimeEpoch: this.#runtimeEpoch,
					channelId: channelId.data,
					serviceId: SSH_PROJECT_ROOT_SERVICE_ID,
					initialCreditBytes: SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES,
				},
			});
		});
	}

	handle(message: SshEnvironmentMessage): 'attachment-invalid' | 'handled' {
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
		const decoded = decodeSshProjectRootInspectionResult(data);
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

	#cancel(channelId: SshEnvironmentChannelId, outcome: SshProjectRootInspectionOutcome): void {
		const channel = this.#channels.get(channelId);
		if (channel === undefined || channel.settled) return;
		this.#settle(channel, outcome);
		this.#beginClose(channelId, channel, 'cancelled');
	}

	#beginClose(
		channelId: SshEnvironmentChannelId,
		channel: InspectionChannel,
		reason: ChannelCloseReason,
	): void {
		channel.phase = 'closing';
		channel.closeReason = reason;
		this.#send({
			type: 'channel-close',
			payload: { runtimeEpoch: this.#runtimeEpoch, channelId, reason },
		});
	}

	#settle(channel: InspectionChannel, outcome: SshProjectRootInspectionOutcome): void {
		if (channel.settled) return;
		channel.settled = true;
		clearTimeout(channel.timeout);
		if (channel.abortSignal !== undefined && channel.abortListener !== undefined) {
			channel.abortSignal.removeEventListener('abort', channel.abortListener);
		}
		channel.resolve(outcome);
	}

	#release(channelId: SshEnvironmentChannelId, channel: InspectionChannel): void {
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

function failed(
	reason: Extract<SshProjectRootInspectionResult, { readonly kind: 'failed' }>['reason'],
): Extract<SshProjectRootInspectionResult, { readonly kind: 'failed' }> {
	return { kind: 'failed', reason };
}
