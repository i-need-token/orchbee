import { Duplex } from 'node:stream';
import { readFile } from 'node:fs/promises';

import {
	ClientIdSchema,
	AttachmentIdSchema,
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

describe('HostConnection', () => {
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
						'../../../../../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.1.bin',
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

		expect(accepted.hostEpoch).toBe(hostEpoch);
		expect(connection.hostEpoch).toBe(hostEpoch);
		expect(connection.protocolVersion).toEqual(currentProtocol);
		expect(listener).toHaveBeenCalledOnce();
		expect(attachmentListener).toHaveBeenCalledTimes(2);
		expect(foreignAttachmentListener).not.toHaveBeenCalled();
		expect(connection.metrics.recordsSent).toBe(1);
		expect(connection.metrics.recordsReceived).toBe(4);
		expect(connection.metrics.terminalFramesReceived).toBe(1);
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
		).rejects.toMatchObject({ code: 'input-backpressure', retryable: true });
		connection.close();
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

		connection.close();
		connection.close();
		connection.onClosed(late);

		await vi.waitFor(() => expect(current).toHaveBeenCalledOnce());
		await vi.waitFor(() => expect(late).toHaveBeenCalledOnce());
		expect(current.mock.calls[0]?.[0].message).toBe('Host connection closed.');
		expect(late.mock.calls[0]?.[0]).toBe(current.mock.calls[0]?.[0]);
	});
});
