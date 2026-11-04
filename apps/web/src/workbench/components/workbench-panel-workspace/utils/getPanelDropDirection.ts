import type { PanelSplitDirection } from '../model/panel-workspace-state';

interface PanelDropPoint {
	readonly x: number;
	readonly y: number;
}

interface PanelDropRect {
	readonly bottom: number;
	readonly height: number;
	readonly left: number;
	readonly right: number;
	readonly top: number;
	readonly width: number;
}

const DROP_EDGE_RATIO = 0.35;

export function getPanelDropDirection(
	rect: PanelDropRect,
	point: PanelDropPoint,
): PanelSplitDirection | null {
	if (rect.width <= 0 || rect.height <= 0) {
		return null;
	}
	const distances = [
		{ direction: 'left', ratio: (point.x - rect.left) / rect.width },
		{ direction: 'right', ratio: (rect.right - point.x) / rect.width },
		{ direction: 'above', ratio: (point.y - rect.top) / rect.height },
		{ direction: 'below', ratio: (rect.bottom - point.y) / rect.height },
	] as const;
	const closestEdge = distances.reduce((closest, candidate) =>
		candidate.ratio < closest.ratio ? candidate : closest,
	);
	return closestEdge.ratio <= DROP_EDGE_RATIO ? closestEdge.direction : null;
}
