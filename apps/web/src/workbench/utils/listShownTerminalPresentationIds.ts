import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	collectPanelLeaves,
	getDisplayedTerminal,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';

/** Every terminal the active Tab's Panels show; none where no Workspace or Tab is open. */
export function listShownTerminalPresentationIds(
	projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime,
): readonly TerminalPresentationId[] {
	const terminalContext = projectWorkspaceRuntime.getActiveTerminalContext();
	if (terminalContext === null) return [];
	const activeTabId = terminalContext.tabModel.getSnapshot().activeTabId;
	const activeTab = terminalContext.panelWorkspaceStore
		.getSnapshot()
		.tabs.find((tab) => tab.tabId === activeTabId);
	return activeTab === undefined
		? []
		: collectPanelLeaves(activeTab.root).map((panel) => getDisplayedTerminal(panel).presentationId);
}
