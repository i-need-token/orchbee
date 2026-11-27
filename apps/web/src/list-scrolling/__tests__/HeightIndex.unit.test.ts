import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { HeightIndex } from '../HeightIndex';

type Command =
	| {
			readonly kind: 'splice';
			readonly start: number;
			readonly deleteCount: number;
			readonly heights: readonly number[];
	  }
	| { readonly kind: 'set'; readonly index: number; readonly height: number };

// Whole-pixel heights, zero included, keep offsets exact; long insertions take the rebuilding path.
const heightArbitrary = fc.nat({ max: 40 });
const heightsArbitrary = fc.oneof(
	fc.array(heightArbitrary, { maxLength: 8 }),
	fc.array(heightArbitrary, { maxLength: 120 }),
);
const commandArbitrary: fc.Arbitrary<Command> = fc.oneof(
	fc.record({
		deleteCount: fc.nat(),
		heights: heightsArbitrary,
		kind: fc.constant('splice' as const),
		start: fc.nat(),
	}),
	fc.record({ height: heightArbitrary, index: fc.nat(), kind: fc.constant('set' as const) }),
);

function apply(command: Command, index: HeightIndex, naive: number[]): void {
	if (command.kind === 'set') {
		if (naive.length === 0) return;
		const row = command.index % naive.length;
		index.setHeight(row, command.height);
		naive[row] = command.height;
		return;
	}
	const start = command.start % (naive.length + 1);
	const deleteCount = command.deleteCount % (naive.length - start + 1);
	index.splice(start, deleteCount, command.heights);
	naive.splice(start, deleteCount, ...command.heights);
}

/**
 * The first answer the index gives that differs from summing the heights, or null. The row holding an
 * offset is, by definition, the number of rows that end at or above it, within the rows.
 */
function firstMismatch(index: HeightIndex, naive: readonly number[]): string | null {
	if (index.count !== naive.length) return `count ${index.count}, not ${naive.length}`;
	const probes = [-1, 0];
	const ends: number[] = [];
	let top = 0;
	for (const [row, height] of naive.entries()) {
		if (index.heightOf(row) !== height)
			return `heightOf(${row}) ${index.heightOf(row)}, not ${height}`;
		if (index.positionOf(row) !== top)
			return `positionOf(${row}) ${index.positionOf(row)}, not ${top}`;
		probes.push(top, top + height / 2, top + height);
		top += height;
		ends.push(top);
	}
	if (index.positionOf(naive.length) !== top) return `positionOf(end), not ${top}`;
	if (index.totalHeight !== top) return `totalHeight ${index.totalHeight}, not ${top}`;
	probes.push(top + 1);
	// Probes and ends both rise, so one pass counts the rows ended at each probe.
	let ended = 0;
	for (const position of probes) {
		while (ended < ends.length && (ends[ended] ?? Infinity) <= position) ended += 1;
		const expected = naive.length === 0 ? -1 : Math.min(ended, naive.length - 1);
		if (index.indexAt(position) !== expected) {
			return `indexAt(${position}) ${index.indexAt(position)}, not ${expected}`;
		}
	}
	return null;
}

describe('HeightIndex', () => {
	it('agrees with summing a height array through random splices and height changes', () => {
		fc.assert(
			fc.property(
				fc.array(heightArbitrary, { maxLength: 60 }),
				fc.array(commandArbitrary, { maxLength: 30 }),
				(initial, commands) => {
					const index = new HeightIndex(initial);
					const naive = [...initial];
					expect(firstMismatch(index, naive)).toBeNull();
					for (const command of commands) {
						apply(command, index, naive);
						expect(firstMismatch(index, naive)).toBeNull();
					}
				},
			),
			{ numRuns: 300, seed: 2_026_11_30 },
		);
	});
});
