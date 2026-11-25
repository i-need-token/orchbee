import { describe, expect, it } from 'vitest';

import { type DiffLine, diffLines, presentDiff, splitLines } from '../line-diff';

/** A diff as one string: ` ` kept, `-` removed, `+` added. */
function signs(lines: readonly DiffLine[]): string {
	return lines
		.map((line) => `${{ added: '+', removed: '-', same: ' ' }[line.kind]}${line.text}`)
		.join('|');
}

describe('splitLines', () => {
	it('drops line terminators and a final newline', () => {
		expect(splitLines('')).toEqual([]);
		expect(splitLines('a\nb\n')).toEqual(['a', 'b']);
		expect(splitLines('a\r\nb')).toEqual(['a', 'b']);
		expect(splitLines('\n')).toEqual(['']);
	});
});

describe('diffLines', () => {
	it('finds the shortest edit between two texts', () => {
		expect(signs(diffLines(['a', 'b', 'c'], ['a', 'x', 'c']))).toBe(' a|-b|+x| c');
		expect(signs(diffLines([], ['a', 'b']))).toBe('+a|+b');
		expect(signs(diffLines(['a', 'b'], []))).toBe('-a|-b');
		expect(
			signs(diffLines(['a', 'b', 'c', 'a', 'b', 'b', 'a'], ['c', 'b', 'a', 'b', 'a', 'c'])),
		).toBe('-a|-b| c|+b| a| b|-b| a|+c');
	});

	it('numbers each line on the sides it appears in', () => {
		expect(diffLines(['a', 'b', 'c'], ['a', 'c', 'd'])).toEqual([
			{ kind: 'same', newNumber: 1, oldNumber: 1, text: 'a' },
			{ kind: 'removed', oldNumber: 2, text: 'b' },
			{ kind: 'same', newNumber: 2, oldNumber: 3, text: 'c' },
			{ kind: 'added', newNumber: 3, text: 'd' },
		]);
	});

	it('shows a middle past the edit limit as removed and added whole', () => {
		const lines = diffLines(['k', 'a', 'b', 'k'], ['k', 'b', 'a', 'k'], 1);
		expect(signs(lines)).toBe(' k|-a|-b|+b|+a| k');
	});

	it('keeps every line of either side exactly once', () => {
		const before = Array.from({ length: 200 }, (_, index) => `line ${index % 17}`);
		const after = Array.from({ length: 180 }, (_, index) => `line ${(index * 3) % 19}`);
		const lines = diffLines(before, after);
		expect(lines.filter((line) => line.kind !== 'added').map((line) => line.text)).toEqual(before);
		expect(lines.filter((line) => line.kind !== 'removed').map((line) => line.text)).toEqual(after);
	});
});

describe('presentDiff', () => {
	it('folds long unchanged runs and keeps context around changes', () => {
		const before = Array.from({ length: 20 }, (_, index) => `${index}`);
		const after = [...before];
		after[10] = 'changed';
		const rows = presentDiff(diffLines(before, after), 2);
		expect(rows.map((row) => (row.kind === 'gap' ? `…${row.hidden}` : row.kind))).toEqual([
			'…8',
			'same',
			'same',
			'removed',
			'added',
			'same',
			'same',
			'…7',
		]);
	});

	it('keeps a short unchanged run whole', () => {
		const rows = presentDiff(diffLines(['a', 'b', 'c', 'd'], ['x', 'b', 'c', 'y']), 1);
		expect(rows.some((row) => row.kind === 'gap')).toBe(false);
	});
});
