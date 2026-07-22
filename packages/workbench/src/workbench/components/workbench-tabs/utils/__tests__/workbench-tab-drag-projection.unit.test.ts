import { createWorkbenchTabsState } from '../../../../../workbench-tabs';
import { describe, expect, it } from 'vitest';

import type { WorkbenchTabStripItemGeometry } from '../../types/workbench-tab-drag';
import {
	resolveWorkbenchTabDragProjection,
	resolveWorkbenchTabGroupDragProjection,
} from '../workbench-tab-drag-projection';

const STATE = createWorkbenchTabsState({
	activeTabId: 'plain-first',
	tabGroups: [{ color: 'blue', id: 'group-a', label: 'Group A' }],
	tabs: [
		{ id: 'pinned-first', isPinned: true, label: 'Pinned First' },
		{ id: 'pinned-second', isPinned: true, label: 'Pinned Second' },
		{ id: 'plain-first', label: 'Plain First' },
		{ groupId: 'group-a', id: 'group-first', label: 'Group First' },
		{ groupId: 'group-a', id: 'group-second', label: 'Group Second' },
		{ id: 'plain-last', label: 'Plain Last' },
	],
});

const GEOMETRY: readonly WorkbenchTabStripItemGeometry[] = [
	{ id: 'pinned-first', kind: 'tab', left: 0, width: 100 },
	{ id: 'pinned-second', kind: 'tab', left: 100, width: 100 },
	{ id: 'plain-first', kind: 'tab', left: 200, width: 120 },
	{ id: 'group-a', kind: 'group', left: 320, width: 80 },
	{ id: 'group-first', kind: 'tab', left: 400, width: 120 },
	{ id: 'group-second', kind: 'tab', left: 520, width: 120 },
	{ id: 'plain-last', kind: 'tab', left: 640, width: 120 },
];

describe('workbench tab drag projection', () => {
	it('moves pinned tabs forward and backward at neighboring centers', () => {
		expect(resolveWorkbenchTabDragProjection(STATE, GEOMETRY, 'pinned-first', 151, 100)).toEqual({
			groupId: undefined,
			targetIndex: 1,
		});
		expect(resolveWorkbenchTabDragProjection(STATE, GEOMETRY, 'pinned-second', 49, 100)).toEqual({
			groupId: undefined,
			targetIndex: 0,
		});
	});

	it('selects the slot whose ideal position is closest to the dragged bounds', () => {
		expect(resolveWorkbenchTabDragProjection(STATE, GEOMETRY, 'pinned-first', 150, 100)).toEqual({
			groupId: undefined,
			targetIndex: 1,
		});
		expect(resolveWorkbenchTabDragProjection(STATE, GEOMETRY, 'pinned-second', 50, 100)).toEqual({
			groupId: undefined,
			targetIndex: 0,
		});
	});

	it('uses the complete group header ideal position for block insertion', () => {
		expect(resolveWorkbenchTabGroupDragProjection(STATE, GEOMETRY, 'group-a', 320)).toBeUndefined();
		expect(resolveWorkbenchTabGroupDragProjection(STATE, GEOMETRY, 'group-a', 440)).toBe(4);
	});

	it('moves ungrouped tabs around a complete group block without depending on hit stacking', () => {
		expect(resolveWorkbenchTabDragProjection(STATE, GEOMETRY, 'plain-first', 701, 120)).toEqual({
			groupId: undefined,
			targetIndex: 5,
		});
		expect(resolveWorkbenchTabDragProjection(STATE, GEOMETRY, 'plain-last', 250, 120)).toEqual({
			groupId: undefined,
			targetIndex: 2,
		});
	});

	it('does not let animated visual offsets change an ideal-layout decision', () => {
		const animatedGeometry = GEOMETRY.map((item, index) => ({
			...item,
			left: item.left + (index % 2 === 0 ? 87 : -43),
		}));
		expect(
			resolveWorkbenchTabDragProjection(STATE, animatedGeometry, 'plain-first', 701, 120),
		).toEqual(resolveWorkbenchTabDragProjection(STATE, GEOMETRY, 'plain-first', 701, 120));
	});

	it('previews group entry from both sides only after crossing the inset range', () => {
		expect(
			resolveWorkbenchTabDragProjection(STATE, GEOMETRY, 'plain-first', 405, 120)?.groupId,
		).toBe('group-a');
		expect(
			resolveWorkbenchTabDragProjection(STATE, GEOMETRY, 'plain-last', 625, 120)?.groupId,
		).toBe('group-a');
		expect(
			resolveWorkbenchTabDragProjection(STATE, GEOMETRY, 'plain-first', 250, 120)?.groupId,
		).toBe(undefined);
	});

	it('keeps the current group at its ideal slot and leaves after crossing an adjacent slot', () => {
		expect(
			resolveWorkbenchTabDragProjection(STATE, GEOMETRY, 'group-first', 460, 120),
		).toBeUndefined();
		expect(
			resolveWorkbenchTabDragProjection(STATE, GEOMETRY, 'group-first', 450, 120),
		).toBeUndefined();
		expect(
			resolveWorkbenchTabDragProjection(STATE, GEOMETRY, 'group-first', 330, 120)?.groupId,
		).toBeUndefined();
		expect(
			resolveWorkbenchTabDragProjection(STATE, GEOMETRY, 'group-second', 700, 120)?.groupId,
		).toBeUndefined();
	});

	it('lets the trailing member leave a group at the end of the strip', () => {
		const trailingGroupState = createWorkbenchTabsState({
			activeTabId: 'group-last',
			tabGroups: [{ color: 'blue', id: 'group-a', label: 'Group A' }],
			tabs: [
				{ id: 'plain-first', label: 'Plain First' },
				{ groupId: 'group-a', id: 'group-first', label: 'Group First' },
				{ groupId: 'group-a', id: 'group-last', label: 'Group Last' },
			],
		});
		const trailingGroupGeometry: readonly WorkbenchTabStripItemGeometry[] = [
			{ id: 'plain-first', kind: 'tab', left: 0, width: 100 },
			{ id: 'group-a', kind: 'group', left: 100, width: 60 },
			{ id: 'group-first', kind: 'tab', left: 160, width: 100 },
			{ id: 'group-last', kind: 'tab', left: 260, width: 100 },
		];

		expect(
			resolveWorkbenchTabDragProjection(
				trailingGroupState,
				trailingGroupGeometry,
				'group-last',
				380,
				100,
			),
		).toEqual({ groupId: undefined, targetIndex: 2 });
	});

	it('moves a complete group block in both directions and preserves the pinned boundary', () => {
		expect(resolveWorkbenchTabGroupDragProjection(STATE, GEOMETRY, 'group-a', 750)).toBe(4);
		expect(resolveWorkbenchTabGroupDragProjection(STATE, GEOMETRY, 'group-a', 210)).toBe(2);
		expect(resolveWorkbenchTabGroupDragProjection(STATE, GEOMETRY, 'group-a', 50)).toBe(2);
	});

	it('rejects missing drag sources', () => {
		expect(resolveWorkbenchTabDragProjection(STATE, GEOMETRY, 'missing', 400, 120)).toBeUndefined();
		expect(resolveWorkbenchTabGroupDragProjection(STATE, GEOMETRY, 'missing', 400)).toBeUndefined();
	});
});
