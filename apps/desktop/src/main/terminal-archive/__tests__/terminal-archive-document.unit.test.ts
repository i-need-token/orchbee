import { describe, expect, it } from 'vitest';

import { decodeTerminalArchive } from '../terminal-archive-document';

describe('terminal archive document boundary', () => {
	it('rejects bytes without a valid archive header', () => {
		expect(decodeTerminalArchive(new Uint8Array([0x4f, 0x42, 0x54, 0x41]))).toMatchObject({
			ok: false,
		});
	});
});
