import { describe, expect, it } from 'vitest';

import {
	TerminalThemeOperationIdSchema,
	identifierBytes,
	identifierText,
	parseAttachmentId,
	parseHostEpoch,
} from '../identifiers';
import { ProtocolErrorCode } from '../protocol-result';

describe('protocol identifiers', () => {
	it('maps canonical lowercase text to exactly 16 bytes and back', () => {
		const value = '0123456789abcdef0123456789abcdef';
		expect(identifierText(identifierBytes(value))).toEqual({ ok: true, value });
		expect(parseHostEpoch(value)).toEqual({ ok: true, value });
		expect(parseAttachmentId(value)).toEqual({ ok: true, value });
		expect(TerminalThemeOperationIdSchema.parse(value)).toBe(value);
	});

	it('rejects zero, uppercase, and incorrectly sized values', () => {
		for (const value of [
			'00000000000000000000000000000000',
			'0123456789ABCDEF0123456789ABCDEF',
			'0123',
		]) {
			expect(parseHostEpoch(value)).toEqual(
				expect.objectContaining({ code: ProtocolErrorCode.invalidIdentifier, ok: false }),
			);
		}
	});
});
