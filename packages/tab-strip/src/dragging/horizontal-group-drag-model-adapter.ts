import { type TabModel, type TabDragRestoreState, type TabModelSnapshot } from '../model';
import {
	createHorizontalGroupDragState,
	transitionHorizontalGroupDrag,
	type HorizontalGroupDragCommand,
	type HorizontalGroupDragState,
} from './horizontal-group-drag-machine';
export function createArmedHorizontalGroupDrag(
	snapshot: Readonly<TabModelSnapshot>,
	groupId: string,
): HorizontalGroupDragState {
	const initial = createHorizontalGroupDragState({
		tabs: snapshot.tabs.map((tab) => ({
			id: tab.id,
			pinned: tab.pinned,
			group: tab.group,
			split: tab.split,
		})),
		groups: snapshot.groups.map((group) => ({
			id: group.id,
			collapsed: group.collapsed,
		})),
		activeTabId: snapshot.activeTabId,
		selectedTabIds: [...snapshot.selectedTabIds],
		selectionAnchorId: snapshot.selectionAnchorId,
		focusedGroupId: snapshot.focusedGroupId,
	});
	return transitionHorizontalGroupDrag(initial, {
		type: 'arm-group',
		groupId,
		collapseExpandedBackingModel: true,
	}).state;
}
export function assertHorizontalGroupDragRefinement(
	model: TabModel,
	expected: Readonly<HorizontalGroupDragState>,
): void {
	const snapshot = model.getSnapshot();
	const actualTabs = snapshot.tabs.map((tab) => [tab.id, tab.pinned, tab.group, tab.split]);
	const expectedTabs = expected.authoritative.tabs.map((tab) => [
		tab.id,
		tab.pinned,
		tab.group,
		tab.split,
	]);
	const actualGroups = snapshot.groups.map((group) => [group.id, group.collapsed]);
	const expectedGroups = expected.authoritative.groups.map((group) => [group.id, group.collapsed]);
	const actualSelected = [...snapshot.selectedTabIds].sort();
	const expectedSelected = [...expected.authoritative.selectedTabIds].sort();
	if (
		JSON.stringify(actualTabs) !== JSON.stringify(expectedTabs) ||
		JSON.stringify(actualGroups) !== JSON.stringify(expectedGroups) ||
		snapshot.activeTabId !== expected.authoritative.activeTabId ||
		JSON.stringify(actualSelected) !== JSON.stringify(expectedSelected) ||
		snapshot.selectionAnchorId !== expected.authoritative.selectionAnchorId ||
		snapshot.focusedGroupId !== expected.authoritative.focusedGroupId
	) {
		throw new Error('Horizontal group drag no longer refines its deterministic semantic state.');
	}
}
export function applyHorizontalGroupDragCommands(
	model: TabModel,
	restoreState: Readonly<TabDragRestoreState>,
	commands: readonly Readonly<HorizontalGroupDragCommand>[],
): void {
	for (const command of commands) {
		if (command.type === 'set-selection') {
			model.setSelection(command.selectedTabIds, command.activeTabId, {
				expandGroups: false,
			});
		} else if (command.type === 'set-group-collapsed') {
			model.updateGroup(command.groupId, {
				collapsed: command.collapsed,
			});
		} else if (command.type === 'move-group') {
			model.moveGroupByDrag(command.groupId, command.preRemovalTargetIndex);
		} else {
			model.restoreCanceledDrag(restoreState);
		}
	}
}
