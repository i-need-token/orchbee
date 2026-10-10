import {
	defineAction,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import type { AgentSnapshotSource } from '@orchbee/agent';
import { workbenchMessages } from '@orchbee/i18n';

import {
	findPanel,
	type TabPanelWorkspace,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { PanelWorkspaceStore } from '../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import {
	resolveWorkbenchAgentNavigationTarget,
	type WorkbenchAgentNavigationTarget,
} from '../model/project-workbench-agent-overview';
import type {
	WorkbenchProjectWorkspaceRuntime,
	WorkbenchTerminalContext,
} from '../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchNavigationPort,
	WorkbenchHostActionResult,
} from '../types/workbench-host-actions';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchAgentNavigateInput,
	workbenchAgentNavigateInput,
} from './workbench-action-ids';

interface RegisterWorkbenchAgentNavigationActionOptions {
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly source: AgentSnapshotSource | undefined;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

interface PreparedWorkbenchAgentNavigation {
	readonly context: WorkbenchTerminalContext;
	readonly target: WorkbenchAgentNavigationTarget;
}

export function registerWorkbenchAgentNavigationAction({
	navigation,
	projectWorkspaceRuntime,
	source,
	scope,
	typedRuntime,
}: RegisterWorkbenchAgentNavigationActionOptions): ActionDisposable {
	return typedRuntime.register(
		scope,
		defineAction({
			category: {
				id: 'navigation',
				title: workbenchMessages.actionCategoryNavigation,
			},
			execution: {
				concurrency: 'replace-latest',
				concurrencyKey: () => 'agent-navigation',
				mode: 'foreground',
			},
			getAvailability: (_context, input) =>
				prepareWorkbenchAgentNavigation(input, projectWorkspaceRuntime, source) === null
					? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
					: { kind: 'enabled' },
			handler: {
				execute: (context, input) =>
					executeWorkbenchAgentNavigation(context, input, {
						navigation,
						projectWorkspaceRuntime,
						source,
					}),
			},
			id: WORKBENCH_ACTION_IDS.navigateAgent,
			input: workbenchAgentNavigateInput,
			title: workbenchMessages.workspaceAgentNavigate,
		}),
	);
}

function executeWorkbenchAgentNavigation(
	actionContext: ActionExecutionContext,
	input: WorkbenchAgentNavigateInput,
	options: Pick<
		RegisterWorkbenchAgentNavigationActionOptions,
		'navigation' | 'projectWorkspaceRuntime' | 'source'
	>,
): WorkbenchHostActionResult {
	if (actionContext.signal.aborted) return { kind: 'cancelled' };
	const prepared = prepareWorkbenchAgentNavigation(
		input,
		options.projectWorkspaceRuntime,
		options.source,
	);
	if (prepared === null) return { code: 'agent-navigation-target-unavailable', kind: 'blocked' };

	actionContext.markCommitted();
	prepared.context.tabModel.selectTab(prepared.target.tabId);
	const store = prepared.context.panelWorkspaceStore;
	const currentTab = panelTabFor(store, prepared.target.tabId);
	if (
		currentTab?.maximizedPanelId !== null &&
		currentTab?.maximizedPanelId !== undefined &&
		currentTab.maximizedPanelId !== prepared.target.panelId
	) {
		store.transition({
			panelId: currentTab.maximizedPanelId,
			tabId: prepared.target.tabId,
			type: 'panel-maximization-toggled',
		});
	}
	const terminalTransition =
		prepared.target.terminalPosition.kind === 'primary'
			? store.transition({
					panelId: prepared.target.panelId,
					tabId: prepared.target.tabId,
					type: 'primary-terminal-shown',
				})
			: store.transition({
					childPresentationId: prepared.target.presentationId,
					panelId: prepared.target.panelId,
					tabId: prepared.target.tabId,
					type: 'child-terminal-activated',
				});
	if (terminalTransition.outcome === 'rejected') {
		return { code: 'agent-navigation-terminal-unavailable', kind: 'blocked' };
	}
	const focusTransition = store.transition({
		panelId: prepared.target.panelId,
		tabId: prepared.target.tabId,
		type: 'panel-focused',
	});
	if (focusTransition.outcome === 'rejected') {
		return { code: 'agent-navigation-panel-unavailable', kind: 'blocked' };
	}

	if (options.navigation !== undefined) {
		return options.navigation.navigate(actionContext, {
			kind: 'workspace',

			workspaceId: input.workspaceId,
		});
	}
	const transition = options.projectWorkspaceRuntime.transition({
		type: 'workspace-activated',
		workspaceId: input.workspaceId,
	});
	if (transition.outcome === 'rejected') {
		return {
			code: transition.rejection ?? 'agent-navigation-workspace-unavailable',
			kind: 'blocked',
		};
	}
	return { kind: transition.outcome === 'applied' ? 'completed' : 'unchanged' };
}

function prepareWorkbenchAgentNavigation(
	input: WorkbenchAgentNavigateInput,
	runtime: WorkbenchProjectWorkspaceRuntime,
	source: AgentSnapshotSource | undefined,
): PreparedWorkbenchAgentNavigation | null {
	if (source === undefined) return null;
	const target = resolveWorkbenchAgentNavigationTarget(input, source.getSnapshot(), runtime);
	if (target === null) return null;
	const context = runtime.getTerminalContext(target.terminalContextId);
	if (context === null) return null;
	const panelTab = panelTabFor(context.panelWorkspaceStore, target.tabId);
	const panel = panelTab === null ? null : findPanel(panelTab.root, target.panelId);
	if (panel === null || panel === undefined) return null;
	const terminalExists =
		target.terminalPosition.kind === 'primary'
			? panel.primaryPresentation.presentationId === target.presentationId
			: panel.childPresentations.some(
					(presentation) => presentation.presentationId === target.presentationId,
				);
	return terminalExists ? { context, target } : null;
}

function panelTabFor(store: PanelWorkspaceStore, tabId: string): TabPanelWorkspace | null {
	return store.getSnapshot().tabs.find((tab) => tab.tabId === tabId) ?? null;
}
