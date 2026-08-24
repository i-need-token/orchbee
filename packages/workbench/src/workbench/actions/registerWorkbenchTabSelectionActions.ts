import {
	defineAction,
	type ActionAvailabilityContext,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionInvocation,
	type ActionRuntime,
	type ActionScope,
	type PreparedActionInvocation,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { TAB_STRIP_ACTION_IDS, type TabSelectionInput } from '@orchbee/tab-strip';
import { workbenchMessages, workbenchTabMessages } from '@orchbee/i18n';

import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import { executePreparedDescendantAction } from './executePreparedDescendantAction';
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
					prepareTabSelection({
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
						const prepared = prepareTabSelection({
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

function prepareTabSelection({
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
	const selectionInput: TabSelectionInput = { mode: 'replace', tabId: tab.id };
	const prepared = runtime
		.discover(discoveryInvocation(context, scope), {
			includeDescendants: true,
			inputByActionId: { [TAB_STRIP_ACTION_IDS.selectTab]: selectionInput },
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
