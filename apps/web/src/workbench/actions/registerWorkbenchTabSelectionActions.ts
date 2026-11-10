import {
	defineAction,
	noActionInput,
	type ActionAvailability,
	type ActionAvailabilityContext,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionMessageDescriptor,
	type ActionRuntime,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages, workbenchTabMessages } from '@orchbee/i18n';
import { TAB_STRIP_ACTION_IDS, type TabSelectionInput } from '@orchbee/tab-strip';

import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import { executePreparedDescendantAction } from './executePreparedDescendantAction';
import {
	prepareDescendantAction,
	type DescendantActionPreparation,
} from './prepareDescendantAction';
import {
	resolveAdjacentTabSelection,
	type AdjacentTabSelectionInput,
} from './resolveAdjacentTabSelection';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchTabPositionInput,
	workbenchTabPositionInput,
} from './workbench-action-ids';

interface RegisterWorkbenchTabSelectionActionsOptions {
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

export function registerWorkbenchTabSelectionActions({
	projectWorkspaceRuntime,
	runtime,
	scope,
	typedRuntime,
}: RegisterWorkbenchTabSelectionActionsOptions): readonly ActionDisposable[] {
	return [
		typedRuntime.register(
			scope,
			adjacentTabSelectionActionDefinition({
				direction: 'previous',
				id: WORKBENCH_ACTION_IDS.selectPreviousTab,
				projectWorkspaceRuntime,
				runtime,
				scope,
				title: workbenchTabMessages.selectPreviousTab,
			}),
		),
		typedRuntime.register(
			scope,
			adjacentTabSelectionActionDefinition({
				direction: 'next',
				id: WORKBENCH_ACTION_IDS.selectNextTab,
				projectWorkspaceRuntime,
				runtime,
				scope,
				title: workbenchTabMessages.selectNextTab,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: {
					id: 'navigation',
					title: workbenchMessages.actionCategoryNavigation,
				},
				execution: {
					concurrency: 'replace-latest',
					concurrencyKey: () => 'tab-selection-by-position',
					mode: 'foreground',
				},
				getAvailability: (context, input) =>
					availabilityOf(
						prepareTabPositionSelection({
							context,
							input,
							projectWorkspaceRuntime,
							runtime,
							scope,
						}),
					),
				handler: {
					execute: async (context, input) => {
						const prepared = prepareTabPositionSelection({
							context,
							input,
							projectWorkspaceRuntime,
							runtime,
							scope,
						});
						return prepared.kind === 'ready'
							? await executePreparedDescendantAction(context, runtime, prepared.invocation)
							: { code: 'tab-position-unavailable', kind: 'blocked' };
					},
				},
				id: WORKBENCH_ACTION_IDS.selectTabByPosition,
				input: workbenchTabPositionInput,
				title: workbenchTabMessages.selectTab,
			}),
		),
	];
}

function adjacentTabSelectionActionDefinition<
	Id extends
		| typeof WORKBENCH_ACTION_IDS.selectNextTab
		| typeof WORKBENCH_ACTION_IDS.selectPreviousTab,
>({
	direction,
	id,
	projectWorkspaceRuntime,
	runtime,
	scope,
	title,
}: Omit<RegisterWorkbenchTabSelectionActionsOptions, 'typedRuntime'> & {
	readonly direction: AdjacentTabSelectionInput['direction'];
	readonly id: Id;
	readonly title: ActionMessageDescriptor;
}) {
	return defineAction({
		category: {
			id: 'navigation',
			title: workbenchMessages.actionCategoryNavigation,
		},
		execution: {
			concurrency: 'replace-latest',
			concurrencyKey: () => 'adjacent-tab-selection',
			mode: 'foreground',
		},
		getAvailability: (context) =>
			availabilityOf(
				prepareAdjacentTabSelection({
					context,
					direction,
					projectWorkspaceRuntime,
					runtime,
					scope,
				}),
			),
		handler: {
			execute: async (context) => {
				const prepared = prepareAdjacentTabSelection({
					context,
					direction,
					projectWorkspaceRuntime,
					runtime,
					scope,
				});
				return prepared.kind === 'ready'
					? await executePreparedDescendantAction(context, runtime, prepared.invocation)
					: { code: 'adjacent-tab-unavailable', kind: 'blocked' };
			},
		},
		id,
		input: noActionInput,
		title,
	});
}

function prepareAdjacentTabSelection({
	context,
	direction,
	projectWorkspaceRuntime,
	runtime,
	scope,
}: {
	readonly context: ActionAvailabilityContext | ActionExecutionContext;
	readonly direction: AdjacentTabSelectionInput['direction'];
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
}): DescendantActionPreparation {
	const snapshot = projectWorkspaceRuntime.getActiveTerminalContext()?.tabModel.getSnapshot();
	if (snapshot === undefined) return WORKSPACE_REQUIRED;
	const tabId = resolveAdjacentTabSelection({
		activeTabId: snapshot.activeTabId,
		direction,
		tabIds: snapshot.tabs.map((tab) => tab.id),
	});
	if (tabId !== null) return prepareTabIdSelection({ context, runtime, scope, tabId });
	return snapshot.tabs.some((tab) => tab.id === snapshot.activeTabId)
		? { kind: 'unavailable', reason: workbenchTabMessages.unavailableOnlyOne }
		: { kind: 'unavailable', reason: workbenchTabMessages.unavailableNoneSelected };
}

function prepareTabPositionSelection({
	context,
	input,
	projectWorkspaceRuntime,
	runtime,
	scope,
}: {
	readonly context: ActionAvailabilityContext | ActionExecutionContext;
	readonly input: WorkbenchTabPositionInput;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
}): DescendantActionPreparation {
	const tabModel = projectWorkspaceRuntime.getActiveTerminalContext()?.tabModel;
	if (tabModel === undefined) return WORKSPACE_REQUIRED;
	const tab = tabModel.getSnapshot().tabs[input.position - 1];
	if (tab === undefined) {
		return { kind: 'unavailable', reason: workbenchTabMessages.unavailablePositionEmpty };
	}
	return prepareTabIdSelection({ context, runtime, scope, tabId: tab.id });
}

function prepareTabIdSelection({
	context,
	runtime,
	scope,
	tabId,
}: {
	readonly context: ActionAvailabilityContext | ActionExecutionContext;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
	readonly tabId: string;
}): DescendantActionPreparation {
	const input: TabSelectionInput = { mode: 'replace', tabId };
	return prepareDescendantAction({
		actionId: TAB_STRIP_ACTION_IDS.selectTab,
		context,
		input,
		runtime,
		scope,
	});
}

const WORKSPACE_REQUIRED = {
	kind: 'unavailable',
	reason: workbenchMessages.workspaceUnavailableRequired,
} as const satisfies DescendantActionPreparation;

function availabilityOf(prepared: DescendantActionPreparation): ActionAvailability {
	switch (prepared.kind) {
		case 'ready':
			return { kind: 'enabled' };
		case 'unavailable':
			return { kind: 'disabled', reason: prepared.reason };
		case 'hidden':
			return prepared;
	}
}
