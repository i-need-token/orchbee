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

	const execution = runtime.executePrepared(prepared.invocation);
	const cancelLowerExecution = () => execution.cancel();
	context.signal.addEventListener('abort', cancelLowerExecution, { once: true });
	if (context.signal.aborted) execution.cancel();
	try {
		const result = await execution.completion;
		if (result.kind === 'completed') context.markCommitted();
		return withoutValue(result);
	} finally {
		context.signal.removeEventListener('abort', cancelLowerExecution);
	}
}

function withoutValue(result: ActionResult): ActionResult<void> {
	switch (result.kind) {
		case 'blocked':
			return {
				code: result.code,
				...(result.details === undefined ? {} : { details: result.details }),
				kind: 'blocked',
			};
		case 'cancelled':
			return { kind: 'cancelled' };
		case 'completed':
			return { kind: 'completed' };
		case 'failed':
			return { failure: result.failure, kind: 'failed' };
		case 'unchanged':
			return { kind: 'unchanged' };
		case 'unsupported':
			return { kind: 'unsupported' };
	}
}
