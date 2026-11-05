import {
	findPanel,
	getDisplayedTerminal,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchFocusedPanel } from '../types/workbench-focused-panel';

/** The active Tab's focused Panel and the terminal it shows; null where no Workspace or Tab is open. */
export function resolveFocusedPanel(
	projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime,
): WorkbenchFocusedPanel | null {
	const location = projectWorkspaceRuntime.getSnapshot().activeLocation;
	const terminalContext = projectWorkspaceRuntime.getActiveTerminalContext();
	if (location.kind === 'no-workspace' || terminalContext === null) return null;
	const activeTabId = terminalContext.tabModel.getSnapshot().activeTabId;
	const activeTab = terminalContext.panelWorkspaceStore
		.getSnapshot()
		.tabs.find((tab) => tab.tabId === activeTabId);
	if (activeTab === undefined) return null;
	const focusedPanel = findPanel(activeTab.root, activeTab.focusedPanelId);
	return focusedPanel === null
		? null
		: {
				panelId: focusedPanel.id,
				presentationId: getDisplayedTerminal(focusedPanel).presentationId,
				workspaceId: location.workspaceId,
			};
}
