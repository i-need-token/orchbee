import { describe, expect, it } from 'vitest';

import {
	decodeProtocolEnvelope,
	encodeProtocolEnvelope,
	MessageKind,
	PayloadEncoding,
} from '../envelope';

describe('transport envelope', () => {
	it('round-trips an ordered bounded control record', () => {
		const encoded = encodeProtocolEnvelope({
			correlationId: '22222222222222222222222222222222',
			event: false,
			hostEpoch: '11111111111111111111111111111111',
			kind: MessageKind.ping,
			optionalMessage: false,
			payload: new TextEncoder().encode('{}'),
			payloadEncoding: PayloadEncoding.controlJson,
			protocol: { major: 1, minor: 0 },
			response: false,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const decoded = decodeProtocolEnvelope(encoded.value);
		expect(decoded).toEqual(
			expect.objectContaining({
				ok: true,
				value: expect.objectContaining({
					kind: MessageKind.ping,
					payload: new TextEncoder().encode('{}'),
				}),
			}),
		);
	});
});
