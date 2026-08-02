import { describe, expect, it } from 'vitest';

import { ProtocolErrorCode } from '../protocol-result';
import {
	decodeSelectionExportChunk,
	decodeSelectionExportChunkRoute,
	encodeSelectionExportChunk,
	SELECTION_EXPORT_CHUNK_HEADER_BYTES,
} from '../selection';

describe('selection export chunks', () => {
	it('round-trips exact bytes and stable chunk metadata', () => {
		const encoded = encodeSelectionExportChunk({
			attachmentId: '22'.repeat(16),
			bytes: new TextEncoder().encode('selected\ntext'),
			chunkSequence: 7,
			exportSequence: 9n,
			final: true,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const decoded = decodeSelectionExportChunk(encoded.value);
		const route = decodeSelectionExportChunkRoute(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		if (!route.ok) throw new Error(route.detail);

		expect(decoded.value).toMatchObject({
			chunkSequence: 7,
			exportSequence: 9n,
			final: true,
		});
		expect(new TextDecoder().decode(decoded.value.bytes)).toBe('selected\ntext');
		expect(route.value).toEqual({
			attachmentId: '22'.repeat(16),
			chunkSequence: 7,
			exportSequence: 9n,
			final: true,
		});
	});

	it('rejects truncated headers and non-zero reserved bytes', () => {
		expect(
			decodeSelectionExportChunk(new Uint8Array(SELECTION_EXPORT_CHUNK_HEADER_BYTES - 1)),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.truncated, ok: false }));
		const encoded = encodeSelectionExportChunk({
			attachmentId: '22'.repeat(16),
			bytes: new Uint8Array(),
			chunkSequence: 0,
			exportSequence: 1n,
			final: false,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const malformed = encoded.value.slice();
		malformed[29] = 1;
		expect(decodeSelectionExportChunk(malformed)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unknownRequiredFlag, ok: false }),
		);
	});
});
