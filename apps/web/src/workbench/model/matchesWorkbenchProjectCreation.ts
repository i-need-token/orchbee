import type {
	WorkbenchSharedDocument,
	WorkbenchSharedPanelNode,
} from '@orchbee/workbench-client-protocol';

import type { WorkbenchProjectCreationReceipt } from '../types/workbench-project-folder-acquisition';

export function matchesWorkbenchProjectCreation(
	document: WorkbenchSharedDocument,
	receipt: WorkbenchProjectCreationReceipt,
): boolean {
	const project = document.projectWorkspace.resourceScopes.find(
		(candidate) => candidate.id === receipt.resourceScopeId,
	);
	const directory = project?.directories.find((candidate) => candidate.id === receipt.directoryId);
	const workspace = document.projectWorkspace.workspaces.find(
		(candidate) => candidate.id === receipt.workspaceId,
	);
	if (
		directory?.kind !== 'folder' ||
		!directory.available ||
		project?.primaryDirectoryId !== receipt.directoryId ||
		directory.placement.kind !== 'local' ||
		directory.path !== receipt.path ||
		workspace?.resourceScopeId !== receipt.resourceScopeId ||
		workspace.primaryDirectoryId !== receipt.directoryId ||
		workspace.terminalContextId !== receipt.terminalContextId
	)
		return false;
	const context = document.terminalContexts.find(
		(candidate) => candidate.terminalContextId === receipt.terminalContextId,
	);
	return (
		context?.panelWorkspace.tabs.some((tab) => containsPresentation(tab.root, receipt)) === true
	);
}

function containsPresentation(
	node: WorkbenchSharedPanelNode,
	receipt: WorkbenchProjectCreationReceipt,
): boolean {
	if (node.kind === 'split')
		return node.children.some((child) => containsPresentation(child.node, receipt));
	// The Server's Workspace launch started this presentation in the receipt's Directory.
	return [node.primaryPresentation, ...node.childPresentations].some(
		(presentation) => presentation.presentationId === receipt.presentationId,
	);
}
