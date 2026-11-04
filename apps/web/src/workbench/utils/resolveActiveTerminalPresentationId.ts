import {
	findPanel,
	getDisplayedTerminal,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';

export function resolveActiveTerminalPresentationId(
	projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime,
): string | null {
	const terminalContext = projectWorkspaceRuntime.getActiveTerminalContext();
	const activeTabId = terminalContext.tabModel.getSnapshot().activeTabId;
	if (activeTabId === null) return null;
	const activeTab = terminalContext.panelWorkspaceStore
		.getSnapshot()
		.tabs.find((tab) => tab.tabId === activeTabId);
	if (activeTab === undefined) return null;
	const focusedPanel = findPanel(activeTab.root, activeTab.focusedPanelId);
	return focusedPanel === null ? null : getDisplayedTerminal(focusedPanel).presentationId;
}
