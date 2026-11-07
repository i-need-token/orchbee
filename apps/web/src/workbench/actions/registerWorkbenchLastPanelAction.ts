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
import { focusWorkbenchPanelTarget } from './workbench-tab-close-focus';

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
	return focusWorkbenchPanelTarget(actionContext, target, options);
}
