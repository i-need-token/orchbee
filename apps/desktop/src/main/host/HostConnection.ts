import { randomBytes } from 'node:crypto';
import { createConnection } from 'node:net';
import type { Duplex } from 'node:stream';

import {
	ClientIdSchema,
	CorrelationIdSchema,
	CURRENT_PROTOCOL_VERSION,
	decodeControlJson,
	decodeProtocolEnvelope,
	encodeProtocolEnvelope,
	HelloAcceptedSchema,
	HelloSchema,
	MessageKind,
	PayloadEncoding,
	PROTOCOL_LIMITS,
	TypedFailureSchema,
	type ClientId,
	type DecodedProtocolEnvelope,
	type HelloAccepted,
	type HostEpoch,
	type MessageKindValue,
	type PayloadEncodingValue,
} from '@orchbee/terminal-protocol';
import type { z } from 'zod';

import { HostRecordAssembler } from './HostRecordAssembler';
import { HostRequestError } from './HostRequestError';

const REQUEST_TIMEOUT_MILLISECONDS = 10_000;
const textEncoder = new TextEncoder();

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

export class HostConnection {
	readonly #assembler: HostRecordAssembler;
	readonly #listeners = new Set<HostEnvelopeListener>();
	readonly #pending = new Map<string, PendingRequest>();
	readonly #stream: Duplex;
	#bytesReceived = 0;
	#bytesSent = 0;
	#closed = false;
	#hostEpoch: HostEpoch | undefined;
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

	async negotiate(clientId: ClientId): Promise<HelloAccepted> {
		if (this.#hostEpoch !== undefined) throw new Error('Host connection is already negotiated.');
		const hello = HelloSchema.parse({
			clientId: ClientIdSchema.parse(clientId),
			frameBytes: PROTOCOL_LIMITS.maxFrameBytes,
			inputBytes: PROTOCOL_LIMITS.maxInputBytes,
			protocol: { major: 1, maximumMinor: 1, minimumMinor: 1 },
			rowWindowBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
			supportedOptionalFeatures: [],
		});
		const envelope = await this.request(
			MessageKind.hello,
			PayloadEncoding.controlJson,
			textEncoder.encode(JSON.stringify(hello)),
			MessageKind.helloAccepted,
			null,
		);
		const accepted = decodeControlJson(envelope.payload, HelloAcceptedSchema);
		if (!accepted.ok) throw new Error(accepted.detail);
		if (accepted.value.protocol.major !== 1 || accepted.value.protocol.minor !== 1) {
			throw new Error('Terminal Host did not negotiate required protocol 1.1.');
		}
		if (envelope.hostEpoch !== accepted.value.hostEpoch) {
			throw new Error('HelloAccepted envelope and payload HostEpoch values differ.');
		}
		this.#hostEpoch = accepted.value.hostEpoch;
		return accepted.value;
	}

	requestControl<Output>(
		kind: MessageKindValue,
		value: unknown,
		expectedKind: MessageKindValue,
		responseSchema: z.ZodType<Output>,
	): Promise<Output> {
		return this.request(
			kind,
			PayloadEncoding.controlJson,
			textEncoder.encode(JSON.stringify(value)),
			expectedKind,
		).then((envelope) => {
			const decoded = decodeControlJson(envelope.payload, responseSchema);
			if (!decoded.ok) throw new Error(decoded.detail);
			return decoded.value;
		});
	}

	request(
		kind: MessageKindValue,
		payloadEncoding: PayloadEncodingValue,
		payload: Uint8Array,
		expectedKind: MessageKindValue,
		hostEpoch: HostEpoch | null = this.#hostEpoch ?? null,
	): Promise<DecodedProtocolEnvelope> {
		if (this.#closed) return Promise.reject(new Error('Host connection is closed.'));
		const correlationId = CorrelationIdSchema.parse(randomBytes(16).toString('hex'));
		const encoded = encodeProtocolEnvelope({
			correlationId,
			event: false,
			hostEpoch,
			kind,
			optionalMessage: false,
			payload,
			payloadEncoding,
			protocol: CURRENT_PROTOCOL_VERSION,
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

	close(): void {
		if (this.#closed) return;
		this.#closed = true;
		this.#stream.destroy();
		this.#rejectPending(new Error('Host connection closed.'));
	}

	#handleRecord(record: Uint8Array): void {
		const decoded = decodeProtocolEnvelope(record);
		if (!decoded.ok) throw new Error(decoded.detail);
		const envelope = decoded.value;
		this.#recordsReceived += 1;
		this.#bytesReceived += record.byteLength;
		if (envelope.protocol.major !== 1 || envelope.protocol.minor !== 1) {
			throw new Error('Host emitted a record outside negotiated protocol 1.1.');
		}
		if (this.#hostEpoch !== undefined && envelope.hostEpoch !== this.#hostEpoch) {
			throw new Error('Host emitted a record for a stale or foreign HostEpoch.');
		}
		if (envelope.kind === MessageKind.snapshotFrame || envelope.kind === MessageKind.deltaFrame) {
			this.#terminalFramesReceived += 1;
		}

		if (envelope.response) {
			this.#handleResponse(envelope);
			return;
		}
		if (!envelope.event) throw new Error('Host emitted an unsolicited request record.');
		for (const listener of this.#listeners) listener(envelope);
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
		if (!this.#closed) {
			try {
				this.#assembler.finish();
			} catch (error) {
				this.#rejectPending(
					error instanceof Error ? error : new Error('Host transport ended unexpectedly.'),
				);
			}
		}
		this.#closed = true;
		this.#rejectPending(new Error('Host connection closed.'));
	}

	#fail(error: Error): void {
		if (this.#closed) return;
		this.#closed = true;
		this.#rejectPending(error);
		this.#stream.destroy(error);
	}

	#rejectPending(error: Error): void {
		for (const pending of this.#pending.values()) {
			clearTimeout(pending.timeout);
			pending.reject(error);
		}
		this.#pending.clear();
	}
}
