import {
	decodeTerminalRowWindow,
	encodeTerminalRowWindow,
	PROTOCOL_LIMITS,
	RowPromptClass,
	TerminalRowWindowDetail,
} from '@orchbee/terminal-protocol';
import { describe, expect, it } from 'vitest';

import {
	bufferRowsFromDecodedWindow,
	TerminalBufferRowDetail,
	validateBufferRowsRequest,
} from '../buffer-rows';

describe('terminal buffer rows', () => {
	it('converts compact identity-ASCII row windows without allocating explicit mappings', () => {
		const encoded = encodeTerminalRowWindow({
			attachmentId: '22'.repeat(16),
			columns: 3,
			detail: TerminalRowWindowDetail.text,
			generation: 1n,
			hostEpoch: '11'.repeat(16),
			revision: 2n,
			rows: [
				{
					index: 4n,
					mappings: [
						{ byteOffset: 0, column: 0, utf16Offset: 0, width: 1 },
						{ byteOffset: 1, column: 1, utf16Offset: 1, width: 1 },
						{ byteOffset: 2, column: 2, utf16Offset: 2, width: 1 },
					],
					promptClass: RowPromptClass.none,
					text: 'cat',
					wrap: false,
					wrapContinuation: false,
				},
			],
			sequence: 3n,
			startRow: 4n,
			totalRows: 5n,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const decoded = decodeTerminalRowWindow(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		const result = bufferRowsFromDecodedWindow(decoded.value, encoded.value.byteLength);

		expect(result).toMatchObject({
			detail: TerminalBufferRowDetail.text,
			rows: [{ identityAscii: true, mappings: [], text: 'cat' }],
			startRow: 4n,
			totalRows: 5n,
		});
	});

	it('rejects requests outside the negotiated row and byte bounds', () => {
		const request = {
			detail: TerminalBufferRowDetail.text,
			generation: 1n,
			maximumBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
			revision: 1n,
			rowCount: 1,
			startRow: 0n,
		};
		expect(validateBufferRowsRequest(request)).toBe(true);
		expect(
			validateBufferRowsRequest({
				...request,
				rowCount: PROTOCOL_LIMITS.maxRowWindowRows + 1,
			}),
		).toBe(false);
		expect(validateBufferRowsRequest({ ...request, maximumBytes: 0 })).toBe(false);
	});
});
