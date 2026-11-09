import {
	defineAction,
	noActionInput,
	type ActionAvailabilityContext,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionMessageDescriptor,
	type ActionResult,
	type ActionRuntime,
	type ActionScope,
	type PreparedActionInvocation,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import { PANEL_ACTION_IDS } from '../components/workbench-panel-workspace/actions/panel-action-contracts';
import {
	resolvePanelFocusByPosition,
	type PanelFocusByPositionResolution,
} from '../components/workbench-panel-workspace/model/resolvePanelFocusByPosition';
import {
	resolveActiveTerminalCommand,
	type ActiveTerminalCommand,
	type ActiveTerminalCommandResolution,
} from '../model/resolveActiveTerminalCommand';
import type { PanelFocusDirection } from '../components/workbench-panel-workspace/model/resolveDirectionalPanelFocus';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAuthority } from '../types/workbench-authority';
import { executePreparedDescendantAction } from './executePreparedDescendantAction';
import { WORKBENCH_DESCENDANT_ACTION_DISCOVERY_INPUTS } from './workbench-descendant-action-discovery';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchPanelPositionInput,
	workbenchPanelPositionInput,
} from './workbench-action-ids';

interface RegisterWorkbenchActiveTerminalActionsOptions {
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
	/** Absent where no close stops where it may continue. */
	readonly stoppedTabClose?: WorkbenchAuthority['stoppedTabClose'] | undefined;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

export function registerWorkbenchActiveTerminalActions({
	projectWorkspaceRuntime,
	runtime,
	scope,
	stoppedTabClose,
	typedRuntime,
}: RegisterWorkbenchActiveTerminalActionsOptions): readonly ActionDisposable[] {
	/** The TUI's Close key retries a close that stopped on the active Tab before it closes anything. */
	const activeTabCloseStopped = (): boolean => {
		const terminalContext = projectWorkspaceRuntime.getActiveTerminalContext();
		if (stoppedTabClose === undefined || terminalContext === null) return false;
		const activeTabId = terminalContext.panelWorkspaceStore.getActiveTabId();
		return (
			activeTabId !== null &&
			stoppedTabClose(terminalContext.terminalContextId, activeTabId) !== null
		);
	};
	const terminalCategory = {
		id: 'terminal',
		title: workbenchMessages.actionCategoryTerminal,
	};
	const panelCategory = {
		id: 'panels',
		title: workbenchMessages.actionCategoryPanels,
	};
	const availabilityFor =
		(command: ActiveTerminalCommand) => (context: ActionAvailabilityContext) => {
			const resolution = resolveForContext(projectWorkspaceRuntime, command, context);
			return resolution.kind === 'ready'
				? ({ kind: 'enabled' } as const)
				: ({ kind: 'disabled', reason: activeTerminalUnavailableReason(resolution) } as const);
		};
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
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-focus-by-position',
					mode: 'foreground',
				},
				getAvailability: (context, input) => {
					const prepared = preparePanelPositionFocus({
						context,
						input,
						projectWorkspaceRuntime,
						runtime,
						scope,
					});
					return prepared.kind === 'ready'
						? { kind: 'enabled' }
						: { kind: 'disabled', reason: panelPositionUnavailableReason(prepared.reason) };
				},
				handler: {
					execute: async (context, input) => {
						const prepared = preparePanelPositionFocus({
							context,
							input,
							projectWorkspaceRuntime,
							runtime,
							scope,
						});
						return prepared.kind === 'unavailable'
							? { code: prepared.reason, kind: 'blocked' }
							: await executePreparedDescendantAction(context, runtime, prepared.invocation);
					},
				},
				id: WORKBENCH_ACTION_IDS.focusActivePanelByPosition,
				input: workbenchPanelPositionInput,
				title: workbenchMessages.panelFocusByPosition,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: terminalCategory,
				// The lower close serializes on its own Panel or Tab, so the Close key closes another
				// Panel at once while an earlier close is followed, as in the TUI.
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-terminal-close',
					mode: 'foreground',
				},
				getAvailability: (context) =>
					activeTabCloseStopped() ? { kind: 'enabled' } : availabilityFor('close')(context),
				handler: {
					execute: async (context) => {
						if (!activeTabCloseStopped()) return await execute('close')(context);
						// Continuing a stopped close is its own question, so it runs as its own Action.
						typedRuntime.execute(WORKBENCH_ACTION_IDS.resumeTabClose, undefined, {
							scope,
							source: context.source,
						});
						return { kind: 'unchanged' };
					},
				},
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
					concurrencyKey: () => 'active-panel-focus-visible',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('focus-visible'),
				handler: { execute: execute('focus-visible') },
				id: WORKBENCH_ACTION_IDS.focusActivePanelVisible,
				input: noActionInput,
				title: workbenchMessages.panelFocusVisible,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-focus-next',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('focus-next'),
				handler: { execute: execute('focus-next') },
				id: WORKBENCH_ACTION_IDS.focusActivePanelNext,
				input: noActionInput,
				title: workbenchMessages.panelFocusNext,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-swap-visible',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('swap-visible'),
				handler: { execute: execute('swap-visible') },
				id: WORKBENCH_ACTION_IDS.swapActivePanelVisible,
				input: noActionInput,
				title: workbenchMessages.panelSwapVisible,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-focus-previous',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('focus-previous'),
				handler: { execute: execute('focus-previous') },
				id: WORKBENCH_ACTION_IDS.focusActivePanelPrevious,
				input: noActionInput,
				title: workbenchMessages.panelFocusPrevious,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-equalize',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('equalize'),
				handler: { execute: execute('equalize') },
				id: WORKBENCH_ACTION_IDS.equalizeActivePanels,
				input: noActionInput,
				title: workbenchMessages.panelEqualize,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-move-down',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('move-down'),
				handler: { execute: execute('move-down') },
				id: WORKBENCH_ACTION_IDS.moveActivePanelDown,
				input: noActionInput,
				title: workbenchMessages.panelMoveDown,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-move-left',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('move-left'),
				handler: { execute: execute('move-left') },
				id: WORKBENCH_ACTION_IDS.moveActivePanelLeft,
				input: noActionInput,
				title: workbenchMessages.panelMoveLeft,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-move-right',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('move-right'),
				handler: { execute: execute('move-right') },
				id: WORKBENCH_ACTION_IDS.moveActivePanelRight,
				input: noActionInput,
				title: workbenchMessages.panelMoveRight,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-move-up',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('move-up'),
				handler: { execute: execute('move-up') },
				id: WORKBENCH_ACTION_IDS.moveActivePanelUp,
				input: noActionInput,
				title: workbenchMessages.panelMoveUp,
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
		// Search holds the directions the panel menu leaves out, as the TUI's global menu does.
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-split-above',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('split-above'),
				handler: { execute: execute('split-above') },
				id: WORKBENCH_ACTION_IDS.splitActivePanelAbove,
				input: noActionInput,
				title: workbenchMessages.panelSplitAbove,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: panelCategory,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'active-panel-split-left',
					mode: 'foreground',
				},
				getAvailability: availabilityFor('split-left'),
				handler: { execute: execute('split-left') },
				id: WORKBENCH_ACTION_IDS.splitActivePanelLeft,
				input: noActionInput,
				title: workbenchMessages.panelSplitLeft,
			}),
		),
	];
}

const NO_PANEL_MESSAGE_BY_DIRECTION = {
	down: workbenchMessages.panelUnavailableNoneBelow,
	left: workbenchMessages.panelUnavailableNoneLeft,
	right: workbenchMessages.panelUnavailableNoneRight,
	up: workbenchMessages.panelUnavailableNoneAbove,
} as const satisfies Readonly<Record<PanelFocusDirection, ActionMessageDescriptor>>;

/** Names what in the active Tab stops the command, so a disabled row says why. */
function activeTerminalUnavailableReason(
	resolution: Extract<ActiveTerminalCommandResolution, { kind: 'unavailable' }>,
): ActionMessageDescriptor {
	switch (resolution.reason) {
		case 'active-tab-unavailable':
		case 'panel-unavailable':
			return workbenchMessages.panelUnavailableFocusedPanelRequired;
		case 'directional-neighbor-unavailable':
			return NO_PANEL_MESSAGE_BY_DIRECTION[resolution.direction];
		case 'panel-maximized':
			return workbenchMessages.panelUnavailableMaximized;
		case 'requires-multiple-panels':
			return workbenchMessages.panelUnavailableMultipleRequired;
	}
}

function panelPositionUnavailableReason(
	reason: Extract<PreparedPanelPositionFocus, { kind: 'unavailable' }>['reason'],
): ActionMessageDescriptor {
	switch (reason) {
		// Only a Panel surface that registers no focus action leaves the host without it.
		case 'active-command-target-unavailable':
			return workbenchMessages.actionUnavailable;
		case 'active-tab-unavailable':
		case 'invalid-panel-order':
		case 'panel-unavailable':
		case 'source-panel-unavailable':
			return workbenchMessages.panelUnavailableFocusedPanelRequired;
		case 'panel-maximized':
			return workbenchMessages.panelUnavailableMaximized;
		case 'panel-position-unavailable':
			return workbenchMessages.panelUnavailablePositionEmpty;
	}
}

type PreparedPanelPositionFocus =
	| {
			readonly invocation: PreparedActionInvocation;
			readonly kind: 'ready';
	  }
	| {
			readonly kind: 'unavailable';
			readonly reason:
				| Extract<PanelFocusByPositionResolution, { kind: 'unavailable' }>['reason']
				| 'active-command-target-unavailable'
				| 'active-tab-unavailable'
				| 'panel-unavailable';
	  };

function preparePanelPositionFocus({
	context,
	input,
	projectWorkspaceRuntime,
	runtime,
	scope,
}: {
	readonly context: ActionAvailabilityContext | ActionExecutionContext;
	readonly input: WorkbenchPanelPositionInput;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
}): PreparedPanelPositionFocus {
	const resolution = resolvePanelPositionForContext(projectWorkspaceRuntime, input, context);
	if (resolution.kind === 'unavailable') return resolution;

	const actionId = PANEL_ACTION_IDS.focus;
	const inputByActionId = { [actionId]: { panelId: resolution.panelId } };
	const prepared = runtime
		.discover(
			{
				scope,
				source: context.source,
				target: { id: resolution.panelId, type: 'panel' },
			},
			{
				includeDescendants: true,
				inputByActionId: {
					...WORKBENCH_DESCENDANT_ACTION_DISCOVERY_INPUTS,
					...inputByActionId,
				},
			},
		)
		.find(
			(result) =>
				result.kind === 'ready' &&
				result.descriptor.id === actionId &&
				result.availability.kind === 'enabled',
		);
	return prepared?.kind === 'ready'
		? { invocation: prepared.invocation, kind: 'ready' }
		: { kind: 'unavailable', reason: 'active-command-target-unavailable' };
}

function resolvePanelPositionForContext(
	projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime,
	input: WorkbenchPanelPositionInput,
	context: ActionAvailabilityContext | ActionExecutionContext,
):
	| PanelFocusByPositionResolution
	| {
			readonly kind: 'unavailable';
			readonly reason: 'active-tab-unavailable' | 'panel-unavailable';
	  } {
	if (context.target !== undefined && context.target.type !== 'panel') {
		return { kind: 'unavailable', reason: 'panel-unavailable' };
	}
	const terminalContext = projectWorkspaceRuntime.getActiveTerminalContext();
	const activeTabId = terminalContext?.panelWorkspaceStore.getActiveTabId() ?? null;
	if (terminalContext === null || activeTabId === null) {
		return { kind: 'unavailable', reason: 'active-tab-unavailable' };
	}
	const activeTab = terminalContext.panelWorkspaceStore
		.getSnapshot()
		.tabs.find((tab) => tab.tabId === activeTabId);
	if (activeTab === undefined) {
		return { kind: 'unavailable', reason: 'active-tab-unavailable' };
	}
	return resolvePanelFocusByPosition({
		maximizedPanelId: activeTab.maximizedPanelId,
		position: input.position,
		root: activeTab.root,
		sourcePanelId: context.target?.id ?? activeTab.focusedPanelId,
	});
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
	if (terminalContext === null) return { kind: 'unavailable', reason: 'active-tab-unavailable' };
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
				inputByActionId: {
					...WORKBENCH_DESCENDANT_ACTION_DISCOVERY_INPUTS,
					[resolution.actionId]: resolution.input,
				},
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
