import { assert as assertProperty, property, uint8Array } from 'fast-check';
import { describe, expect, it } from 'vitest';

import { isValidUtf8, utf8ByteLength } from '../utf8';

const fatalUtf8Decoder = new TextDecoder('utf-8', { fatal: true });

function platformAcceptsUtf8(bytes: Uint8Array): boolean {
	try {
		fatalUtf8Decoder.decode(bytes);
		return true;
	} catch {
		return false;
	}
}

describe('allocation-free UTF-8 validation', () => {
	it('measures scalar and replacement encodings without allocating bytes', () => {
		const encoder = new TextEncoder();
		for (const value of ['', 'ASCII', '蜂', '🐝', 'A🐝蜂', '\ud800x', 'x\udc00']) {
			expect(utf8ByteLength(value)).toBe(encoder.encode(value).byteLength);
		}
	});

	it('matches the platform fatal decoder under a stable property seed', () => {
		expect(() =>
			assertProperty(
				property(uint8Array({ maxLength: 128 }), (bytes) => {
					return isValidUtf8(bytes) === platformAcceptsUtf8(bytes);
				}),
				{ numRuns: 5000, seed: 0x08f8_3629 },
			),
		).not.toThrow();
	});

	it('rejects overlong, surrogate, truncated, and out-of-range encodings', () => {
		for (const bytes of [
			new Uint8Array([0xc0, 0x80]),
			new Uint8Array([0xe0, 0x80, 0x80]),
			new Uint8Array([0xed, 0xa0, 0x80]),
			new Uint8Array([0xf0, 0x90, 0x80]),
			new Uint8Array([0xf4, 0x90, 0x80, 0x80]),
		]) {
			expect(isValidUtf8(bytes)).toBe(false);
		}
	});
});
