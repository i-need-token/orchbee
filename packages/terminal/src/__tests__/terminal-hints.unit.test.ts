import { RowPromptClass, SemanticContent } from '@orchbee/terminal-protocol';
import { describe, expect, it } from 'vitest';

import type { TerminalBufferRenderCell, TerminalBufferRow } from '../buffer-rows';
import {
	completeTerminalHintRows,
	detectTerminalHints,
	terminalHintLabels,
} from '../terminal-hints';

describe('terminal hints', () => {
	it('detects URLs, paths, file locations, and hashes in screen order', () => {
		const text =
			'https://orchbee.dev ./README.md src/main.ts:42:7 deadbee 0123456789abcdef0123456789abcdef01234567';

		expect(detectTerminalHints([row(4n, text)], '/workspace')).toEqual([
			expect.objectContaining({
				kind: 'url',
				target: { kind: 'url', url: 'https://orchbee.dev' },
			}),
			expect.objectContaining({
				kind: 'path',
				target: { kind: 'file', path: '/workspace/README.md' },
				text: './README.md',
			}),
			expect.objectContaining({
				kind: 'file-line',
				target: { column: 7, kind: 'file', line: 42, path: '/workspace/src/main.ts' },
				text: 'src/main.ts:42:7',
			}),
			expect.objectContaining({ kind: 'hash', text: 'deadbee' }),
			expect.objectContaining({
				kind: 'hash',
				text: '0123456789abcdef0123456789abcdef01234567',
			}),
		]);
	});

	it('does not duplicate a hash-like URL component', () => {
		const hints = detectTerminalHints([row(0n, 'https://orchbee.dev/abcdef0')]);

		expect(hints).toHaveLength(1);
		expect(hints[0]?.kind).toBe('url');
	});

	it('drops logical lines clipped by either viewport edge', () => {
		const rows = [
			row(8n, 'continued', true, true),
			row(9n, 'end', false, true),
			row(10n, './kept.ts'),
			row(11n, './clipped.', true),
		];

		expect(completeTerminalHintRows(rows).map(({ index }) => index)).toEqual([10n]);
	});

	it('generates equal-width unique labels without prefix ambiguity', () => {
		expect(terminalHintLabels(3)).toEqual(['0', '1', '2']);
		const labels = terminalHintLabels(37);

		expect(labels).toHaveLength(37);
		expect(new Set(labels).size).toBe(37);
		expect(labels.every((label) => label.length === 2)).toBe(true);
		expect(
			labels.every((label, index) =>
				labels.every((candidate, candidateIndex) =>
					index === candidateIndex ? true : !candidate.startsWith(label),
				),
			),
		).toBe(true);
	});
});

function row(
	index: bigint,
	text: string,
	wrap = false,
	wrapContinuation = false,
): TerminalBufferRow {
	const cells: TerminalBufferRenderCell[] = Array.from(text, (grapheme, column) => ({
		column,
		grapheme,
		hyperlink: null,
		semanticContent: SemanticContent.output,
		width: 1,
	}));
	return {
		cells,
		index,
		mappings: Array.from(text, (_character, column) => ({
			byteOffset: column,
			column,
			utf16Offset: column,
			width: 1,
		})),
		promptClass: RowPromptClass.none,
		text,
		wrap,
		wrapContinuation,
	};
}
