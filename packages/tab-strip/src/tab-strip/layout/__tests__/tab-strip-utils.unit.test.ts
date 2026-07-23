import { describe, expect, it } from 'vitest';
import {
	getTabStripView,
	getTabStripViewTargetBounds,
	type TabStripHierarchyNode,
} from '../tab-strip-utils';

describe('common tab strip utils', () => {
	it('returns target bounds under each animating collection container', () => {
		const parent = {
			kind: 'group-container',
			bounds: { x: 0, y: 0, width: 100, height: 40 },
		} as const;
		expect(
			getTabStripViewTargetBounds({
				kind: 'view',
				parent,
				bounds: { x: 1, y: 2, width: 3, height: 4 },
				targetBounds: { x: 5, y: 6, width: 7, height: 8 },
			}),
		).toEqual({ x: 5, y: 6, width: 7, height: 8 });
	});

	it('falls back to current bounds outside an animating collection', () => {
		expect(
			getTabStripViewTargetBounds({
				kind: 'view',
				parent: { kind: 'view', bounds: { x: 0, y: 0, width: 0, height: 0 } },
				bounds: { x: 1, y: 2, width: 3, height: 4 },
				targetBounds: { x: 5, y: 6, width: 7, height: 8 },
			}),
		).toEqual({ x: 1, y: 2, width: 3, height: 4 });
	});

	it('walks the parent chain to the tab-strip view', () => {
		const strip: TabStripHierarchyNode = {
			kind: 'tab-strip',
			bounds: { x: 0, y: 0, width: 200, height: 40 },
		};
		const group: TabStripHierarchyNode = {
			kind: 'group-container',
			bounds: strip.bounds,
			parent: strip,
		};
		const tab: TabStripHierarchyNode = { kind: 'view', bounds: strip.bounds, parent: group };
		expect(getTabStripView(tab)).toBe(strip);
		expect(getTabStripView(strip)).toBeNull();
	});
});
