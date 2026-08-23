import { Duplex } from 'node:stream';
import { readFile } from 'node:fs/promises';

import {
	AttachmentCapabilitiesSchema,
	ClientIdSchema,
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
	CURRENT_PROTOCOL_VERSION,
	decodeControlJson,
	decodeProtocolEnvelope,
	EmptyControlSchema,
	encodeProtocolEnvelope,
	HelloSchema,
	HostEpochSchema,
	MessageKind,
	PayloadEncoding,
	PROTOCOL_LIMITS,
} from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import { HostConnection, type HostEnvelopeListener } from '../HostConnection';

const currentProtocol = CURRENT_PROTOCOL_VERSION;
const capabilityRevision = CanonicalUint64TextSchema.parse('1');

function createControlledHostStream(highWaterMark: number) {
	const blockedWriteCallbacks: Array<(error?: Error | null) => void> = [];
	const hostEpoch = HostEpochSchema.parse('aa'.repeat(16));
	const stream = new Duplex({
		highWaterMark,
		read() {},
		write(chunk, _encoding, callback) {
			const request = decodeProtocolEnvelope(new Uint8Array(chunk));
			if (!request.ok) throw new Error(request.detail);
			if (request.value.kind !== MessageKind.hello) {
				blockedWriteCallbacks.push(callback);
				return;
			}
			const response = encodeProtocolEnvelope({
				correlationId: request.value.correlationId,
				event: false,
				hostEpoch,
				kind: MessageKind.helloAccepted,
				optionalMessage: false,
				payload: new TextEncoder().encode(
					JSON.stringify({
						frameBytes: PROTOCOL_LIMITS.maxFrameBytes,
						hostEpoch,
						hostVersion: '0.1.0',
						inputBytes: PROTOCOL_LIMITS.maxInputBytes,
						protocol: currentProtocol,
						rowWindowBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
						supportedOptionalFeatures: [],
					}),
				),
				payloadEncoding: PayloadEncoding.controlJson,
				protocol: currentProtocol,
				response: true,
			});
			if (!response.ok) throw new Error(response.detail);
			stream.push(response.value);
			callback();
		},
	});
	return { blockedWriteCallbacks, hostEpoch, stream };
}

async function createNegotiatedControlledHost(highWaterMark: number) {
	const controlled = createControlledHostStream(highWaterMark);
	const connection = new HostConnection(controlled.stream);
	await connection.negotiate(ClientIdSchema.parse('bb'.repeat(16)));
	return { connection, ...controlled };
}

function concatenateRecords(records: readonly Uint8Array[]): Uint8Array {
	const byteLength = records.reduce((total, record) => total + record.byteLength, 0);
	const combined = new Uint8Array(byteLength);
	let offset = 0;
	for (const record of records) {
		combined.set(record, offset);
		offset += record.byteLength;
	}
	return combined;
}

describe('HostConnection', () => {
	it('decodes an all-false current capability response after an input-only downgrade', async () => {
		const hostEpoch = HostEpochSchema.parse('10'.repeat(16));
		const attachmentId = AttachmentIdSchema.parse('20'.repeat(16));
		const terminalId = '30'.repeat(16);
		let stream: Duplex;
		stream = new Duplex({
			read() {},
			write(chunk, _encoding, callback) {
				const request = decodeProtocolEnvelope(new Uint8Array(chunk));
				if (!request.ok) throw new Error(request.detail);
				const hello = request.value.kind === MessageKind.hello;
				if (!hello) {
					expect(request.value.kind).toBe(MessageKind.updateAttachmentInputCapability);
				}
				const response = encodeProtocolEnvelope({
					correlationId: request.value.correlationId,
					event: false,
					hostEpoch,
					kind: hello ? MessageKind.helloAccepted : MessageKind.attachmentCapabilities,
					optionalMessage: false,
					payload: new TextEncoder().encode(
						JSON.stringify(
							hello
								? {
										frameBytes: PROTOCOL_LIMITS.maxFrameBytes,
										hostEpoch,
										hostVersion: '0.1.0',
										inputBytes: PROTOCOL_LIMITS.maxInputBytes,
										protocol: currentProtocol,
										rowWindowBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
										supportedOptionalFeatures: [],
									}
								: {
										attachmentId,
										capabilities: { input: false, read: false, resize: false },
										capabilityRevision: '2',
										resizeLease: false,
										terminalId,
									},
						),
					),
					payloadEncoding: PayloadEncoding.controlJson,
					protocol: currentProtocol,
					response: true,
				});
				if (!response.ok) throw new Error(response.detail);
				stream.push(response.value);
				callback();
			},
		});
		const connection = new HostConnection(stream);
		await connection.negotiate(ClientIdSchema.parse('40'.repeat(16)));

		const capabilities = await connection.requestControl(
			MessageKind.updateAttachmentInputCapability,
			{ attachmentId, capabilityRevision: '2', input: false },
			AttachmentCapabilitiesSchema,
		);

		expect(capabilities.capabilities).toEqual({ input: false, read: false, resize: false });
		expect(capabilities.capabilityRevision).toBe('2');
		connection.close();
	});

	it('offers the current protocol and routes current-version events', async () => {
		const hostEpoch = HostEpochSchema.parse('11'.repeat(16));
		let stream: Duplex;
		stream = new Duplex({
			read() {},
			write(chunk, _encoding, callback) {
				const request = decodeProtocolEnvelope(new Uint8Array(chunk));
				if (!request.ok) throw new Error(request.detail);
				expect(request.value.protocol).toEqual(CURRENT_PROTOCOL_VERSION);
				const hello = decodeControlJson(request.value.payload, HelloSchema);
				if (!hello.ok) throw new Error(hello.detail);
				expect(hello.value.protocol).toEqual(CURRENT_PROTOCOL_VERSION);
				const response = encodeProtocolEnvelope({
					correlationId: request.value.correlationId,
					event: false,
					hostEpoch,
					kind: MessageKind.helloAccepted,
					optionalMessage: false,
					payload: new TextEncoder().encode(
						JSON.stringify({
							frameBytes: PROTOCOL_LIMITS.maxFrameBytes,
							hostEpoch,
							hostVersion: '0.1.0',
							inputBytes: PROTOCOL_LIMITS.maxInputBytes,
							protocol: currentProtocol,
							rowWindowBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
							supportedOptionalFeatures: [],
						}),
					),
					payloadEncoding: PayloadEncoding.controlJson,
					protocol: currentProtocol,
					response: true,
				});
				if (!response.ok) throw new Error(response.detail);
				stream.push(response.value);
				callback();
			},
		});
		const connection = new HostConnection(stream);
		const accepted = await connection.negotiate(ClientIdSchema.parse('22'.repeat(16)));
		await new Promise<void>((resolve) => setImmediate(resolve));
		const listener = vi.fn<HostEnvelopeListener>();
		const attachmentListener = vi.fn<HostEnvelopeListener>();
		const foreignAttachmentListener = vi.fn<HostEnvelopeListener>();
		connection.onEnvelope(listener);
		connection.onAttachmentEnvelope(AttachmentIdSchema.parse('22'.repeat(16)), attachmentListener);
		connection.onAttachmentEnvelope(
			AttachmentIdSchema.parse('33'.repeat(16)),
			foreignAttachmentListener,
		);
		const event = encodeProtocolEnvelope({
			correlationId: null,
			event: true,
			hostEpoch,
			kind: MessageKind.terminalStarted,
			optionalMessage: false,
			payload: new TextEncoder().encode('{}'),
			payloadEncoding: PayloadEncoding.controlJson,
			protocol: currentProtocol,
			response: false,
		});
		if (!event.ok) throw new Error(event.detail);
		stream.push(event.value);
		const frame = encodeProtocolEnvelope({
			correlationId: null,
			event: true,
			hostEpoch,
			kind: MessageKind.deltaFrame,
			optionalMessage: false,
			payload: new Uint8Array(
				await readFile(
					new URL(
						'../../../../../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin',
						import.meta.url,
					),
				),
			),
			payloadEncoding: PayloadEncoding.terminalFrame,
			protocol: currentProtocol,
			response: false,
		});
		if (!frame.ok) throw new Error(frame.detail);
		stream.push(frame.value);
		const selection = encodeProtocolEnvelope({
			correlationId: null,
			event: true,
			hostEpoch,
			kind: MessageKind.selectionChanged,
			optionalMessage: false,
			payload: new TextEncoder().encode(
				JSON.stringify({ attachmentId: '22'.repeat(16), state: 'none' }),
			),
			payloadEncoding: PayloadEncoding.controlJson,
			protocol: currentProtocol,
			response: false,
		});
		if (!selection.ok) throw new Error(selection.detail);
		stream.push(selection.value);
		const bell = encodeProtocolEnvelope({
			correlationId: null,
			event: true,
			hostEpoch,
			kind: MessageKind.terminalBell,
			optionalMessage: false,
			payload: new TextEncoder().encode(
				JSON.stringify({ attachmentId: '22'.repeat(16), sequence: '1' }),
			),
			payloadEncoding: PayloadEncoding.controlJson,
			protocol: currentProtocol,
			response: false,
		});
		if (!bell.ok) throw new Error(bell.detail);
		stream.push(bell.value);
		const clipboardWrite = encodeProtocolEnvelope({
			correlationId: null,
			event: true,
			hostEpoch,
			kind: MessageKind.terminalClipboardWriteRequest,
			optionalMessage: false,
			payload: new TextEncoder().encode(
				JSON.stringify({
					attachmentId: '22'.repeat(16),
					sequence: '2',
					text: 'terminal text',
				}),
			),
			payloadEncoding: PayloadEncoding.controlJson,
			protocol: currentProtocol,
			response: false,
		});
		if (!clipboardWrite.ok) throw new Error(clipboardWrite.detail);
		stream.push(clipboardWrite.value);

		expect(accepted.hostEpoch).toBe(hostEpoch);
		expect(connection.hostEpoch).toBe(hostEpoch);
		expect(connection.protocolVersion).toEqual(currentProtocol);
		expect(listener).toHaveBeenCalledOnce();
		expect(attachmentListener).toHaveBeenCalledTimes(4);
		expect(foreignAttachmentListener).not.toHaveBeenCalled();
		expect(connection.metrics.recordsSent).toBe(1);
		expect(connection.metrics.recordsReceived).toBe(6);
		expect(connection.metrics.terminalFramesReceived).toBe(1);
		expect(connection.metrics).toMatchObject({
			outboundAdmittedBytes: 0,
			outboundAdmittedRecords: 0,
		});
		connection.close();
	});

	it('rejects a different Host protocol instead of entering a compatibility mode', async () => {
		const hostEpoch = HostEpochSchema.parse('12'.repeat(16));
		const differentProtocol = {
			...CURRENT_PROTOCOL_VERSION,
			patch: CURRENT_PROTOCOL_VERSION.patch + 1,
		};
		let stream: Duplex;
		stream = new Duplex({
			read() {},
			write(chunk, _encoding, callback) {
				const request = decodeProtocolEnvelope(new Uint8Array(chunk));
				if (!request.ok) throw new Error(request.detail);
				const response = encodeProtocolEnvelope({
					correlationId: request.value.correlationId,
					event: false,
					hostEpoch,
					kind: MessageKind.helloAccepted,
					optionalMessage: false,
					payload: new TextEncoder().encode(
						JSON.stringify({
							frameBytes: PROTOCOL_LIMITS.maxFrameBytes,
							hostEpoch,
							hostVersion: '0.1.0',
							inputBytes: PROTOCOL_LIMITS.maxInputBytes,
							protocol: differentProtocol,
							rowWindowBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
							supportedOptionalFeatures: [],
						}),
					),
					payloadEncoding: PayloadEncoding.controlJson,
					protocol: differentProtocol,
					response: true,
				});
				if (!response.ok) throw new Error(response.detail);
				stream.push(response.value);
				callback();
			},
		});
		const connection = new HostConnection(stream);

		await expect(connection.negotiate(ClientIdSchema.parse('23'.repeat(16)))).rejects.toThrow(
			'outside the negotiated Desktop protocol',
		);
	});

	it('rejects a correlated typed failure without treating it as an event', async () => {
		const hostEpoch = HostEpochSchema.parse('33'.repeat(16));
		let requestCount = 0;
		let stream: Duplex;
		stream = new Duplex({
			read() {},
			write(chunk, _encoding, callback) {
				requestCount += 1;
				const request = decodeProtocolEnvelope(new Uint8Array(chunk));
				if (!request.ok) throw new Error(request.detail);
				const hello = requestCount === 1;
				const response = encodeProtocolEnvelope({
					correlationId: request.value.correlationId,
					event: false,
					hostEpoch,
					kind: hello ? MessageKind.helloAccepted : MessageKind.typedFailure,
					optionalMessage: false,
					payload: new TextEncoder().encode(
						JSON.stringify(
							hello
								? {
										frameBytes: PROTOCOL_LIMITS.maxFrameBytes,
										hostEpoch,
										hostVersion: '0.1.0',
										inputBytes: PROTOCOL_LIMITS.maxInputBytes,
										protocol: currentProtocol,
										rowWindowBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
										supportedOptionalFeatures: [],
									}
								: {
										code: 'input-backpressure',
										message: 'The bounded input queue is full.',
										retryable: true,
									},
						),
					),
					payloadEncoding: PayloadEncoding.controlJson,
					protocol: currentProtocol,
					response: true,
				});
				if (!response.ok) throw new Error(response.detail);
				stream.push(response.value);
				callback();
			},
		});
		const connection = new HostConnection(stream);
		await connection.negotiate(ClientIdSchema.parse('44'.repeat(16)));

		await expect(
			connection.requestControl(MessageKind.ping, {}, EmptyControlSchema),
		).rejects.toMatchObject({
			code: 'input-backpressure',
			correlationId: expect.stringMatching(/^[0-9a-f]{32}$/),
			retryable: true,
		});
		expect(connection.metrics).toMatchObject({
			outboundAdmittedBytes: 0,
			outboundAdmittedRecords: 0,
		});
		connection.close();
	});

	it('delivers events and correlated responses in Host wire order across Promise settlement', async () => {
		const controlled = await createNegotiatedControlledHost(64 * 1024);
		const write = vi.spyOn(controlled.stream, 'write');
		const deliveryOrder: string[] = [];
		let eventSequence = 0;
		controlled.connection.onEnvelope(() => {
			eventSequence += 1;
			deliveryOrder.push(`event-${eventSequence}`);
		});
		const firstResponse = controlled.connection
			.requestControl(MessageKind.ping, {}, EmptyControlSchema)
			.then(() => deliveryOrder.push('response-1'));
		const secondResponse = controlled.connection
			.requestControl(MessageKind.ping, {}, EmptyControlSchema)
			.then(() => deliveryOrder.push('response-2'));

		const correlationIds = write.mock.calls.map((call) => {
			const chunk: unknown = call[0];
			if (!(chunk instanceof Uint8Array)) throw new Error('Expected a binary Host request.');
			const request = decodeProtocolEnvelope(chunk);
			if (!request.ok) throw new Error(request.detail);
			if (request.value.correlationId === null) {
				throw new Error('Expected a correlated Host request.');
			}
			return request.value.correlationId;
		});
		const responseRecords = correlationIds.map((correlationId) => {
			const response = encodeProtocolEnvelope({
				correlationId,
				event: false,
				hostEpoch: controlled.hostEpoch,
				kind: MessageKind.ping,
				optionalMessage: false,
				payload: new TextEncoder().encode('{}'),
				payloadEncoding: PayloadEncoding.controlJson,
				protocol: currentProtocol,
				response: true,
			});
			if (!response.ok) throw new Error(response.detail);
			return response.value;
		});
		const eventRecords = Array.from({ length: 3 }, () => {
			const event = encodeProtocolEnvelope({
				correlationId: null,
				event: true,
				hostEpoch: controlled.hostEpoch,
				kind: MessageKind.terminalStarted,
				optionalMessage: false,
				payload: new TextEncoder().encode('{}'),
				payloadEncoding: PayloadEncoding.controlJson,
				protocol: currentProtocol,
				response: false,
			});
			if (!event.ok) throw new Error(event.detail);
			return event.value;
		});
		const firstEncodedResponse = responseRecords[0];
		const secondEncodedResponse = responseRecords[1];
		const eventBefore = eventRecords[0];
		const eventBetween = eventRecords[1];
		const eventAfter = eventRecords[2];
		if (
			firstEncodedResponse === undefined ||
			secondEncodedResponse === undefined ||
			eventBefore === undefined ||
			eventBetween === undefined ||
			eventAfter === undefined
		) {
			throw new Error('Expected complete Host ordering fixtures.');
		}

		controlled.stream.emit(
			'data',
			concatenateRecords([
				eventBefore,
				firstEncodedResponse,
				eventBetween,
				secondEncodedResponse,
				eventAfter,
			]),
		);
		await Promise.all([firstResponse, secondResponse]);
		await new Promise<void>((resolve) => setImmediate(resolve));

		expect(deliveryOrder).toEqual(['event-1', 'response-1', 'event-2', 'response-2', 'event-3']);
		for (const callback of controlled.blockedWriteCallbacks) callback();
		controlled.connection.close();
	});

	it('lets an attach-response continuation install frame and close listeners before later wire items', async () => {
		const controlled = await createNegotiatedControlledHost(64 * 1024);
		await new Promise<void>((resolve) => setImmediate(resolve));
		const write = vi.spyOn(controlled.stream, 'write');
		const attachmentId = AttachmentIdSchema.parse('cc'.repeat(16));
		const deliveryOrder: string[] = [];
		const attachmentListener = vi.fn<HostEnvelopeListener>(() => deliveryOrder.push('event'));
		const closeListener = vi.fn<(reason: Error) => void>(() => deliveryOrder.push('close'));
		const response = controlled.connection
			.requestControl(MessageKind.ping, {}, EmptyControlSchema)
			.then(() => {
				deliveryOrder.push('response');
				controlled.connection.onAttachmentEnvelope(attachmentId, attachmentListener);
				controlled.connection.onClosed(closeListener);
			});
		const chunk: unknown = write.mock.calls.at(-1)?.[0];
		if (!(chunk instanceof Uint8Array)) throw new Error('Expected a binary Host request.');
		const request = decodeProtocolEnvelope(chunk);
		if (!request.ok || request.value.correlationId === null) {
			throw new Error('Expected a correlated Host request.');
		}
		const encodedResponse = encodeProtocolEnvelope({
			correlationId: request.value.correlationId,
			event: false,
			hostEpoch: controlled.hostEpoch,
			kind: MessageKind.ping,
			optionalMessage: false,
			payload: new TextEncoder().encode('{}'),
			payloadEncoding: PayloadEncoding.controlJson,
			protocol: currentProtocol,
			response: true,
		});
		const encodedEvent = encodeProtocolEnvelope({
			correlationId: null,
			event: true,
			hostEpoch: controlled.hostEpoch,
			kind: MessageKind.selectionChanged,
			optionalMessage: false,
			payload: new TextEncoder().encode(JSON.stringify({ attachmentId, state: 'none' })),
			payloadEncoding: PayloadEncoding.controlJson,
			protocol: currentProtocol,
			response: false,
		});
		if (!encodedResponse.ok) throw new Error(encodedResponse.detail);
		if (!encodedEvent.ok) throw new Error(encodedEvent.detail);

		controlled.stream.emit('data', concatenateRecords([encodedResponse.value, encodedEvent.value]));
		controlled.stream.destroy();

		await response;
		await vi.waitFor(() => expect(closeListener).toHaveBeenCalledOnce());
		expect(attachmentListener).toHaveBeenCalledOnce();
		expect(deliveryOrder).toEqual(['response', 'event', 'close']);
	});

	it('rejects a different registered correlated response kind without replaying', async () => {
		const hostEpoch = HostEpochSchema.parse('55'.repeat(16));
		let nonHelloRequestCount = 0;
		let stream: Duplex;
		stream = new Duplex({
			read() {},
			write(chunk, _encoding, callback) {
				const request = decodeProtocolEnvelope(new Uint8Array(chunk));
				if (!request.ok) throw new Error(request.detail);
				const hello = request.value.kind === MessageKind.hello;
				if (!hello) {
					nonHelloRequestCount += 1;
					expect(request.value.kind).toBe(MessageKind.ping);
				}
				const response = encodeProtocolEnvelope({
					correlationId: request.value.correlationId,
					event: false,
					hostEpoch,
					kind: hello ? MessageKind.helloAccepted : MessageKind.listTerminals,
					optionalMessage: false,
					payload: new TextEncoder().encode(
						JSON.stringify(
							hello
								? {
										frameBytes: PROTOCOL_LIMITS.maxFrameBytes,
										hostEpoch,
										hostVersion: '0.1.0',
										inputBytes: PROTOCOL_LIMITS.maxInputBytes,
										protocol: currentProtocol,
										rowWindowBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
										supportedOptionalFeatures: [],
									}
								: {},
						),
					),
					payloadEncoding: PayloadEncoding.controlJson,
					protocol: currentProtocol,
					response: true,
				});
				if (!response.ok) throw new Error(response.detail);
				stream.push(response.value);
				callback();
			},
		});
		const connection = new HostConnection(stream);
		await connection.negotiate(ClientIdSchema.parse('66'.repeat(16)));

		await expect(connection.requestControlEnvelope(MessageKind.ping, {})).rejects.toThrow(
			`Host response kind ${MessageKind.listTerminals} does not match ${MessageKind.ping}.`,
		);
		await new Promise<void>((resolve) => setImmediate(resolve));
		expect(nonHelloRequestCount).toBe(1);
		expect(connection.metrics.recordsSent).toBe(2);
		expect(connection.metrics).toMatchObject({
			outboundAdmittedBytes: 0,
			outboundAdmittedRecords: 0,
		});
		connection.close();
	});

	it('stops request writes after Writable backpressure until drain', async () => {
		const attachmentId = AttachmentIdSchema.parse('88'.repeat(16));
		const { blockedWriteCallbacks, connection, stream } =
			await createNegotiatedControlledHost(4096);
		const write = vi.spyOn(stream, 'write');
		const submitLargeInput = () =>
			connection.requestInput(MessageKind.textInput, {
				attachmentId,
				capabilityRevision,
				text: 'x'.repeat(PROTOCOL_LIMITS.maxInputBytes),
			});
		const submitControl = () => connection.requestControlEnvelope(MessageKind.ping, {});
		const requests = [
			submitLargeInput(),
			submitControl(),
			connection.requestInput(MessageKind.focusInput, {
				attachmentId,
				capabilityRevision,
				focused: true,
			}),
		];

		try {
			const rejected = await Promise.allSettled(requests.slice(1));
			expect(write.mock.results[0]?.value).toBe(false);
			expect(stream.writableNeedDrain).toBe(true);
			expect(write).toHaveBeenCalledOnce();
			expect(rejected).toEqual([
				expect.objectContaining({
					reason: expect.objectContaining({
						code: 'host-transport-backpressure',
						retryable: true,
					}),
					status: 'rejected',
				}),
				expect.objectContaining({
					reason: expect.objectContaining({
						code: 'host-transport-backpressure',
						retryable: true,
					}),
					status: 'rejected',
				}),
			]);
			expect(connection.metrics.outboundAdmittedRecords).toBe(1);
			expect(connection.metrics.outboundAdmittedBytes).toBe(stream.writableLength);
			expect(connection.metrics.outboundAdmittedBytes).toBeLessThanOrEqual(
				HostConnection.outboundAdmissionLimits.bytes,
			);
			expect(connection.metrics.outboundWriteBlocked).toBe(true);
			const drained = new Promise<void>((resolve) => stream.once('drain', resolve));
			blockedWriteCallbacks.shift()?.();
			await drained;
			expect(connection.metrics.outboundWriteBlocked).toBe(false);
			requests.push(submitControl());
			expect(write).toHaveBeenCalledTimes(2);
			expect(write.mock.results[1]?.value).toBe(true);
		} finally {
			connection.close();
			await Promise.allSettled(requests);
			expect(connection.metrics).toMatchObject({
				outboundAdmittedBytes: 0,
				outboundAdmittedRecords: 0,
			});
		}
	});

	it('enforces aggregate byte and record admission limits across callers', async () => {
		const attachmentId = AttachmentIdSchema.parse('cc'.repeat(16));
		const maximumText = 'x'.repeat(PROTOCOL_LIMITS.maxInputBytes);
		const byteControlled = await createNegotiatedControlledHost(
			HostConnection.outboundAdmissionLimits.bytes + 1024,
		);
		const byteWrite = vi.spyOn(byteControlled.stream, 'write');
		const submitFromFirstCaller = () =>
			byteControlled.connection.requestInput(MessageKind.textInput, {
				attachmentId,
				capabilityRevision,
				text: maximumText,
			});
		const submitFromSecondCaller = () =>
			byteControlled.connection.requestInput(MessageKind.pasteInput, {
				attachmentId,
				bracketed: false,
				capabilityRevision,
				text: maximumText,
			});
		const byteRequests = [
			submitFromFirstCaller(),
			submitFromSecondCaller(),
			submitFromFirstCaller(),
		];
		const byteOverflow = submitFromSecondCaller();

		try {
			await expect(byteOverflow).rejects.toMatchObject({
				code: 'host-transport-backpressure',
				retryable: true,
			});
			expect(byteWrite).toHaveBeenCalledTimes(3);
			expect(byteControlled.connection.metrics.outboundAdmittedRecords).toBe(3);
			expect(byteControlled.connection.metrics.outboundAdmittedBytes).toBe(
				byteControlled.stream.writableLength,
			);
			expect(byteControlled.connection.metrics.outboundAdmittedBytes).toBeLessThanOrEqual(
				HostConnection.outboundAdmissionLimits.bytes,
			);
		} finally {
			byteControlled.connection.close();
			await Promise.allSettled([...byteRequests, byteOverflow]);
		}

		const recordControlled = await createNegotiatedControlledHost(64 * 1024);
		const recordWrite = vi.spyOn(recordControlled.stream, 'write');
		const recordRequests = Array.from(
			{ length: HostConnection.outboundAdmissionLimits.records },
			(_, index) =>
				index % 2 === 0
					? recordControlled.connection.requestControlEnvelope(MessageKind.ping, {})
					: recordControlled.connection.requestInput(MessageKind.focusInput, {
							attachmentId,
							capabilityRevision,
							focused: true,
						}),
		);
		const recordOverflow = recordControlled.connection.requestControlEnvelope(MessageKind.ping, {});

		try {
			await expect(recordOverflow).rejects.toMatchObject({
				code: 'host-transport-backpressure',
				retryable: true,
			});
			expect(recordWrite).toHaveBeenCalledTimes(HostConnection.outboundAdmissionLimits.records);
			expect(recordControlled.connection.metrics.outboundAdmittedRecords).toBe(
				HostConnection.outboundAdmissionLimits.records,
			);
			expect(recordControlled.connection.metrics.outboundAdmittedBytes).toBeLessThanOrEqual(
				HostConnection.outboundAdmissionLimits.bytes,
			);
		} finally {
			recordControlled.connection.close();
			await Promise.allSettled([...recordRequests, recordOverflow]);
		}
	});

	it('retires every handed-off request as outcome unknown at the first deadline', async () => {
		vi.useFakeTimers();
		const controlled = await createNegotiatedControlledHost(4096);
		try {
			const attachmentId = AttachmentIdSchema.parse('dd'.repeat(16));
			const write = vi.spyOn(controlled.stream, 'write');
			const closeListener = vi.fn<(reason: Error) => void>();
			controlled.connection.onClosed(closeListener);
			const requests = [
				controlled.connection.requestControlEnvelope(MessageKind.ping, {}),
				controlled.connection.requestInput(MessageKind.textInput, {
					attachmentId,
					capabilityRevision,
					text: 'x'.repeat(PROTOCOL_LIMITS.maxInputBytes),
				}),
			];
			const settlementCounts = [0, 0];
			const observedRequests = requests.map((request, index) =>
				request.then(
					(value) => {
						settlementCounts[index] = (settlementCounts[index] ?? 0) + 1;
						return value;
					},
					(error: unknown) => {
						settlementCounts[index] = (settlementCounts[index] ?? 0) + 1;
						return error;
					},
				),
			);

			expect(write.mock.results.map((result) => result.value)).toEqual([true, false]);
			expect(controlled.connection.metrics).toMatchObject({
				outboundAdmittedBytes: controlled.stream.writableLength,
				outboundAdmittedRecords: 2,
				outboundWriteBlocked: true,
			});

			await vi.advanceTimersByTimeAsync(10_000);
			const outcomes = await Promise.all(observedRequests);
			expect(outcomes[0]).toMatchObject({
				code: 'host-request-outcome-unknown',
				retryable: false,
			});
			expect(outcomes[1]).toBe(outcomes[0]);
			expect(settlementCounts).toEqual([1, 1]);
			expect(controlled.stream.destroyed).toBe(true);
			expect(controlled.connection.metrics).toMatchObject({
				outboundAdmittedBytes: 0,
				outboundAdmittedRecords: 0,
				outboundWriteBlocked: false,
			});
			await Promise.resolve();
			expect(closeListener).toHaveBeenCalledOnce();
			expect(closeListener.mock.calls[0]?.[0]).toBe(outcomes[0]);

			let futureOutcome: unknown;
			try {
				await controlled.connection.requestControlEnvelope(MessageKind.ping, {});
			} catch (error) {
				futureOutcome = error;
			}
			expect(futureOutcome).toBe(outcomes[0]);

			const firstChunk: unknown = write.mock.calls[0]?.[0];
			if (!(firstChunk instanceof Uint8Array)) {
				throw new Error('Expected the first handed-off request to be binary.');
			}
			const firstRequest = decodeProtocolEnvelope(firstChunk);
			if (!firstRequest.ok) throw new Error(firstRequest.detail);
			const lateResponse = encodeProtocolEnvelope({
				correlationId: firstRequest.value.correlationId,
				event: false,
				hostEpoch: controlled.hostEpoch,
				kind: MessageKind.ping,
				optionalMessage: false,
				payload: new TextEncoder().encode('{}'),
				payloadEncoding: PayloadEncoding.controlJson,
				protocol: currentProtocol,
				response: true,
			});
			if (!lateResponse.ok) throw new Error(lateResponse.detail);
			const lateCloseListener = vi.fn<(reason: Error) => void>();
			controlled.connection.onClosed(lateCloseListener);
			controlled.blockedWriteCallbacks.shift()?.();
			controlled.stream.emit('drain');
			controlled.stream.emit('data', lateResponse.value);
			await Promise.resolve();

			expect(settlementCounts).toEqual([1, 1]);
			expect(closeListener).toHaveBeenCalledOnce();
			expect(lateCloseListener).toHaveBeenCalledOnce();
			expect(lateCloseListener.mock.calls[0]?.[0]).toBe(outcomes[0]);
			expect(controlled.connection.metrics).toMatchObject({
				outboundAdmittedBytes: 0,
				outboundAdmittedRecords: 0,
				recordsReceived: 1,
			});
		} finally {
			controlled.connection.close();
			controlled.blockedWriteCallbacks.shift()?.();
			vi.useRealTimers();
		}
	});

	it('keeps repeated 64-command caller waves bounded around saturation and retirement', async () => {
		vi.useFakeTimers();
		const controlled = await createNegotiatedControlledHost(4096);
		try {
			const attachmentId = AttachmentIdSchema.parse('ee'.repeat(16));
			const write = vi.spyOn(controlled.stream, 'write');
			const handedOff = controlled.connection.requestInput(MessageKind.textInput, {
				attachmentId,
				capabilityRevision,
				text: 'x'.repeat(PROTOCOL_LIMITS.maxInputBytes),
			});
			const handedOffOutcome = handedOff.catch((error: unknown) => error);
			const saturatedWritableLength = controlled.stream.writableLength;

			const submitSaturatedWave = async () => {
				const waveResults = await Promise.allSettled(
					Array.from({ length: 64 }, (_, index) =>
						index % 2 === 0
							? controlled.connection.requestControlEnvelope(MessageKind.ping, {})
							: controlled.connection.requestInput(MessageKind.focusInput, {
									attachmentId,
									capabilityRevision,
									focused: true,
								}),
					),
				);
				expect(waveResults).toHaveLength(64);
				expect(
					waveResults.every(
						(result) =>
							result.status === 'rejected' &&
							result.reason instanceof Error &&
							'code' in result.reason &&
							result.reason.code === 'host-transport-backpressure',
					),
				).toBe(true);
				expect(write).toHaveBeenCalledOnce();
				expect(controlled.stream.writableLength).toBe(saturatedWritableLength);
				expect(controlled.connection.metrics).toMatchObject({
					outboundAdmittedBytes: saturatedWritableLength,
					outboundAdmittedRecords: 1,
				});
			};
			await submitSaturatedWave();
			await vi.advanceTimersByTimeAsync(3000);
			await submitSaturatedWave();
			await vi.advanceTimersByTimeAsync(3000);
			await submitSaturatedWave();
			await vi.advanceTimersByTimeAsync(3000);

			await vi.advanceTimersByTimeAsync(1000);
			const timeoutOutcome = await handedOffOutcome;
			expect(timeoutOutcome).toMatchObject({
				code: 'host-request-outcome-unknown',
				retryable: false,
			});
			await vi.advanceTimersByTimeAsync(20_000);
			const retiredWave = await Promise.allSettled(
				Array.from({ length: 64 }, () =>
					controlled.connection.requestControlEnvelope(MessageKind.ping, {}),
				),
			);
			expect(
				retiredWave.every(
					(result) => result.status === 'rejected' && result.reason === timeoutOutcome,
				),
			).toBe(true);
			expect(write).toHaveBeenCalledOnce();
			expect(controlled.stream.writableLength).toBeLessThanOrEqual(saturatedWritableLength);
			expect(controlled.connection.metrics).toMatchObject({
				outboundAdmittedBytes: 0,
				outboundAdmittedRecords: 0,
			});
		} finally {
			controlled.connection.close();
			controlled.blockedWriteCallbacks.shift()?.();
			vi.useRealTimers();
		}
	});

	it('releases all admission when a handed-off write callback fails', async () => {
		const controlled = await createNegotiatedControlledHost(4096);
		const request = controlled.connection.requestControlEnvelope(MessageKind.ping, {});
		const outcome = request.catch((error: unknown) => error);
		expect(controlled.connection.metrics.outboundAdmittedRecords).toBe(1);

		controlled.blockedWriteCallbacks.shift()?.(new Error('blocked write failed'));
		await expect(outcome).resolves.toMatchObject({
			code: 'host-request-outcome-unknown',
			retryable: false,
		});
		expect(controlled.connection.metrics).toMatchObject({
			outboundAdmittedBytes: 0,
			outboundAdmittedRecords: 0,
		});
		expect(controlled.stream.destroyed).toBe(true);
	});

	it('reports an external close with handed-off work as outcome unknown', async () => {
		const controlled = await createNegotiatedControlledHost(4096);
		const outcome = controlled.connection
			.requestControlEnvelope(MessageKind.ping, {})
			.catch((error: unknown) => error);
		expect(controlled.connection.metrics.outboundAdmittedRecords).toBe(1);

		controlled.stream.destroy();
		await expect(outcome).resolves.toMatchObject({
			code: 'host-request-outcome-unknown',
			retryable: false,
		});
		expect(controlled.connection.metrics).toMatchObject({
			outboundAdmittedBytes: 0,
			outboundAdmittedRecords: 0,
		});
	});

	it('notifies current and late close observers exactly once', async () => {
		const stream = new Duplex({
			read() {},
			write(_chunk, _encoding, callback) {
				callback();
			},
		});
		const connection = new HostConnection(stream);
		const current = vi.fn<(reason: Error) => void>();
		const late = vi.fn<(reason: Error) => void>();
		connection.onClosed(current);
		const negotiationOutcome = connection
			.negotiate(ClientIdSchema.parse('ff'.repeat(16)))
			.catch((error: unknown) => error);
		expect(connection.metrics.outboundAdmittedRecords).toBe(1);

		connection.close();
		connection.close();
		connection.onClosed(late);

		await vi.waitFor(() => expect(current).toHaveBeenCalledOnce());
		await vi.waitFor(() => expect(late).toHaveBeenCalledOnce());
		expect(current.mock.calls[0]?.[0].message).toBe('Host connection closed.');
		expect(late.mock.calls[0]?.[0]).toBe(current.mock.calls[0]?.[0]);
		expect(await negotiationOutcome).toBe(current.mock.calls[0]?.[0]);
		expect(connection.metrics).toMatchObject({
			outboundAdmittedBytes: 0,
			outboundAdmittedRecords: 0,
		});
	});

	it('exposes request-transport retirement before close continuation delivery', async () => {
		const stream = new Duplex({
			read() {},
			write(_chunk, _encoding, callback) {
				callback();
			},
		});
		const connection = new HostConnection(stream);
		const closeListener = vi.fn<(reason: Error) => void>();
		connection.onClosed(closeListener);

		expect(connection.requestTransportEnded).toBe(false);
		connection.close();
		expect(connection.requestTransportEnded).toBe(true);
		await vi.waitFor(() => expect(closeListener).toHaveBeenCalledOnce());
	});
});
