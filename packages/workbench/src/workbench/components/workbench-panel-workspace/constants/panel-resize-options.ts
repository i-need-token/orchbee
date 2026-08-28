import type { PanelResizeAdjustment } from '../model/resolvePanelResize';
import type { WorkbenchPanelWorkspaceView } from '../types/panel-workspace-view';

export const PANEL_RESIZE_OPTIONS = Object.freeze([
	{ adjustment: 'wider', key: 'W', message: 'resizeWider' },
	{ adjustment: 'narrower', key: 'N', message: 'resizeNarrower' },
	{ adjustment: 'taller', key: 'T', message: 'resizeTaller' },
	{ adjustment: 'shorter', key: 'S', message: 'resizeShorter' },
	{ adjustment: 'reset', key: 'R', message: 'resizeReset' },
] as const satisfies readonly {
	readonly adjustment: PanelResizeAdjustment;
	readonly key: string;
	readonly message: keyof WorkbenchPanelWorkspaceView['messages'];
}[]);
