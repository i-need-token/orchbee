import { collectPanelLeaves, type PanelLayoutNode } from './panel-workspace-state';

export type PanelFocusCycleDirection = 'next' | 'previous';

interface ResolveCyclicPanelFocusOptions {
	readonly direction: PanelFocusCycleDirection;
	readonly maximizedPanelId: string | null;
	readonly root: PanelLayoutNode;
	readonly sourcePanelId: string;
}

export function resolveCyclicPanelFocus({
	direction,
	maximizedPanelId,
	root,
	sourcePanelId,
}: ResolveCyclicPanelFocusOptions): string | null {
	if (maximizedPanelId !== null) return null;

	const panels = collectPanelLeaves(root);
	if (panels.length < 2) return null;
	const sourceIndex = panels.findIndex((panel) => panel.id === sourcePanelId);
	if (sourceIndex < 0) return null;

	const delta = direction === 'next' ? 1 : -1;
	const targetIndex = (sourceIndex + delta + panels.length) % panels.length;
	return panels[targetIndex]?.id ?? null;
}
