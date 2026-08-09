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
	SelectionChangedSchema,
	SelectionExportFinishedSchema,
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
interface PendingRequest {
	readonly expectedKind: MessageKindValue;
	readonly reject: (error: Error) => void;
	readonly resolve: (envelope: DecodedProtocolEnvelope) => void;
	readonly timeout: ReturnType<typeof setTimeout>;
}

export interface HostConnectionMetrics {
	readonly bytesReceived: number;
	readonly bytesSent: number;
	readonly recordsReceived: number;
	readonly recordsSent: number;
	readonly terminalFramesReceived: number;
}

export type HostEnvelopeListener = (envelope: DecodedProtocolEnvelope) => void;
export type HostClosedListener = (reason: Error) => void;

export class HostConnection {
	readonly #assembler: HostRecordAssembler;
	readonly #attachmentListeners = new Map<AttachmentId, Set<HostEnvelopeListener>>();
	readonly #closedListeners = new Set<HostClosedListener>();
	readonly #listeners = new Set<HostEnvelopeListener>();
	readonly #pending = new Map<string, PendingRequest>();
	readonly #stream: Duplex;
	#bytesReceived = 0;
	#bytesSent = 0;
	#closed = false;
	#closedReason: Error | undefined;
	#hostEpoch: HostEpoch | undefined;
	#protocolVersion: ProtocolVersion | undefined;
	#recordsReceived = 0;
	#recordsSent = 0;
	#terminalFramesReceived = 0;

	constructor(stream: Duplex) {
		this.#stream = stream;
		this.#assembler = new HostRecordAssembler((record) => this.#handleRecord(record));
		stream.on('data', (chunk: unknown) => {
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
		stream.once('error', (error: Error) => this.#fail(error));
		stream.once('close', () => this.#handleClose());
	}

	static connect(endpoint: string): Promise<HostConnection> {
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
		if (this.#closed) return Promise.reject(new Error('Host connection is closed.'));
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

		return new Promise((resolve, reject) => {
			const timeout = setTimeout(() => {
				this.#pending.delete(correlationId);
				reject(new Error(`Terminal Host request ${kind} timed out.`));
			}, REQUEST_TIMEOUT_MILLISECONDS);
			this.#pending.set(correlationId, { expectedKind, reject, resolve, timeout });
			this.#recordsSent += 1;
			this.#bytesSent += encoded.value.byteLength;
			this.#stream.write(encoded.value, (error) => {
				if (error === null || error === undefined) return;
				const pending = this.#pending.get(correlationId);
				if (pending === undefined) return;
				clearTimeout(pending.timeout);
				this.#pending.delete(correlationId);
				pending.reject(error);
			});
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
		if (this.#closed) return;
		this.#completeClose(new Error('Host connection closed.'));
		this.#stream.destroy();
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
			this.#handleResponse(envelope);
			return;
		}
		if (!envelope.event) throw new Error('Host emitted an unsolicited request record.');
		const attachmentId = this.#routeAttachmentEvent(envelope);
		if (attachmentId !== null) {
			for (const listener of this.#attachmentListeners.get(attachmentId) ?? []) {
				listener(envelope);
			}
			return;
		}
		for (const listener of this.#listeners) listener(envelope);
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
			case MessageKind.projectionBlocked:
				return decodeControlAttachmentId(envelope, ProjectionBlockedSchema);
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

	#handleResponse(envelope: DecodedProtocolEnvelope): void {
		const correlationId = envelope.correlationId;
		if (correlationId === null) throw new Error('Host response omitted its correlation ID.');
		const pending = this.#pending.get(correlationId);
		if (pending === undefined) throw new Error('Host response correlation is unknown or expired.');
		clearTimeout(pending.timeout);
		this.#pending.delete(correlationId);
		if (envelope.kind === MessageKind.typedFailure) {
			const failure = decodeControlJson(envelope.payload, TypedFailureSchema);
			pending.reject(failure.ok ? new HostRequestError(failure.value) : new Error(failure.detail));
			return;
		}
		if (envelope.kind !== pending.expectedKind) {
			pending.reject(
				new Error(`Host response kind ${envelope.kind} does not match ${pending.expectedKind}.`),
			);
			return;
		}
		pending.resolve(envelope);
	}

	#handleClose(): void {
		if (this.#closed) return;
		let reason = new Error('Host connection closed.');
		try {
			this.#assembler.finish();
		} catch (error) {
			reason = error instanceof Error ? error : new Error('Host transport ended unexpectedly.');
		}
		this.#completeClose(reason);
	}

	#fail(error: Error): void {
		if (this.#closed) return;
		this.#completeClose(error);
		this.#stream.destroy(error);
	}

	#completeClose(reason: Error): void {
		if (this.#closed) return;
		this.#closed = true;
		this.#closedReason = reason;
		this.#rejectPending(reason);
		for (const listener of this.#closedListeners) queueMicrotask(() => listener(reason));
		this.#closedListeners.clear();
	}

	#rejectPending(error: Error): void {
		for (const pending of this.#pending.values()) {
			clearTimeout(pending.timeout);
			pending.reject(error);
		}
		this.#pending.clear();
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
