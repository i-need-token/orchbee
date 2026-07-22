import { createWorkbenchTabsState } from '../../../../../workbench-tabs';
import { describe, expect, it } from 'vitest';

import {
	getWorkbenchTabsTopologyKey,
	hasWorkbenchTabDragActivePointer,
} from '../workbench-tab-drag-state';

describe('workbench tab drag state', () => {
	it('keys only authoritative topology that can invalidate a drag', () => {
		const state = createWorkbenchTabsState({
			activeTabId: 'first',
			tabGroups: [{ color: 'blue', id: 'group-a', label: 'Group A' }],
			tabs: [
				{ id: 'first', label: 'First' },
				{ groupId: 'group-a', id: 'second', label: 'Second' },
			],
		});
		const presentationOnlyChange = {
			...state,
			activeTabId: 'second',
			tabGroups: [{ color: 'red' as const, id: 'group-a', label: 'Renamed' }],
			tabs: state.tabs.map((tab) => ({ ...tab, label: `Renamed ${tab.label}` })),
		};

		expect(getWorkbenchTabsTopologyKey(presentationOnlyChange)).toBe(
			getWorkbenchTabsTopologyKey(state),
		);
		expect(
			getWorkbenchTabsTopologyKey({
				...state,
				tabs: [
					{ groupId: 'group-a', id: 'second', label: 'Second' },
					{ id: 'first', label: 'First' },
				],
			}),
		).not.toBe(getWorkbenchTabsTopologyKey(state));
	});

	it('excludes the post-release settling phase from pointer cancellation', () => {
		const state = createWorkbenchTabsState({ tabs: [{ id: 'first', label: 'First' }] });

		expect(hasWorkbenchTabDragActivePointer({ kind: 'idle' })).toBe(false);
		expect(hasWorkbenchTabDragActivePointer({ kind: 'settling', previewState: state })).toBe(false);
		expect(
			hasWorkbenchTabDragActivePointer({
				draggedTabId: 'first',
				draggedWidth: 100,
				grabOffset: 50,
				kind: 'armed-tab',
				layout: [{ id: 'first', kind: 'tab', left: 0, width: 100 }],
				pointerId: 1,
				pointerX: 50,
				pointerY: 0,
				sourceLeft: 0,
				sourcePointerX: 50,
				sourcePointerY: 0,
				sourceState: state,
			}),
		).toBe(true);
	});
});
