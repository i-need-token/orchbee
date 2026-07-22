import type { WorkbenchTab } from '../../../types/workbench-tab';

interface WorkbenchTabCloseTargets {
	readonly allTabIds: readonly string[];
	readonly otherTabIds: readonly string[];
	readonly rightTabIds: readonly string[];
}

export function getWorkbenchTabCloseTargets(
	tabs: readonly WorkbenchTab[],
	targetIndex: number,
): WorkbenchTabCloseTargets {
	return {
		allTabIds: tabs.map((tab) => tab.id),
		otherTabIds: tabs.filter((_tab, index) => index !== targetIndex).map((tab) => tab.id),
		rightTabIds: tabs.slice(targetIndex + 1).map((tab) => tab.id),
	};
}
