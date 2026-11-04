import { findWorkbenchWorkspace } from '../../project-workspace';
import { collectPanelLeaves } from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchProjectCreationReceipt } from '../types/workbench-project-folder-acquisition';
import type { WorkbenchProjectWorkspaceRuntime } from './WorkbenchProjectWorkspaceRuntime';

/**
 * Whether what this Client holds still shows what the receipt says a Project creation made: its
 * Workspace in the receipt's folder and context, and the first terminal the Server's Workspace
 * launch started there.
 */
export function matchesWorkbenchProjectCreation(
	projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime,
	receipt: WorkbenchProjectCreationReceipt,
): boolean {
	const workspace = findWorkbenchWorkspace(
		projectWorkspaceRuntime.getSnapshot(),
		receipt.workspaceId,
	);
	if (
		workspace?.directory?.id !== receipt.directoryId ||
		workspace.directory.path !== receipt.path ||
		workspace.terminalContextId !== receipt.terminalContextId
	)
		return false;
	const context = projectWorkspaceRuntime.getTerminalContext(receipt.terminalContextId);
	return (
		context?.panelWorkspaceStore
			.getSnapshot()
			.tabs.some((tab) =>
				collectPanelLeaves(tab.root).some((panel) =>
					[panel.primaryPresentation, ...panel.childPresentations].some(
						(presentation) => presentation.presentationId === receipt.presentationId,
					),
				),
			) === true
	);
}
