import type { WorkbenchTabsState } from '../../../../workbench-tabs';
import type { WorkbenchTabDragState } from '../types/workbench-tab-drag';

export function getWorkbenchTabsTopologyKey(state: WorkbenchTabsState): string {
	const tabs = state.tabs
		.map((tab) => `${tab.id}:${tab.isPinned === true ? 'pinned' : 'open'}:${tab.groupId ?? ''}`)
		.join('|');
	const groups = state.tabGroups
		.map((group) => `${group.id}:${group.isCollapsed === true ? 'collapsed' : 'expanded'}`)
		.join('|');
	return `${tabs}/${groups}`;
}

export function hasWorkbenchTabDragActivePointer(
	state: WorkbenchTabDragState,
): state is Extract<WorkbenchTabDragState, { readonly pointerId: number }> {
	return (
		state.kind === 'armed-group' ||
		state.kind === 'armed-tab' ||
		state.kind === 'dragging-group' ||
		state.kind === 'dragging-tab'
	);
}
