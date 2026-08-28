import {
	defineAction,
	noActionInput,
	type ActionAvailabilityContext,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionResult,
	type ActionRuntime,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import {
	resolveActiveTerminalCommand,
	type ActiveTerminalCommand,
	type ActiveTerminalCommandResolution,
} from '../model/resolveActiveTerminalCommand';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import { executePreparedDescendantAction } from './executePreparedDescendantAction';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from './workbench-action-ids';

interface RegisterWorkbenchActiveTerminalActionsOptions {
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

export function registerWorkbenchActiveTerminalActions({
	projectWorkspaceRuntime,
	runtime,
	scope,
	typedRuntime,
}: RegisterWorkbenchActiveTerminalActionsOptions): readonly ActionDisposable[] {
	const terminalCategory = {
		id: 'terminal',
		title: workbenchMessages.actionCategoryTerminal,
	};
	const panelCategory = {
		id: 'panels',
		title: workbenchMessages.actionCategoryPanels,
	};
	const availabilityFor =
		(command: ActiveTerminalCommand) => (context: ActionAvailabilityContext) =>
			resolveForContext(projectWorkspaceRuntime, command, context).kind === 'ready'
				? ({ kind: 'enabled' } as const)
				: ({ kind: 'disabled', reason: workbenchMessages.actionUnavailable } as const);
	const execute = (command: ActiveTerminalCommand) => async (context: ActionExecutionContext) =>
		await executeResolvedCommand({
			command,
			context,
			projectWorkspaceRuntime,
			runtime,
			scope,
		});

	return [
		typedRuntime.register(
			scope,
			defineAction({
				category: terminalCategory,
				execution: {
					concurrency: 'serialize',
					concurrencyKey: () => 'active-terminal-close',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('close'),
				handler: { execute: execute('close') },
				id: WORKBENCH_ACTION_IDS.closeActiveTerminal,
				input: noActionInput,
				title: workbenchMessages.terminalCloseActive,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-resize',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('start-resize'),
				handler: { execute: execute('start-resize') },
				id: WORKBENCH_ACTION_IDS.resizeActivePanel,
				input: noActionInput,
				title: workbenchMessages.panelResize,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-focus-down',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('focus-down'),
				handler: { execute: execute('focus-down') },
				id: WORKBENCH_ACTION_IDS.focusActivePanelDown,
				input: noActionInput,
				title: workbenchMessages.panelFocusDown,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-focus-left',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('focus-left'),
				handler: { execute: execute('focus-left') },
				id: WORKBENCH_ACTION_IDS.focusActivePanelLeft,
				input: noActionInput,
				title: workbenchMessages.panelFocusLeft,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-focus-right',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('focus-right'),
				handler: { execute: execute('focus-right') },
				id: WORKBENCH_ACTION_IDS.focusActivePanelRight,
				input: noActionInput,
				title: workbenchMessages.panelFocusRight,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-focus-up',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('focus-up'),
				handler: { execute: execute('focus-up') },
				id: WORKBENCH_ACTION_IDS.focusActivePanelUp,
				input: noActionInput,
				title: workbenchMessages.panelFocusUp,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-maximization',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('toggle-maximization'),
				handler: { execute: execute('toggle-maximization') },
				id: WORKBENCH_ACTION_IDS.toggleActivePanelMaximization,
				input: noActionInput,
				title: workbenchMessages.panelToggleMaximization,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-split-below',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('split-below'),
				handler: { execute: execute('split-below') },
				id: WORKBENCH_ACTION_IDS.splitActivePanelBelow,
				input: noActionInput,
				title: workbenchMessages.panelSplitBelow,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-split-right',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('split-right'),
				handler: { execute: execute('split-right') },
				id: WORKBENCH_ACTION_IDS.splitActivePanelRight,
				input: noActionInput,
				title: workbenchMessages.panelSplitRight,
			}),
		),
	];
}

function resolveForContext(
	projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime,
	command: ActiveTerminalCommand,
	context: ActionAvailabilityContext,
): ActiveTerminalCommandResolution {
	if (context.target !== undefined && context.target.type !== 'panel') {
		return { kind: 'unavailable', reason: 'panel-unavailable' };
	}
	const terminalContext = projectWorkspaceRuntime.getActiveTerminalContext();
	return resolveActiveTerminalCommand({
		activeTabId: terminalContext.panelWorkspaceStore.getActiveTabId(),
		command,
		state: terminalContext.panelWorkspaceStore.getSnapshot(),
		targetPanelId: context.target?.id ?? null,
	});
}

async function executeResolvedCommand({
	command,
	context,
	projectWorkspaceRuntime,
	runtime,
	scope,
}: {
	readonly command: ActiveTerminalCommand;
	readonly context: ActionExecutionContext;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
}): Promise<ActionResult<void>> {
	const resolution = resolveForContext(projectWorkspaceRuntime, command, context);
	if (resolution.kind === 'unavailable') {
		return { code: resolution.reason, kind: 'blocked' };
	}
	const prepared = runtime
		.discover(
			{
				scope,
				source: context.source,
				target: { id: resolution.panelId, type: 'panel' },
			},
			{
				includeDescendants: true,
				inputByActionId: { [resolution.actionId]: resolution.input },
			},
		)
		.find(
			(result) =>
				result.kind === 'ready' &&
				result.descriptor.id === resolution.actionId &&
				result.availability.kind === 'enabled',
		);
	if (prepared?.kind !== 'ready') {
		return { code: 'active-command-target-unavailable', kind: 'blocked' };
	}

	return await executePreparedDescendantAction(context, runtime, prepared.invocation);
}
