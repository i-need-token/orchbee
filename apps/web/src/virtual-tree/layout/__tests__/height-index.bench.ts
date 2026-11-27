import { bench, type BenchOptions } from 'vitest';

import { HeightIndex } from '../HeightIndex';

// The variable-height scenarios of the File Bar proposal's performance targets: 100,000 rows whose
// heights alternate, which is VS Code's worst case of one range group per row. Lookups run 1,000 at a
// time, so a case's time divided by 1,000 is the cost of one lookup.

const N = 100_000;
const LOOKUPS = 1_000;

const OPTIONS: BenchOptions = { iterations: 7, time: 0, warmupIterations: 2, warmupTime: 0 };

function alternating(count: number): Float64Array {
	const heights = new Float64Array(count);
	for (let index = 0; index < count; index += 1) heights[index] = index % 2 === 0 ? 22 : 44;
	return heights;
}

const heights = alternating(N);
const index = new HeightIndex(heights);
const total = index.totalHeight;
/** Offsets spread over the whole list, visited out of order so no lookup warms the next. */
const offsets = Array.from(
	{ length: LOOKUPS },
	(_, step) => ((step * 7_919) % LOOKUPS) * (total / LOOKUPS),
);
const rows = Array.from(
	{ length: LOOKUPS },
	(_, step) => ((step * 7_919) % LOOKUPS) * (N / LOOKUPS),
);

function check(condition: boolean, message: string): void {
	if (!condition) throw new Error(message);
}

bench(
	'finds the row at 1,000 offsets among 100,000 rows of alternating heights',
	() => {
		let sum = 0;
		for (const offset of offsets) sum += index.indexAt(offset);
		check(sum > 0, 'Lookups found no rows');
	},
	OPTIONS,
);

bench(
	'finds the offset of 1,000 rows among 100,000 rows of alternating heights',
	() => {
		let sum = 0;
		for (const row of rows) sum += index.positionOf(row);
		check(sum > 0, 'Lookups found no offsets');
	},
	OPTIONS,
);

bench(
	'indexes 100,000 rows of alternating heights',
	() => {
		const built = new HeightIndex(heights);
		check(built.totalHeight === total, 'The index lost rows');
	},
	OPTIONS,
);

bench(
	'inserts and removes one row above 100,000 rows of alternating heights',
	() => {
		index.splice(0, 0, [30]);
		index.splice(0, 1);
		check(index.totalHeight === total, 'The splices did not cancel out');
	},
	OPTIONS,
);

bench(
	'changes the heights of 1,000 rows among 100,000',
	() => {
		for (const row of rows) index.setHeight(row, 30);
		for (const row of rows) index.setHeight(row, heights[row] ?? 0);
		check(index.totalHeight === total, 'The changes did not cancel out');
	},
	OPTIONS,
);
