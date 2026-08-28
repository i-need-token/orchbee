import type { PanelLayoutNode, PanelLeaf } from './panel-layout-types';

export type PanelSwapResolution =
	| {
			readonly kind: 'applied';
			readonly root: PanelLayoutNode;
	  }
	| {
			readonly kind: 'unchanged';
			readonly reason: 'same-panel';
			readonly root: PanelLayoutNode;
	  }
	| {
			readonly kind: 'rejected';
			readonly reason: 'panel-not-found';
			readonly root: PanelLayoutNode;
	  };

export function resolvePanelSwap(
	root: PanelLayoutNode,
	sourcePanelId: string,
	targetPanelId: string,
): PanelSwapResolution {
	if (sourcePanelId === targetPanelId) {
		return containsPanel(root, sourcePanelId)
			? { kind: 'unchanged', reason: 'same-panel', root }
			: { kind: 'rejected', reason: 'panel-not-found', root };
	}
	const source = findPanel(root, sourcePanelId);
	const target = findPanel(root, targetPanelId);
	if (source === null || target === null) {
		return { kind: 'rejected', reason: 'panel-not-found', root };
	}
	return {
		kind: 'applied',
		root: replacePanels(root, source, target),
	};
}

function containsPanel(node: PanelLayoutNode, panelId: string): boolean {
	return findPanel(node, panelId) !== null;
}

function findPanel(node: PanelLayoutNode, panelId: string): PanelLeaf | null {
	if (node.kind === 'panel') return node.id === panelId ? node : null;
	for (const child of node.children) {
		const panel = findPanel(child.node, panelId);
		if (panel !== null) return panel;
	}
	return null;
}

function replacePanels(
	node: PanelLayoutNode,
	source: PanelLeaf,
	target: PanelLeaf,
): PanelLayoutNode {
	if (node.kind === 'panel') {
		if (node.id === source.id) return target;
		if (node.id === target.id) return source;
		return node;
	}
	let changed = false;
	const children = node.children.map((child) => {
		const nextNode = replacePanels(child.node, source, target);
		if (nextNode === child.node) return child;
		changed = true;
		return { ...child, node: nextNode };
	});
	return changed ? { ...node, children } : node;
}
