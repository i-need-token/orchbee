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
import { TAB_STRIP_ACTION_IDS } from '@orchbee/tab-strip';

import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import { executePreparedDescendantAction } from './executePreparedDescendantAction';
import { resolveActiveTabMovement, type ActiveTabMovementInput } from './resolveActiveTabMovement';
import { WORKBENCH_DESCENDANT_ACTION_DISCOVERY_INPUTS } from './workbench-descendant-action-discovery';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from './workbench-action-ids';

interface RegisterWorkbenchActiveTabMovementActionsOptions {
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

export function registerWorkbenchActiveTabMovementActions({
	projectWorkspaceRuntime,
	runtime,
	scope,
	typedRuntime,
}: RegisterWorkbenchActiveTabMovementActionsOptions): readonly ActionDisposable[] {
	return [
		typedRuntime.register(
			scope,
			activeTabMovementActionDefinition({
				direction: 'backward',
				id: WORKBENCH_ACTION_IDS.moveActiveTabBackward,
				projectWorkspaceRuntime,
				runtime,
				scope,
				title: workbenchTabMessages.moveActiveTabBackward,
			}),
		),
		typedRuntime.register(
			scope,
			activeTabMovementActionDefinition({
				direction: 'forward',
				id: WORKBENCH_ACTION_IDS.moveActiveTabForward,
				projectWorkspaceRuntime,
				runtime,
				scope,
				title: workbenchTabMessages.moveActiveTabForward,
			}),
		),
	];
}

function activeTabMovementActionDefinition<
	Id extends
		| typeof WORKBENCH_ACTION_IDS.moveActiveTabBackward
		| typeof WORKBENCH_ACTION_IDS.moveActiveTabForward,
>({
	direction,
	id,
	projectWorkspaceRuntime,
	runtime,
	scope,
	title,
}: Omit<RegisterWorkbenchActiveTabMovementActionsOptions, 'typedRuntime'> & {
	readonly direction: ActiveTabMovementInput['direction'];
	readonly id: Id;
	readonly title: ActionMessageDescriptor;
}) {
	return defineAction({
		category: {
			id: 'tabs',
			title: workbenchTabMessages.actionCategoryTabs,
		},
		execution: {
			concurrency: 'parallel',
			concurrencyKey: () => 'active-tab-movement',
			mode: 'foreground',
		},
		getAvailability: (context) =>
			prepareActiveTabMovement({
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
				const prepared = prepareActiveTabMovement({
					context,
					direction,
					projectWorkspaceRuntime,
					runtime,
					scope,
				});
				return prepared === null
					? { code: 'active-tab-movement-unavailable', kind: 'blocked' }
					: await executePreparedDescendantAction(context, runtime, prepared);
			},
		},
		id,
		input: noActionInput,
		title,
	});
}

function prepareActiveTabMovement({
	context,
	direction,
	projectWorkspaceRuntime,
	runtime,
	scope,
}: {
	readonly context: ActionAvailabilityContext | ActionExecutionContext;
	readonly direction: ActiveTabMovementInput['direction'];
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
}): PreparedActionInvocation | null {
	const snapshot = projectWorkspaceRuntime.getActiveTerminalContext().tabModel.getSnapshot();
	const moveInput = resolveActiveTabMovement({
		activeTabId: snapshot.activeTabId,
		direction,
		tabs: snapshot.tabs.map((tab) => ({ id: tab.id, pinned: tab.pinned })),
	});
	if (moveInput === null) return null;
	const prepared = runtime
		.discover(discoveryInvocation(context, scope), {
			includeDescendants: true,
			inputByActionId: {
				...WORKBENCH_DESCENDANT_ACTION_DISCOVERY_INPUTS,
				[TAB_STRIP_ACTION_IDS.moveTab]: moveInput,
			},
		})
		.find(
			(result) =>
				result.kind === 'ready' &&
				result.descriptor.id === TAB_STRIP_ACTION_IDS.moveTab &&
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
