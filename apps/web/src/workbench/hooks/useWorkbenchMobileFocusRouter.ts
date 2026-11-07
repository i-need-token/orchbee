import { useMemoizedFn } from '@orchbee/foundation/react';

import type {
	WorkbenchFocusIntent,
	WorkbenchFocusResolution,
} from '../model/resolveWorkbenchFocusTarget';
import type { WorkbenchFocusRouting } from '../types/workbench-focus-routing';
import type { WorkbenchSidebarPresentationView } from './useWorkbenchSidebarPresentation';

const TAB_STRIP_TARGET: WorkbenchFocusResolution = { kind: 'target', region: 'tab-strip' };

/**
 * The focus Actions' router in every layout. In the mobile layout the Tab strip lives in the Tab
 * list, so focusing it opens the list, which then takes the keyboard; and the keyboard never goes
 * to a terminal that a full-window list covers, so focusing the terminal closes both lists.
 */
export function useWorkbenchMobileFocusRouter({
	focusRouting,
	hasTabs,
	sidebar,
}: {
	readonly focusRouting: Pick<WorkbenchFocusRouting, 'focus' | 'getResolution'>;
	/** Whether a Workspace's Tabs are shown, without which there is no Tab list to open. */
	readonly hasTabs: boolean;
	readonly sidebar: WorkbenchSidebarPresentationView;
}): Pick<WorkbenchFocusRouting, 'focus' | 'getResolution'> {
	const opensTabList = (intent: WorkbenchFocusIntent, resolution: WorkbenchFocusResolution) =>
		sidebar.mobile &&
		!sidebar.tabListOpen &&
		hasTabs &&
		intent.kind === 'direct' &&
		intent.region === 'tab-strip' &&
		!(resolution.kind === 'unavailable' && resolution.reason === 'exclusive-surface-active');
	const getResolution = useMemoizedFn((intent: WorkbenchFocusIntent) => {
		const resolution = focusRouting.getResolution(intent);
		return opensTabList(intent, resolution) ? TAB_STRIP_TARGET : resolution;
	});
	const focus = useMemoizedFn((intent: WorkbenchFocusIntent) => {
		if (opensTabList(intent, focusRouting.getResolution(intent))) {
			sidebar.toggleTabList();
			return TAB_STRIP_TARGET;
		}
		const resolution = focusRouting.focus(intent);
		if (sidebar.mobile && resolution.kind === 'target' && resolution.region === 'terminal') {
			sidebar.dismissTabList();
			sidebar.dismissOverlay();
		}
		return resolution;
	});
	return { focus, getResolution };
}
