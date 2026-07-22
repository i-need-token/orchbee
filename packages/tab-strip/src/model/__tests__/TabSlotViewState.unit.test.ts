import { describe, expect, it } from 'vitest';
import { TabSlotViewState } from '../TabSlotViewState';

describe('TabSlotViewState', () => {
	it('tracks group, split, drag, and animation state', () => {
		const state = new TabSlotViewState();
		state.setGroup('group');
		state.setSplit('split');
		state.setDragging(true);
		state.setAnimating(true);
		expect(state).toMatchObject({
			group: 'group',
			split: 'split',
			dragging: true,
			animating: true,
			detached: false,
		});
	});

	it('marks detach irreversibly and tightens anchor bounds vertically', () => {
		const state = new TabSlotViewState();
		state.setDetached();
		state.setDragging(false);
		expect(state.detached).toBe(true);
		expect(state.getAnchorBoundsInScreen({ x: 10, y: 20, width: 30, height: 10 })).toEqual({
			x: 10,
			y: 22,
			width: 30,
			height: 6,
		});
	});
});
