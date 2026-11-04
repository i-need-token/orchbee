import type { PanelLayoutNode, PanelSplitOrientation } from './panel-layout-types';

type PanelEqualizationResolution =
	| {
			readonly kind: 'applied';
			readonly root: PanelLayoutNode;
	  }
	| {
			readonly kind: 'unchanged';
			readonly reason: 'already-equal';
			readonly root: PanelLayoutNode;
	  };

const TOTAL_WEIGHT = 100;

export function resolvePanelEqualization(root: PanelLayoutNode): PanelEqualizationResolution {
	const equalizedRoot = equalizeNode(root);
	return equalizedRoot === root
		? { kind: 'unchanged', reason: 'already-equal', root }
		: { kind: 'applied', root: equalizedRoot };
}

function equalizeNode(node: PanelLayoutNode): PanelLayoutNode {
	if (node.kind === 'panel') return node;

	const childUnits = node.children.map((child) => axisUnits(child.node, node.orientation));
	const totalUnits = childUnits.reduce((total, units) => total + units, 0);
	let allocatedWeight = 0;
	let changed = false;
	const children = node.children.map((child, index) => {
		const equalizedChild = equalizeNode(child.node);
		const weight =
			index === node.children.length - 1
				? TOTAL_WEIGHT - allocatedWeight
				: (TOTAL_WEIGHT * (childUnits[index] ?? 0)) / totalUnits;
		allocatedWeight += weight;
		if (equalizedChild === child.node && Math.abs(child.weight - weight) <= Number.EPSILON) {
			return child;
		}
		changed = true;
		return { ...child, node: equalizedChild, weight };
	});

	return changed ? { ...node, children } : node;
}

function axisUnits(node: PanelLayoutNode, orientation: PanelSplitOrientation): number {
	return node.kind === 'split' && node.orientation === orientation
		? node.children.reduce((total, child) => total + axisUnits(child.node, orientation), 0)
		: 1;
}
