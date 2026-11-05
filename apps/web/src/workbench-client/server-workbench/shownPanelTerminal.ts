import type { WorkbenchPanelTerminal } from '../../workbench';
import type {
	TerminalPresentationId,
	WorkbenchPanelNode,
	WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

type SnapshotPanel = Extract<WorkbenchPanelNode, { readonly kind: 'panel' }>;

function panelsOf(node: WorkbenchPanelNode): readonly SnapshotPanel[] {
	return node.kind === 'panel' ? [node] : node.children.flatMap((child) => panelsOf(child.node));
}

/**
 * The Panel and the terminal shown in it as the Workspace holds them now, so a read names only what
 * the Server can answer for; null once the Workspace holds neither.
 */
export function shownPanelTerminal(
	snapshot: WorkbenchSnapshot,
	workspaceId: string,
	shown: WorkbenchPanelTerminal,
): {
	readonly panelId: SnapshotPanel['panelId'];
	readonly presentationId: TerminalPresentationId;
} | null {
	const workspace = snapshot.navigation.workspaces.find(
		(candidate) => candidate.workspaceId === workspaceId,
	);
	const context = snapshot.terminalContexts.find(
		(candidate) => candidate.terminalContextId === workspace?.terminalContextId,
	);
	const panel = context?.tabs
		.flatMap((tab) => panelsOf(tab.root))
		.find((candidate) => candidate.panelId === shown.panelId);
	const presentation = [panel?.primaryPresentation, ...(panel?.childPresentations ?? [])].find(
		(candidate) => candidate?.presentationId === shown.presentationId,
	);
	return panel === undefined || presentation === undefined
		? null
		: { panelId: panel.panelId, presentationId: presentation.presentationId };
}
