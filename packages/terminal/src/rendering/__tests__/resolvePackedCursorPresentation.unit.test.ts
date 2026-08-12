import { TerminalCellWidthCode } from '@orchbee/terminal-protocol';
import { describe, expect, it } from 'vitest';

import { resolvePackedCursorPresentation } from '../resolvePackedCursorPresentation';

describe('resolvePackedCursorPresentation', () => {
	it.each([
		{
			cursor: { column: 1, wideTail: false },
			expected: { column: 1, width: 1 },
			name: 'narrow cursor cell',
			widths: [TerminalCellWidthCode.narrow, TerminalCellWidthCode.narrow],
		},
		{
			cursor: { column: 0, wideTail: false },
			expected: { column: 0, width: 2 },
			name: 'wide cursor head',
			widths: [TerminalCellWidthCode.wide, TerminalCellWidthCode.spacerTail],
		},
		{
			cursor: { column: 1, wideTail: true },
			expected: { column: 0, width: 2 },
			name: 'wide cursor tail',
			widths: [TerminalCellWidthCode.wide, TerminalCellWidthCode.spacerTail],
		},
		{
			cursor: { column: 0, wideTail: true },
			expected: { column: 0, width: 1 },
			name: 'invalid leading wide-tail flag',
			widths: [TerminalCellWidthCode.narrow],
		},
	])('projects the $name without changing the canonical cursor', ({ cursor, expected, widths }) => {
		const original = { ...cursor };
		expect(resolvePackedCursorPresentation(cursor, Uint8Array.from(widths))).toEqual(expected);
		expect(cursor).toEqual(original);
	});
});
