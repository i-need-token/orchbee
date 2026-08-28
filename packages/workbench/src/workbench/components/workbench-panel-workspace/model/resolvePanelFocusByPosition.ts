import { collectPanelLeaves, type PanelLayoutNode } from './panel-workspace-state';

export type PanelFocusPosition = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10;

interface ResolvePanelFocusByPositionOptions {
	readonly maximizedPanelId: string | null;
	readonly position: PanelFocusPosition;
	readonly root: PanelLayoutNode;
	readonly sourcePanelId: string;
}

export type PanelFocusByPositionResolution =
	| {
			readonly kind: 'ready';
			readonly panelId: string;
	  }
	| {
			readonly kind: 'unavailable';
			readonly reason:
				| 'invalid-panel-order'
				| 'panel-maximized'
				| 'panel-position-unavailable'
				| 'source-panel-unavailable';
	  };

/** Resolve one exact one-based position in canonical depth-first Panel-leaf order. */
export function resolvePanelFocusByPosition({
	maximizedPanelId,
	position,
	root,
	sourcePanelId,
}: ResolvePanelFocusByPositionOptions): PanelFocusByPositionResolution {
	if (maximizedPanelId !== null) {
		return { kind: 'unavailable', reason: 'panel-maximized' };
	}

	const panelIds = collectPanelLeaves(root).map((panel) => panel.id);
	if (!panelIds.includes(sourcePanelId)) {
		return { kind: 'unavailable', reason: 'source-panel-unavailable' };
	}
	if (new Set(panelIds).size !== panelIds.length) {
		return { kind: 'unavailable', reason: 'invalid-panel-order' };
	}

	const panelId = panelIds[position - 1];
	return panelId === undefined
		? { kind: 'unavailable', reason: 'panel-position-unavailable' }
		: { kind: 'ready', panelId };
}
