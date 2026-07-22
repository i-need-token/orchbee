import type { WorkbenchTab } from '../../../types/workbench-tab';

export function resolveWorkbenchTabGroupDragActiveTabId(
	tabs: readonly WorkbenchTab[],
	activeTabId: string | undefined,
	groupId: string,
): string | undefined {
	const activeTab = tabs.find((tab) => tab.id === activeTabId);
	if (activeTab?.groupId === groupId) return activeTab.id;
	return tabs.find((tab) => tab.groupId === groupId)?.id;
}
