import { RowPromptClass } from '@orchbee/terminal-protocol';
import { describe, expect, it } from 'vitest';

import type { TerminalBufferRow } from '../../buffer-rows';
import { ProjectedSearchAccumulator } from '../ProjectedSearchAccumulator';

describe('ProjectedSearchAccumulator', () => {
	it('preserves one soft-wrapped logical line across page boundaries', () => {
		const accumulator = new ProjectedSearchAccumulator('edva', {});

		expect(accumulator.push([row(10n, 'wrapped', true)], false)).toBeNull();
		expect(accumulator.push([row(11n, 'value', false)], true)).toEqual([
			{
				end: { column: 1, row: 11 },
				start: { column: 5, row: 10 },
				text: 'edva',
			},
		]);
	});

	it('combines matches scanned from several completed pages in buffer order', () => {
		const accumulator = new ProjectedSearchAccumulator('needle', {});

		expect(accumulator.push([row(20n, 'first needle', false)], false)).toBeNull();
		expect(accumulator.push([row(21n, 'second needle', false)], true)).toEqual([
			expect.objectContaining({ start: { column: 6, row: 20 } }),
			expect.objectContaining({ start: { column: 7, row: 21 } }),
		]);
	});

	it('keeps span-lines content pending and honors the bounded result cap', () => {
		const spanLines = new ProjectedSearchAccumulator('first\nsecond', { spanLines: true });
		expect(spanLines.push([row(30n, 'first', false)], false)).toBeNull();
		expect(spanLines.push([row(31n, 'second', false)], true)).toHaveLength(1);

		const capped = new ProjectedSearchAccumulator('x', { maxResults: 1 });
		expect(capped.push([row(40n, 'x x', false)], false)).toBeNull();
		expect(capped.push([row(41n, 'x', false)], true)).toHaveLength(1);
	});
});

function row(index: bigint, text: string, wrap: boolean): TerminalBufferRow {
	return {
		cells: null,
		index,
		mappings: Array.from(text, (_character, column) => ({
			byteOffset: column,
			column,
			utf16Offset: column,
			width: 1 as const,
		})),
		promptClass: RowPromptClass.none,
		text,
		wrap,
		wrapContinuation: false,
	};
}
