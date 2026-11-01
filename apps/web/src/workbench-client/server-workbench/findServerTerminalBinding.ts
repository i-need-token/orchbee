import type {
	WorkbenchPanelNode,
	WorkbenchPresentation,
	WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

import type { ServerTerminalBinding } from '../server-connection';

/** The terminal a Presentation is bound to in this snapshot, if it holds one. */
export function findServerTerminalBinding(
	snapshot: WorkbenchSnapshot,
	presentationId: string,
): ServerTerminalBinding | null {
	for (const context of snapshot.terminalContexts) {
		for (const tab of context.tabs) {
			const presentation = findPresentation(tab.root, presentationId);
			if (presentation !== null) {
				return presentation.binding === null
					? null
					: { sessionId: snapshot.sessionId, ...presentation.binding };
			}
		}
	}
	return null;
}

function findPresentation(
	node: WorkbenchPanelNode,
	presentationId: string,
): WorkbenchPresentation | null {
	if (node.kind === 'panel') {
		return (
			[node.primaryPresentation, ...node.childPresentations].find(
				(presentation) => presentation.presentationId === presentationId,
			) ?? null
		);
	}
	for (const slot of node.children) {
		const presentation = findPresentation(slot.node, presentationId);
		if (presentation !== null) return presentation;
	}
	return null;
}
