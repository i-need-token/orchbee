import { describe, expect, it } from 'vitest';
import {
	areDraggedTabIdsConsecutive,
	calculateHorizontalTabStripTrailingEdge,
	nextAvailableEntityId,
} from '../tab-strip-internals';

describe('tab-strip internals', () => {
	it('preserves deterministic identifiers and dragged-tab adjacency', () => {
		expect(nextAvailableEntityId('group', new Set(['group-1', 'group-2']))).toBe('group-3');
		expect(areDraggedTabIdsConsecutive([{ id: 'a' }, { id: 'b' }, { id: 'c' }], ['a', 'b'])).toBe(
			true,
		);
		expect(areDraggedTabIdsConsecutive([{ id: 'a' }, { id: 'b' }, { id: 'c' }], ['a', 'c'])).toBe(
			false,
		);
	});

	it('keeps the ideal trailing edge as the floor while dragging', () => {
		expect(
			calculateHorizontalTabStripTrailingEdge({
				idealTrailingEdge: 384,
				paintedTrailingEdge: 200,
				dragTrailingEdge: 200,
			}),
		).toBe(384);
		expect(
			calculateHorizontalTabStripTrailingEdge({
				idealTrailingEdge: 384,
				paintedTrailingEdge: 384,
				dragTrailingEdge: 460,
			}),
		).toBe(460);
		expect(
			calculateHorizontalTabStripTrailingEdge({
				idealTrailingEdge: 384,
				paintedTrailingEdge: 310,
				dragTrailingEdge: null,
			}),
		).toBe(310);
	});
});
