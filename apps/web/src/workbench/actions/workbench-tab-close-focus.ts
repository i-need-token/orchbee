import type { ActionExecutionContext } from '@orchbee/actions';

import type {
	WorkbenchLastPanelTarget,
	WorkbenchPanelFocusTrail,
} from '../model/WorkbenchPanelFocusTrail';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAuthorityClosure } from '../types/workbench-authority';
import type {
	WorkbenchHostActionResult,
	WorkbenchNavigationPort,
} from '../types/workbench-host-actions';

interface WorkbenchPanelFocusTargetOptions {
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
}

/**
 * Where keyboard focus goes after a close. `prepare` runs before the close and returns the step that
 * moves focus, or null where focus stays; the caller runs that step once the close leaves this view.
 */
export interface WorkbenchFocusAfterTabClose {
	readonly prepare: (
		context: ActionExecutionContext,
		closure: WorkbenchAuthorityClosure,
	) => (() => void) | null;
}

/**
 * Selects the Tab and Workspace a target shows, then focuses its Panel. Last Panel and a Tab close
 * that returns focus share this step, so both reach a Panel the same way.
 */
export function focusWorkbenchPanelTarget(
	context: ActionExecutionContext,
	target: WorkbenchLastPanelTarget,
	{ navigation, projectWorkspaceRuntime }: WorkbenchPanelFocusTargetOptions,
): WorkbenchHostActionResult {
	target.context.tabModel.selectTab(target.tabId);
	const store = target.context.panelWorkspaceStore;
	const maximizedPanelId =
		store.getSnapshot().tabs.find((tab) => tab.tabId === target.tabId)?.maximizedPanelId ?? null;
	if (maximizedPanelId !== null && maximizedPanelId !== target.panelId) {
		store.transition({
			panelId: maximizedPanelId,
			tabId: target.tabId,
			type: 'panel-maximization-toggled',
		});
	}
	const focusTransition = store.transition({
		panelId: target.panelId,
		tabId: target.tabId,
		type: 'panel-focused',
	});
	if (focusTransition.outcome === 'rejected') {
		return { code: 'panel-target-unavailable', kind: 'blocked' };
	}

	if (navigation !== undefined) {
		return navigation.navigate(context, { kind: 'workspace', workspaceId: target.workspaceId });
	}
	const transition = projectWorkspaceRuntime.transition({
		type: 'workspace-activated',
		workspaceId: target.workspaceId,
	});
	if (transition.outcome === 'rejected') {
		return {
			code: transition.rejection ?? 'panel-target-unavailable',
			kind: 'blocked',
		};
	}
	return { kind: 'completed' };
}

/** The Tabs a close of Tabs takes as a whole; null for a close that takes none of them. */
function closedTabIdsOf(closure: WorkbenchAuthorityClosure): readonly string[] | null {
	switch (closure.type) {
		case 'tabs-close':
		case 'tabs-close-if-idle':
			return closure.tabIds;
		case 'tab-close-resume':
			return [closure.tabId];
		default:
			return null;
	}
}

/**
 * Returns focus after a Tab close to the Panel the trail picks before the close runs. The step checks
 * again that the Panel is still open, so a Tab that went away during the close is never selected.
 */
export function createWorkbenchFocusAfterTabClose({
	navigation,
	projectWorkspaceRuntime,
	trail,
}: WorkbenchPanelFocusTargetOptions & {
	readonly trail: WorkbenchPanelFocusTrail;
}): WorkbenchFocusAfterTabClose {
	return {
		prepare: (context, closure) => {
			const closedTabIds = closedTabIdsOf(closure);
			const picked =
				closedTabIds === null ? null : trail.returnTargetAfterTabClose(new Set(closedTabIds));
			if (picked === null) return null;
			return () => {
				const target = trail.reopen(picked);
				if (target === null) return;
				focusWorkbenchPanelTarget(context, target, { navigation, projectWorkspaceRuntime });
			};
		},
	};
}
