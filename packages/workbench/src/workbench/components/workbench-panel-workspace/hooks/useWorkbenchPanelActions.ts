import {
	createTypedActionRuntime,
	defineAction,
	type ActionCatalogId,
	type ActionExecution,
	type ActionInputFor,
	type ActionInvocationSource,
	type ActionOutputFor,
	type ActionRuntime,
	type ActionScope,
} from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { workbenchMessages } from '@orchbee/i18n';
import * as React from 'react';

import type { TerminalPresentationId } from '../../../../terminal-presentation';
import {
	PANEL_ACTION_IDS,
	panelChildTerminalInput,
	panelCloseInput,
	panelMoveInput,
	panelResizeInput,
	panelSplitInput,
	panelSwapInput,
	panelTargetInput,
	type PanelActionCatalog,
	type PanelCloseInput,
} from '../actions/panel-action-contracts';
import {
	findPanel,
	terminalRequiresCloseConfirmation,
	type PanelWorkspaceTransition,
	type TabPanelWorkspace,
} from '../model/panel-workspace-state';

interface PanelActionOperations {
	readonly activateChildTerminal: (
		panelId: string,
		childPresentationId: TerminalPresentationId,
	) => PanelWorkspaceTransition | null;
	readonly closeDisplayedTerminal: (input: PanelCloseInput) => PanelWorkspaceTransition | null;
	readonly createChildTerminal: (panelId: string) => PanelWorkspaceTransition | null;
	readonly focusPanel: (panelId: string) => PanelWorkspaceTransition | null;
	readonly movePanel: (
		panelId: string,
		targetPanelId: string,
		direction: 'above' | 'below' | 'left' | 'right',
	) => PanelWorkspaceTransition | null;
	readonly resizePanel: (
		panelId: string,
		adjustment: 'narrower' | 'reset' | 'shorter' | 'taller' | 'wider',
		multiplier: 1 | 2,
	) => PanelWorkspaceTransition | null;
	readonly showPrimaryTerminal: (panelId: string) => PanelWorkspaceTransition | null;
	readonly splitPanel: (
		panelId: string,
		direction: 'above' | 'below' | 'left' | 'right',
	) => { readonly panelId: string; readonly transition: PanelWorkspaceTransition } | null;
	readonly startPanelResize: (panelId: string) => boolean;
	readonly swapPanels: (panelId: string, targetPanelId: string) => PanelWorkspaceTransition | null;
	readonly togglePanelMaximization: (panelId: string) => PanelWorkspaceTransition | null;
}

interface UseWorkbenchPanelActionsOptions {
	readonly activeTab: TabPanelWorkspace | null;
	readonly operations: PanelActionOperations;
	readonly panelCount: number;
	readonly parentScope: ActionScope;
	readonly runtime: ActionRuntime;
}

interface WorkbenchPanelActions {
	readonly execute: <Id extends ActionCatalogId<PanelActionCatalog>>(
		actionId: Id,
		input: ActionInputFor<PanelActionCatalog, Id>,
		source: ActionInvocationSource,
	) => ActionExecution<ActionOutputFor<PanelActionCatalog, Id>>;
	readonly scope: ActionScope;
}

export function useWorkbenchPanelActions({
	activeTab,
	operations,
	panelCount,
	parentScope,
	runtime,
}: UseWorkbenchPanelActionsOptions): WorkbenchPanelActions {
	const scope = React.useMemo(
		() => runtime.createScope('panel-workspace', parentScope),
		[parentScope, runtime],
	);
	const typedRuntime = React.useMemo(
		() => createTypedActionRuntime<PanelActionCatalog>(runtime),
		[runtime],
	);

	React.useEffect(() => {
		const category = {
			id: 'panels',
			title: workbenchMessages.actionCategoryPanels,
		};
		const execution = {
			concurrency: 'parallel' as const,
			concurrencyKey: (input: { readonly panelId: string }) => input.panelId,
			mode: 'foreground' as const,
		};
		const findTarget = (panelId: string) =>
			activeTab === null ? null : findPanel(activeTab.root, panelId);
		const findCloseTarget = (input: PanelCloseInput) => {
			if (activeTab?.tabId !== input.tabId) {
				return null;
			}
			const panel = findTarget(input.panelId);
			return panel?.activeChildPresentationId === input.childPresentationId ? panel : null;
		};
		const runningTerminalCount = (input: PanelCloseInput) => {
			const panel = findCloseTarget(input);
			if (panel === null) {
				return 0;
			}
			const affectedTerminals =
				input.childPresentationId === null
					? [panel.primaryPresentation, ...panel.childPresentations]
					: panel.childPresentations.filter(
							(terminal) => terminal.presentationId === input.childPresentationId,
						);
			return affectedTerminals.filter(terminalRequiresCloseConfirmation).length;
		};
		const targetAvailability = (panelId: string) =>
			findTarget(panelId) === null
				? {
						kind: 'disabled' as const,
						reason: workbenchMessages.actionUnavailable,
					}
				: { kind: 'enabled' as const };
		const resultFor = (transition: PanelWorkspaceTransition | null) => {
			if (transition === null || transition.outcome === 'rejected') {
				return {
					code: transition?.rejection ?? 'active-tab-unavailable',
					kind: 'blocked' as const,
				};
			}
			return transition.outcome === 'unchanged'
				? { kind: 'unchanged' as const }
				: { kind: 'completed' as const };
		};
		const registrations = [
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) => targetAvailability(input.panelId),
					handler: {
						execute: (context, input) => {
							context.markCommitted();
							return resultFor(
								operations.activateChildTerminal(input.panelId, input.childPresentationId),
							);
						},
					},
					id: PANEL_ACTION_IDS.activateChildTerminal,
					input: panelChildTerminalInput,
					title: workbenchMessages.panelActivateChild,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) => {
						const sourcePanel = findTarget(input.panelId);
						const targetPanel = findTarget(input.targetPanelId);
						return activeTab?.maximizedPanelId !== null ||
							panelCount < 2 ||
							sourcePanel === null ||
							targetPanel === null ||
							input.panelId === input.targetPanelId
							? {
									kind: 'disabled',
									reason: workbenchMessages.actionUnavailable,
								}
							: { kind: 'enabled' };
					},
					handler: {
						execute: (context, input) => {
							const result = resultFor(operations.swapPanels(input.panelId, input.targetPanelId));
							if (result.kind === 'completed') {
								context.markCommitted();
							}
							return result;
						},
					},
					id: PANEL_ACTION_IDS.swap,
					input: panelSwapInput,
					title: workbenchMessages.panelMove,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) => {
						if (activeTab?.maximizedPanelId !== null) {
							return {
								kind: 'disabled',
								reason: workbenchMessages.actionUnavailable,
							};
						}
						return targetAvailability(input.panelId);
					},
					handler: {
						execute: (context, input) => {
							const result = resultFor(
								operations.resizePanel(input.panelId, input.adjustment, input.multiplier),
							);
							if (result.kind === 'completed') {
								context.markCommitted();
							}
							return result;
						},
					},
					id: PANEL_ACTION_IDS.resize,
					input: panelResizeInput,
					title: workbenchMessages.panelResize,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					confirmation: (input) => {
						const runningCount = runningTerminalCount(input);
						return runningCount === 0
							? null
							: {
									cancelLabel: workbenchMessages.panelCancel,
									confirmLabel: workbenchMessages.panelCloseAnyway,
									description: {
										...workbenchMessages.panelCloseRunningDescription,
										values: { count: runningCount },
									},
									risk: 'destructive',
									title: workbenchMessages.panelCloseRunningTitle,
								};
					},
					execution: {
						concurrency: 'serialize',
						concurrencyKey: () => 'panel-close-confirmation',
						mode: 'foreground',
					},
					getAvailability: (_context, input) => {
						const panel = findCloseTarget(input);
						return panel === null || (input.childPresentationId === null && panelCount < 2)
							? {
									kind: 'disabled',
									reason: workbenchMessages.actionUnavailable,
								}
							: { kind: 'enabled' };
					},
					handler: {
						execute: (context, input) => {
							const transition = operations.closeDisplayedTerminal(input);
							if (transition === null) {
								return { code: 'stale-target', kind: 'blocked' };
							}
							const result = resultFor(transition);
							if (result.kind === 'completed') {
								context.markCommitted();
							}
							return result;
						},
					},
					id: PANEL_ACTION_IDS.close,
					input: panelCloseInput,
					title: workbenchMessages.panelClose,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) => targetAvailability(input.panelId),
					handler: {
						execute: (_context, input) =>
							operations.startPanelResize(input.panelId)
								? { kind: 'completed' }
								: { code: 'stale-target', kind: 'blocked' },
					},
					id: PANEL_ACTION_IDS.startResize,
					input: panelTargetInput,
					title: workbenchMessages.panelResize,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) => targetAvailability(input.panelId),
					handler: {
						execute: (context, input) => {
							context.markCommitted();
							return resultFor(operations.createChildTerminal(input.panelId));
						},
					},
					id: PANEL_ACTION_IDS.createChildTerminal,
					input: panelTargetInput,
					title: workbenchMessages.panelNewChildTerminal,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) => targetAvailability(input.panelId),
					handler: {
						execute: (context, input) => {
							context.markCommitted();
							return resultFor(operations.focusPanel(input.panelId));
						},
					},
					id: PANEL_ACTION_IDS.focus,
					input: panelTargetInput,
					title: workbenchMessages.panelFocus,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) => {
						const sourcePanel = findTarget(input.panelId);
						const targetPanel = findTarget(input.targetPanelId);
						return panelCount < 2 ||
							sourcePanel === null ||
							targetPanel === null ||
							input.panelId === input.targetPanelId
							? {
									kind: 'disabled',
									reason: workbenchMessages.actionUnavailable,
								}
							: { kind: 'enabled' };
					},
					handler: {
						execute: (context, input) => {
							context.markCommitted();
							return resultFor(
								operations.movePanel(input.panelId, input.targetPanelId, input.direction),
							);
						},
					},
					id: PANEL_ACTION_IDS.move,
					input: panelMoveInput,
					title: workbenchMessages.panelMove,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) => {
						const panel = findTarget(input.panelId);
						return panel?.activeChildPresentationId === null || panel === null
							? {
									kind: 'disabled',
									reason: workbenchMessages.actionUnavailable,
								}
							: { kind: 'enabled' };
					},
					handler: {
						execute: (context, input) => {
							context.markCommitted();
							return resultFor(operations.showPrimaryTerminal(input.panelId));
						},
					},
					id: PANEL_ACTION_IDS.showPrimaryTerminal,
					input: panelTargetInput,
					title: workbenchMessages.panelReturnToPrimary,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) => targetAvailability(input.panelId),
					handler: {
						execute: (context, input) => {
							context.markCommitted();
							const result = operations.splitPanel(input.panelId, input.direction);
							if (result === null || result.transition.outcome === 'rejected') {
								return {
									code: result?.transition.rejection ?? 'active-tab-unavailable',
									kind: 'blocked',
								};
							}
							return result.transition.outcome === 'unchanged'
								? { kind: 'unchanged' }
								: { kind: 'completed', value: result.panelId };
						},
					},
					id: PANEL_ACTION_IDS.split,
					input: panelSplitInput,
					title: workbenchMessages.panelSplit,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) =>
						panelCount < 2
							? {
									kind: 'disabled',
									reason: workbenchMessages.actionUnavailable,
								}
							: targetAvailability(input.panelId),
					handler: {
						execute: (context, input) => {
							context.markCommitted();
							return resultFor(operations.togglePanelMaximization(input.panelId));
						},
					},
					id: PANEL_ACTION_IDS.toggleMaximization,
					input: panelTargetInput,
					title: workbenchMessages.panelMaximize,
				}),
			),
		];

		return () => {
			for (const registration of registrations) {
				registration.dispose();
			}
		};
	}, [activeTab, operations, panelCount, scope, typedRuntime]);

	const execute = useMemoizedFn(
		<Id extends ActionCatalogId<PanelActionCatalog>>(
			actionId: Id,
			input: ActionInputFor<PanelActionCatalog, Id>,
			source: ActionInvocationSource,
		): ActionExecution<ActionOutputFor<PanelActionCatalog, Id>> => {
			const panelId =
				typeof input === 'object' &&
				input !== null &&
				'panelId' in input &&
				typeof input.panelId === 'string'
					? input.panelId
					: null;
			return typedRuntime.execute(actionId, input, {
				scope,
				source,
				...(panelId === null ? {} : { target: { id: panelId, type: 'panel' } }),
			});
		},
	);

	return { execute, scope };
}
