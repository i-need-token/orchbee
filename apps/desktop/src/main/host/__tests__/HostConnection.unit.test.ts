import { Duplex } from 'node:stream';

import {
	ClientIdSchema,
	CURRENT_PROTOCOL_VERSION,
	decodeProtocolEnvelope,
	encodeProtocolEnvelope,
	HostEpochSchema,
	MessageKind,
	PayloadEncoding,
	PROTOCOL_LIMITS,
} from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import { HostConnection, type HostEnvelopeListener } from '../HostConnection';

describe('HostConnection', () => {
	it('negotiates the current protocol and routes validated Host events', async () => {
		const hostEpoch = HostEpochSchema.parse('11'.repeat(16));
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
							inputBytes: PROTOCOL_LIMITS.maxInputBytes,
							protocol: CURRENT_PROTOCOL_VERSION,
							rowWindowBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
							supportedOptionalFeatures: [],
						}),
					),
					payloadEncoding: PayloadEncoding.controlJson,
					protocol: CURRENT_PROTOCOL_VERSION,
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
		connection.onEnvelope(listener);
		const event = encodeProtocolEnvelope({
			correlationId: null,
			event: true,
			hostEpoch,
			kind: MessageKind.terminalStarted,
			optionalMessage: false,
			payload: new TextEncoder().encode('{}'),
			payloadEncoding: PayloadEncoding.controlJson,
			protocol: CURRENT_PROTOCOL_VERSION,
			response: false,
		});
		if (!event.ok) throw new Error(event.detail);
		stream.push(event.value);

		expect(accepted.hostEpoch).toBe(hostEpoch);
		expect(connection.hostEpoch).toBe(hostEpoch);
		expect(listener).toHaveBeenCalledOnce();
		expect(connection.metrics.recordsSent).toBe(1);
		expect(connection.metrics.recordsReceived).toBe(2);
		connection.close();
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
										inputBytes: PROTOCOL_LIMITS.maxInputBytes,
										protocol: CURRENT_PROTOCOL_VERSION,
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
					protocol: CURRENT_PROTOCOL_VERSION,
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
			connection.request(
				MessageKind.ping,
				PayloadEncoding.controlJson,
				new TextEncoder().encode('{}'),
				MessageKind.ping,
			),
		).rejects.toMatchObject({ code: 'input-backpressure', retryable: true });
		connection.close();
	});
});
