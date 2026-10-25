import {
	RowPromptClass,
	SemanticContent,
	TerminalSemanticContentCode,
	TerminalRowPromptClassCode,
	TerminalRowFlag,
	TerminalCellWidthCode,
	decodeTerminalFrame,
} from '@orchbee/terminal-protocol';
import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';

import type { TerminalBufferRow } from '../../buffer-rows';
import {
	resolveTerminalSearchResultLimit,
	visitTerminalBufferMatches,
	searchMatchContainsCell,
	validateTerminalSearchQuery,
} from '../terminal-search';

describe('projected terminal search', () => {
	it('supports case, whole-word, and regular-expression searches', () => {
		const rows = [row(0n, 'Error error_error error 42')];
		expect(collectMatches(rows, 'error')).toHaveLength(4);
		expect(collectMatches(rows, 'error', { caseSensitive: true })).toHaveLength(3);
		expect(collectMatches(rows, 'error', { wholeWord: true })).toHaveLength(2);
		expect(collectMatches(rows, 'e(?:rror)?', { regex: true })).toHaveLength(4);
	});

	it('preserves absolute coordinates and wide-cell columns', () => {
		const rows = [rowFromGraphemes(41n, ['A', '蜂'])];
		expect(collectMatches(rows, '蜂')).toEqual([
			{
				end: { column: 1, row: 41 },
				start: { column: 1, row: 41 },
				text: '蜂',
			},
		]);
	});

	it('maps implicit narrow ASCII offsets without materializing mappings', () => {
		const rows = [identityAsciiRow(400n, 'alpha beta')];
		expect(collectMatches(rows, 'beta')).toEqual([
			{
				end: { column: 9, row: 400 },
				start: { column: 6, row: 400 },
				text: 'beta',
			},
		]);
	});

	it('searches one logical line across soft-wrapped projected rows', () => {
		const rows = [row(10n, 'wrapped', true), row(11n, 'value')];
		expect(collectMatches(rows, 'edva')).toEqual([
			{
				text: 'edva',
				start: { column: 5, row: 10 },
				end: { column: 1, row: 11 },
			},
		]);
	});

	it('crosses hard line breaks only when requested', () => {
		const rows = [row(20n, 'test 2'), row(21n, 'test 3')];
		expect(collectMatches(rows, 'test 2\ntest 3')).toEqual([]);
		expect(collectMatches(rows, 'test 2\ntest 3', { spanLines: true })).toEqual([
			{
				text: 'test 2\ntest 3',
				start: { column: 0, row: 20 },
				end: { column: 5, row: 21 },
			},
		]);
		expect(
			collectMatches(rows, String.raw`2\s+test`, {
				regex: true,
				spanLines: true,
			}),
		).toEqual([
			{
				text: '2\ntest',
				start: { column: 5, row: 20 },
				end: { column: 3, row: 21 },
			},
		]);
	});

	it('returns repeated matches in forward order without overlap', () => {
		const rows = [row(0n, 'hello boo! hello boo!')];
		expect(collectMatches(rows, 'boo!')).toEqual([
			{
				text: 'boo!',
				start: { column: 6, row: 0 },
				end: { column: 9, row: 0 },
			},
			{
				text: 'boo!',
				start: { column: 17, row: 0 },
				end: { column: 20, row: 0 },
			},
		]);
		expect(collectMatches([row(0n, 'xxx')], 'xx')).toHaveLength(1);
		expect(collectMatches([row(0n, 'ababa')], 'aba', { regex: true })).toHaveLength(1);
	});

	it('bounds results and handles empty or invalid regular expressions', () => {
		expect(collectMatches([row(0n, 'x x x x')], 'x', { maxResults: 2 })).toHaveLength(2);
		expect(collectMatches([row(0n, 'xxx')], 'x', { maxResults: 0 })).toEqual([]);
		expect(collectMatches([row(0n, 'xxx')], '(?=x)', { regex: true })).toEqual([]);
		expect(collectMatches([row(0n, 'x'.repeat(10_000))], '.*', { regex: true })).toHaveLength(1);
		expect(validateTerminalSearchQuery('[', { regex: true })).toContain(
			'Invalid regular expression',
		);
		expect(collectMatches([row(0n, 'text')], '[', { regex: true })).toEqual([]);
	});

	it('maps combined graphemes to one projected terminal cell', () => {
		const rows = [rowFromGraphemes(7n, ['A', 'e\u0301', 'B'])];
		expect(collectMatches(rows, 'e\u0301')).toEqual([
			{
				text: 'e\u0301',
				start: { column: 1, row: 7 },
				end: { column: 1, row: 7 },
			},
		]);
	});

	it('finds literal text through combining marks while preserving original text', () => {
		const first = ['z\u0336\u0301', 'a\u0335\u0316', 'l\u0337\u0307'];
		const second = ['Z\u0335\u030c', 'a\u0337\u031b', 'l\u0334\u0302'];
		const matches = collectMatches([rowFromGraphemes(5n, [...first, ' ', ...second])], 'zal');
		expect(matches.map((match) => match.text)).toEqual([
			'z\u0336\u0301a\u0335\u0316l\u0337\u0307',
			'Z\u0335\u030ca\u0337\u031bl\u0334\u0302',
		]);
	});

	it('treats Unicode letters and numbers as whole-word characters', () => {
		const matches = collectMatches([row(0n, '猫42 猫 猫_咪')], '猫', {
			wholeWord: true,
		});
		expect(matches).toEqual([
			{
				text: '猫',
				start: { column: 5, row: 0 },
				end: { column: 5, row: 0 },
			},
		]);
	});

	it('tests cells inside a multi-row projected match', () => {
		const match = {
			text: 'wrapped',
			start: { column: 4, row: 2 },
			end: { column: 2, row: 4 },
		};
		expect(searchMatchContainsCell(match, 3, 2)).toBe(false);
		expect(searchMatchContainsCell(match, 4, 2)).toBe(true);
		expect(searchMatchContainsCell(match, 0, 3)).toBe(true);
		expect(searchMatchContainsCell(match, 2, 4)).toBe(true);
		expect(searchMatchContainsCell(match, 3, 4)).toBe(false);
	});
});

function identityAsciiRow(index: bigint, text: string): TerminalBufferRow {
	return {
		cells: null,
		identityAscii: true,
		index,
		mappings: [],
		promptClass: 'none',
		text,
		wrap: false,
		wrapContinuation: false,
	};
}

function row(index: bigint, text: string, wrap = false): TerminalBufferRow {
	return rowFromGraphemes(index, [...text], wrap);
}

function rowFromGraphemes(
	index: bigint,
	graphemes: readonly string[],
	wrap = false,
): TerminalBufferRow {
	let byteOffset = 0;
	let column = 0;
	let text = '';
	let utf16Offset = 0;
	const mappings: TerminalBufferRow['mappings'][number][] = [];
	for (const grapheme of graphemes) {
		const width = /\p{Script=Han}/u.test(grapheme) ? 2 : 1;
		mappings.push({ byteOffset, column, utf16Offset, width });
		text += grapheme;
		byteOffset += new TextEncoder().encode(grapheme).byteLength;
		column += width;
		utf16Offset += grapheme.length;
	}
	return {
		cells: null,
		index,
		mappings,
		promptClass: 'none',
		text,
		wrap,
		wrapContinuation: false,
	};
}

function collectMatches(
	rows: Parameters<typeof visitTerminalBufferMatches>[0],
	query: string,
	options: Parameters<typeof visitTerminalBufferMatches>[2] = {},
) {
	const matches: Parameters<Parameters<typeof visitTerminalBufferMatches>[3]>[0][] = [];
	const limit = resolveTerminalSearchResultLimit(options.maxResults);
	if (limit > 0)
		visitTerminalBufferMatches(rows, query, options, (match) => {
			matches.push(match);
			return matches.length >= limit;
		});
	return matches;
}

it('preserves native Ghostty soft wraps and hard search boundaries', async () => {
	const decoded = decodeTerminalFrame(
		await readFile(
			new URL('../../../../../schemas/terminal/v1/fixtures/ghostty-surface.bin', import.meta.url),
		),
	);
	if (!decoded.ok) throw new Error(decoded.detail);
	const frame = decoded.value,
		encoder = new TextEncoder();
	const rows: TerminalBufferRow[] = Array.from(
		frame.rowData.indices.slice(0, 3),
		(rowIndex, rowOffset) => {
			const start = frame.rowData.cellOffsets[rowOffset];
			if (start === undefined) throw new Error('Fixture row is missing');
			const cells: NonNullable<TerminalBufferRow['cells']>[number][] = Array.from(
				{ length: frame.columns },
				(_, column) => {
					const index = start + column,
						width = frame.cells.widths[index],
						style = frame.cells.styleIndices[index];
					const graphemeIndex = frame.cells.graphemeStringIndices[index],
						linkIndex =
							style === undefined ? undefined : frame.styles.hyperlinkStringIndices[style];
					const semantic = frame.cells.semanticContents[index];
					let cellWidth: 0 | 1 | 2 = 0;
					if (width === TerminalCellWidthCode.wide) cellWidth = 2;
					else if (width === TerminalCellWidthCode.narrow) cellWidth = 1;
					let semanticContent: SemanticContent = SemanticContent.output;
					if (semantic === TerminalSemanticContentCode.prompt)
						semanticContent = SemanticContent.prompt;
					else if (semantic === TerminalSemanticContentCode.input)
						semanticContent = SemanticContent.input;
					return {
						column,
						grapheme: graphemeIndex === undefined ? '' : (frame.strings[graphemeIndex] ?? ''),
						hyperlink: linkIndex === undefined ? null : (frame.strings[linkIndex] ?? null),
						width: cellWidth,
						semanticContent,
					};
				},
			);
			let last = -1;
			for (let column = cells.length - 1; column >= 0; column--) {
				const cell = cells[column];
				if (
					cell !== undefined &&
					cell.width > 0 &&
					(cell.grapheme !== '' || cell.hyperlink !== null)
				) {
					last = column;
					break;
				}
			}
			let text = '',
				byteOffset = 0;
			const mappings: TerminalBufferRow['mappings'][number][] = [];
			for (const cell of cells) {
				if (cell.column > last || cell.width === 0) continue;
				const grapheme = cell.grapheme || ' ';
				mappings.push({
					byteOffset,
					column: cell.column,
					utf16Offset: text.length,
					width: cell.width,
				});
				text += grapheme;
				byteOffset += encoder.encode(grapheme).length;
			}
			const flags = frame.rowData.flags[rowOffset] ?? 0,
				prompt = frame.rowData.promptClasses[rowOffset];
			let promptClass: RowPromptClass = RowPromptClass.none;
			if (prompt === TerminalRowPromptClassCode.prompt) promptClass = RowPromptClass.prompt;
			else if (prompt === TerminalRowPromptClassCode.promptContinuation)
				promptClass = RowPromptClass.promptContinuation;
			return {
				cells,
				index: BigInt(rowIndex),
				mappings,
				text,
				promptClass,
				wrap: (flags & TerminalRowFlag.wrap) !== 0,
				wrapContinuation: (flags & TerminalRowFlag.wrapContinuation) !== 0,
			};
		},
	);
	expect(collectMatches(rows, 'x界')).toEqual([
		{
			end: { column: 0, row: 1 },
			start: { column: 6, row: 0 },
			text: 'x界',
		},
	]);
	expect(collectMatches(rows, '界H')).toEqual([]);
	expect(collectMatches(rows, '界\nH', { spanLines: true })).toEqual([
		{
			end: { column: 0, row: 2 },
			start: { column: 0, row: 1 },
			text: '界\nH',
		},
	]);
});
