import { describe, expect, it } from 'vitest';

import {
	resolveSiblingReorder,
	resolveSidebarNavigationKey,
	type SidebarNavigationRow,
} from '../sidebar-navigation-keys';

// A Project open on two Workspaces, the first open on two Agents; then a folded Project.
const ROWS: readonly SidebarNavigationRow[] = [
	{ expanded: true, kind: 'heading' },
	{ expanded: true, kind: 'workspace' },
	{ expanded: null, kind: 'agent' },
	{ expanded: null, kind: 'agent' },
	{ expanded: null, kind: 'workspace' },
	{ expanded: false, kind: 'heading' },
];

describe('resolveSidebarNavigationKey', () => {
	it('moves between visible rows and stops at either end', () => {
		expect(resolveSidebarNavigationKey(ROWS, 2, 'ArrowDown')).toEqual({ index: 3, kind: 'focus' });
		expect(resolveSidebarNavigationKey(ROWS, 2, 'ArrowUp')).toEqual({ index: 1, kind: 'focus' });
		expect(resolveSidebarNavigationKey(ROWS, 5, 'ArrowDown')).toBeNull();
		expect(resolveSidebarNavigationKey(ROWS, 0, 'ArrowUp')).toBeNull();
		expect(resolveSidebarNavigationKey(ROWS, 3, 'Home')).toEqual({ index: 0, kind: 'focus' });
		expect(resolveSidebarNavigationKey(ROWS, 3, 'End')).toEqual({ index: 5, kind: 'focus' });
	});

	it('unfolds with → and then moves to the first child', () => {
		expect(resolveSidebarNavigationKey(ROWS, 5, 'ArrowRight')).toEqual({
			index: 5,
			kind: 'unfold',
		});
		expect(resolveSidebarNavigationKey(ROWS, 0, 'ArrowRight')).toEqual({ index: 1, kind: 'focus' });
		expect(resolveSidebarNavigationKey(ROWS, 1, 'ArrowRight')).toEqual({ index: 2, kind: 'focus' });
		// Nothing to unfold or enter: a Workspace without Agents, and an Agent.
		expect(resolveSidebarNavigationKey(ROWS, 4, 'ArrowRight')).toBeNull();
		expect(resolveSidebarNavigationKey(ROWS, 2, 'ArrowRight')).toBeNull();
	});

	it('folds with ← and then moves to the parent', () => {
		expect(resolveSidebarNavigationKey(ROWS, 1, 'ArrowLeft')).toEqual({ index: 1, kind: 'fold' });
		expect(resolveSidebarNavigationKey(ROWS, 3, 'ArrowLeft')).toEqual({ index: 1, kind: 'focus' });
		expect(resolveSidebarNavigationKey(ROWS, 4, 'ArrowLeft')).toEqual({ index: 0, kind: 'focus' });
		// A folded heading has no parent.
		expect(resolveSidebarNavigationKey(ROWS, 5, 'ArrowLeft')).toBeNull();
	});

	it('folds a heading with Enter and opens a Workspace or Agent', () => {
		expect(resolveSidebarNavigationKey(ROWS, 0, 'Enter')).toEqual({ index: 0, kind: 'fold' });
		expect(resolveSidebarNavigationKey(ROWS, 5, 'Enter')).toEqual({ index: 5, kind: 'unfold' });
		expect(resolveSidebarNavigationKey(ROWS, 1, 'Enter')).toEqual({ index: 1, kind: 'open' });
		expect(resolveSidebarNavigationKey(ROWS, 2, 'Enter')).toEqual({ index: 2, kind: 'open' });
	});
});

describe('resolveSiblingReorder', () => {
	const IDS = ['a', 'b', 'c'];

	it('moves an item one place up or down among its siblings', () => {
		expect(resolveSiblingReorder(IDS, 'b', 'up')).toEqual({ beforeId: 'a' });
		expect(resolveSiblingReorder(IDS, 'a', 'down')).toEqual({ beforeId: 'c' });
		expect(resolveSiblingReorder(IDS, 'b', 'down')).toEqual({ beforeId: null });
	});

	it('leaves an item at its edge, or not among the siblings, in place', () => {
		expect(resolveSiblingReorder(IDS, 'a', 'up')).toBeUndefined();
		expect(resolveSiblingReorder(IDS, 'c', 'down')).toBeUndefined();
		expect(resolveSiblingReorder(IDS, 'x', 'up')).toBeUndefined();
	});
});
