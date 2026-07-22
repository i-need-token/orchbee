import { createWorkbenchTabsState, reduceWorkbenchTabs } from '../../../../../workbench-tabs';
import type { WorkbenchTabsState } from '../../../../../workbench-tabs';
import { describe, expect, it } from 'vitest';

import type {
	WorkbenchTabDragEvent,
	WorkbenchTabDragState,
	WorkbenchTabStripItemGeometry,
} from '../../types/workbench-tab-drag';
import { transitionWorkbenchTabDrag } from '../transitionWorkbenchTabDrag';

const SOURCE_STATE = createWorkbenchTabsState({
	activeTabId: 'first',
	tabGroups: [{ color: 'blue', id: 'group-a', label: 'Group A' }],
	tabs: [
		{ id: 'first', label: 'First' },
		{ groupId: 'group-a', id: 'second', label: 'Second' },
		{ groupId: 'group-a', id: 'third', label: 'Third' },
		{ id: 'fourth', label: 'Fourth' },
	],
});

const GEOMETRY: readonly WorkbenchTabStripItemGeometry[] = [
	{ id: 'first', kind: 'tab', left: 0, width: 100 },
	{ id: 'group-a', kind: 'group', left: 100, width: 60 },
	{ id: 'second', kind: 'tab', left: 160, width: 100 },
	{ id: 'third', kind: 'tab', left: 260, width: 100 },
	{ id: 'fourth', kind: 'tab', left: 360, width: 100 },
];

type WorkbenchTabDragMoveEvent = Extract<WorkbenchTabDragEvent, { readonly kind: 'move' }>;
type TestWorkbenchTabDragEvent =
	| Exclude<WorkbenchTabDragEvent, WorkbenchTabDragMoveEvent>
	| (Omit<WorkbenchTabDragMoveEvent, 'primaryButtonPressed'> & {
			readonly primaryButtonPressed?: boolean;
	  });

function transition(
	state: WorkbenchTabDragState,
	event: TestWorkbenchTabDragEvent,
): ReturnType<typeof transitionWorkbenchTabDrag> {
	return transitionWorkbenchTabDrag(
		state,
		event.kind === 'move'
			? { ...event, primaryButtonPressed: event.primaryButtonPressed ?? true }
			: event,
	);
}

function createGeometry(state: WorkbenchTabsState): readonly WorkbenchTabStripItemGeometry[] {
	const geometry: WorkbenchTabStripItemGeometry[] = [];
	const visitedGroups = new Set<string>();
	const groupsById = new Map(state.tabGroups.map((group) => [group.id, group]));
	let left = 0;
	for (const tab of state.tabs) {
		if (tab.groupId !== undefined && !visitedGroups.has(tab.groupId)) {
			visitedGroups.add(tab.groupId);
			geometry.push({ id: tab.groupId, kind: 'group', left, width: 60 });
			left += 60;
			if (groupsById.get(tab.groupId)?.isCollapsed === true) continue;
		}
		if (tab.groupId !== undefined && groupsById.get(tab.groupId)?.isCollapsed === true) {
			continue;
		}
		geometry.push({ id: tab.id, kind: 'tab', left, width: 100 });
		left += 100;
	}
	return geometry;
}

describe('transitionWorkbenchTabDrag', () => {
	it('keeps sub-threshold movement armed and releases it as a click', () => {
		const armed = transition(
			{ kind: 'idle' },
			{
				draggedTabId: 'first',
				draggedWidth: 100,
				grabOffset: 20,
				kind: 'arm-tab',
				layout: GEOMETRY,
				pointerId: 1,
				pointerX: 20,
				pointerY: 0,
				sourceLeft: 0,
				state: SOURCE_STATE,
			},
		);
		const moved = transition(armed.state, {
			kind: 'move',
			pointerId: 1,
			pointerX: 23,
			pointerY: 0,
		});

		expect(moved.state.kind).toBe('armed-tab');
		expect(moved.commands).toEqual([]);
		expect(transition(moved.state, { kind: 'release', pointerId: 1 })).toEqual({
			commands: [],
			state: { kind: 'idle' },
		});
	});

	it('cancels an armed gesture when movement arrives without the primary button pressed', () => {
		const armed = transition(
			{ kind: 'idle' },
			{
				draggedGroupId: 'group-a',
				draggedWidth: 260,
				grabOffset: 30,
				kind: 'arm-group',
				layout: GEOMETRY,
				pointerId: 3,
				pointerX: 130,
				pointerY: 0,
				sourceLeft: 100,
				state: SOURCE_STATE,
			},
		);

		expect(
			transition(armed.state, {
				kind: 'move',
				pointerId: 3,
				pointerX: 430,
				pointerY: 0,
				primaryButtonPressed: false,
			}),
		).toEqual({ commands: [], state: { kind: 'idle' } });
	});

	it('activates once at the threshold, previews without mutating source, and commits once', () => {
		const armed = transition(
			{ kind: 'idle' },
			{
				draggedTabId: 'first',
				draggedWidth: 100,
				grabOffset: 20,
				kind: 'arm-tab',
				layout: GEOMETRY,
				pointerId: 1,
				pointerX: 20,
				pointerY: 0,
				sourceLeft: 0,
				state: SOURCE_STATE,
			},
		);
		const dragging = transition(armed.state, {
			kind: 'move',
			pointerId: 1,
			pointerX: 430,
			pointerY: 0,
		});

		expect(dragging.commands).toEqual([{ kind: 'activate-tab', tabId: 'first' }]);
		expect(dragging.state.kind).toBe('dragging-tab');
		if (dragging.state.kind !== 'dragging-tab') throw new Error('Expected a tab drag');
		expect(dragging.state.sourceState).toBe(SOURCE_STATE);
		expect(dragging.state.previewState.tabs.map((tab) => tab.id)).toEqual([
			'second',
			'third',
			'fourth',
			'first',
		]);

		const repeated = transition(dragging.state, {
			kind: 'move',
			pointerId: 1,
			pointerX: 431,
			pointerY: 0,
		});
		expect(repeated.commands).toEqual([]);
		if (repeated.state.kind !== 'dragging-tab') throw new Error('Expected a tab drag');
		const released = transition(repeated.state, { kind: 'release', pointerId: 1 });
		expect(released.commands).toEqual([
			{
				groupId: undefined,
				kind: 'commit-tab-move',
				tabId: 'first',
				targetIndex: 3,
			},
		]);
		expect(released.state).toEqual({
			kind: 'settling',
			previewState: repeated.state.previewState,
		});
		expect(transition(released.state, { kind: 'acknowledge-commit' })).toEqual({
			commands: [],
			state: { kind: 'idle' },
		});
	});

	it('requires a reverse crossing margin after accepting a new slot', () => {
		const sourceState = createWorkbenchTabsState({
			activeTabId: 'first',
			tabs: [
				{ id: 'first', label: 'First' },
				{ id: 'second', label: 'Second' },
				{ id: 'third', label: 'Third' },
			],
		});
		const previewState = createWorkbenchTabsState({
			activeTabId: 'first',
			tabs: [
				{ id: 'second', label: 'Second' },
				{ id: 'first', label: 'First' },
				{ id: 'third', label: 'Third' },
			],
		});
		const dragging: WorkbenchTabDragState = {
			draggedTabId: 'first',
			draggedWidth: 100,
			grabOffset: 50,
			kind: 'dragging-tab',
			lastAcceptedPointerX: 95,
			layout: [
				{ id: 'first', kind: 'tab', left: 0, width: 100 },
				{ id: 'second', kind: 'tab', left: 100, width: 100 },
				{ id: 'third', kind: 'tab', left: 200, width: 100 },
			],
			pointerId: 4,
			pointerX: 95,
			pointerY: 0,
			previewState,
			sourceLeft: 0,
			sourcePointerX: 50,
			sourcePointerY: 0,
			sourceState,
		};

		const jitter = transition(dragging, {
			kind: 'move',
			pointerId: 4,
			pointerX: 90,
			pointerY: 0,
		});
		expect(jitter.state.kind).toBe('dragging-tab');
		if (jitter.state.kind !== 'dragging-tab') throw new Error('Expected a tab drag');
		expect(jitter.state.previewState).toBe(previewState);

		const reversed = transition(jitter.state, {
			kind: 'move',
			pointerId: 4,
			pointerX: 80,
			pointerY: 0,
		});
		expect(reversed.state.kind).toBe('dragging-tab');
		if (reversed.state.kind !== 'dragging-tab') throw new Error('Expected a tab drag');
		expect(reversed.state.previewState.tabs.map((tab) => tab.id)).toEqual([
			'first',
			'second',
			'third',
		]);
	});

	it('previews and commits a trailing group member leaving at the end of the strip', () => {
		const sourceState = createWorkbenchTabsState({
			activeTabId: 'group-last',
			tabGroups: [{ color: 'blue', id: 'group-a', label: 'Group A' }],
			tabs: [
				{ id: 'plain-first', label: 'Plain First' },
				{ groupId: 'group-a', id: 'group-first', label: 'Group First' },
				{ groupId: 'group-a', id: 'group-last', label: 'Group Last' },
			],
		});
		const geometry: readonly WorkbenchTabStripItemGeometry[] = [
			{ id: 'plain-first', kind: 'tab', left: 0, width: 100 },
			{ id: 'group-a', kind: 'group', left: 100, width: 60 },
			{ id: 'group-first', kind: 'tab', left: 160, width: 100 },
			{ id: 'group-last', kind: 'tab', left: 260, width: 100 },
		];
		const armed = transition(
			{ kind: 'idle' },
			{
				draggedTabId: 'group-last',
				draggedWidth: 100,
				grabOffset: 50,
				kind: 'arm-tab',
				layout: geometry,
				pointerId: 5,
				pointerX: 310,
				pointerY: 0,
				sourceLeft: 260,
				state: sourceState,
			},
		);

		const dragging = transition(armed.state, {
			kind: 'move',
			pointerId: 5,
			pointerX: 380,
			pointerY: 0,
		});
		expect(dragging.state.kind).toBe('dragging-tab');
		if (dragging.state.kind !== 'dragging-tab') throw new Error('Expected a tab drag');
		expect(dragging.state.previewState.tabs.at(-1)).toMatchObject({
			groupId: undefined,
			id: 'group-last',
		});
		expect(transition(dragging.state, { kind: 'release', pointerId: 5 }).commands).toEqual([
			{
				groupId: undefined,
				kind: 'commit-tab-move',
				tabId: 'group-last',
				targetIndex: 2,
			},
		]);
	});

	it('commits exactly the final preview for every tab path', () => {
		const sourceStates = [
			createWorkbenchTabsState({
				activeTabId: 'a',
				tabs: [
					{ id: 'a', label: 'A' },
					{ id: 'b', label: 'B' },
					{ id: 'c', label: 'C' },
					{ id: 'd', label: 'D' },
				],
			}),
			createWorkbenchTabsState({
				activeTabId: 'a',
				tabGroups: [{ color: 'blue', id: 'group-a', label: 'Group A' }],
				tabs: [
					{ id: 'a', label: 'A' },
					{ groupId: 'group-a', id: 'b', label: 'B' },
					{ groupId: 'group-a', id: 'c', label: 'C' },
					{ id: 'd', label: 'D' },
				],
			}),
			createWorkbenchTabsState({
				activeTabId: 'b',
				tabGroups: [
					{ color: 'blue', id: 'group-a', label: 'Group A' },
					{ color: 'red', id: 'group-b', label: 'Group B' },
				],
				tabs: [
					{ id: 'pinned', isPinned: true, label: 'Pinned' },
					{ groupId: 'group-a', id: 'a', label: 'A' },
					{ groupId: 'group-a', id: 'b', label: 'B' },
					{ id: 'plain', label: 'Plain' },
					{ groupId: 'group-b', id: 'c', label: 'C' },
					{ groupId: 'group-b', id: 'd', label: 'D' },
				],
			}),
		] as const;

		for (const sourceState of sourceStates) {
			const geometry = createGeometry(sourceState);
			const stripWidth = geometry.reduce(
				(maximum, item) => Math.max(maximum, item.left + item.width),
				0,
			);
			for (const draggedTab of sourceState.tabs) {
				const sourceGeometry = geometry.find(
					(item) => item.kind === 'tab' && item.id === draggedTab.id,
				);
				if (sourceGeometry === undefined) continue;
				const sourcePointerX = sourceGeometry.left + sourceGeometry.width / 2;
				const destinations = Array.from(
					{ length: Math.ceil((stripWidth + 200) / 25) },
					(_, index) => index * 25 - 100,
				);
				for (const destination of destinations) {
					for (const pointerPath of [
						[destination],
						[sourcePointerX + 12, destination],
						[sourcePointerX - 12, destination],
					]) {
						let current = transition(
							{ kind: 'idle' },
							{
								draggedTabId: draggedTab.id,
								draggedWidth: sourceGeometry.width,
								grabOffset: sourceGeometry.width / 2,
								kind: 'arm-tab',
								layout: geometry,
								pointerId: 42,
								pointerX: sourcePointerX,
								pointerY: 0,
								sourceLeft: sourceGeometry.left,
								state: sourceState,
							},
						).state;
						for (const pointerX of pointerPath) {
							current = transition(current, {
								kind: 'move',
								pointerId: 42,
								pointerX,
								pointerY: 0,
							}).state;
						}
						if (current.kind !== 'dragging-tab') continue;
						const previewState = current.previewState;
						const released = transition(current, { kind: 'release', pointerId: 42 });
						const command = released.commands[0];
						const committedState =
							command?.kind === 'commit-tab-move'
								? reduceWorkbenchTabs(sourceState, {
										groupId: command.groupId,
										kind: 'move-tab',
										tabId: command.tabId,
										targetIndex: command.targetIndex,
									})
								: sourceState;

						expect(committedState, `${draggedTab.id} -> ${destination}`).toEqual(previewState);
					}
				}
			}
		}
	});

	it('cancels active tab and group previews without a commit', () => {
		const armedTab = transition(
			{ kind: 'idle' },
			{
				draggedTabId: 'fourth',
				draggedWidth: 100,
				grabOffset: 50,
				kind: 'arm-tab',
				layout: GEOMETRY,
				pointerId: 2,
				pointerX: 410,
				pointerY: 0,
				sourceLeft: 360,
				state: SOURCE_STATE,
			},
		);
		const draggingTab = transition(armedTab.state, {
			kind: 'move',
			pointerId: 2,
			pointerX: 40,
			pointerY: 0,
		});
		expect(transition(draggingTab.state, { kind: 'cancel', pointerId: 2 })).toEqual({
			commands: [],
			state: { kind: 'idle' },
		});

		const armedGroup = transition(
			{ kind: 'idle' },
			{
				draggedGroupId: 'group-a',
				draggedWidth: 260,
				grabOffset: 30,
				kind: 'arm-group',
				layout: GEOMETRY,
				pointerId: 3,
				pointerX: 130,
				pointerY: 0,
				sourceLeft: 100,
				state: SOURCE_STATE,
			},
		);
		const draggingGroup = transition(armedGroup.state, {
			kind: 'move',
			pointerId: 3,
			pointerX: 430,
			pointerY: 0,
		});
		expect(draggingGroup.commands).toEqual([{ groupId: 'group-a', kind: 'activate-tab-group' }]);
		expect(transition(draggingGroup.state, { kind: 'stale-input' })).toEqual({
			commands: [],
			state: { kind: 'idle' },
		});
	});

	it('moves a complete group when its directional edge crosses a neighboring center', () => {
		const armGroup = (): WorkbenchTabDragState =>
			transition(
				{ kind: 'idle' },
				{
					draggedGroupId: 'group-a',
					draggedWidth: 260,
					grabOffset: 30,
					kind: 'arm-group',
					layout: GEOMETRY,
					pointerId: 3,
					pointerX: 130,
					pointerY: 0,
					sourceLeft: 100,
					state: SOURCE_STATE,
				},
			).state;

		const movedTowardEnd = transition(armGroup(), {
			kind: 'move',
			pointerId: 3,
			pointerX: 181,
			pointerY: 0,
		});
		expect(movedTowardEnd.state.kind).toBe('dragging-group');
		if (movedTowardEnd.state.kind !== 'dragging-group') throw new Error('Expected a group drag');
		expect(movedTowardEnd.state.previewState.tabs.map((tab) => tab.id)).toEqual([
			'first',
			'fourth',
			'second',
			'third',
		]);

		const movedTowardStart = transition(armGroup(), {
			kind: 'move',
			pointerId: 3,
			pointerX: 79,
			pointerY: 0,
		});
		expect(movedTowardStart.state.kind).toBe('dragging-group');
		if (movedTowardStart.state.kind !== 'dragging-group') throw new Error('Expected a group drag');
		expect(movedTowardStart.state.previewState.tabs.map((tab) => tab.id)).toEqual([
			'second',
			'third',
			'first',
			'fourth',
		]);
	});

	it('ignores out-of-order pointer events and repeated arming', () => {
		const armed = transition(
			{ kind: 'idle' },
			{
				draggedTabId: 'first',
				draggedWidth: 100,
				grabOffset: 20,
				kind: 'arm-tab',
				layout: GEOMETRY,
				pointerId: 1,
				pointerX: 20,
				pointerY: 0,
				sourceLeft: 0,
				state: SOURCE_STATE,
			},
		);
		expect(
			transition(armed.state, {
				kind: 'move',
				pointerId: 99,
				pointerX: 300,
				pointerY: 0,
			}),
		).toEqual({ commands: [], state: armed.state });
		expect(
			transition(armed.state, {
				draggedGroupId: 'group-a',
				draggedWidth: 260,
				grabOffset: 30,
				kind: 'arm-group',
				layout: GEOMETRY,
				pointerId: 2,
				pointerX: 130,
				pointerY: 0,
				sourceLeft: 100,
				state: SOURCE_STATE,
			}),
		).toEqual({ commands: [], state: armed.state });
	});
});
