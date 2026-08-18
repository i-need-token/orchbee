import { randomBytes } from 'node:crypto';
import { createConnection } from 'node:net';
import type { Duplex } from 'node:stream';

import {
	ClientIdSchema,
	CorrelationIdSchema,
	CURRENT_PROTOCOL_VERSION,
	decodeControlJson,
	decodeProtocolEnvelope,
	decodeSelectionExportChunkRoute,
	decodeTerminalFrameRoute,
	encodeControlRequest,
	encodeInputRequest,
	encodeProtocolEnvelope,
	HelloAcceptedSchema,
	HelloSchema,
	HistoryChangedSchema,
	messagePayloadEncoding,
	MessageKind,
	PayloadEncoding,
	PROTOCOL_LIMITS,
	ProjectionBlockedSchema,
	ResizeLeaseChangedSchema,
	SelectionChangedSchema,
	SelectionExportFinishedSchema,
	TerminalBellSchema,
	TerminalClipboardWriteRequestSchema,
	TypedFailureSchema,
	type ClientId,
	type AttachmentId,
	type DecodedProtocolEnvelope,
	type HelloAccepted,
	type HostEpoch,
	type MessageKindValue,
	type MessageRole,
	requestMessageContract,
	type PayloadEncodingValue,
	type ProtocolVersion,
} from '@orchbee/terminal-protocol';
import type { z } from 'zod';

import { HostRecordAssembler } from './HostRecordAssembler';
import { HostRequestError } from './HostRequestError';

const REQUEST_TIMEOUT_MILLISECONDS = 10_000;
const MAX_ADMITTED_REQUEST_BYTES = 4 * 1024 * 1024;
const MAX_ADMITTED_REQUEST_RECORDS = 64;
interface PendingRequest {
	readonly expectedKind: MessageKindValue;
	readonly recordBytes: number;
	readonly reject: (error: Error) => void;
	readonly resolve: (envelope: DecodedProtocolEnvelope) => void;
	readonly timeout: ReturnType<typeof setTimeout>;
}

type InboundResponseDelivery =
	| {
			readonly envelope: DecodedProtocolEnvelope;
			readonly kind: 'response';
			readonly outcome: 'resolve';
			readonly pending: PendingRequest;
	  }
	| {
			readonly error: Error;
			readonly kind: 'response';
			readonly outcome: 'reject';
			readonly pending: PendingRequest;
	  };

type InboundDelivery =
	| InboundResponseDelivery
	| {
			readonly attachmentId: AttachmentId | null;
			readonly envelope: DecodedProtocolEnvelope;
			readonly kind: 'event';
	  }
	| { readonly kind: 'close'; readonly reason: Error };

export interface HostConnectionMetrics {
	readonly bytesReceived: number;
	readonly bytesSent: number;
	readonly outboundAdmittedBytes: number;
	readonly outboundAdmittedRecords: number;
	readonly outboundWriteBlocked: boolean;
	readonly recordsReceived: number;
	readonly recordsSent: number;
	readonly terminalFramesReceived: number;
}

export type HostEnvelopeListener = (envelope: DecodedProtocolEnvelope) => void;
export type HostClosedListener = (reason: Error) => void;

export class HostConnection {
	static readonly outboundAdmissionLimits = Object.freeze({
		bytes: MAX_ADMITTED_REQUEST_BYTES,
		records: MAX_ADMITTED_REQUEST_RECORDS,
	});

	readonly #assembler: HostRecordAssembler;
	readonly #attachmentListeners = new Map<AttachmentId, Set<HostEnvelopeListener>>();
	readonly #closedListeners = new Set<HostClosedListener>();
	readonly #inboundDeliveries: InboundDelivery[] = [];
	readonly #listeners = new Set<HostEnvelopeListener>();
	readonly #pending = new Map<string, PendingRequest>();
	readonly #stream: Duplex;
	#bytesReceived = 0;
	#bytesSent = 0;
	#closed = false;
	#closedReason: Error | undefined;
	#hostEpoch: HostEpoch | undefined;
	#inboundResponseBarrier = false;
	#outboundAdmittedBytes = 0;
	#outboundAdmittedRecords = 0;
	#outboundWriteBlocked = false;
	#protocolVersion: ProtocolVersion | undefined;
	#recordsReceived = 0;
	#recordsSent = 0;
	#terminalFramesReceived = 0;
	#transportEnded = false;
	#transportEndReason: Error | undefined;

	constructor(stream: Duplex) {
		this.#stream = stream;
		this.#assembler = new HostRecordAssembler((record) => this.#handleRecord(record));
		stream.on('data', (chunk: unknown) => {
			if (this.#transportEnded) return;
			if (!(chunk instanceof Uint8Array)) {
				this.#fail(new Error('Host transport emitted a non-binary chunk.'));
				return;
			}
			try {
				this.#assembler.push(chunk);
			} catch (error) {
				this.#fail(error instanceof Error ? error : new Error('Host record assembly failed.'));
			}
		});
		stream.on('drain', () => {
			if (!this.#transportEnded) this.#outboundWriteBlocked = false;
		});
		stream.once('error', (error: Error) => this.#fail(error));
		stream.once('close', () => this.#handleClose());
	}

	static connectNamedPipe(endpoint: string): Promise<HostConnection> {
		return new Promise((resolve, reject) => {
			const socket = createConnection(endpoint);
			const handleError = (error: Error): void => reject(error);
			socket.once('error', handleError);
			socket.once('connect', () => {
				socket.off('error', handleError);
				resolve(new HostConnection(socket));
			});
		});
	}

	get hostEpoch(): HostEpoch {
		if (this.#hostEpoch === undefined) throw new Error('Host connection is not negotiated.');
		return this.#hostEpoch;
	}

	get metrics(): HostConnectionMetrics {
		return {
			bytesReceived: this.#bytesReceived,
			bytesSent: this.#bytesSent,
			outboundAdmittedBytes: this.#outboundAdmittedBytes,
			outboundAdmittedRecords: this.#outboundAdmittedRecords,
			outboundWriteBlocked: this.#outboundWriteBlocked,
			recordsReceived: this.#recordsReceived,
			recordsSent: this.#recordsSent,
			terminalFramesReceived: this.#terminalFramesReceived,
		};
	}

	get protocolVersion(): ProtocolVersion {
		if (this.#protocolVersion === undefined) {
			throw new Error('Host connection is not negotiated.');
		}
		return this.#protocolVersion;
	}

	async negotiate(clientId: ClientId): Promise<HelloAccepted> {
		if (this.#hostEpoch !== undefined) throw new Error('Host connection is already negotiated.');
		const hello = HelloSchema.parse({
			clientId: ClientIdSchema.parse(clientId),
			frameBytes: PROTOCOL_LIMITS.maxFrameBytes,
			inputBytes: PROTOCOL_LIMITS.maxInputBytes,
			protocol: CURRENT_PROTOCOL_VERSION,
			rowWindowBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
			supportedOptionalFeatures: [],
		});
		const helloPayload = encodeControlRequest(MessageKind.hello, hello);
		if (!helloPayload.ok) throw new Error(helloPayload.detail);
		const helloContract = requestMessageContract(MessageKind.hello);
		if (helloContract === null) throw new Error('Hello request contract is missing.');
		const envelope = await this.#request(
			MessageKind.hello,
			helloContract.payloadEncoding,
			helloPayload.value,
			helloContract.successKind,
			null,
		);
		const accepted = decodeControlJson(envelope.payload, HelloAcceptedSchema);
		if (!accepted.ok) throw new Error(accepted.detail);
		if (!desktopSupportsProtocol(accepted.value.protocol)) {
			throw new Error('Terminal Host did not negotiate a supported Desktop protocol.');
		}
		if (
			envelope.protocol.major !== accepted.value.protocol.major ||
			envelope.protocol.minor !== accepted.value.protocol.minor ||
			envelope.protocol.patch !== accepted.value.protocol.patch
		) {
			throw new Error('HelloAccepted envelope and payload protocol versions differ.');
		}
		if (envelope.hostEpoch !== accepted.value.hostEpoch) {
			throw new Error('HelloAccepted envelope and payload HostEpoch values differ.');
		}
		this.#protocolVersion = accepted.value.protocol;
		this.#hostEpoch = accepted.value.hostEpoch;
		return accepted.value;
	}

	requestControlEnvelope(kind: MessageKindValue, value: unknown): Promise<DecodedProtocolEnvelope> {
		const contract = requestMessageContract(kind);
		if (contract?.payloadEncoding !== PayloadEncoding.controlJson) {
			return Promise.reject(new Error(`Message kind ${kind} is not a control request.`));
		}
		const payload = encodeControlRequest(kind, value);
		if (!payload.ok) return Promise.reject(new Error(payload.detail));
		return this.#request(kind, contract.payloadEncoding, payload.value, contract.successKind);
	}

	requestControl<Output>(
		kind: MessageKindValue,
		value: unknown,
		responseSchema: z.ZodType<Output>,
	): Promise<Output> {
		return this.requestControlEnvelope(kind, value).then((envelope) => {
			const decoded = decodeControlJson(envelope.payload, responseSchema);
			if (!decoded.ok) throw new Error(decoded.detail);
			return decoded.value;
		});
	}

	requestInput(kind: MessageKindValue, value: unknown): Promise<DecodedProtocolEnvelope> {
		const contract = requestMessageContract(kind);
		if (contract?.payloadEncoding !== PayloadEncoding.rawBytes) {
			return Promise.reject(new Error(`Message kind ${kind} is not an input request.`));
		}
		const payload = encodeInputRequest(kind, value);
		if (!payload.ok) return Promise.reject(new Error(payload.detail));
		return this.#request(kind, contract.payloadEncoding, payload.value, contract.successKind);
	}

	#request(
		kind: MessageKindValue,
		payloadEncoding: PayloadEncodingValue,
		payload: Uint8Array,
		expectedKind: MessageKindValue,
		hostEpoch: HostEpoch | null = this.#hostEpoch ?? null,
	): Promise<DecodedProtocolEnvelope> {
		if (this.#transportEnded) {
			return Promise.reject(
				this.#closedReason ?? this.#transportEndReason ?? new Error('Host connection is closed.'),
			);
		}
		if (kind !== MessageKind.hello && this.#protocolVersion === undefined) {
			return Promise.reject(new Error('Host connection is not negotiated.'));
		}
		const correlationId = CorrelationIdSchema.parse(randomBytes(16).toString('hex'));
		const encoded = encodeProtocolEnvelope({
			correlationId,
			event: false,
			hostEpoch,
			kind,
			optionalMessage: false,
			payload,
			payloadEncoding,
			protocol: this.#protocolVersion ?? CURRENT_PROTOCOL_VERSION,
			response: false,
		});
		if (!encoded.ok) return Promise.reject(new Error(encoded.detail));
		if (this.#pending.has(correlationId)) {
			return Promise.reject(new Error('Terminal Host request correlation is already pending.'));
		}

		return new Promise((resolve, reject) => {
			const admissionError = this.#admitOutbound(encoded.value.byteLength);
			if (admissionError !== undefined) {
				reject(admissionError);
				return;
			}
			const timeout = setTimeout(() => {
				if (!this.#pending.has(correlationId)) return;
				this.#retireOutcomeUnknown(
					`Terminal Host request ${kind} reached its deadline after transport handoff; its outcome is unknown.`,
				);
			}, REQUEST_TIMEOUT_MILLISECONDS);
			this.#pending.set(correlationId, {
				expectedKind,
				recordBytes: encoded.value.byteLength,
				reject,
				resolve,
				timeout,
			});
			this.#recordsSent += 1;
			this.#bytesSent += encoded.value.byteLength;
			try {
				const canContinueWriting = this.#stream.write(encoded.value, (error) => {
					if (error === null || error === undefined || !this.#pending.has(correlationId)) {
						return;
					}
					this.#retireOutcomeUnknown(
						'Terminal Host transport failed after request handoff; pending outcomes are unknown.',
						error,
					);
				});
				if (!canContinueWriting && !this.#closed) this.#outboundWriteBlocked = true;
			} catch (error) {
				const writeError =
					error instanceof Error ? error : new Error('Terminal Host transport write failed.');
				this.#retireOutcomeUnknown(
					'Terminal Host transport failed after request handoff; pending outcomes are unknown.',
					writeError,
				);
			}
		});
	}

	#admitOutbound(recordBytes: number): Error | undefined {
		if (!Number.isSafeInteger(recordBytes) || recordBytes <= 0) {
			return new Error('Terminal Host outbound record length is invalid.');
		}
		if (this.#outboundWriteBlocked) return this.#backpressureError();
		const nextBytes = this.#outboundAdmittedBytes + recordBytes;
		const nextRecords = this.#outboundAdmittedRecords + 1;
		if (!Number.isSafeInteger(nextBytes) || !Number.isSafeInteger(nextRecords)) {
			return new Error('Terminal Host outbound admission accounting overflowed.');
		}
		if (nextBytes > MAX_ADMITTED_REQUEST_BYTES || nextRecords > MAX_ADMITTED_REQUEST_RECORDS) {
			return this.#backpressureError();
		}
		this.#outboundAdmittedBytes = nextBytes;
		this.#outboundAdmittedRecords = nextRecords;
		return undefined;
	}

	#backpressureError(): HostRequestError {
		return new HostRequestError({
			code: 'host-transport-backpressure',
			message: 'The bounded Desktop-to-Host request transport is saturated.',
			retryable: true,
		});
	}

	onEnvelope(listener: HostEnvelopeListener): () => void {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	onAttachmentEnvelope(attachmentId: AttachmentId, listener: HostEnvelopeListener): () => void {
		const listeners =
			this.#attachmentListeners.get(attachmentId) ?? new Set<HostEnvelopeListener>();
		listeners.add(listener);
		this.#attachmentListeners.set(attachmentId, listeners);
		return () => {
			listeners.delete(listener);
			if (listeners.size === 0) this.#attachmentListeners.delete(attachmentId);
		};
	}

	onClosed(listener: HostClosedListener): () => void {
		const reason = this.#closedReason;
		if (reason !== undefined) {
			queueMicrotask(() => listener(reason));
			return () => undefined;
		}
		this.#closedListeners.add(listener);
		return () => this.#closedListeners.delete(listener);
	}

	close(): void {
		this.#beginTransportEnd(new Error('Host connection closed.'));
	}

	#handleRecord(record: Uint8Array): void {
		const decoded = decodeProtocolEnvelope(record);
		if (!decoded.ok) throw new Error(decoded.detail);
		const envelope = decoded.value;
		this.#recordsReceived += 1;
		this.#bytesReceived += record.byteLength;
		const protocolVersion = this.#protocolVersion;
		if (
			(protocolVersion === undefined && !desktopSupportsProtocol(envelope.protocol)) ||
			(protocolVersion !== undefined &&
				(envelope.protocol.major !== protocolVersion.major ||
					envelope.protocol.minor !== protocolVersion.minor ||
					envelope.protocol.patch !== protocolVersion.patch))
		) {
			throw new Error('Host emitted a record outside the negotiated Desktop protocol.');
		}
		if (this.#hostEpoch !== undefined && envelope.hostEpoch !== this.#hostEpoch) {
			throw new Error('Host emitted a record for a stale or foreign HostEpoch.');
		}
		if (!envelope.knownMessage) return;
		let role: MessageRole = 'request';
		if (envelope.response) role = 'response';
		else if (envelope.event) role = 'event';
		const expectedEncoding = messagePayloadEncoding(envelope.kind, role);
		if (expectedEncoding === null || envelope.payloadEncoding !== expectedEncoding) {
			throw new Error(`Terminal Host emitted an invalid ${role} message contract.`);
		}
		if (envelope.response) {
			this.#enqueueInboundDelivery(this.#prepareResponse(envelope));
			return;
		}
		if (!envelope.event) throw new Error('Host emitted an unsolicited request record.');
		const attachmentId = this.#routeAttachmentEvent(envelope);
		this.#enqueueInboundDelivery({ attachmentId, envelope, kind: 'event' });
	}

	#routeAttachmentEvent(envelope: DecodedProtocolEnvelope): AttachmentId | null {
		switch (envelope.kind) {
			case MessageKind.snapshotFrame:
			case MessageKind.deltaFrame: {
				if (envelope.payloadEncoding !== PayloadEncoding.terminalFrame) {
					throw new Error('Terminal Host frame used the wrong payload encoding.');
				}
				const route = decodeTerminalFrameRoute(envelope.payload);
				if (!route.ok) throw new Error(route.detail);
				if (route.value.hostEpoch !== envelope.hostEpoch) {
					throw new Error('Terminal frame and envelope HostEpoch values differ.');
				}
				this.#terminalFramesReceived += 1;
				return route.value.attachmentId;
			}
			case MessageKind.historyChanged:
				return decodeControlAttachmentId(envelope, HistoryChangedSchema);
			case MessageKind.terminalBell:
				return decodeControlAttachmentId(envelope, TerminalBellSchema);
			case MessageKind.terminalClipboardWriteRequest:
				return decodeControlAttachmentId(envelope, TerminalClipboardWriteRequestSchema);
			case MessageKind.projectionBlocked:
				return decodeControlAttachmentId(envelope, ProjectionBlockedSchema);
			case MessageKind.resizeLeaseChanged:
				return decodeControlAttachmentId(envelope, ResizeLeaseChangedSchema);
			case MessageKind.selectionChanged:
				return decodeControlAttachmentId(envelope, SelectionChangedSchema);
			case MessageKind.selectionExportFinished:
				return decodeControlAttachmentId(envelope, SelectionExportFinishedSchema);
			case MessageKind.selectionExportChunk: {
				if (envelope.payloadEncoding !== PayloadEncoding.rawBytes) {
					throw new Error('Terminal Host selection export used the wrong payload encoding.');
				}
				const route = decodeSelectionExportChunkRoute(envelope.payload);
				if (!route.ok) throw new Error(route.detail);
				return route.value.attachmentId;
			}
			default:
				return null;
		}
	}

	#prepareResponse(envelope: DecodedProtocolEnvelope): InboundResponseDelivery {
		const correlationId = envelope.correlationId;
		if (correlationId === null) throw new Error('Host response omitted its correlation ID.');
		const pending = this.#takePending(correlationId);
		if (pending === undefined) throw new Error('Host response correlation is unknown or expired.');
		if (envelope.kind === MessageKind.typedFailure) {
			const failure = decodeControlJson(envelope.payload, TypedFailureSchema);
			return {
				error: failure.ok
					? new HostRequestError(failure.value, correlationId)
					: new Error(failure.detail),
				kind: 'response',
				outcome: 'reject',
				pending,
			};
		}
		if (envelope.kind !== pending.expectedKind) {
			return {
				error: new Error(
					`Host response kind ${envelope.kind} does not match ${pending.expectedKind}.`,
				),
				kind: 'response',
				outcome: 'reject',
				pending,
			};
		}
		return { envelope, kind: 'response', outcome: 'resolve', pending };
	}

	#enqueueInboundDelivery(delivery: InboundDelivery): void {
		if (this.#inboundResponseBarrier || this.#inboundDeliveries.length > 0) {
			this.#inboundDeliveries.push(delivery);
			return;
		}
		this.#deliverInbound(delivery);
	}

	#deliverInbound(delivery: InboundDelivery): void {
		switch (delivery.kind) {
			case 'close':
				this.#completeClose(delivery.reason);
				return;
			case 'event':
				if (delivery.attachmentId !== null) {
					for (const listener of this.#attachmentListeners.get(delivery.attachmentId) ?? []) {
						listener(delivery.envelope);
					}
					return;
				}
				for (const listener of this.#listeners) listener(delivery.envelope);
				return;
			case 'response':
				if (delivery.outcome === 'resolve') delivery.pending.resolve(delivery.envelope);
				else delivery.pending.reject(delivery.error);
				this.#inboundResponseBarrier = true;
				setImmediate(() => this.#resumeInboundDelivery());
		}
	}

	#resumeInboundDelivery(): void {
		if (!this.#inboundResponseBarrier || this.#closed) return;
		this.#inboundResponseBarrier = false;
		while (!this.#inboundResponseBarrier && !this.#closed) {
			const delivery = this.#inboundDeliveries.shift();
			if (delivery === undefined) return;
			try {
				this.#deliverInbound(delivery);
			} catch (error) {
				this.#failInboundDelivery(
					error instanceof Error ? error : new Error('Host inbound delivery failed.'),
				);
			}
		}
	}

	#handleClose(): void {
		if (this.#transportEnded) return;
		let reason = new Error('Host connection closed.');
		try {
			this.#assembler.finish();
		} catch (error) {
			reason = error instanceof Error ? error : new Error('Host transport ended unexpectedly.');
		}
		if (this.#hostEpoch !== undefined && this.#pending.size > 0) {
			reason = this.#outcomeUnknownError(
				'Terminal Host connection closed after request handoff; pending outcomes are unknown.',
			);
		}
		this.#beginTransportEnd(reason);
	}

	#fail(error: Error): void {
		if (this.#transportEnded) return;
		if (this.#hostEpoch !== undefined && this.#pending.size > 0) {
			this.#beginTransportEnd(
				this.#outcomeUnknownError(
					'Terminal Host connection failed after request handoff; pending outcomes are unknown.',
				),
				error,
			);
			return;
		}
		this.#beginTransportEnd(error, error);
	}

	#retireOutcomeUnknown(message: string, transportError?: Error): void {
		if (this.#transportEnded) return;
		const reason = this.#outcomeUnknownError(message);
		this.#beginTransportEnd(reason, transportError ?? reason);
	}

	#beginTransportEnd(reason: Error, destroyError?: Error): void {
		if (this.#transportEnded) return;
		this.#transportEnded = true;
		this.#transportEndReason = reason;
		this.#enqueueInboundDelivery({ kind: 'close', reason });
		this.#stream.destroy(destroyError);
	}

	#failInboundDelivery(error: Error): void {
		const reason =
			this.#hostEpoch !== undefined && this.#pending.size > 0
				? this.#outcomeUnknownError(
						'Terminal Host connection failed during inbound delivery; pending outcomes are unknown.',
					)
				: error;
		this.#transportEnded = true;
		this.#transportEndReason = reason;
		this.#inboundResponseBarrier = false;
		for (const delivery of this.#inboundDeliveries.splice(0)) {
			if (delivery.kind === 'response') delivery.pending.reject(reason);
		}
		this.#completeClose(reason);
		this.#stream.destroy(error);
	}

	#outcomeUnknownError(message: string): HostRequestError {
		return new HostRequestError({
			code: 'host-request-outcome-unknown',
			message,
			retryable: false,
		});
	}

	#completeClose(reason: Error): void {
		if (this.#closed) return;
		this.#transportEnded = true;
		this.#transportEndReason = reason;
		this.#closed = true;
		this.#closedReason = reason;
		this.#outboundWriteBlocked = false;
		this.#rejectPending(reason);
		for (const listener of this.#closedListeners) queueMicrotask(() => listener(reason));
		this.#closedListeners.clear();
	}

	#rejectPending(error: Error): void {
		for (const correlationId of this.#pending.keys()) {
			this.#takePending(correlationId)?.reject(error);
		}
		if (this.#outboundAdmittedBytes !== 0 || this.#outboundAdmittedRecords !== 0) {
			throw new Error('Terminal Host outbound admission accounting did not reach zero.');
		}
	}

	#takePending(correlationId: string): PendingRequest | undefined {
		const pending = this.#pending.get(correlationId);
		if (pending === undefined) return undefined;
		this.#pending.delete(correlationId);
		clearTimeout(pending.timeout);
		const nextBytes = this.#outboundAdmittedBytes - pending.recordBytes;
		const nextRecords = this.#outboundAdmittedRecords - 1;
		if (
			!Number.isSafeInteger(nextBytes) ||
			!Number.isSafeInteger(nextRecords) ||
			nextBytes < 0 ||
			nextRecords < 0
		) {
			throw new Error('Terminal Host outbound admission accounting underflowed.');
		}
		this.#outboundAdmittedBytes = nextBytes;
		this.#outboundAdmittedRecords = nextRecords;
		return pending;
	}
}

function desktopSupportsProtocol(protocol: ProtocolVersion): boolean {
	return (
		protocol.major === CURRENT_PROTOCOL_VERSION.major &&
		protocol.minor === CURRENT_PROTOCOL_VERSION.minor &&
		protocol.patch === CURRENT_PROTOCOL_VERSION.patch
	);
}

function decodeControlAttachmentId<Output extends { readonly attachmentId: AttachmentId }>(
	envelope: DecodedProtocolEnvelope,
	schema: z.ZodType<Output>,
): AttachmentId {
	if (envelope.payloadEncoding !== PayloadEncoding.controlJson) {
		throw new Error('Terminal Host attachment event used the wrong payload encoding.');
	}
	const decoded = decodeControlJson(envelope.payload, schema);
	if (!decoded.ok) throw new Error(decoded.detail);
	return decoded.value.attachmentId;
}
