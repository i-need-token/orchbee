import type { PanelLayoutNode, TabPanelWorkspace } from './panel-workspace-state';

type PanelRevision = readonly [
	'panel',
	panelId: string,
	primaryPresentationId: string,
	childPresentationIds: readonly string[],
	displayedChildPresentationId: string | null,
];

type SplitRevision = readonly [
	'split',
	splitId: string,
	orientation: 'horizontal' | 'vertical',
	children: readonly (readonly [slotId: string, node: LayoutRevisionNode])[],
];

type LayoutRevisionNode = PanelRevision | SplitRevision;

/** Projects only discrete facts that can replace a mounted terminal layout. */
export function projectTerminalTabPresentationRevision(
	presentationScopeId: string,
	tab: TabPanelWorkspace | null,
): string | null {
	if (tab === null) return null;
	return JSON.stringify([
		presentationScopeId,
		tab.tabId,
		tab.maximizedPanelId,
		projectLayoutRevisionNode(tab.root),
	]);
}

function projectLayoutRevisionNode(node: PanelLayoutNode): LayoutRevisionNode {
	if (node.kind === 'panel') {
		return [
			'panel',
			node.id,
			node.primaryPresentation.presentationId,
			node.childPresentations.map((presentation) => presentation.presentationId),
			node.activeChildPresentationId,
		];
	}
	return [
		'split',
		node.id,
		node.orientation,
		node.children.map((child): readonly [string, LayoutRevisionNode] => [
			child.id,
			projectLayoutRevisionNode(child.node),
		]),
	];
}
