import { TabModel } from '../../model/tab-model';
import { describe, expect, it } from 'vitest';
import {
	areDraggedTabIdsConsecutive,
	calculateHorizontalTabStripTrailingEdge,
	createNewTabGroup,
} from '../tab-strip-internals';

describe('tab-strip internals', () => {
	it('preserves deterministic identifiers and dragged-tab adjacency', () => {
		const model = new TabModel();
		createNewTabGroup(model);
		createNewTabGroup(model);
		createNewTabGroup(model);
		expect(model.getSnapshot().groups.map((group) => group.id)).toEqual([
			'group-1',
			'group-2',
			'group-3',
		]);
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
