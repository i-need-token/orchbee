import { describe, expect, it } from 'vitest';
import { findTerminalMatches, searchMatchContainsCell } from '../terminal-search';
import type { CellData } from '../../types';

describe('findTerminalMatches', () => {
	it('supports case, whole-word, and regular-expression searches', () => {
		const lines = [line('Error error_error error 42')];
		expect(findTerminalMatches(lines, 'error')).toHaveLength(4);
		expect(findTerminalMatches(lines, 'error', { caseSensitive: true })).toHaveLength(3);
		expect(findTerminalMatches(lines, 'error', { wholeWord: true })).toHaveLength(2);
		expect(findTerminalMatches(lines, 'e(?:rror)?', { regex: true })).toHaveLength(4);
	});

	it('maps UTF-16 matches to wide terminal cell columns', () => {
		const lines = [line('A你好B')];
		expect(findTerminalMatches(lines, '你好')).toEqual([
			{
				text: '你好',
				start: { column: 1, row: 0 },
				end: { column: 3, row: 0 },
			},
		]);
	});

	it('does not search terminal padding after the last content cell', () => {
		expect(findTerminalMatches([[...line('text'), emptyCell(), emptyCell()]], '  ')).toEqual([]);
	});

	it('searches a logical line across soft-wrapped physical rows', () => {
		expect(
			findTerminalMatches([line('wrapped'), line('value')], 'edva', {}, [true, false]),
		).toEqual([
			{
				text: 'edva',
				start: { column: 5, row: 0 },
				end: { column: 1, row: 1 },
			},
		]);
	});

	it('does not match across a hard line break', () => {
		expect(findTerminalMatches([line('hello,'), line(' world')], 'hello, world')).toEqual([]);
	});

	it('returns repeated matches in forward screen order', () => {
		expect(findTerminalMatches([line('hello boo! hello boo!')], 'boo!')).toEqual([
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
	});

	it('finds repeated single-cell matches', () => {
		expect(findTerminalMatches([line('boo boo')], 'b')).toEqual([
			{
				text: 'b',
				start: { column: 0, row: 0 },
				end: { column: 0, row: 0 },
			},
			{
				text: 'b',
				start: { column: 4, row: 0 },
				end: { column: 4, row: 0 },
			},
		]);
	});

	it('returns no matches for an empty query or whitespace-only rows', () => {
		expect(findTerminalMatches([line('text')], '')).toEqual([]);
		expect(findTerminalMatches([[emptyCell(), emptyCell()]], 'x')).toEqual([]);
	});

	it('searches meaningful interior whitespace without including trailing padding', () => {
		expect(findTerminalMatches([[...line('A  B'), emptyCell(), emptyCell()]], '  ')).toEqual([
			{
				text: '  ',
				start: { column: 1, row: 0 },
				end: { column: 2, row: 0 },
			},
		]);
	});

	it('maps a multi-codepoint grapheme to one terminal cell', () => {
		const combining = cell('e\u0301', 1);
		expect(findTerminalMatches([[cell('A', 1), combining, cell('B', 1)]], 'e\u0301')).toEqual([
			{
				text: 'e\u0301',
				start: { column: 1, row: 0 },
				end: { column: 1, row: 0 },
			},
		]);
	});

	it('treats Unicode letters and numbers as whole-word characters', () => {
		const lines = [line('猫42 猫 猫_咪')];
		expect(findTerminalMatches(lines, '猫', { wholeWord: true })).toEqual([
			{
				text: '猫',
				start: { column: 5, row: 0 },
				end: { column: 5, row: 0 },
			},
		]);
	});

	it('tests cells inside a multi-row match', () => {
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

function line(value: string): CellData[] {
	const cells: CellData[] = [];
	for (const character of value) {
		const width = /\p{Script=Han}/u.test(character) ? 2 : 1;
		cells.push(cell(character, width));
		if (width === 2) cells.push(cell('', 0));
	}
	return cells;
}

function cell(text: string, width: number): CellData {
	return {
		codepoint: text.codePointAt(0) ?? 0,
		text: text || ' ',
		fg: null,
		bg: null,
		selected: false,
		hasStyling: false,
		flags: 0,
		width,
		hyperlinkId: 0,
		graphemeLen: text ? [...text].length : 0,
		semanticContent: 0,
		underlineStyle: 0,
		underlineColor: null,
		overline: false,
	};
}

function emptyCell(): CellData {
	return cell('', 1);
}
