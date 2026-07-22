import { describe, expect, it } from 'vitest';
import { moveContiguousBlock } from '../moveContiguousBlock';

describe('moveContiguousBlock', () => {
	it('moves a block right using a pre-removal target', () => {
		expect(
			moveContiguousBlock(['r1', 'r2', 'b1', 'b2'], {
				start: 0,
				length: 2,
				preRemovalTargetIndex: 4,
			}),
		).toEqual(['b1', 'b2', 'r1', 'r2']);
	});

	it('moves a block left without changing its internal order', () => {
		expect(
			moveContiguousBlock(['r1', 'r2', 'b1', 'b2'], {
				start: 2,
				length: 2,
				preRemovalTargetIndex: 0,
			}),
		).toEqual(['b1', 'b2', 'r1', 'r2']);
	});

	it('treats a target inside the moving block as a no-op', () => {
		const input = ['a', 'r1', 'r2', 'b'];
		expect(
			moveContiguousBlock(input, {
				start: 1,
				length: 2,
				preRemovalTargetIndex: 2,
			}),
		).toEqual(input);
	});

	it('rejects a block outside the input', () => {
		expect(() =>
			moveContiguousBlock(['a'], {
				start: 1,
				length: 1,
				preRemovalTargetIndex: 0,
			}),
		).toThrow(RangeError);
	});
});
