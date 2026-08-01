import {
	CURRENT_PROTOCOL_VERSION,
	encodeProtocolEnvelope,
	MessageKind,
	PayloadEncoding,
	PROTOCOL_LIMITS,
} from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import { HostRecordAssembler } from '../HostRecordAssembler';

function record(payloadText: string): Uint8Array {
	const encoded = encodeProtocolEnvelope({
		correlationId: null,
		event: false,
		hostEpoch: null,
		kind: MessageKind.ping,
		optionalMessage: false,
		payload: new TextEncoder().encode(payloadText),
		payloadEncoding: PayloadEncoding.controlJson,
		protocol: CURRENT_PROTOCOL_VERSION,
		response: false,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

describe('HostRecordAssembler', () => {
	it('assembles fragmented and coalesced records without retaining consumed bytes', () => {
		const first = record('{}');
		const second = record('{"next":true}');
		const combined = new Uint8Array(first.byteLength + second.byteLength);
		combined.set(first);
		combined.set(second, first.byteLength);
		const onRecord = vi.fn<(record: Uint8Array) => void>();
		const assembler = new HostRecordAssembler(onRecord);

		assembler.push(combined.subarray(0, 7));
		assembler.push(combined.subarray(7, first.byteLength + 3));
		assembler.push(combined.subarray(first.byteLength + 3));

		expect(onRecord).toHaveBeenCalledTimes(2);
		expect(onRecord.mock.calls[0]?.[0]).toEqual(first);
		expect(onRecord.mock.calls[1]?.[0]).toEqual(second);
		expect(assembler.bufferedBytes).toBe(0);
		expect(() => assembler.finish()).not.toThrow();
	});

	it('rejects invalid magic and records beyond the transport bound', () => {
		const invalidMagic = record('{}').slice();
		invalidMagic[0] = 0;
		expect(() =>
			new HostRecordAssembler(vi.fn<(record: Uint8Array) => void>()).push(invalidMagic),
		).toThrow('invalid magic');

		const oversized = record('{}').slice();
		new DataView(oversized.buffer).setUint32(8, PROTOCOL_LIMITS.maxTransportRecordBytes + 1, true);
		expect(() =>
			new HostRecordAssembler(vi.fn<(record: Uint8Array) => void>()).push(oversized),
		).toThrow('outside its bound');
	});

	it('reports a partial trailing record when the socket closes', () => {
		const assembler = new HostRecordAssembler(vi.fn<(record: Uint8Array) => void>());
		assembler.push(record('{}').subarray(0, 20));

		expect(() => assembler.finish()).toThrow('truncated record');
	});
});
