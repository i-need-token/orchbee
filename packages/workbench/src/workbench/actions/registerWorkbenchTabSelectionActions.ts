import {
	defineAction,
	noActionInput,
	type ActionAvailabilityContext,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionInvocation,
	type ActionMessageDescriptor,
	type ActionRuntime,
	type ActionScope,
	type PreparedActionInvocation,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages, workbenchTabMessages } from '@orchbee/i18n';
import { TAB_STRIP_ACTION_IDS, type TabSelectionInput } from '@orchbee/tab-strip';

import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import { executePreparedDescendantAction } from './executePreparedDescendantAction';
import {
	resolveAdjacentTabSelection,
	type AdjacentTabSelectionInput,
} from './resolveAdjacentTabSelection';
import { WORKBENCH_DESCENDANT_ACTION_DISCOVERY_INPUTS } from './workbench-descendant-action-discovery';
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
					prepareTabPositionSelection({
						context,
						input,
						projectWorkspaceRuntime,
						runtime,
						scope,
					}) === null
						? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
						: { kind: 'enabled' },
				handler: {
					execute: async (context, input) => {
						const prepared = prepareTabPositionSelection({
							context,
							input,
							projectWorkspaceRuntime,
							runtime,
							scope,
						});
						return prepared === null
							? { code: 'tab-position-unavailable', kind: 'blocked' }
							: await executePreparedDescendantAction(context, runtime, prepared);
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
			prepareAdjacentTabSelection({
				context,
				direction,
				projectWorkspaceRuntime,
				runtime,
				scope,
			}) === null
				? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
				: { kind: 'enabled' },
		handler: {
			execute: async (context) => {
				const prepared = prepareAdjacentTabSelection({
					context,
					direction,
					projectWorkspaceRuntime,
					runtime,
					scope,
				});
				return prepared === null
					? { code: 'adjacent-tab-unavailable', kind: 'blocked' }
					: await executePreparedDescendantAction(context, runtime, prepared);
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
}): PreparedActionInvocation | null {
	const snapshot = projectWorkspaceRuntime.getActiveTerminalContext().tabModel.getSnapshot();
	const tabId = resolveAdjacentTabSelection({
		activeTabId: snapshot.activeTabId,
		direction,
		tabIds: snapshot.tabs.map((tab) => tab.id),
	});
	return tabId === null ? null : prepareTabIdSelection({ context, runtime, scope, tabId });
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
}): PreparedActionInvocation | null {
	const tab = projectWorkspaceRuntime.getActiveTerminalContext().tabModel.getSnapshot().tabs[
		input.position - 1
	];
	if (tab === undefined) return null;
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
}): PreparedActionInvocation | null {
	const selectionInput: TabSelectionInput = { mode: 'replace', tabId };
	const prepared = runtime
		.discover(discoveryInvocation(context, scope), {
			includeDescendants: true,
			inputByActionId: {
				...WORKBENCH_DESCENDANT_ACTION_DISCOVERY_INPUTS,
				[TAB_STRIP_ACTION_IDS.selectTab]: selectionInput,
			},
		})
		.find(
			(result) =>
				result.kind === 'ready' &&
				result.descriptor.id === TAB_STRIP_ACTION_IDS.selectTab &&
				result.availability.kind === 'enabled',
		);
	return prepared?.kind === 'ready' ? prepared.invocation : null;
}

function discoveryInvocation(
	context: ActionAvailabilityContext | ActionExecutionContext,
	scope: ActionScope,
): ActionInvocation {
	return {
		...(context.capabilities === undefined ? {} : { capabilities: context.capabilities }),
		...(context.focus === undefined ? {} : { focus: context.focus }),
		scope,
		source: context.source,
		...(context.target === undefined ? {} : { target: context.target }),
	};
}
