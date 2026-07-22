import type { Rect } from '../geometry';
import type { Size } from '../layout';

export const PROJECTS_PANEL_MIN_WIDTH = 240;
export const PROJECTS_PANEL_INTERIOR_MARGIN = 12;
export const PROJECTS_PANEL_SHOW_DURATION_MS = 250;
export const PROJECTS_PANEL_HIDE_DURATION_MS = 200;
export const PROJECTS_PANEL_CORNER_RADIUS = 16;
export const PROJECTS_PANEL_MAX_RECENT_THREADS = 300;
export const PROJECTS_PANEL_COLLAPSED_THREAD_COUNT = 3;

export interface ProjectsPanelLayoutInput {
	width?: number;
	height?: number;
	controlsVisible: boolean;
	controlsPreferredHeight: number;
}

export interface ProjectsPanelLayoutResult {
	hostSize: Size;
	controlsBounds: Rect | null;
}

/** Port of ProjectsPanelView::AnimationProgressed's macOS-only close-button fade. */
export function getProjectsPanelCloseButtonOpacity(animationValue: number, isMac: boolean): number {
	if (!isMac) return 1;
	return Math.max(0, Math.min(1, (animationValue - 0.5) * 2));
}

/** Direct port of ProjectsPanelViewLayout::CalculateProposedLayout. */
export function calculateProjectsPanelLayout(
	input: ProjectsPanelLayoutInput,
): ProjectsPanelLayoutResult {
	const hostWidth = input.width ?? PROJECTS_PANEL_MIN_WIDTH;
	const x = PROJECTS_PANEL_INTERIOR_MARGIN;
	const y = PROJECTS_PANEL_INTERIOR_MARGIN;
	const width = Math.max(0, hostWidth - PROJECTS_PANEL_INTERIOR_MARGIN * 2);
	const controlsHeight = input.controlsVisible ? input.controlsPreferredHeight : 0;
	const totalHeight = y + controlsHeight + PROJECTS_PANEL_INTERIOR_MARGIN;
	return {
		hostSize: { width: hostWidth, height: input.height ?? totalHeight },
		controlsBounds: input.controlsVisible ? { x, y, width, height: controlsHeight } : null,
	};
}
