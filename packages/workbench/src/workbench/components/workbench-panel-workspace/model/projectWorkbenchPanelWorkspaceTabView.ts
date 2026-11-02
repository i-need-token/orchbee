import { collectPanelLeaves, type TabPanelWorkspace } from './panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../types/panel-workspace-view';

/**
 * Rebinds read-only layout projection to one presentation layer. A drag shows only in the layer of
 * the revision it started from, which keeps a released drop on screen until its replacement activates.
 */
export function projectWorkbenchPanelWorkspaceTabView(
	view: WorkbenchPanelWorkspaceView,
	tab: TabPanelWorkspace,
	revision: string,
): WorkbenchPanelWorkspaceView {
	const holdsDrag = view.panelDrag.sourceRevision === revision;
	if (view.activeTab === tab && (view.panelDrag.sourceRevision === null || holdsDrag)) return view;
	return {
		...view,
		activeTab: tab,
		displayedRoot: tab.root,
		maximizedPanelId: tab.maximizedPanelId,
		panelCount: collectPanelLeaves(tab.root).length,
		panelDrag: holdsDrag
			? view.panelDrag
			: {
					draggingPanelId: null,
					dropTarget: null,
					ghost: null,
					previewRoot: null,
					sourceRevision: null,
				},
		panelVisualSelection: null,
	};
}
