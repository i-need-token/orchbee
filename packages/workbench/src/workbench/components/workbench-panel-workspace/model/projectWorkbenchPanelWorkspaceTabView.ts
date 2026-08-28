import { collectPanelLeaves, type TabPanelWorkspace } from './panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../types/panel-workspace-view';

/** Rebinds read-only layout projection to a retained Tab while disabling drag preview state. */
export function projectWorkbenchPanelWorkspaceTabView(
	view: WorkbenchPanelWorkspaceView,
	tab: TabPanelWorkspace,
): WorkbenchPanelWorkspaceView {
	if (view.activeTab?.tabId === tab.tabId) return view;
	return {
		...view,
		activeTab: tab,
		displayedRoot: tab.root,
		maximizedPanelId: tab.maximizedPanelId,
		panelCount: collectPanelLeaves(tab.root).length,
		panelDrag: {
			draggingPanelId: null,
			dropTarget: null,
			ghost: null,
			previewRoot: null,
		},
		panelVisualSelection: null,
	};
}
