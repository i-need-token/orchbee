import type { TabDropEdge } from './resolveTabDropPosition';

export function resolveTabDropEdge(
	pointerX: number,
	targetLeft: number,
	targetWidth: number,
): TabDropEdge {
	return pointerX < targetLeft + targetWidth / 2 ? 'before' : 'after';
}
