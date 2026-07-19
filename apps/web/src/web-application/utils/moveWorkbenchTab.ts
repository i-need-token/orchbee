import type { WorkbenchTab } from '@orchbee/workbench';

export function moveWorkbenchTab(
	tabs: readonly WorkbenchTab[],
	tabId: string,
	targetIndex: number,
): readonly WorkbenchTab[] {
	const sourceIndex = tabs.findIndex((tab) => tab.id === tabId);
	if (sourceIndex === -1 || targetIndex < 0 || targetIndex >= tabs.length) return tabs;
	if (sourceIndex === targetIndex) return tabs;

	const reorderedTabs = [...tabs];
	const [tab] = reorderedTabs.splice(sourceIndex, 1);
	if (tab === undefined) return tabs;

	reorderedTabs.splice(targetIndex, 0, tab);
	return reorderedTabs;
}
