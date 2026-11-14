import type { PanelLayoutNode, PanelSplit } from './panel-layout-types';

export type PanelResizeAdjustment = 'narrower' | 'shorter' | 'taller' | 'wider';

type PanelResizeResolution =
	| {
			readonly kind: 'applied';
			readonly root: PanelLayoutNode;
	  }
	| {
			readonly kind: 'unchanged';
			readonly reason: 'axis-unavailable' | 'boundary';
			readonly root: PanelLayoutNode;
	  }
	| {
			readonly kind: 'rejected';
			readonly reason: 'panel-not-found';
			readonly root: PanelLayoutNode;
	  };

const KEYBOARD_RESIZE_STEP_PERCENT = 5;
const MINIMUM_VISIBLE_BRANCH_PERCENT = 5;

interface ResizeTraversal {
	readonly containsTarget: boolean;
	readonly handledAxis: boolean;
	readonly node: PanelLayoutNode;
}

export function resolvePanelResize(
	root: PanelLayoutNode,
	panelId: string,
	adjustment: PanelResizeAdjustment,
	minimumBranchPercent: number = MINIMUM_VISIBLE_BRANCH_PERCENT,
): PanelResizeResolution {
	if (!containsPanel(root, panelId)) {
		return { kind: 'rejected', reason: 'panel-not-found', root };
	}

	const orientation =
		adjustment === 'narrower' || adjustment === 'wider' ? 'horizontal' : 'vertical';
	const increase = adjustment === 'taller' || adjustment === 'wider';
	const traversal = resizeNearestAncestor(
		root,
		panelId,
		orientation,
		(increase ? 1 : -1) * KEYBOARD_RESIZE_STEP_PERCENT,
		minimumBranchPercent,
	);
	if (!traversal.handledAxis) {
		return { kind: 'unchanged', reason: 'axis-unavailable', root };
	}
	return traversal.node === root
		? { kind: 'unchanged', reason: 'boundary', root }
		: { kind: 'applied', root: traversal.node };
}

function containsPanel(node: PanelLayoutNode, panelId: string): boolean {
	return (
		(node.kind === 'panel' && node.id === panelId) ||
		(node.kind === 'split' && node.children.some((child) => containsPanel(child.node, panelId)))
	);
}

function resizeNearestAncestor(
	node: PanelLayoutNode,
	panelId: string,
	orientation: PanelSplit['orientation'],
	activeBranchDelta: number,
	minimumBranchPercent: number,
): ResizeTraversal {
	if (node.kind === 'panel') {
		return { containsTarget: node.id === panelId, handledAxis: false, node };
	}

	for (const [index, child] of node.children.entries()) {
		const childResult = resizeNearestAncestor(
			child.node,
			panelId,
			orientation,
			activeBranchDelta,
			minimumBranchPercent,
		);
		if (!childResult.containsTarget) continue;
		if (childResult.handledAxis) {
			if (childResult.node === child.node) return { ...childResult, node };
			const children = [...node.children];
			children[index] = { ...child, node: childResult.node };
			return { containsTarget: true, handledAxis: true, node: { ...node, children } };
		}
		if (node.orientation !== orientation) {
			return { containsTarget: true, handledAxis: false, node };
		}
		return {
			containsTarget: true,
			handledAxis: true,
			node: resizeSplitBranch(node, index, activeBranchDelta, minimumBranchPercent),
		};
	}
	return { containsTarget: false, handledAxis: false, node };
}

function resizeSplitBranch(
	split: PanelSplit,
	activeChildIndex: number,
	activeBranchDelta: number,
	minimumBranchPercent: number,
): PanelSplit {
	const activeChild = split.children[activeChildIndex];
	const otherChildIndex = activeChildIndex === 0 ? 1 : 0;
	const otherChild = split.children[otherChildIndex];
	if (activeChild === undefined || otherChild === undefined) return split;
	const pairWeight = activeChild.weight + otherChild.weight;
	const minimumWeight = Math.min(minimumBranchPercent, pairWeight / 2);
	if (
		(activeBranchDelta < 0 && activeChild.weight <= minimumWeight) ||
		(activeBranchDelta > 0 && otherChild.weight <= minimumWeight)
	) {
		return split;
	}
	const nextActiveWeight = Math.min(
		Math.max(activeChild.weight + activeBranchDelta, minimumWeight),
		pairWeight - minimumWeight,
	);
	if (Math.abs(nextActiveWeight - activeChild.weight) <= Number.EPSILON) return split;

	const children = [...split.children];
	children[activeChildIndex] = { ...activeChild, weight: nextActiveWeight };
	children[otherChildIndex] = { ...otherChild, weight: pairWeight - nextActiveWeight };
	return { ...split, children };
}
