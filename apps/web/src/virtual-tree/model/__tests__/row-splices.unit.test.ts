import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { SpliceList } from '../row-splices';

describe('SpliceList', () => {
	it('composes splices in any order into ordered splices that replay to the same rows', () => {
		fc.assert(
			fc.property(
				fc.nat({ max: 80 }),
				fc.array(
					fc.record({ at: fc.nat(), remove: fc.nat({ max: 6 }), insert: fc.nat({ max: 6 }) }),
					{
						maxLength: 30,
					},
				),
				fc.integer({ max: 40, min: 1 }),
				(length, edits, limit) => {
					const before = Array.from({ length }, (_, index) => index);
					const rows = [...before];
					const list = new SpliceList(limit);
					let next = length;
					for (const edit of edits) {
						const start = edit.at % (rows.length + 1);
						const deleteCount = Math.min(edit.remove, rows.length - start);
						const inserted = Array.from({ length: edit.insert }, () => next++);
						rows.splice(start, deleteCount, ...inserted);
						list.add(start, deleteCount, inserted.length);
					}
					const replay = [...before];
					let end = 0;
					for (const splice of list.take(0, length, rows.length)) {
						expect(splice.start).toBeGreaterThanOrEqual(end);
						replay.splice(
							splice.start,
							splice.deleteCount,
							...rows.slice(splice.start, splice.start + splice.insertCount),
						);
						end = splice.start + splice.insertCount;
					}
					expect(replay).toEqual(rows);
				},
			),
			{ numRuns: 400, seed: 2_026_11_29 },
		);
	});
});
