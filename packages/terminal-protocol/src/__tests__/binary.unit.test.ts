import { describe, expect, it } from 'vitest';

import { BinaryReader, BinaryWriter, checkedAdd, checkedMultiply } from '../binary';
import { ProtocolErrorCode } from '../protocol-result';

describe('binary primitives', () => {
	it('round-trips little-endian integers and bounded byte slices', () => {
		const writer = new BinaryWriter(15);
		writer.writeUint8(7);
		writer.writeUint16(0x1234);
		writer.writeUint32(0x89abcdef);
		writer.writeUint64(42n);

		const reader = new BinaryReader(writer.bytes());
		expect(reader.readUint8()).toBe(7);
		expect(reader.readUint16()).toBe(0x1234);
		expect(reader.readUint32()).toBe(0x89abcdef);
		expect(reader.readUint64()).toBe(42n);
		expect(reader.result()).toEqual({ ok: true, value: 15 });
	});

	it('reports truncation and arithmetic overflow without throwing', () => {
		const reader = new BinaryReader(new Uint8Array(1));
		reader.readUint32();
		expect(reader.failure).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.truncated, ok: false }),
		);
		expect(checkedAdd(Number.MAX_SAFE_INTEGER, 1)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.arithmeticOverflow, ok: false }),
		);
		expect(checkedMultiply(Number.MAX_SAFE_INTEGER, 2)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.arithmeticOverflow, ok: false }),
		);
	});
});
