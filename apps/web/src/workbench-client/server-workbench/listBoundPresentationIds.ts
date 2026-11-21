import type {
	TerminalPresentationId,
	WorkbenchPanelNode,
	WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

/** Every Presentation that holds a terminal, in snapshot order. */
export function listBoundPresentationIds(snapshot: WorkbenchSnapshot): TerminalPresentationId[] {
	const presentationIds: TerminalPresentationId[] = [];
	const collect = (node: WorkbenchPanelNode): void => {
		if (node.kind === 'panel') {
			for (const presentation of [node.primaryPresentation, ...node.childPresentations]) {
				if (presentation.binding !== null) presentationIds.push(presentation.presentationId);
			}
			return;
		}
		for (const slot of node.children) collect(slot.node);
	};
	for (const context of snapshot.terminalContexts) {
		for (const tab of context.tabs) collect(tab.root);
	}
	return presentationIds;
}
