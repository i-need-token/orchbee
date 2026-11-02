import type {
	WorkbenchPanelNode,
	WorkbenchPresentation,
	WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

/** A Presentation as this snapshot holds it, in whichever Tab and Panel it sits. */
export function findServerPresentation(
	snapshot: WorkbenchSnapshot,
	presentationId: string,
): WorkbenchPresentation | null {
	for (const context of snapshot.terminalContexts) {
		for (const tab of context.tabs) {
			const presentation = findInNode(tab.root, presentationId);
			if (presentation !== null) return presentation;
		}
	}
	return null;
}

function findInNode(
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
		const presentation = findInNode(slot.node, presentationId);
		if (presentation !== null) return presentation;
	}
	return null;
}
