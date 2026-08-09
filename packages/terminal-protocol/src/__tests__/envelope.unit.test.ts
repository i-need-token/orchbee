import { describe, expect, it } from 'vitest';

import {
	decodeProtocolEnvelope,
	encodeProtocolEnvelope,
	MessageKind,
	PayloadEncoding,
} from '../envelope';
import { ProtocolErrorCode } from '../protocol-result';
import { CURRENT_PROTOCOL_VERSION } from '../version';

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
			protocol: CURRENT_PROTOCOL_VERSION,
			response: false,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		expect(new DataView(encoded.value.buffer).getUint16(56, true)).toBe(
			CURRENT_PROTOCOL_VERSION.patch,
		);
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

	it('requires the exact three-part envelope version shape', () => {
		const missingPatch = encodeProtocolEnvelope({
			correlationId: null,
			event: true,
			hostEpoch: null,
			kind: MessageKind.ping,
			optionalMessage: false,
			payload: new Uint8Array(),
			payloadEncoding: PayloadEncoding.controlJson,
			protocol: { major: 0, minor: 1 },
			response: false,
		});
		expect(missingPatch).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});

	it('does not reserve unimplemented lifecycle or attachment messages', () => {
		for (const kind of [16, 17, 23]) {
			expect(
				encodeProtocolEnvelope({
					correlationId: null,
					event: true,
					hostEpoch: '11111111111111111111111111111111',
					kind,
					optionalMessage: false,
					payload: new TextEncoder().encode('{}'),
					payloadEncoding: PayloadEncoding.controlJson,
					protocol: { major: 0, minor: 0, patch: 1 },
					response: false,
				}),
			).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));
		}
	});

	it('rejects a registered kind with the wrong role or payload encoding', () => {
		const base = {
			correlationId: '22222222222222222222222222222222',
			event: false,
			hostEpoch: '11111111111111111111111111111111',
			optionalMessage: false,
			payload: new TextEncoder().encode('{}'),
			protocol: { major: 0, minor: 0, patch: 1 },
			response: false,
		};
		expect(
			encodeProtocolEnvelope({
				...base,
				kind: MessageKind.terminalStarted,
				payloadEncoding: PayloadEncoding.controlJson,
			}),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));
		expect(
			encodeProtocolEnvelope({
				...base,
				kind: MessageKind.writeInput,
				payloadEncoding: PayloadEncoding.controlJson,
			}),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));
	});
});
