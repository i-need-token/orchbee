import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import type { WorkbenchPanelFocusTrail } from '../model/WorkbenchPanelFocusTrail';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchHostActionResult,
	WorkbenchNavigationPort,
} from '../types/workbench-host-actions';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from './workbench-action-ids';

interface RegisterWorkbenchLastPanelActionOptions {
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly scope: ActionScope;
	readonly trail: WorkbenchPanelFocusTrail;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

/**
 * Last Panel: focuses the most recent other Panel this window focused that still exists, selecting
 * its Tab and Workspace as Agent navigation does. Without one it is unavailable, so its key falls
 * through.
 */
export function registerWorkbenchLastPanelAction({
	navigation,
	projectWorkspaceRuntime,
	scope,
	trail,
	typedRuntime,
}: RegisterWorkbenchLastPanelActionOptions): ActionDisposable {
	return typedRuntime.register(
		scope,
		defineAction({
			category: {
				id: 'navigation',
				title: workbenchMessages.actionCategoryNavigation,
			},
			execution: {
				concurrency: 'replace-latest',
				concurrencyKey: () => 'last-panel',
				mode: 'foreground',
			},
			getAvailability: () =>
				trail.target() === null
					? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
					: { kind: 'enabled' },
			handler: {
				execute: (context) =>
					executeWorkbenchLastPanel(context, { navigation, projectWorkspaceRuntime, trail }),
			},
			id: WORKBENCH_ACTION_IDS.focusLastPanel,
			input: noActionInput,
			title: workbenchMessages.panelFocusLast,
		}),
	);
}

function executeWorkbenchLastPanel(
	actionContext: ActionExecutionContext,
	options: Pick<
		RegisterWorkbenchLastPanelActionOptions,
		'navigation' | 'projectWorkspaceRuntime' | 'trail'
	>,
): WorkbenchHostActionResult {
	if (actionContext.signal.aborted) return { kind: 'cancelled' };
	const target = options.trail.target();
	if (target === null) return { code: 'last-panel-target-unavailable', kind: 'blocked' };

	actionContext.markCommitted();
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
		return { code: 'last-panel-target-unavailable', kind: 'blocked' };
	}

	if (options.navigation !== undefined) {
		return options.navigation.navigate(actionContext, {
			kind: 'workspace',
			workspaceId: target.workspaceId,
		});
	}
	const transition = options.projectWorkspaceRuntime.transition({
		type: 'workspace-activated',
		workspaceId: target.workspaceId,
	});
	if (transition.outcome === 'rejected') {
		return {
			code: transition.rejection ?? 'last-panel-target-unavailable',
			kind: 'blocked',
		};
	}
	return { kind: 'completed' };
}
