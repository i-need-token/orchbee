import type { TerminalPresentationId } from '../../../../terminal-presentation';
import {
	findPanel,
	getDisplayedTerminal,
	type PanelLayoutNode,
	type TabPanelWorkspace,
} from './panel-workspace-state';

/** Projects the terminal identities that can contribute pixels to the current Tab layout. */
export function projectTerminalTabPresentationHandoffTarget(
	tab: TabPanelWorkspace | null,
	terminalSurfaceAvailable: boolean,
): readonly TerminalPresentationId[] {
	if (tab === null || !terminalSurfaceAvailable) return Object.freeze([]);
	if (tab.maximizedPanelId !== null) {
		const maximizedPanel = findPanel(tab.root, tab.maximizedPanelId);
		if (maximizedPanel !== null) {
			return Object.freeze([getDisplayedTerminal(maximizedPanel).presentationId]);
		}
	}

	const presentationIds: TerminalPresentationId[] = [];
	collectContributingPresentationIds(tab.root, presentationIds);
	return Object.freeze(presentationIds);
}

function collectContributingPresentationIds(
	node: PanelLayoutNode,
	presentationIds: TerminalPresentationId[],
): void {
	if (node.kind === 'panel') {
		presentationIds.push(getDisplayedTerminal(node).presentationId);
		return;
	}
	for (const child of node.children) {
		if (child.weight > 0) collectContributingPresentationIds(child.node, presentationIds);
	}
}
