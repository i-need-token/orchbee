import type { PanelSplit } from '../../../../../model/panel-workspace-state';
import { WORKBENCH_RESIZE_TARGET_SIZE_PX } from '../../../../../../../model/workbench-resize-target';

export const MINIMUM_PANEL_SIZE_PX = 30;
export const PANEL_RESIZE_TARGET_SIZE_PX = WORKBENCH_RESIZE_TARGET_SIZE_PX;
export const DISABLED_SEPARATOR_KEYS = new Set(['End', 'Enter', 'F6', 'Home']);
export const RESIZE_ARROW_KEYS = {
	horizontal: new Set(['ArrowLeft', 'ArrowRight']),
	vertical: new Set(['ArrowDown', 'ArrowUp']),
} as const satisfies Readonly<Record<PanelSplit['orientation'], ReadonlySet<string>>>;
