import { describe, expect, it } from 'vitest';

import { MAXIMUM_PROJECTED_TERMINAL_PASTE_PREVIEW_CHARACTERS } from '../../constants/projected-terminal-paste-review';
import { projectedTerminalPastePreview } from '../projectedTerminalPastePreview';

describe('projectedTerminalPastePreview', () => {
	it('makes controls visible while preserving newlines', () => {
		expect(projectedTerminalPastePreview('a\t\r\n\u001b\u0085b')).toEqual({
			text: 'a\\t\\r\n\\u001B\\u0085b',
			truncated: false,
		});
	});

	it('truncates by Unicode code point', () => {
		const text = `${'a'.repeat(MAXIMUM_PROJECTED_TERMINAL_PASTE_PREVIEW_CHARACTERS - 1)}😀z`;
		const preview = projectedTerminalPastePreview(text);
		expect(Array.from(preview.text)).toHaveLength(
			MAXIMUM_PROJECTED_TERMINAL_PASTE_PREVIEW_CHARACTERS,
		);
		expect(preview.text.endsWith('😀')).toBe(true);
		expect(preview.truncated).toBe(true);
	});
});
