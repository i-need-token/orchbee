import type { PanelLayoutNode } from './panel-workspace-state';

export type PanelFocusDirection = 'down' | 'left' | 'right' | 'up';

interface PanelRectangle {
	readonly bottom: number;
	readonly left: number;
	readonly panelId: string;
	readonly right: number;
	readonly top: number;
	readonly visualIndex: number;
}

interface DirectionalCandidate {
	readonly centerDistance: number;
	readonly gap: number;
	readonly panelId: string;
	readonly visualIndex: number;
}

const FLOAT_TOLERANCE = 1e-9;

export function resolveDirectionalPanelFocus(
	root: PanelLayoutNode,
	sourcePanelId: string,
	direction: PanelFocusDirection,
): string | null {
	const rectangles: PanelRectangle[] = [];
	collectPanelRectangles(root, { bottom: 1, left: 0, right: 1, top: 0 }, rectangles);
	const source = rectangles.find((rectangle) => rectangle.panelId === sourcePanelId);
	if (source === undefined) return null;

	const candidates = rectangles.flatMap((rectangle) => {
		if (rectangle.panelId === sourcePanelId) return [];
		const candidate = projectDirectionalCandidate(source, rectangle, direction);
		return candidate === null ? [] : [candidate];
	});
	candidates.sort(compareDirectionalCandidates);
	return candidates[0]?.panelId ?? null;
}

function collectPanelRectangles(
	node: PanelLayoutNode,
	bounds: Omit<PanelRectangle, 'panelId' | 'visualIndex'>,
	output: PanelRectangle[],
): void {
	if (node.kind === 'panel') {
		output.push({ ...bounds, panelId: node.id, visualIndex: output.length });
		return;
	}

	const totalWeight = node.children.reduce((total, child) => total + child.weight, 0);
	const equalWeight = node.children.length === 0 ? 0 : 1 / node.children.length;
	const horizontal = node.orientation === 'horizontal';
	const start = horizontal ? bounds.left : bounds.top;
	const end = horizontal ? bounds.right : bounds.bottom;
	const span = end - start;
	let cursor = start;

	for (const [index, child] of node.children.entries()) {
		const isLast = index === node.children.length - 1;
		const fraction = totalWeight > FLOAT_TOLERANCE ? child.weight / totalWeight : equalWeight;
		const next = isLast ? end : cursor + span * fraction;
		collectPanelRectangles(
			child.node,
			horizontal
				? { ...bounds, left: cursor, right: next }
				: { ...bounds, bottom: next, top: cursor },
			output,
		);
		cursor = next;
	}
}

function projectDirectionalCandidate(
	source: PanelRectangle,
	candidate: PanelRectangle,
	direction: PanelFocusDirection,
): DirectionalCandidate | null {
	const horizontalMovement = direction === 'left' || direction === 'right';
	const perpendicularOverlap = horizontalMovement
		? overlap(source.top, source.bottom, candidate.top, candidate.bottom)
		: overlap(source.left, source.right, candidate.left, candidate.right);
	if (perpendicularOverlap <= FLOAT_TOLERANCE) return null;

	const eligible = isInDirection(source, candidate, direction);
	if (!eligible) return null;

	return {
		centerDistance: horizontalMovement
			? Math.abs(center(source.top, source.bottom) - center(candidate.top, candidate.bottom))
			: Math.abs(center(source.left, source.right) - center(candidate.left, candidate.right)),
		gap: directionalGap(source, candidate, direction),
		panelId: candidate.panelId,
		visualIndex: candidate.visualIndex,
	};
}

function isInDirection(
	source: PanelRectangle,
	candidate: PanelRectangle,
	direction: PanelFocusDirection,
): boolean {
	switch (direction) {
		case 'down':
			return candidate.top >= source.bottom - FLOAT_TOLERANCE;
		case 'left':
			return candidate.right <= source.left + FLOAT_TOLERANCE;
		case 'right':
			return candidate.left >= source.right - FLOAT_TOLERANCE;
		case 'up':
			return candidate.bottom <= source.top + FLOAT_TOLERANCE;
	}
}

function directionalGap(
	source: PanelRectangle,
	candidate: PanelRectangle,
	direction: PanelFocusDirection,
): number {
	switch (direction) {
		case 'down':
			return Math.max(0, candidate.top - source.bottom);
		case 'left':
			return Math.max(0, source.left - candidate.right);
		case 'right':
			return Math.max(0, candidate.left - source.right);
		case 'up':
			return Math.max(0, source.top - candidate.bottom);
	}
}

function compareDirectionalCandidates(
	left: DirectionalCandidate,
	right: DirectionalCandidate,
): number {
	const gapOrder = compareNumbers(left.gap, right.gap);
	if (gapOrder !== 0) return gapOrder;
	const centerOrder = compareNumbers(left.centerDistance, right.centerDistance);
	return centerOrder === 0 ? left.visualIndex - right.visualIndex : centerOrder;
}

function compareNumbers(left: number, right: number): number {
	return Math.abs(left - right) <= FLOAT_TOLERANCE ? 0 : left - right;
}

function center(start: number, end: number): number {
	return (start + end) / 2;
}

function overlap(
	firstStart: number,
	firstEnd: number,
	secondStart: number,
	secondEnd: number,
): number {
	return Math.min(firstEnd, secondEnd) - Math.max(firstStart, secondStart);
}
