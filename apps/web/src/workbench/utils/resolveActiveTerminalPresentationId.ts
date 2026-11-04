import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	findPanel,
	getDisplayedTerminal,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';

/** The terminal the focused Panel of the active Tab shows; null where no Workspace or Tab is open. */
export function resolveActiveTerminalPresentationId(
	projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime,
): TerminalPresentationId | null {
	const terminalContext = projectWorkspaceRuntime.getActiveTerminalContext();
	if (terminalContext === null) return null;
	const activeTabId = terminalContext.tabModel.getSnapshot().activeTabId;
	if (activeTabId === null) return null;
	const activeTab = terminalContext.panelWorkspaceStore
		.getSnapshot()
		.tabs.find((tab) => tab.tabId === activeTabId);
	if (activeTab === undefined) return null;
	const focusedPanel = findPanel(activeTab.root, activeTab.focusedPanelId);
	return focusedPanel === null ? null : getDisplayedTerminal(focusedPanel).presentationId;
}
