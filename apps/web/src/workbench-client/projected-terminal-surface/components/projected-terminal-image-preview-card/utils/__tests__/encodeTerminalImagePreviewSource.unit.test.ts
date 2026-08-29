import { describe, expect, it } from 'vitest';

import { encodeTerminalImagePreviewSource } from '../encodeTerminalImagePreviewSource';

describe('encodeTerminalImagePreviewSource', () => {
	it('encodes content across the divisible chunk boundary without corrupting base64', () => {
		const bytes = new Uint8Array(24 * 1024 + 3);
		for (let index = 0; index < bytes.byteLength; index += 1) bytes[index] = index % 251;
		let binary = '';
		for (const byte of bytes) binary += String.fromCharCode(byte);

		expect(encodeTerminalImagePreviewSource(bytes.buffer, 'image/avif')).toBe(
			`data:image/avif;base64,${globalThis.btoa(binary)}`,
		);
	});
});
