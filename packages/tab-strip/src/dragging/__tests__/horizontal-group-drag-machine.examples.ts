import { describe, expect, it } from 'vitest';
import {
	createHorizontalGroupDragState,
	getHorizontalGroupDragActiveSurface,
	getHorizontalGroupDragInvariantViolations,
	transitionHorizontalGroupDrag,
	type HorizontalGroupDragAuthoritativeState,
	type HorizontalGroupDragState,
} from '../horizontal-group-drag-machine';

function makeState(
	input: Partial<HorizontalGroupDragAuthoritativeState> = {},
): HorizontalGroupDragState {
	return createHorizontalGroupDragState({
		tabs: [
			{ id: 'r1', pinned: false, group: 'research' },
			{ id: 'r2', pinned: false, group: 'research' },
			{ id: 'b1', pinned: false, group: 'build' },
			{ id: 'b2', pinned: false, group: 'build' },
		],
		groups: [
			{ id: 'research', collapsed: false },
			{ id: 'build', collapsed: false },
		],
		activeTabId: 'b1',
		selectedTabIds: ['b1'],
		selectionAnchorId: 'b1',
		focusedGroupId: null,
		...input,
	});
}

function apply(
	state: HorizontalGroupDragState,
	event: Parameters<typeof transitionHorizontalGroupDrag>[1],
): HorizontalGroupDragState {
	return transitionHorizontalGroupDrag(state, event).state;
}

describe('horizontal group drag machine', () => {
	it('moves an expanded group as a full visual and restores one active selection', () => {
		let state = makeState();
		state = apply(state, { type: 'arm-group', groupId: 'research' });
		expect(state.drag).toMatchObject({
			kind: 'armed',
			recordedActiveTabId: 'r1',
			visualKind: 'expanded-group',
		});

		const activated = transitionHorizontalGroupDrag(state, { type: 'activate-drag' });
		state = activated.state;
		expect(activated.commands).toEqual([
			{ type: 'set-selection', selectedTabIds: ['r1', 'r2'], activeTabId: 'r1' },
			{ type: 'set-group-collapsed', groupId: 'research', collapsed: true },
		]);
		expect(state.authoritative.groups[0]?.collapsed).toBe(true);
		expect(getHorizontalGroupDragInvariantViolations(state)).toEqual([]);

		state = apply(state, { type: 'move-to', preRemovalTargetIndex: 4 });
		expect(state.authoritative.tabs.map((tab) => tab.id)).toEqual(['b1', 'b2', 'r1', 'r2']);
		expect(getHorizontalGroupDragInvariantViolations(state)).toEqual([]);

		const committed = transitionHorizontalGroupDrag(state, { type: 'commit' });
		state = committed.state;
		expect(committed.commands).toEqual([
			{ type: 'set-group-collapsed', groupId: 'research', collapsed: false },
			{ type: 'set-selection', selectedTabIds: ['r1'], activeTabId: 'r1' },
		]);
		expect(state.authoritative.selectedTabIds).toEqual(['r1']);
		expect(state.authoritative.groups[0]?.collapsed).toBe(false);
		expect(state.drag.kind).toBe('settling');
		expect(getHorizontalGroupDragInvariantViolations(state)).toEqual([]);

		state = apply(state, { type: 'settle' });
		expect(state.drag.kind).toBe('idle');
	});

	it('keeps a previously collapsed group compact and projects its recorded active through the header', () => {
		let state = makeState({
			groups: [
				{ id: 'research', collapsed: true },
				{ id: 'build', collapsed: false },
			],
		});
		state = apply(state, { type: 'arm-group', groupId: 'research' });
		const activated = transitionHorizontalGroupDrag(state, { type: 'activate-drag' });
		state = activated.state;

		expect(state.drag).toMatchObject({ kind: 'dragging', visualKind: 'collapsed-header' });
		expect(activated.commands).toEqual([
			{ type: 'set-selection', selectedTabIds: ['r1', 'r2'], activeTabId: 'r1' },
		]);
		expect(state.authoritative.groups[0]?.collapsed).toBe(true);

		const committed = transitionHorizontalGroupDrag(state, { type: 'commit' });
		state = committed.state;
		expect(state.authoritative.groups[0]?.collapsed).toBe(true);
		expect(committed.commands).toEqual([
			{ type: 'set-selection', selectedTabIds: ['r1'], activeTabId: 'r1' },
		]);
		expect(state.authoritative.activeTabId).toBe('r1');
		expect(state.authoritative.selectedTabIds).toEqual(['r1']);
		expect(getHorizontalGroupDragActiveSurface(state.authoritative)).toEqual({
			kind: 'group-header',
			id: 'research',
			activeTabId: 'r1',
		});
		expect(getHorizontalGroupDragInvariantViolations(state)).toEqual([]);
	});

	it('maps a grouped active identity onto its collapsed header surface', () => {
		const state = makeState({
			groups: [
				{ id: 'research', collapsed: true },
				{ id: 'build', collapsed: false },
			],
			activeTabId: 'r1',
			selectedTabIds: ['r1'],
			selectionAnchorId: 'r1',
		});

		expect(getHorizontalGroupDragActiveSurface(state.authoritative)).toEqual({
			kind: 'group-header',
			id: 'research',
			activeTabId: 'r1',
		});
		expect(getHorizontalGroupDragInvariantViolations(state)).toEqual([]);
	});

	it('records the active member when the active tab already belongs to the group', () => {
		let state = makeState({ activeTabId: 'r2', selectedTabIds: ['r2'], selectionAnchorId: 'r2' });
		state = apply(state, { type: 'arm-group', groupId: 'research' });
		state = apply(state, { type: 'activate-drag' });
		state = apply(state, { type: 'commit' });

		expect(state.authoritative.activeTabId).toBe('r2');
		expect(state.authoritative.selectedTabIds).toEqual(['r2']);
	});

	it('rejects a target inside a neighboring group', () => {
		let state = makeState();
		state = apply(state, { type: 'arm-group', groupId: 'research' });
		state = apply(state, { type: 'activate-drag' });
		const result = transitionHorizontalGroupDrag(state, {
			type: 'move-to',
			preRemovalTargetIndex: 3,
		});

		expect(result.commands).toEqual([]);
		expect(result.state.authoritative.tabs.map((tab) => tab.id)).toEqual(['r1', 'r2', 'b1', 'b2']);
	});

	it('restores the complete authoritative state when canceled', () => {
		const original = makeState();
		let state = apply(original, { type: 'arm-group', groupId: 'research' });
		state = apply(state, { type: 'activate-drag' });
		state = apply(state, { type: 'move-to', preRemovalTargetIndex: 4 });
		const canceled = transitionHorizontalGroupDrag(state, { type: 'cancel' });

		expect(canceled.commands).toEqual([{ type: 'restore' }]);
		expect(canceled.state).toEqual(original);
		expect(getHorizontalGroupDragInvariantViolations(canceled.state)).toEqual([]);
	});

	it('makes repeated and out-of-phase events explicit no-ops', () => {
		const state = makeState();
		expect(transitionHorizontalGroupDrag(state, { type: 'activate-drag' })).toEqual({
			state,
			commands: [],
		});
		expect(transitionHorizontalGroupDrag(state, { type: 'commit' })).toEqual({
			state,
			commands: [],
		});
	});
});
