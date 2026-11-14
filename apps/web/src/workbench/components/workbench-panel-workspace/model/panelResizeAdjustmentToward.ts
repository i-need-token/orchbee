import type { PanelLayoutNode, PanelSplit } from './panel-layout-types';
import type { PanelResizeAdjustment } from './resolvePanelResize';

export type PanelResizeDirection = 'down' | 'left' | 'right' | 'up';

/**
 * The size change that moves the divider beside the Panel toward `direction`, as the TUI moves it:
 * the nearest split on that axis moves, whichever side of it the Panel is on. Null when no divider
 * on that axis touches the Panel.
 */
export function panelResizeAdjustmentToward(
	root: PanelLayoutNode,
	panelId: string,
	direction: PanelResizeDirection,
): Exclude<PanelResizeAdjustment, 'reset'> | null {
	const horizontal = direction === 'left' || direction === 'right';
	const side = nearestSplitSide(root, panelId, horizontal ? 'horizontal' : 'vertical');
	if (side === 'absent' || side === 'no-split') return null;
	const towardEnd = direction === 'right' || direction === 'down';
	const grows = side === 'first' ? towardEnd : !towardEnd;
	if (horizontal) return grows ? 'wider' : 'narrower';
	return grows ? 'taller' : 'shorter';
}

function nearestSplitSide(
	node: PanelLayoutNode,
	panelId: string,
	orientation: PanelSplit['orientation'],
): 'absent' | 'first' | 'no-split' | 'second' {
	if (node.kind === 'panel') return node.id === panelId ? 'no-split' : 'absent';
	for (const [index, child] of node.children.entries()) {
		const side = nearestSplitSide(child.node, panelId, orientation);
		if (side === 'absent') continue;
		if (side !== 'no-split' || node.orientation !== orientation) return side;
		return index === 0 ? 'first' : 'second';
	}
	return 'absent';
}
