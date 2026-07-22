import {
	createWorkbenchTabStripModel,
	projectWorkbenchTabsState,
} from './workbench-tab-strip-model';
import type { WorkbenchTabsState, WorkbenchTabsStateInput } from './workbench-tabs-state';

export function createWorkbenchTabsState(input: WorkbenchTabsStateInput): WorkbenchTabsState {
	const state = projectWorkbenchTabsState(createWorkbenchTabStripModel(input));
	const suppliedGroups = input.tabGroups ?? [];
	const tabs =
		state.tabs.length === input.tabs.length &&
		state.tabs.every((tab, index) => tab === input.tabs[index])
			? input.tabs
			: state.tabs;
	const tabGroups =
		state.tabGroups.length === suppliedGroups.length &&
		state.tabGroups.every((group, index) => group === suppliedGroups[index])
			? suppliedGroups
			: state.tabGroups;
	return tabs === state.tabs && tabGroups === state.tabGroups
		? state
		: { ...state, tabGroups, tabs };
}
