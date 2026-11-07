import {
	defineAction,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchEarlierLayouts } from '../types/workbench-authority';
import type {
	WorkbenchHostActionResult,
	WorkbenchNavigationPort,
} from '../types/workbench-host-actions';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchEarlierLayoutInput,
	workbenchEarlierLayoutInput,
} from './workbench-action-ids';
import { focusWorkbenchPanelTarget } from './workbench-tab-close-focus';

interface RegisterWorkbenchEarlierLayoutActionOptions {
	readonly earlierLayouts: WorkbenchEarlierLayouts;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

/**
 * Earlier layouts: Search lists the layouts the Server kept and this reopens the chosen one, as the
 * TUI's picker does. The Server reopens only the Workspaces and Tabs that are not open now; this
 * window then selects the first reopened Tab. The reopen is sent once and never repeated.
 */
export function registerWorkbenchEarlierLayoutAction({
	earlierLayouts,
	navigation,
	projectWorkspaceRuntime,
	scope,
	typedRuntime,
}: RegisterWorkbenchEarlierLayoutActionOptions): ActionDisposable {
	return typedRuntime.register(
		scope,
		defineAction({
			category: { id: 'projects', title: workbenchMessages.actionCategoryProjects },
			execution: {
				concurrency: 'reject-duplicate',
				concurrencyKey: () => 'earlier-layout',
				mode: 'foreground',
			},
			// The Server answers whether anything is kept or missing; Search's picker reads it.
			getAvailability: () => ({ kind: 'enabled' }),
			// Search lists an Action that still needs its layout only when it declares this.
			getDiscoveryAvailability: () => ({ kind: 'enabled' }),
			handler: {
				execute: (context, input) =>
					reopenEarlierLayout(context, input, {
						earlierLayouts,
						navigation,
						projectWorkspaceRuntime,
					}),
			},
			id: WORKBENCH_ACTION_IDS.reopenEarlierLayout,
			input: workbenchEarlierLayoutInput,
			title: workbenchMessages.earlierLayouts,
		}),
	);
}

async function reopenEarlierLayout(
	context: ActionExecutionContext,
	input: WorkbenchEarlierLayoutInput,
	options: Pick<
		RegisterWorkbenchEarlierLayoutActionOptions,
		'earlierLayouts' | 'navigation' | 'projectWorkspaceRuntime'
	>,
): Promise<Awaited<WorkbenchHostActionResult>> {
	if (context.signal.aborted) return { kind: 'cancelled' };
	context.markCommitted();
	const outcome = await options.earlierLayouts.reopen(input.savedAt);
	if (outcome.kind !== 'reopened') {
		return { failure: { code: `earlier-layout-${outcome.kind}` }, kind: 'failed' };
	}
	if (outcome.focus === null) return { kind: 'completed' };
	return focusReopenedTab(context, outcome.focus, options);
}

/** Selects the first reopened Tab and its Workspace; a Tab that is gone by then selects nothing. */
function focusReopenedTab(
	context: ActionExecutionContext,
	focus: { readonly tabId: string; readonly terminalContextId: string },
	{
		navigation,
		projectWorkspaceRuntime,
	}: Pick<RegisterWorkbenchEarlierLayoutActionOptions, 'navigation' | 'projectWorkspaceRuntime'>,
): WorkbenchHostActionResult {
	const workspace = projectWorkspaceRuntime
		.getSnapshot()
		.workspaces.find(({ terminalContextId }) => terminalContextId === focus.terminalContextId);
	const terminalContext = projectWorkspaceRuntime.getTerminalContext(focus.terminalContextId);
	const tab = terminalContext?.panelWorkspaceStore
		.getSnapshot()
		.tabs.find((candidate) => candidate.tabId === focus.tabId);
	if (workspace === undefined || terminalContext === null || tab === undefined) {
		return { kind: 'completed' };
	}
	return focusWorkbenchPanelTarget(
		context,
		{
			context: terminalContext,
			panelId: tab.focusedPanelId,
			tabId: tab.tabId,
			workspaceId: workspace.id,
		},
		{ navigation, projectWorkspaceRuntime },
	);
}
