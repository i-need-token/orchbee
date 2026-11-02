import { describe, expect, it } from 'vitest';

import { describeTabDragRelease } from '../describeTabDragRelease';
import { TabModel, type TabData, type TabGroupData } from '../tab-model';

const GROUPS: readonly TabGroupData[] = [
	{ collapsed: false, color: 'blue', id: 'g', title: 'G' },
	{ collapsed: false, color: 'red', id: 'h', title: 'H' },
];

function tab(id: string, group: string | null = null, pinned = false): TabData {
	return { group, id, pinned, title: id };
}

function modelOf(tabs: readonly TabData[]): TabModel {
	return new TabModel({ groups: [...GROUPS], tabs: [...tabs] });
}

function arrangement(model: TabModel): readonly (readonly [string, string | null])[] {
	return model.getSnapshot().tabs.map((candidate) => [candidate.id, candidate.group]);
}

/** Moves as a pointer does, one step per position it crosses, each within the pinned partition. */
function drag(
	model: TabModel,
	tabIds: readonly string[],
	steps: readonly (readonly [number, string | null])[],
): void {
	for (const [targetIndex, group] of steps) {
		const snapshot = model.getSnapshot();
		const partition = snapshot.tabs.filter((candidate) => candidate.pinned).length;
		const pinned =
			snapshot.tabs.find((candidate) => tabIds.includes(candidate.id))?.pinned === true;
		model.moveTabsByDrag(
			tabIds,
			targetIndex,
			pinned ? 0 : partition,
			pinned ? partition : snapshot.tabs.length,
			group,
			{ coalesceNonconsecutive: true },
		);
	}
}

describe('describeTabDragRelease', () => {
	it.each<{
		readonly name: string;
		readonly steps: readonly (readonly [number, string | null])[];
		readonly tabIds: readonly string[];
		readonly tabs: readonly TabData[];
	}>([
		{
			name: 'forward over one Tab',
			steps: [[2, null]],
			tabIds: ['a'],
			tabs: [tab('a'), tab('b'), tab('c')],
		},
		{
			name: 'to the end through several positions',
			steps: [
				[2, null],
				[3, null],
				[4, null],
			],
			tabIds: ['a'],
			tabs: [tab('a'), tab('b'), tab('c'), tab('d')],
		},
		{
			name: 'backward to the start',
			steps: [
				[2, null],
				[1, null],
				[0, null],
			],
			tabIds: ['c'],
			tabs: [tab('a'), tab('b'), tab('c')],
		},
		{
			name: 'into a group',
			steps: [[2, 'g']],
			tabIds: ['a'],
			tabs: [tab('a'), tab('b', 'g'), tab('c', 'g'), tab('d')],
		},
		{
			name: 'out of a group',
			steps: [[3, null]],
			tabIds: ['b'],
			tabs: [tab('a', 'g'), tab('b', 'g'), tab('c')],
		},
		{
			name: 'a non-contiguous selection, which closes up on its first step',
			steps: [[4, null]],
			tabIds: ['a', 'c'],
			tabs: [tab('a'), tab('b'), tab('c'), tab('d')],
		},
		{
			name: 'a part of one group into another',
			steps: [[4, 'h']],
			tabIds: ['b'],
			tabs: [tab('a', 'g'), tab('b', 'g'), tab('c', 'h'), tab('d', 'h')],
		},
		{
			name: 'a whole group past a loose Tab',
			steps: [[3, null]],
			tabIds: ['a', 'b'],
			tabs: [tab('a', 'g'), tab('b', 'g'), tab('c'), tab('d')],
		},
		{
			name: 'a group of one past a loose Tab',
			steps: [[2, null]],
			tabIds: ['a'],
			tabs: [tab('a', 'g'), tab('b'), tab('c')],
		},
		{
			name: 'within the pinned Tabs',
			steps: [[2, null]],
			tabIds: ['a'],
			tabs: [tab('a', null, true), tab('b', null, true), tab('c')],
		},
		{
			name: 'an unpinned Tab held off the pinned ones',
			steps: [[0, null]],
			tabIds: ['c'],
			tabs: [tab('a', null, true), tab('b'), tab('c')],
		},
	])('replays $name as the same arrangement', ({ steps, tabIds, tabs }) => {
		const moved = modelOf(tabs);
		const before = moved.captureDragRestoreState();
		drag(moved, tabIds, steps);

		const release = describeTabDragRelease(before, moved.captureDragRestoreState(), tabIds);

		if (release === null) throw new Error('Expected the drag to change the arrangement');
		expect(release.sourceOrder).toEqual(tabs.map((candidate) => candidate.id));
		// The corpus proves this call answers as the Server's move-tabs-by-drag does.
		const replayed = modelOf(tabs);
		drag(replayed, release.tabIds, [[release.insertIndex, release.groupId]]);
		expect(arrangement(replayed)).toEqual(arrangement(moved));
	});

	it('says nothing when the Tabs end where they started', () => {
		const model = modelOf([tab('a'), tab('b'), tab('c')]);
		const before = model.captureDragRestoreState();
		drag(
			model,
			['a'],
			[
				[2, null],
				[0, null],
			],
		);
		expect(describeTabDragRelease(before, model.captureDragRestoreState(), ['a'])).toBeNull();
	});
});
